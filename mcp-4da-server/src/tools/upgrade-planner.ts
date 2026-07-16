// SPDX-License-Identifier: Apache-2.0
/**
 * upgrade_planner tool
 *
 * Ranked standalone upgrade recommendations for project dependencies.
 * Combines version freshness, vulnerability data, and deprecation status
 * to produce a prioritized upgrade plan.
 *
 * Honesty contract (Phase 0 hardening):
 * - Transitive CVEs are surfaced as `waiting_on_upstream` steps instead of
 *   being counted-but-never-recommended. No dependency-graph parsing happens
 *   here — parent mapping is the desktop core's job; the honest label is ours.
 * - Output carries explicit heuristic provenance: this is a point-in-time
 *   standalone plan, not the 4DA app's cross-project, version-confirmed plan.
 * - `projectPath` reports the directory set the dependencies were actually
 *   resolved from, not whatever `process.cwd()` happens to be.
 * - When no vulnerability scan has run, the plan says so instead of silently
 *   producing a CVE-blind ranking.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import { maxSemver } from "../live/semver-utils.js";

export interface UpgradePlannerParams {
  include_dev?: boolean;
  max_recommendations?: number;
  risk_threshold?: "all" | "low" | "medium" | "high" | "critical";
}

interface UpgradeRecommendation {
  package: string;
  ecosystem: string;
  currentVersion: string | null;
  targetVersion: string | null;
  upgradeType: "patch" | "minor" | "major" | "unknown";
  risk: "low" | "medium" | "high" | "critical";
  reasons: string[];
  breaking: boolean;
  isDev: boolean;
  /** Whether this package is declared in a manifest (direct) or only present via the lockfile (transitive). */
  scope: "direct" | "transitive";
  /** Direct deps you can bump yourself; transitive fixes arrive via a parent update or lockfile refresh. */
  action: "upgrade_direct" | "waiting_on_upstream";
  /** False when the dependency is gated behind a target spec not active on this host (label, never suppressed). */
  platformActive: boolean;
}

interface UpgradePlanResult {
  generatedAt: string;
  projectPath: string;
  totalDeps: number;
  recommendations: UpgradeRecommendation[];
  summary: string;
  quickWins: number;
  breakingChanges: number;
  waitingOnUpstream: number;
  /** False when no vulnerability scan has run this session — the ranking is then CVE-blind and says so. */
  vulnerabilityDataAvailable: boolean;
  provenance: {
    mode: "standalone_heuristic";
    note: string;
  };
}

const PROVENANCE_NOTE =
  "Point-in-time heuristic from local lockfiles plus live registry/OSV lookups. " +
  "Not the 4DA desktop app's cross-project, version-confirmed plan.";

export const upgradePlannerTool = {
  name: "upgrade_planner",
  description:
    "Ranked standalone upgrade recommendations for dependencies — call before upgrading, adding, or auditing any dependency to pick the safest order. Prioritizes by vulnerability severity (direct AND transitive), deprecation, and version distance. Splits quick wins (patch/minor) from breaking changes (major) and from transitive CVEs waiting on upstream. Run vulnerability_scan first for a CVE-aware plan. Privacy: live mode may query public registries and OSV with package names and versions; it never sends source code.",
  inputSchema: {
    type: "object" as const,
    properties: {
      include_dev: {
        type: "boolean",
        description: "Include devDependencies. Default: false",
      },
      max_recommendations: {
        type: "number",
        description: "Max recommendations to return. Default: 20",
      },
      risk_threshold: {
        type: "string",
        enum: ["all", "low", "medium", "high", "critical"],
        description: "Only show upgrades at or above this risk level. Default: all",
      },
    },
  },
};

const RISK_LEVELS: Record<string, number> = { low: 0, medium: 1, high: 2, critical: 3 };

function severityToRisk(severities: string[]): UpgradeRecommendation["risk"] {
  if (severities.includes("critical")) return "critical";
  if (severities.includes("high")) return "high";
  return "medium";
}

