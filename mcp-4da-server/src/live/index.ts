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
import { resolveAuditVersions, resolveVersions, mapEcosystem } from "./version-resolver.js";
import { computeSemverDistance } from "./semver-utils.js";
import { NpmRegistry } from "./npm-registry.js";
import { CratesRegistry } from "./crates-registry.js";
import { PyPIRegistry } from "./pypi-registry.js";
import { GoRegistry } from "./go-registry.js";
import type {
  ResolvedDependency,
  RegistryPackageInfo,
  VulnerabilityScanResult,
  LiveHeadline,
  LiveIntelligenceStatus,
} from "./types.js";

export type { VulnerabilityScanResult, VulnerabilityEntry, LiveHeadline, LiveIntelligenceStatus, RegistryPackageInfo, DependencyHealthResult } from "./types.js";

/** Minimum gap between vulnerability warmup attempts after an offline result. */
const WARMUP_RETRY_MS = 60_000;

export class LiveIntelligence {
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
  /**
   * The background vulnerability scan started at server init. Tools that need
   * scan data before answering (`what_should_i_know`) await this through
   * `ensureVulnerabilities` instead of reading the cache and finding it empty.
   */
  private warmup: Promise<VulnerabilityScanResult> | null = null;
  /** When the last warmup came back offline; gates the retry so an offline host is not re-probed on every call. */
  private warmupFailedAt: number | null = null;

