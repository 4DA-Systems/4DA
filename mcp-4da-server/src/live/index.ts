// SPDX-License-Identifier: Apache-2.0
/**
 * Live Intelligence Coordinator
 *
 * Orchestrates vulnerability scanning and headline fetching.
 * Manages cache lifecycle and offline fallback.
 *
 * Privacy: Only sends package names/versions (public) and tech keywords (generic).
 * Set FOURDA_OFFLINE=true to disable all network calls.
 */

import type Database from "better-sqlite3";
import { LiveCache } from "./cache.js";
import { RateLimiter, DEFAULT_RATE_LIMITS } from "./rate-limiter.js";
import { OsvScanner } from "./osv-scanner.js";
import { HNFetcher } from "./hn-fetcher.js";
import { NpmRegistry } from "./npm-registry.js";
import { CratesRegistry } from "./crates-registry.js";
import { PyPIRegistry } from "./pypi-registry.js";
import { GoRegistry } from "./go-registry.js";
import { fetchRegistryHealthFor } from "./registry-health.js";
import { commonPathRoot, dedupeDependencies, emptyVulnResult } from "./dependency-set.js";
import { groupIsStale, resolveGroup, type GroupResolution, type ResolutionGroup } from "./resolution.js";
import { applyInstanceDevScope } from "./dev-scope.js";
import type {
  InstallDriftRecord,
  ResolutionProvenance,
  ResolutionSourceRecord,
  ResolvedDependency,
  RegistryPackageInfo,
  VulnerabilityScanResult,
  LiveHeadline,
  LiveIntelligenceStatus,
} from "./types.js";

export type {
  VulnerabilityScanResult,
  VulnerabilityEntry,
  LiveHeadline,
  LiveIntelligenceStatus,
  RegistryPackageInfo,
  DependencyHealthResult,
  InstallDriftRecord,
  ResolutionProvenance,
} from "./types.js";

/** Minimum gap between vulnerability warmup attempts after an offline result. */
const WARMUP_RETRY_MS = 60_000;

export class LiveIntelligence {
  private db: Database.Database;
  private cache: LiveCache;
  private rateLimiter: RateLimiter;
  private osvScanner: OsvScanner;
  private hnFetcher: HNFetcher;
  private npmRegistry: NpmRegistry;
  private cratesRegistry: CratesRegistry;
  private pypiRegistry: PyPIRegistry;
  private goRegistry: GoRegistry;
  private enabled: boolean;

  private lastVulnScan: VulnerabilityScanResult | null = null;
  private lastHeadlines: LiveHeadline[] = [];
  private resolvedDeps: ResolvedDependency[] = [];
  private auditDeps: ResolvedDependency[] = [];
  private initialized = false;
  private projectRoot: string | null = null;
  /** One entry per resolution group: its inputs, result, and the file signatures that say when it went stale. */
  private groupResults: GroupResolution[] = [];
  /** When the dependency set in use was assembled: at init, or at the last re-resolution. */
  private resolvedAt: string | null = null;
  /**
   * Bumped by every re-resolution. A scan started under an older generation
   * describes a dependency set that no longer exists: it may still be handed
   * to the call that asked for it, but it is never stored or served later.
   */
  private generation = 0;
  /**
   * The background vulnerability scan started at server init. Tools that need
   * scan data before answering (`what_should_i_know`) await this through
   * `ensureVulnerabilities` instead of reading the cache and finding it empty.
   */
  private warmup: Promise<VulnerabilityScanResult> | null = null;
  /** When the last warmup came back offline; gates the retry so an offline host is not re-probed on every call. */
  private warmupFailedAt: number | null = null;

  constructor(db: Database.Database) {
    this.db = db;
    this.enabled = process.env.FOURDA_OFFLINE !== "true";
    this.cache = new LiveCache(db);
    this.rateLimiter = new RateLimiter(DEFAULT_RATE_LIMITS);
    this.osvScanner = new OsvScanner(this.cache, this.rateLimiter);
    this.hnFetcher = new HNFetcher(this.cache, this.rateLimiter);
    this.npmRegistry = new NpmRegistry(this.cache, this.rateLimiter);
    this.cratesRegistry = new CratesRegistry(this.cache, this.rateLimiter);
    this.pypiRegistry = new PyPIRegistry(this.cache, this.rateLimiter);
    this.goRegistry = new GoRegistry(this.cache, this.rateLimiter);
  }

  /**
   * Initialize with project data. Call once after project scan.
   * @deprecated Use initFromMultiEcosystem for correct per-ecosystem resolution.
   */
  initFromProject(
    projectPath: string,
    deps: string[],
    devDeps: string[],
    language: string,
  ): void {
    this.applyGroups([{ dir: projectPath, language, deps, devDeps, targets: {} }], projectPath);
  }

