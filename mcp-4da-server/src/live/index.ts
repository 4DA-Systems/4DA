// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
import { resolveVersions, mapEcosystem } from "./version-resolver.js";
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
  private initialized = false;

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
   */
  initFromProject(
    projectPath: string,
    deps: string[],
    devDeps: string[],
    language: string,
  ): void {
    this.resolvedDeps = resolveVersions(projectPath, deps, devDeps, language);
    this.initialized = true;
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
      ? this.resolvedDeps
      : this.resolvedDeps.filter((d) => !d.isDev);

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
          return await registry.getPackageInfo(dep.name, dep.version, dep.isDev);
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

function emptyVulnResult(projectPath: string, offline: boolean): VulnerabilityScanResult {
  return {
    scannedAt: new Date().toISOString(),
    projectPath,
    ecosystemsScanned: [],
    totalScanned: 0,
    totalVulnerable: 0,
    bySeverity: { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 },
    vulnerabilities: [],
    cleanCount: 0,
    scanDurationMs: 0,
    cached: false,
    offline,
  };
}