  constructor(db: Database.Database) {
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
    this.resolvedDeps = resolveVersions(projectPath, deps, devDeps, language);
    this.auditDeps = resolveAuditVersions(projectPath, deps, devDeps, language);
    this.projectRoot = projectPath;
    this.initialized = true;
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
    const allResolved: ResolvedDependency[] = [];
    const allAudit: ResolvedDependency[] = [];
    for (const [language, { deps, devDeps }] of Object.entries(depsByEcosystem)) {
      allResolved.push(...resolveVersions(projectPath, deps, devDeps, language, depTargets));
      allAudit.push(...resolveAuditVersions(projectPath, deps, devDeps, language, depTargets));
    }
    this.resolvedDeps = dedupeDependencies(allResolved);
    this.auditDeps = dedupeDependencies(allAudit);
    this.projectRoot = projectPath;
    this.initialized = true;
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
    const allResolved: ResolvedDependency[] = [];
    const allAudit: ResolvedDependency[] = [];
    for (const { dir, language, deps, devDeps } of groups) {
      // Each resolver stamps `sourceDirs: [dir]` at the point of resolution, so
      // the dedupe below can union provenance instead of discarding it.
      allResolved.push(...resolveVersions(dir, deps, devDeps, language));
      allAudit.push(...resolveAuditVersions(dir, deps, devDeps, language));
    }
    this.resolvedDeps = dedupeDependencies(allResolved);
    this.auditDeps = dedupeDependencies(allAudit);
    this.projectRoot = commonPathRoot(groups.map((g) => g.dir));
    this.initialized = true;
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
   */
  async scanVulnerabilities(
    projectPath: string,
    options?: { includeDev?: boolean; forceRefresh?: boolean },
  ): Promise<VulnerabilityScanResult> {
    if (!this.enabled) {
      return emptyVulnResult(projectPath, true);
    }

    const deps = options?.includeDev
      ? this.auditDeps
      : this.auditDeps.filter((d) => !d.isDev);

    if (deps.length === 0) {
      return emptyVulnResult(projectPath, false);
    }

    if (options?.forceRefresh) {
      this.cache.invalidateSource("osv");
    }

    try {
      this.lastVulnScan = await this.osvScanner.scan(deps, projectPath);
      return this.lastVulnScan;
    } catch {
      // Network failure — return last known or empty
      if (this.lastVulnScan) return { ...this.lastVulnScan, offline: true, cached: true };
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
   * A usable vulnerability scan, or null. Returns the stored scan when one
   * exists, otherwise awaits the warmup (starting one if none is running)
   * for at most `timeoutMs`. Null means the scan is not available: disabled,
   * still running past the deadline, or finished offline (an offline scan
   * makes no clean-dependency claims, so it cannot support a "safe" verdict).
   * Never throws. A timed-out warmup keeps running so a later call can use it.
   */
  async ensureVulnerabilities(
    projectPath: string,
    timeoutMs: number,
  ): Promise<VulnerabilityScanResult | null> {
    if (!this.enabled) return null;
    if (this.lastVulnScan && !this.lastVulnScan.offline) return this.lastVulnScan;

    if (!this.warmup) {
      if (this.warmupFailedAt !== null && Date.now() - this.warmupFailedAt < WARMUP_RETRY_MS) {
        return null;
      }
      this.startVulnerabilityWarmup(projectPath);
    }
    const pending = this.warmup;
    if (!pending) return null;

    let timer: ReturnType<typeof setTimeout> | undefined;
    const deadline = new Promise<null>((resolve) => {
      timer = setTimeout(() => resolve(null), Math.max(0, timeoutMs));
    });
    try {
      const settled = await Promise.race([pending, deadline]);
      if (settled === null) return null; // still scanning; keep the warmup alive
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
    if (!this.enabled) {
      return deps.map((d) => ({
        name: d.name, ecosystem: d.ecosystem, currentVersion: d.version,
        latestVersion: null, latestStableVersion: null, versionsBehind: null,
        deprecated: false, deprecationMessage: null, lastPublished: null,
        license: null, weeklyDownloads: null, isDev: d.isDev, fetchError: "Offline mode",
      }));
    }

    const registryForEcosystem = (eco: string) => {
      switch (eco) {
        case "npm": return this.npmRegistry;
        case "crates.io": return this.cratesRegistry;
        case "PyPI": return this.pypiRegistry;
        case "Go": return this.goRegistry;
        default: return null;
      }
    };

    const results = await Promise.all(
      deps.map(async (dep) => {
        const registry = registryForEcosystem(dep.ecosystem);
        if (!registry) {
          return {
            name: dep.name, ecosystem: dep.ecosystem, currentVersion: dep.version,
            latestVersion: null, latestStableVersion: null, versionsBehind: null,
            deprecated: false, deprecationMessage: null, lastPublished: null,
            license: null, weeklyDownloads: null, isDev: dep.isDev,
            fetchError: `No registry fetcher for ${dep.ecosystem}`,
          } as RegistryPackageInfo;
        }
        try {
          const info = await registry.getPackageInfo(dep.name, dep.version, dep.isDev);
          return restampRegistryContext(info, dep);
        } catch {
          return {
            name: dep.name, ecosystem: dep.ecosystem, currentVersion: dep.version,
            latestVersion: null, latestStableVersion: null, versionsBehind: null,
            deprecated: false, deprecationMessage: null, lastPublished: null,
            license: null, weeklyDownloads: null, isDev: dep.isDev,
            fetchError: "Registry fetch failed",
          } as RegistryPackageInfo;
        }
      }),
    );

    // Bulk fetch npm downloads for npm deps
    const npmDeps = deps.filter((d) => d.ecosystem === "npm");
    if (npmDeps.length > 0) {
      try {
        const downloads = await this.npmRegistry.getBulkDownloads(npmDeps.map((d) => d.name));
        for (const result of results) {
          if (result.ecosystem === "npm" && downloads.has(result.name)) {
            result.weeklyDownloads = downloads.get(result.name) || null;
          }
        }
      } catch {
        // Downloads are nice-to-have, not critical
      }
    }

    return results;
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

/**
 * Deepest common ancestor of a set of directories (segment-wise, both slash
 * styles). Null for an empty set; a single dir is its own root.
 */
function commonPathRoot(dirs: string[]): string | null {
  if (dirs.length === 0) return null;
  const split = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").split("/");
  let common = split(dirs[0]);
  for (const dir of dirs.slice(1)) {
    const parts = split(dir);
    let i = 0;
    while (i < common.length && i < parts.length && common[i].toLowerCase() === parts[i].toLowerCase()) i++;
    common = common.slice(0, i);
    if (common.length === 0) break;
  }
  return common.length > 0 ? common.join("/") : null;
}

/**
 * Registry caches key by package NAME, but the cached record embeds the
 * QUERYING dependency's `currentVersion`, `versionsBehind`, and `isDev`. Two
 * instances of one package at different versions (better-sqlite3 11.10.0 and
 * 12.11.1 across workspaces) therefore both came back wearing the first
 * instance's version — the upgrade planner then showed two identical rows and
 * lost the older instance entirely. Registry facts (latest version,
 * deprecation, downloads) are per-package and cacheable; the per-instance
 * fields are re-stamped here from the dep actually being asked about.
 */
function restampRegistryContext(
  info: RegistryPackageInfo,
  dep: ResolvedDependency,
): RegistryPackageInfo {
  const latest = info.latestStableVersion || info.latestVersion;
  return {
    ...info,
    currentVersion: dep.version,
    isDev: dep.isDev,
    versionsBehind: dep.version && latest ? computeSemverDistance(dep.version, latest) : null,
  };
}

function dedupeDependencies(deps: ResolvedDependency[]): ResolvedDependency[] {
  const unique = new Map<string, ResolvedDependency>();
  for (const dep of deps) {
    const key = `${dep.ecosystem}\0${dep.name}\0${dep.version ?? ""}`;
    const existing = unique.get(key);
    if (existing) {
      existing.isDirect ||= dep.isDirect;
      existing.devScopeKnown &&= dep.devScopeKnown;
      existing.isDev = existing.devScopeKnown && existing.isDev && dep.isDev;
      // A crate reachable via ANY active path is active; keep a target label if present.
      existing.platformActive ||= dep.platformActive;
      existing.target = existing.target ?? dep.target;
      // Union the provenance rather than discarding it — the same version can
      // legitimately be pinned by several workspaces, and the reader needs all
      // of them to know where to apply the fix.
      for (const dir of dep.sourceDirs ?? []) {
        if (!existing.sourceDirs.includes(dir)) existing.sourceDirs.push(dir);
      }
    } else {
      unique.set(key, { ...dep, sourceDirs: [...(dep.sourceDirs ?? [])] });
    }
  }
  return [...unique.values()];
}

function emptyVulnResult(projectPath: string, offline: boolean): VulnerabilityScanResult {
  return {
    scannedAt: new Date().toISOString(),
    projectPath,
    ecosystemsScanned: [],
    totalScanned: 0,
    totalVulnerable: 0,
    platformInactiveVulnerable: 0,
    bySeverity: { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 },
    vulnerabilities: [],
    cleanCount: 0,
    scanDurationMs: 0,
    cached: false,
    offline,
  };
}
