// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * project_health tool
 *
 * Get project health summary across all tracked projects.
 */

import type { FourDADatabase } from "../db.js";
import type { ProjectSummaryRow, DependencyRow, CountRow } from "../types.js";
import { getLiveIntelligence } from "../live-singleton.js";

export interface ProjectHealthParams {
  project_path?: string;
}

export const projectHealthTool = {
  name: "project_health",
  description: `Get project health radar - dependency freshness, security exposure, ecosystem momentum, and community signals for your tracked projects.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      project_path: {
        type: "string",
        description: "Specific project path to check. Omit for all projects.",
      },
    },
  },
};

export function executeProjectHealth(
  db: FourDADatabase,
  params: ProjectHealthParams,
) {
  const rawDb = db.getRawDb();

  // Get all projects with their dependency counts
  let query = `SELECT project_path, COUNT(*) as dep_count,
                      GROUP_CONCAT(package_name, ', ') as packages
               FROM project_dependencies`;

  const queryParams: string[] = [];
  if (params.project_path) {
    query += " WHERE project_path = ?";
    queryParams.push(params.project_path);
  }
  query += " GROUP BY project_path";

  let projects: ProjectSummaryRow[];
  try {
    projects = rawDb.prepare(query).all(...queryParams) as ProjectSummaryRow[];
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    return {
      projects: [],
      total_projects: 0,
      summary: "Project health unavailable — database may need updating",
      error: msg,
    };
  }

  const healthReports = projects.map((proj) => {
    // Get dependencies for this project
    const deps = rawDb
      .prepare(
        "SELECT package_name, version, language, is_dev FROM project_dependencies WHERE project_path = ?",
      )
      .all(proj.project_path) as DependencyRow[];

    // Security: use live OSV data if available, fall back to source_items pattern matching
    let securityIssues = 0;
    const depVulnDetails: Array<{ name: string; vulnCount: number; highestSeverity: string }> = [];
    const liveIntel = getLiveIntelligence();
    const vulnResult = liveIntel?.getVulnerabilities();

    if (vulnResult && vulnResult.vulnerabilities.length > 0) {
      // Use real vulnerability data from OSV
      const vulnByPkg = new Map<string, { count: number; severity: string }>();
      for (const v of vulnResult.vulnerabilities) {
        const existing = vulnByPkg.get(v.package);
        if (!existing) {
          vulnByPkg.set(v.package, { count: 1, severity: v.severity });
        } else {
          existing.count++;
          const order: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1, unknown: 0 };
          if ((order[v.severity] || 0) > (order[existing.severity] || 0)) {
            existing.severity = v.severity;
          }
        }
      }
      for (const dep of deps) {
        const vuln = vulnByPkg.get(dep.package_name);
        if (vuln) {
          securityIssues++;
          depVulnDetails.push({ name: dep.package_name, vulnCount: vuln.count, highestSeverity: vuln.severity });
        }
      }
    } else {
      // Fallback: search source_items for security mentions (only if table has expected columns)
      try {
        for (const dep of deps.slice(0, 20)) {
          const pattern = `%${dep.package_name}%`;
          const securityMentions = rawDb
            .prepare(
              `SELECT COUNT(*) as cnt FROM source_items
               WHERE (title LIKE ? OR content LIKE ?)
               AND (title LIKE '%CVE%' OR title LIKE '%vulnerability%')`,
            )
            .get(pattern, pattern) as CountRow | undefined;
          if (securityMentions && securityMentions.cnt > 0) securityIssues++;
        }
      } catch {
        // source_items table may not exist or have different schema in standalone
      }
    }

    const securityScore = deps.length > 0 ? 1.0 - securityIssues / deps.length : 1.0;

    return {
      project_path: proj.project_path,
      project_name: proj.project_path.split(/[/\\]/).pop() || proj.project_path,
      dependency_count: proj.dep_count,
      dependencies: deps.slice(0, 20).map((d) => ({
        name: d.package_name,
        version: d.version,
        language: d.language,
      })),
      health: {
        security_score: Math.max(0, securityScore),
        security_issues: securityIssues,
        vulnerable_packages: depVulnDetails.length > 0 ? depVulnDetails : undefined,
        data_source: vulnResult ? "osv.dev" : "source_items",
      },
    };
  });

  return {
    projects: healthReports,
    total_projects: healthReports.length,
    summary: `${healthReports.length} project${healthReports.length !== 1 ? "s" : ""} tracked`,
  };
}
