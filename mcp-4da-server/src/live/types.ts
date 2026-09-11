// SPDX-License-Identifier: Apache-2.0

export type OsvEcosystem =
  | "npm"
  | "crates.io"
  | "PyPI"
  | "Go"
  | "Maven"
  | "NuGet"
  | "RubyGems"
  | "Packagist"
  | "Pub";

/** The reinstall command that brings node_modules back in line with a directory's lockfile. */
export type InstallFixCommand = "pnpm install" | "npm ci" | "yarn install";

export interface ResolvedDependency {
  name: string;
  version: string | null;
  ecosystem: OsvEcosystem;
  isDev: boolean;
  isDirect: boolean;
  devScopeKnown: boolean;
  /** Target spec gating this dep (e.g. `cfg(windows)`), or null if unconditional. */
  target: string | null;
  /** False when `target` is not active on the host platform (advisory is not relevant). */
  platformActive: boolean;
  /**
   * Manifest directories this exact (ecosystem, name, version) resolved from.
   *
   * A scan root can span several independent workspaces — `src-tauri/`,
   * `relay/`, `victauri-gauntlet/` all sit under one repo and pin their own
   * versions. Without this, dedupe collapsed them and the report named a
   * vulnerable version with no way to say WHICH workspace pinned it, so a
   * patched primary crate looked vulnerable because a sibling lagged.
   */
  sourceDirs: string[];
  /**
   * npm DIRECT deps resolved from a lockfile only: the version node_modules
   * actually holds (walking up for hoisted workspaces), or null when it is not
   * installed or unreadable. Undefined when the check did not run (another
   * ecosystem, no node_modules, or versions read from a manifest).
   */
  installedVersion?: string | null;
  /**
   * Set on the extra audit entry that asks OSV about an INSTALLED version the
   * lockfile does not pin: the lockfile's version of the same package. The
   * entry exists so `vulnerable_installed` is OSV's answer, not a guess.
   */
  installDriftOf?: string;
  /** On drift entries: the command that reinstalls from the lockfile. */
  installFix?: InstallFixCommand;
}

/**
 * One npm direct dependency whose installed copy differs from its lockfile.
 * `dir` is the manifest directory, spelled as the resolution group spells it.
 */
export interface InstallDriftRecord {
  package: string;
  dir: string;
  lockfileVersion: string;
  installedVersion: string;
  fix: InstallFixCommand;
  isDev: boolean;
}

/** A file one resolution group read its versions from. */
export interface ResolutionSourceRecord {
  /** Absolute path. */
  path: string;
  kind: "lockfile" | "manifest";
  /** Modification time when it was read, or null if it vanished since. */
  mtimeMs: number | null;
}

/** When the dependency set in use was assembled, and from which files. */
export interface ResolutionProvenance {
  resolvedAt: string;
  sources: ResolutionSourceRecord[];
}

/**
 * An OSV record. The `/v1/querybatch` endpoint returns only `{ id, modified }`
 * per vulnerability (a lightweight index); the rich fields are populated only
 * after hydrating each advisory via `/v1/vulns/{id}`. Hence every field beyond
 * `id` is optional — consumers must tolerate the un-hydrated shape.
 */
export interface OsvVulnerability {
  id: string;
  summary?: string;
  details?: string;
  aliases?: string[];
  severity?: Array<{ type: string; score: string }>;
  affected?: Array<{
    package: { name: string; ecosystem: string };
    ranges: Array<{
      type: string;
      events: Array<{ introduced?: string; fixed?: string }>;
    }>;
  }>;
  references?: Array<{ type: string; url: string }>;
  /** GitHub-advisory severity label: LOW | MODERATE | HIGH | CRITICAL. */
  database_specific?: { severity?: string };
  /** Set (to an RFC3339 timestamp) when the advisory has been withdrawn/retracted. */
  withdrawn?: string;
  published?: string;
  modified?: string;
}

