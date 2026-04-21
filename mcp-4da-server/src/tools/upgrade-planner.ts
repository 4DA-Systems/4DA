// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * upgrade_planner tool
 *
 * Ranked upgrade recommendations for project dependencies.
 * Combines version freshness, vulnerability data, and deprecation status
 * to produce a prioritized upgrade plan.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";

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
}

interface UpgradePlanResult {
  generatedAt: string;
  projectPath: string;
  totalDeps: number;
  recommendations: UpgradeRecommendation[];
  summary: string;
  quickWins: number;
  breakingChanges: number;
}

export const upgradePlannerTool = {
  name: "upgrade_planner",
  description:
    "Ranked upgrade recommendations for dependencies. Prioritizes by vulnerability severity, deprecation, and version distance. Shows quick wins (patch/minor) vs breaking changes (major).",
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
    };
  }

  const includeDev = params.include_dev ?? false;
  let deps = liveIntel.getResolvedDeps();
  if (!includeDev) deps = deps.filter((d) => !d.isDev);

  // Fetch registry data
  const registryData = await liveIntel.fetchRegistryHealth(deps);

  // Get vulnerability data
  const vulnResult = liveIntel.getVulnerabilities();
  const vulnsByPackage = new Map<string, Array<{ severity: string; vulnId: string; summary: string; fixedVersion: string | null }>>();
  if (vulnResult) {
    for (const v of vulnResult.vulnerabilities) {
      if (!vulnsByPackage.has(v.package)) vulnsByPackage.set(v.package, []);
      vulnsByPackage.get(v.package)!.push({
        severity: v.severity,
        vulnId: v.vulnId,
        summary: v.summary,
        fixedVersion: v.fixedVersion,
      });
    }
  }

  // Build recommendations
  const recommendations: UpgradeRecommendation[] = [];

  for (const dep of registryData) {
    const reasons: string[] = [];
    let risk: UpgradeRecommendation["risk"] = "low";
    let targetVersion = dep.latestStableVersion || dep.latestVersion;

    // Check vulnerabilities
    const vulns = vulnsByPackage.get(dep.name);
    if (vulns && vulns.length > 0) {
      const hasCritical = vulns.some((v) => v.severity === "critical");
      const hasHigh = vulns.some((v) => v.severity === "high");
      if (hasCritical) risk = "critical";
      else if (hasHigh) risk = "high";
      else risk = "medium";

      reasons.push(`${vulns.length} known CVE${vulns.length !== 1 ? "s" : ""} (${vulns.map((v) => v.vulnId).join(", ")})`);

      // Use fixed version as target if available
      const fixedVersions = vulns.map((v) => v.fixedVersion).filter(Boolean);
      if (fixedVersions.length > 0 && !targetVersion) {
        targetVersion = fixedVersions[0];
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

    // Skip if no upgrade needed
    if (reasons.length === 0) continue;
    if (dep.versionsBehind?.label === "up-to-date" && !dep.deprecated && !vulns) continue;

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
    });
  }

  // Filter by risk threshold
  const riskLevels: Record<string, number> = { low: 0, medium: 1, high: 2, critical: 3 };
  const threshold = params.risk_threshold ?? "all";
  const filtered = threshold === "all"
    ? recommendations
    : recommendations.filter((r) => riskLevels[r.risk] >= riskLevels[threshold]);

  // Sort: critical > high > medium > low, then by breaking (non-breaking first for quick wins)
  filtered.sort((a, b) => {
    const riskDiff = riskLevels[b.risk] - riskLevels[a.risk];
    if (riskDiff !== 0) return riskDiff;
    if (a.breaking !== b.breaking) return a.breaking ? 1 : -1;
    return 0;
  });

  const maxRecs = params.max_recommendations ?? 20;
  const limited = filtered.slice(0, maxRecs);
  const quickWins = limited.filter((r) => !r.breaking).length;
  const breakingChanges = limited.filter((r) => r.breaking).length;

  // Summary
  const parts: string[] = [];
  parts.push(`${limited.length} upgrade${limited.length !== 1 ? "s" : ""} recommended`);
  if (quickWins > 0) parts.push(`${quickWins} quick win${quickWins !== 1 ? "s" : ""} (patch/minor)`);
  if (breakingChanges > 0) parts.push(`${breakingChanges} breaking change${breakingChanges !== 1 ? "s" : ""} (major)`);
  const criticalCount = limited.filter((r) => r.risk === "critical").length;
  if (criticalCount > 0) parts.push(`${criticalCount} critical`);

  return {
    generatedAt: new Date().toISOString(),
    projectPath: process.cwd(),
    totalDeps: deps.length,
    recommendations: limited,
    summary: parts.join(". ") + ".",
    quickWins,
    breakingChanges,
  };
}
