// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * dependency_health tool
 *
 * Assesses health of project dependencies by checking version freshness,
 * deprecation status, and known vulnerabilities across npm, crates.io,
 * PyPI, and Go ecosystems. Zero config — auto-detects from lock files.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { DependencyHealthResult, RegistryPackageInfo } from "../live/types.js";

export interface DependencyHealthParams {
  include_dev?: boolean;
  ecosystem_filter?: string;
  sort_by?: "risk" | "name" | "outdated";
  limit?: number;
}

export const dependencyHealthTool = {
  name: "dependency_health",
  description:
    "Assess health of project dependencies — version freshness, deprecation status, known CVEs. Auto-detects stack from lock files. Covers npm, Rust, Python, and Go.",
  inputSchema: {
    type: "object" as const,
    properties: {
      include_dev: {
        type: "boolean",
        description: "Include devDependencies. Default: false",
      },
      ecosystem_filter: {
        type: "string",
        enum: ["npm", "crates.io", "PyPI", "Go"],
        description: "Only show deps from this ecosystem. Default: all detected.",
      },
      sort_by: {
        type: "string",
        enum: ["risk", "name", "outdated"],
        description: "Sort order. 'risk' prioritizes vulnerable+deprecated+outdated. Default: risk",
      },
      limit: {
        type: "number",
        description: "Max dependencies to return. Default: 50",
      },
    },
  },
};

export async function executeDependencyHealth(
  _db: FourDADatabase,
  params: DependencyHealthParams,
  liveIntel: LiveIntelligence | null,
): Promise<DependencyHealthResult> {
  const start = Date.now();

  if (!liveIntel || !liveIntel.isInitialized()) {
    return {
      scannedAt: new Date().toISOString(),
      projectPath: process.cwd(),
      ecosystemsScanned: [],
      totalDeps: 0,
      outdatedCount: 0,
      deprecatedCount: 0,
      vulnerableCount: 0,
      healthScore: 0,
      dependencies: [],
      vulnerabilitySummary: null,
      summary: "Live intelligence not initialized. No project manifests detected.",
      scanDurationMs: Date.now() - start,
      cached: false,
    };
  }

  // Get resolved deps
  const includeDev = params.include_dev ?? false;
  let deps = liveIntel.getResolvedDeps();
  if (!includeDev) {
    deps = deps.filter((d) => !d.isDev);
  }
  if (params.ecosystem_filter) {
    deps = deps.filter((d) => d.ecosystem === params.ecosystem_filter);
  }

  const ecosystems = [...new Set(deps.map((d) => d.ecosystem))];

  // Fetch registry health for all deps
  const registryData = await liveIntel.fetchRegistryHealth(deps);

  // Merge vulnerability data
  const vulnResult = liveIntel.getVulnerabilities();
  const vulnMap = new Map<string, number>();
  if (vulnResult) {
    for (const v of vulnResult.vulnerabilities) {
      vulnMap.set(v.package, (vulnMap.get(v.package) || 0) + 1);
    }
  }

  // Compute stats
  let outdated = 0;
  let deprecated = 0;
  let vulnerable = 0;

  for (const dep of registryData) {
    if (dep.versionsBehind && dep.versionsBehind.label !== "up-to-date") outdated++;
    if (dep.deprecated) deprecated++;
    if (vulnMap.has(dep.name)) vulnerable++;
  }

  // Sort
  const sortBy = params.sort_by ?? "risk";
  const sorted = [...registryData].sort((a, b) => {
    if (sortBy === "name") return a.name.localeCompare(b.name);
    if (sortBy === "outdated") {
      const aD = a.versionsBehind ? a.versionsBehind.major * 100 + a.versionsBehind.minor * 10 + a.versionsBehind.patch : 0;
      const bD = b.versionsBehind ? b.versionsBehind.major * 100 + b.versionsBehind.minor * 10 + b.versionsBehind.patch : 0;
      return bD - aD;
    }
    // risk: vulnerable > deprecated > major outdated > minor > patch > current
    const riskScore = (dep: RegistryPackageInfo) => {
      let score = 0;
      if (vulnMap.has(dep.name)) score += 1000;
      if (dep.deprecated) score += 500;
      if (dep.versionsBehind) {
        score += dep.versionsBehind.major * 100 + dep.versionsBehind.minor * 10 + dep.versionsBehind.patch;
      }
      return score;
    };
    return riskScore(b) - riskScore(a);
  });

  const limit = params.limit ?? 50;
  const limited = sorted.slice(0, limit);

  // Health score: 100 = perfect, 0 = everything is on fire
  const total = registryData.length || 1;
  const vulnPenalty = Math.min(vulnerable * 15, 50);
  const deprecatedPenalty = Math.min(deprecated * 10, 30);
  const outdatedPenalty = Math.min(outdated * 2, 40);
  const healthScore = Math.max(0, 100 - vulnPenalty - deprecatedPenalty - outdatedPenalty);

  const vulnSummary = vulnResult ? {
    critical: vulnResult.bySeverity.critical,
    high: vulnResult.bySeverity.high,
    medium: vulnResult.bySeverity.medium,
    low: vulnResult.bySeverity.low,
  } : null;

  // Summary
  const parts: string[] = [];
  parts.push(`${total} dependenc${total !== 1 ? "ies" : "y"} scanned`);
  if (vulnerable > 0) parts.push(`${vulnerable} vulnerable`);
  if (deprecated > 0) parts.push(`${deprecated} deprecated`);
  if (outdated > 0) parts.push(`${outdated} outdated`);
  if (vulnerable === 0 && deprecated === 0 && outdated === 0) parts.push("all healthy");

  return {
    scannedAt: new Date().toISOString(),
    projectPath: process.cwd(),
    ecosystemsScanned: ecosystems,
    totalDeps: total,
    outdatedCount: outdated,
    deprecatedCount: deprecated,
    vulnerableCount: vulnerable,
    healthScore,
    dependencies: limited,
    vulnerabilitySummary: vulnSummary,
    summary: `Health: ${healthScore}/100. ${parts.join(", ")}.`,
    scanDurationMs: Date.now() - start,
    cached: false,
  };
}