export async function executeUpgradePlanner(
  _db: FourDADatabase,
  params: UpgradePlannerParams,
  liveIntel: LiveIntelligence | null,
): Promise<UpgradePlanResult> {
  if (!liveIntel || !liveIntel.isInitialized()) {
    return {
      generatedAt: new Date().toISOString(),
      projectPath: process.cwd(),
      totalDeps: 0,
      recommendations: [],
      summary: "No project detected. Run from a directory with package.json, Cargo.toml, pyproject.toml, or go.mod.",
      quickWins: 0,
      breakingChanges: 0,
      waitingOnUpstream: 0,
      vulnerabilityDataAvailable: false,
      provenance: { mode: "standalone_heuristic", note: PROVENANCE_NOTE },
    };
  }

  const includeDev = params.include_dev ?? false;
  let deps = liveIntel.getResolvedDeps();
  if (!includeDev) deps = deps.filter((d) => !d.isDev);

  // Fetch registry data (live lookups for direct deps; cached where fresh)
  const registryData = await liveIntel.fetchRegistryHealth(deps);

  // Vulnerability data from the last scan this session (no network here).
  const vulnResult = liveIntel.getVulnerabilities();
  const vulnerabilityDataAvailable = vulnResult !== null;
  const vulnsByPackage = new Map<
    string,
    Array<{ severity: string; vulnId: string; summary: string; fixedVersion: string | null; platformActive: boolean }>
  >();
  if (vulnResult) {
    for (const v of vulnResult.vulnerabilities) {
      if (!vulnsByPackage.has(v.package)) vulnsByPackage.set(v.package, []);
      vulnsByPackage.get(v.package)!.push({
        severity: v.severity,
        vulnId: v.vulnId,
        summary: v.summary,
        fixedVersion: v.fixedVersion,
        platformActive: v.platformActive,
      });
    }
  }

  // Build recommendations — direct deps (registry-informed)
  const recommendations: UpgradeRecommendation[] = [];
  const directPackages = new Set<string>();

  for (const dep of registryData) {
    directPackages.add(`${dep.ecosystem}\0${dep.name}`);
    const reasons: string[] = [];
    let risk: UpgradeRecommendation["risk"] = "low";
    let targetVersion = dep.latestStableVersion || dep.latestVersion;
    let platformActive = true;

    // Check vulnerabilities
    const vulns = vulnsByPackage.get(dep.name);
    if (vulns && vulns.length > 0) {
      risk = severityToRisk(vulns.map((v) => v.severity));
      reasons.push(`${vulns.length} known CVE${vulns.length !== 1 ? "s" : ""} (${vulns.map((v) => v.vulnId).join(", ")})`);
      if (vulns.every((v) => !v.platformActive)) {
        platformActive = false;
        reasons.push("Affected code is gated behind a target not active on this platform");
      }

      // Use fixed version as target if available
      const fixedVersions = vulns.map((v) => v.fixedVersion).filter((f): f is string => Boolean(f));
      if (fixedVersions.length > 0 && !targetVersion) {
        targetVersion = maxSemver(fixedVersions);
      }
    }

    // Check deprecation
    if (dep.deprecated) {
      if (risk === "low") risk = "high";
      reasons.push(dep.deprecationMessage ? `Deprecated: ${dep.deprecationMessage}` : "Package is deprecated");
    }

    // Check version distance
    if (dep.versionsBehind) {
      const d = dep.versionsBehind;
      if (d.label === "major") {
        reasons.push(`${d.major} major version${d.major !== 1 ? "s" : ""} behind`);
        if (risk === "low") risk = "medium";
      } else if (d.label === "minor") {
        reasons.push(`${d.minor} minor version${d.minor !== 1 ? "s" : ""} behind`);
      } else if (d.label === "patch") {
        reasons.push(`${d.patch} patch${d.patch !== 1 ? "es" : ""} behind`);
      }
    }

    // Skip if no upgrade needed (no vuln, not deprecated, not behind => no reasons)
    if (reasons.length === 0) continue;

    const label = dep.versionsBehind?.label;
    const upgradeType: UpgradeRecommendation["upgradeType"] =
      !label || label === "up-to-date" ? "patch" : label;

    recommendations.push({
      package: dep.name,
      ecosystem: dep.ecosystem,
      currentVersion: dep.currentVersion,
      targetVersion,
      upgradeType,
      risk,
      reasons,
      breaking: dep.versionsBehind?.label === "major",
      isDev: dep.isDev,
      scope: "direct",
      action: "upgrade_direct",
      platformActive,
    });
  }

  // Transitive vulnerable packages — previously counted in scan totals but
  // never recommended. Surfaced as waiting_on_upstream with honest labels.
  if (vulnResult) {
    const transitive = new Map<string, typeof vulnResult.vulnerabilities>();
    for (const v of vulnResult.vulnerabilities) {
      if (v.isDirect) continue;
      if (directPackages.has(`${v.ecosystem}\0${v.package}`)) continue;
      if (!includeDev && v.isDev && v.devScopeKnown) continue;
      const key = `${v.ecosystem}\0${v.package}`;
      if (!transitive.has(key)) transitive.set(key, []);
      transitive.get(key)!.push(v);
    }

    for (const entries of transitive.values()) {
      const first = entries[0];
      const fixedVersions = entries.map((e) => e.fixedVersion).filter((f): f is string => Boolean(f));
      const platformActive = !entries.every((e) => !e.platformActive);
      const reasons = [
        `${entries.length} known CVE${entries.length !== 1 ? "s" : ""} (${entries.map((e) => e.vulnId).join(", ")})`,
        "Transitive dependency — not declared in your manifest; the fix arrives via a parent package update or a lockfile refresh",
      ];
      if (!platformActive) {
        reasons.push("Affected code is gated behind a target not active on this platform");
      }

      recommendations.push({
        package: first.package,
        ecosystem: first.ecosystem,
        currentVersion: first.currentVersion,
        targetVersion: fixedVersions.length > 0 ? maxSemver(fixedVersions) : null,
        upgradeType: "unknown",
        risk: severityToRisk(entries.map((e) => e.severity)),
        reasons,
        breaking: false,
        isDev: entries.every((e) => e.isDev),
        scope: "transitive",
        action: "waiting_on_upstream",
        platformActive,
      });
    }
  }

  // Filter by risk threshold
  const threshold = params.risk_threshold ?? "all";
  const filtered = threshold === "all"
    ? recommendations
    : recommendations.filter((r) => RISK_LEVELS[r.risk] >= RISK_LEVELS[threshold]);

  // Sort: critical > high > medium > low, then fixable-now (direct) before
  // waiting-on-upstream (transitive), then non-breaking first (quick wins).
  filtered.sort((a, b) => {
    const riskDiff = RISK_LEVELS[b.risk] - RISK_LEVELS[a.risk];
    if (riskDiff !== 0) return riskDiff;
    if (a.scope !== b.scope) return a.scope === "direct" ? -1 : 1;
    if (a.breaking !== b.breaking) return a.breaking ? 1 : -1;
    return 0;
  });

  const maxRecs = params.max_recommendations ?? 20;
  const limited = filtered.slice(0, maxRecs);
  const direct = limited.filter((r) => r.scope === "direct");
  const quickWins = direct.filter((r) => !r.breaking).length;
  const breakingChanges = direct.filter((r) => r.breaking).length;
  const waitingOnUpstream = limited.filter((r) => r.scope === "transitive").length;

  // Summary
  const parts: string[] = [];
  parts.push(`${limited.length} recommendation${limited.length !== 1 ? "s" : ""}`);
  if (direct.length > 0) parts.push(`${direct.length} fixable directly (${quickWins} quick win${quickWins !== 1 ? "s" : ""}, ${breakingChanges} breaking)`);
  if (waitingOnUpstream > 0) parts.push(`${waitingOnUpstream} transitive, waiting on upstream`);
  const criticalCount = limited.filter((r) => r.risk === "critical").length;
  if (criticalCount > 0) parts.push(`${criticalCount} critical`);
  if (!vulnerabilityDataAvailable) {
    parts.push("CVE data not loaded — run vulnerability_scan first for a security-aware plan");
  }

  return {
    generatedAt: new Date().toISOString(),
    projectPath: liveIntel.getProjectRoot() ?? vulnResult?.projectPath ?? process.cwd(),
    totalDeps: deps.length,
    recommendations: limited,
    summary: parts.join(". ") + ".",
    quickWins,
    breakingChanges,
    waitingOnUpstream,
    vulnerabilityDataAvailable,
    provenance: { mode: "standalone_heuristic", note: PROVENANCE_NOTE },
  };
}
