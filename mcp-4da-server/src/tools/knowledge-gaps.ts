/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps - dependencies with relevant content you haven't engaged with.
 */

import type { FourDADatabase } from "../db.js";
import type { DependencyWithProjectRow, SourceItemBriefRow } from "../types.js";

export interface KnowledgeGapsParams {
  min_severity?: string;
}

export const knowledgeGapsTool = {
  name: "knowledge_gaps",
  description: `Detect knowledge gaps by cross-referencing your project dependencies with source items you haven't engaged with. Identifies things you should know about but might have missed.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      min_severity: {
        type: "string",
        enum: ["critical", "high", "medium", "low"],
        description: "Minimum gap severity to include. Default: medium",
        default: "medium",
      },
    },
  },
};

export interface KnowledgeGap {
  dependency: string;
  version: string;
  project_path: string;
  language: string;
  missed_items: SourceItemBriefRow[];
  gap_severity: string;
  missed_count: number;
}

export function executeKnowledgeGaps(
  db: FourDADatabase,
  params: KnowledgeGapsParams,
) {
  const rawDb = db.getRawDb();

  // Get all tracked dependencies
  const deps = rawDb
    .prepare("SELECT package_name, version, project_path, language FROM project_dependencies")
    .all() as DependencyWithProjectRow[];

  if (deps.length === 0) {
    return {
      gaps: [],
      summary: "No project dependencies tracked. Add context directories to enable knowledge gap detection.",
    };
  }

  const gaps: KnowledgeGap[] = [];

  for (const dep of deps) {
    // Find source items mentioning this dependency
    const pattern = `%${dep.package_name}%`;
    const mentionedItems = rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, si.created_at
        FROM source_items si
        WHERE (si.title LIKE ? OR si.content LIKE ?)
        AND si.id NOT IN (SELECT source_item_id FROM interactions WHERE action IN ('click', 'save'))
        ORDER BY si.created_at DESC LIMIT 5`)
      .all(pattern, pattern) as SourceItemBriefRow[];

    if (mentionedItems.length > 0) {
      // Check if any are security-related
      const hasSecurityMention = mentionedItems.some(
        (item) =>
          item.title?.toLowerCase().includes("cve") ||
          item.title?.toLowerCase().includes("security") ||
          item.title?.toLowerCase().includes("vulnerability"),
      );

      const severity = hasSecurityMention
        ? "critical"
        : mentionedItems.length >= 3
          ? "high"
          : "medium";

      gaps.push({
        dependency: dep.package_name,
        version: dep.version,
        project_path: dep.project_path,
        language: dep.language,
        missed_items: mentionedItems,
        gap_severity: severity,
        missed_count: mentionedItems.length,
      });
    }
  }

  // Filter by severity
  const severityOrder: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
  const minLevel = severityOrder[params.min_severity || "medium"] || 2;
  const filtered = gaps.filter(
    (g) => (severityOrder[g.gap_severity] || 0) >= minLevel,
  );

  return {
    gaps: filtered.sort(
      (a, b) =>
        (severityOrder[b.gap_severity] || 0) - (severityOrder[a.gap_severity] || 0),
    ),
    total_dependencies: deps.length,
    gaps_found: filtered.length,
    summary: `${filtered.length} knowledge gaps across ${deps.length} tracked dependencies`,
  };
}