  /**
   * Initialize with per-ecosystem project data. Resolves versions per language
   * so Rust crates go to crates.io, npm packages to npm, etc.
   * Call once after project scan.
   */
  initFromMultiEcosystem(
    projectPath: string,
    depsByEcosystem: Record<string, { deps: string[]; devDeps: string[] }>,
    depTargets: Record<string, string> = {},
  ): void {
    const groups = Object.entries(depsByEcosystem).map(([language, { deps, devDeps }]) => ({
      dir: projectPath,
      language,
      deps,
      devDeps,
      targets: depTargets,
    }));
    this.applyGroups(groups, projectPath);
  }

  /**
   * Initialize from dependency groups that each carry their own resolution
   * directory. Used in 4DA database mode, where dependencies span multiple
   * manifests in different locations (Rust crates under src-tauri/, relay/,
   * etc.; npm packages at the repo root and in sub-packages). Each group
   * resolves versions from its own lock file, so a single global cwd no longer
   * silently drops every dependency whose manifest lives in a subdirectory.
   * Deduplicates by ecosystem+name+version so a crate shared across workspaces
   * is scanned once.
   */
  initFromDependencyGroups(
    groups: Array<{ dir: string; language: string; deps: string[]; devDeps: string[] }>,
  ): void {
    // Each resolver stamps `sourceDirs: [dir]` at the point of resolution, so
    // the dedupe can union provenance instead of discarding it.
    this.applyGroups(
      groups.map((g) => ({ ...g, targets: {} })),
      commonPathRoot(groups.map((g) => g.dir)),
    );
  }

  private applyGroups(groups: ResolutionGroup[], projectRoot: string | null): void {
    this.groupResults = groups.map(resolveGroup);
    this.rebuildDependencySet();
    this.projectRoot = projectRoot;
    this.initialized = true;
  }

  private rebuildDependencySet(): void {
    this.resolvedDeps = dedupeDependencies(this.groupResults.flatMap((r) => r.resolved));
    this.auditDeps = dedupeDependencies(this.groupResults.flatMap((r) => r.audit));
    this.resolvedAt = new Date().toISOString();
  }

  /**
   * Re-resolve every group whose inputs changed since they were read (a
   * lockfile rewritten, created or deleted; the fallback manifest; for npm,
   * the node_modules install state), and drop every answer that described the
   * old set: the stored scan and the warmup. Returns true when anything was
   * re-resolved. Stat calls only, so it is cheap enough for the top of every
   * tool call. A process that started before a `git pull` no longer answers
   * for the dependency set it saw at startup.
   */
  refreshIfLockfilesChanged(): boolean {
    if (!this.initialized || this.groupResults.length === 0) return false;
    let changed = false;
    this.groupResults = this.groupResults.map((result) => {
      if (!groupIsStale(result)) return result;
      changed = true;
      return resolveGroup(result.group);
    });
    if (!changed) return false;
    this.rebuildDependencySet();
    this.generation++;
    this.lastVulnScan = null;
    this.warmup = null;
    this.warmupFailedAt = null;
    return true;
  }

  /** When the dependency set in use was resolved, and from which files. Null before init. */
  getResolutionProvenance(): ResolutionProvenance | null {
    if (!this.initialized || !this.resolvedAt) return null;
    const seen = new Set<string>();
    const sources: ResolutionSourceRecord[] = [];
    for (const result of this.groupResults) {
      if (!result.source || seen.has(result.source.path)) continue;
      seen.add(result.source.path);
      sources.push(result.source);
    }
    return { resolvedAt: this.resolvedAt, sources };
  }

  /** npm direct dependencies whose installed copy differs from the lockfile, per manifest directory. */
  getInstallDrift(): InstallDriftRecord[] {
    return this.groupResults.flatMap((r) => r.drift);
  }

  /**
   * Root directory the dependency set was resolved from, or null before init.
   * For multi-manifest (grouped) init this is the deepest common ancestor of
   * the group directories — honest scope reporting for tool output, instead
   * of whatever `process.cwd()` happens to be.
   */
  getProjectRoot(): string | null {
    return this.projectRoot;
  }

  /**
   * Run vulnerability scan (returns cached if fresh, fetches otherwise).
   *
   * Without `includeDev`, only DIRECT devDependencies are left out (the
   * documented contract). A transitive stays in whatever its scope: an
   * unknown scope must not be hidden, and a known dev-only one is graded
   * down by the shared severity rule, not dropped.
   */
  async scanVulnerabilities(
    projectPath: string,
    options?: { includeDev?: boolean; forceRefresh?: boolean },
  ): Promise<VulnerabilityScanResult> {
    if (!this.enabled) {
      return emptyVulnResult(projectPath, true);
    }

    const scoped = options?.includeDev
      ? this.auditDeps
      : this.auditDeps.filter((d) => !(d.isDirect && d.isDev));
    // Read at scan time, not init time: the app rewrites its inventory on
    // every rescan, and the freshest determination is the one to grade by.
    const deps = applyInstanceDevScope(this.db, scoped);

    if (deps.length === 0) {
      return emptyVulnResult(projectPath, false);
    }

    if (options?.forceRefresh) {
      this.cache.invalidateSource("osv");
    }

    const generation = this.generation;
    try {
      const result = await this.osvScanner.scan(deps, projectPath);
      if (generation === this.generation) this.lastVulnScan = result;
      return result;
    } catch {
      // Network failure — return last known or empty
      if (generation === this.generation && this.lastVulnScan) {
        return { ...this.lastVulnScan, offline: true, cached: true };
      }
      return emptyVulnResult(projectPath, true);
    }
  }

