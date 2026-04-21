// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_relevant_content tool
 *
 * Get filtered relevant content from 4DA's personalized feed.
 */

import type { FourDADatabase } from "../db.js";
import type { GetRelevantContentParams, RelevantItem } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getRelevantContentTool = {
  name: "get_relevant_content",
  description: `Get filtered relevant content from 4DA's personalized feed.

Returns content items that match the user's interests, tech stack, and learned preferences.
Items are scored based on:
- Explicit interests declared by the user
- Tech stack and domains
- ACE-detected context (recent files, projects, git activity)
- Learned preferences from past interactions

Each item includes necessity fields (necessity_score, necessity_reason, necessity_category, necessity_urgency) indicating how critical the item is — e.g., security vulnerabilities affecting your deps, breaking changes in your stack, or deprecation notices. These are populated from the full PASIFA analysis pipeline when available.

Use this to find content relevant to the user's current context.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      min_score: {
        type: "number",
        description: "Minimum relevance score (0.0-1.0). Default: 0.35",
        default: 0.35,
      },
      source_type: {
        type: "string",
        description:
          'Filter by source type: "hackernews", "arxiv", or "reddit". Leave empty for all sources.',
        enum: ["hackernews", "arxiv", "reddit"],
      },
      limit: {
        type: "number",
        description: "Maximum number of items to return. Default: 20, max: 100",
        default: 20,
      },
      since_hours: {
        type: "number",
        description: "Only include items discovered in the last N hours. Default: 24",
        default: 24,
      },
    },
  },
};

/**
 * Execute the get_relevant_content tool
 */
export function executeGetRelevantContent(
  db: FourDADatabase,
  params: GetRelevantContentParams
): RelevantItem[] {
  const minScore = Math.max(0, Math.min(1, params.min_score ?? 0.35));
  const limit = Math.max(1, Math.min(100, params.limit ?? 20));
  const sinceHours = Math.max(1, Math.min(168, params.since_hours ?? 24)); // Max 1 week

  // Try requested window first, then expand progressively
  let items = db.getRelevantContent(minScore, params.source_type, limit, sinceHours);
  if (items.length === 0 && sinceHours < 168) {
    items = db.getRelevantContent(minScore, params.source_type, limit, 168);
  }
  if (items.length === 0) {
    items = db.getRelevantContent(0.0, params.source_type, limit, 720);
  }
  return items;
}
