// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps - dependencies with relevant content you haven't engaged with.
 */

import type { FourDADatabase } from "../db.js";
import type { DependencyWithProjectRow, SourceItemBriefRow } from "../types.js";

// Word-boundary matching prevents "cve" matching inside "achieve", "receiver", etc.
function hasWordBoundary(text: string, term: string): boolean {
  const regex = new RegExp(`\\b${term}\\b`, 'i');
  return regex.test(text);
}

export interface KnowledgeGapsParams {
  min_severity?: string;
  limit?: number;
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
      limit: {
        type: "number",
        description: "Maximum gaps to return. Default: 15",
        default: 15,
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
    .prepare("SELECT package_name, version, project_path, language FROM project_dependencies LIMIT 100")
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
          hasWordBoundary(item.title || "", "cve") ||
          hasWordBoundary(item.title || "", "security") ||
          hasWordBoundary(item.title || "", "vulnerability"),
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
        missed_items: mentionedItems.map(item => ({
          ...item,
          title: item.title && item.title.length > 120 ? item.title.substring(0, 120) + "..." : item.title,
        })),
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

  const maxGaps = Math.min(Math.max(1, params.limit || 15), 50);

  return {
    gaps: filtered.sort(
      (a, b) =>
        (severityOrder[b.gap_severity] || 0) - (severityOrder[a.gap_severity] || 0),
    ).slice(0, maxGaps),
    total_dependencies: deps.length,
    gaps_found: filtered.length,
    gaps_returned: Math.min(filtered.length, maxGaps),
    summary: `${filtered.length} knowledge gaps across ${deps.length} tracked dependencies (showing top ${Math.min(filtered.length, maxGaps)})`,
  };
}
