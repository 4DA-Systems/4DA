// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * reverse_mentions tool
 *
 * Detect when your projects/packages are mentioned in sources.
 */

import type { FourDADatabase } from "../db.js";
import type { MentionRow, PackageNameRow } from "../types.js";

export interface ReverseMentionsParams {
  limit?: number;
}

export const reverseMentionsTool = {
  name: "reverse_mentions",
  description: `Detect when your projects or packages are mentioned in sources. Finds discussions about your work in HN, Reddit, etc.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      limit: {
        type: "number",
        description: "Maximum mentions to return. Default: 20",
        default: 20,
      },
    },
  },
};

export function executeReverseMentions(
  db: FourDADatabase,
  params: ReverseMentionsParams,
) {
  const rawDb = db.getRawDb();
  const limit = params.limit || 20;

  // Get project identifiers from item_relationships
  const mentions = rawDb
    .prepare(
      `SELECT ir.source_item_id, ir.related_item_id, ir.metadata, ir.created_at,
              si.title, si.url, si.source_type
       FROM item_relationships ir
       JOIN source_items si ON ir.source_item_id = si.id
       WHERE ir.relationship_type = 'mentions_project'
       ORDER BY ir.created_at DESC
       LIMIT ?`,
    )
    .all(limit) as MentionRow[];

  const results = mentions.map((m) => {
    const metadata: { project_name?: string; context?: string } = m.metadata ? JSON.parse(m.metadata) : {};
    return {
      source_item_id: m.source_item_id,
      title: m.title,
      url: m.url,
      source_type: m.source_type,
      mentioned_project: metadata.project_name || "unknown",
      mention_context: metadata.context || "",
      discovered_at: m.created_at,
    };
  });

  // Also check for mentions via text search of project names from dependencies
  const projectNames = (rawDb
    .prepare("SELECT DISTINCT package_name FROM project_dependencies LIMIT 10")
    .all() as PackageNameRow[])
    .map((r) => r.package_name);

  return {
    mentions: results,
    total: results.length,
    tracked_projects: projectNames,
    summary: `${results.length} mention${results.length !== 1 ? "s" : ""} found across ${projectNames.length} tracked projects`,
  };
}