export interface VulnerabilityEntry {
  package: string;
  currentVersion: string;
  ecosystem: OsvEcosystem;
  isDev: boolean;
  isDirect: boolean;
  devScopeKnown: boolean;
  vulnId: string;
  aliases: string[];
  severity: "critical" | "high" | "medium" | "low" | "unknown";
  cvssScore: number | null;
  summary: string;
  fixedVersion: string | null;
  published: string;
  references: string[];
  /** Target spec gating the affected dep, or null if unconditional. */
  target: string | null;
  /** False when the affected dep is not active on the host platform. */
  platformActive: boolean;
  /**
   * Manifest directories that pin this vulnerable version. Answers the first
   * question a reader asks — *which project do I go fix?* — which a flat
   * package-name list cannot.
   */
  sourceDirs: string[];
  /**
   * Set when this row is about an INSTALLED version the lockfile does not pin
   * (node_modules drift): the lockfile's version of the same package.
   */
  installDriftOf?: string;
  /** On drift rows: the command that reinstalls from the lockfile. */
  installFix?: InstallFixCommand;
}

export interface VulnerabilityScanResult {
  scannedAt: string;
  projectPath: string;
  ecosystemsScanned: string[];
  totalScanned: number;
  totalVulnerable: number;
  /** Vulnerable packages that are NOT active on the host platform (relevance noise). */
  platformInactiveVulnerable: number;
  bySeverity: { critical: number; high: number; medium: number; low: number; unknown: number };
  vulnerabilities: VulnerabilityEntry[];
  cleanCount: number;
  scanDurationMs: number;
  cached: boolean;
  offline: boolean;
}

export interface LiveHeadline {
  id: string;
  title: string;
  url: string | null;
  source: "hacker_news";
  points: number;
  comments: number;
  published: string;
  relevanceScore: number;
  relevanceReason: string;
}

export interface LiveIntelligenceStatus {
  enabled: boolean;
  offline: boolean;
  lastOsvRefresh: string | null;
  lastHnRefresh: string | null;
  cachedVulnCount: number;
  cachedHeadlineCount: number;
}

// =============================================================================
// Registry Package Info (Phase 2)
// =============================================================================

export interface RegistryPackageInfo {
  name: string;
  ecosystem: OsvEcosystem;
  currentVersion: string | null;
  latestVersion: string | null;
  latestStableVersion: string | null;
  versionsBehind: SemverDistance | null;
  deprecated: boolean;
  deprecationMessage: string | null;
  lastPublished: string | null;
  license: string | null;
  weeklyDownloads: number | null;
  isDev: boolean;
  fetchError: string | null;
  /**
   * The version node_modules actually holds, present only when it differs
   * from `currentVersion` (the lockfile's). Several differing installs across
   * workspaces are listed comma-separated.
   */
  installedVersion?: string;
}

export interface SemverDistance {
  major: number;
  minor: number;
  patch: number;
  label: "up-to-date" | "patch" | "minor" | "major";
}

export interface DependencyHealthResult {
  scannedAt: string;
  projectPath: string;
  ecosystemsScanned: string[];
  totalDeps: number;
  outdatedCount: number;
  deprecatedCount: number;
  /**
   * Distinct packages with an advisory that is built on this host and is not a
   * maintenance notice — the same set `vulnerability_scan` reports as
   * `total_vulnerable_packages`, so the two tools agree over one scan.
   */
  vulnerableCount: number;
  /**
   * Every advisory row the scan returned, before the platform and maintenance
   * splits and before alias collapsing. Present so nothing is hidden by the
   * `vulnerableCount` filter: the difference is platform-inactive packages and
   * unmaintained-dependency notices.
   */
  advisoryCount: number;
  healthScore: number;
  dependencies: RegistryPackageInfo[];
  vulnerabilitySummary: { critical: number; high: number; medium: number; low: number } | null;
  summary: string;
  scanDurationMs: number;
  cached: boolean;
}