  /**
   * Start the vulnerability scan in the background without blocking startup.
   * Never rejects: a failed scan resolves to an empty OFFLINE result so the
   * warmup can be awaited by anyone. Idempotent while a warmup is in flight.
   *
   * Before this existed, full-database mode initialised the dependency set
   * and warmed headlines but never scanned, so `lastVulnScan` stayed null
   * until some tool happened to call `vulnerability_scan` — and the first
   * `what_should_i_know` of a session read an empty cache and reported
   * "safe_to_delegate" for a task naming a package with an open advisory.
   */
  startVulnerabilityWarmup(projectPath: string): void {
    if (this.warmup) return;
    this.warmupFailedAt = null;
    this.warmup = this.scanVulnerabilities(projectPath).catch(() =>
      emptyVulnResult(projectPath, true),
    );
  }

  /**
   * A usable vulnerability scan, or null. Re-resolves first when a lockfile
   * changed. Returns the stored scan when one exists, otherwise awaits the
   * warmup (starting one if none is running) for at most `timeoutMs`. Null
   * means the scan is not available: disabled, still running past the
   * deadline, finished offline (an offline scan makes no clean-dependency
   * claims, so it cannot support a "safe" verdict), or overtaken by a
   * re-resolution while it ran. Never throws. A timed-out warmup keeps running
   * so a later call can use it.
   */
  async ensureVulnerabilities(
    projectPath: string,
    timeoutMs: number,
  ): Promise<VulnerabilityScanResult | null> {
    if (!this.enabled) return null;
    this.refreshIfLockfilesChanged();
    if (this.lastVulnScan && !this.lastVulnScan.offline) return this.lastVulnScan;

    if (!this.warmup) {
      if (this.warmupFailedAt !== null && Date.now() - this.warmupFailedAt < WARMUP_RETRY_MS) {
        return null;
      }
      this.startVulnerabilityWarmup(projectPath);
    }
    const pending = this.warmup;
    if (!pending) return null;
    const generation = this.generation;

    let timer: ReturnType<typeof setTimeout> | undefined;
    const deadline = new Promise<null>((resolve) => {
      timer = setTimeout(() => resolve(null), Math.max(0, timeoutMs));
    });
    try {
      const settled = await Promise.race([pending, deadline]);
      if (settled === null) return null; // still scanning; keep the warmup alive
      // Re-resolved while waiting: that scan describes a set that no longer exists.
      if (generation !== this.generation) return null;
      const result = this.lastVulnScan ?? settled;
      if (result.offline) {
        // Failed or rate-limited: allow a fresh attempt after the backoff.
        this.warmup = null;
        this.warmupFailedAt = Date.now();
        return null;
      }
      return result;
    } catch {
      return null;
    } finally {
      if (timer !== undefined) clearTimeout(timer);
    }
  }

  /**
   * Fetch relevant headlines for the user's tech stack.
   */
  async fetchHeadlines(techStack: string[]): Promise<LiveHeadline[]> {
    if (!this.enabled) return [];

    try {
      this.lastHeadlines = await this.hnFetcher.fetch(techStack);
      return this.lastHeadlines;
    } catch {
      return this.lastHeadlines; // Return last known
    }
  }

  /**
   * Get last vulnerability scan result (from cache/memory, no network).
   */
  getVulnerabilities(): VulnerabilityScanResult | null {
    return this.lastVulnScan;
  }

  /**
   * Get last headlines (from cache/memory, no network).
   */
  getHeadlines(): LiveHeadline[] {
    return this.lastHeadlines;
  }

  /**
   * Get resolved dependencies with versions.
   */
  getResolvedDeps(): ResolvedDependency[] {
    return this.resolvedDeps;
  }

  getAuditDeps(): ResolvedDependency[] {
    return this.auditDeps;
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  isInitialized(): boolean {
    return this.initialized;
  }

  async fetchRegistryHealth(deps: ResolvedDependency[]): Promise<RegistryPackageInfo[]> {
    // Registries are read at call time (not captured at construction) so a
    // test can substitute one.
    return fetchRegistryHealthFor(
      deps,
      { npm: this.npmRegistry, crates: this.cratesRegistry, pypi: this.pypiRegistry, go: this.goRegistry },
      this.enabled,
    );
  }

  getStatus(): LiveIntelligenceStatus {
    return {
      enabled: this.enabled,
      offline: !this.enabled,
      lastOsvRefresh: this.lastVulnScan?.scannedAt || null,
      lastHnRefresh: this.lastHeadlines.length > 0 ? new Date().toISOString() : null,
      cachedVulnCount: this.lastVulnScan?.totalVulnerable || 0,
      cachedHeadlineCount: this.lastHeadlines.length,
    };
  }
}
