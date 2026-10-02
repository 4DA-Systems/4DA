// SPDX-License-Identifier: Apache-2.0
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

Returns content items that match the user's explicit interests, tech stack, and ACE-detected context.
Items are scored based on:
- Explicit interests declared by the user
- Tech stack and domains
- ACE-detected context (recent files, projects, git activity)

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
        description: "Only items from this source. Leave empty for all sources.",
        // Every source the app fetches; the enum used to list three of them.
        enum: [
          "hackernews", "reddit", "lobsters", "devto", "arxiv", "huggingface", "papers_with_code",
          "github", "crates_io", "npm_registry", "pypi", "go_modules", "cve", "osv", "stackoverflow",
          "rss", "youtube", "mastodon", "lemmy", "bluesky", "twitter", "producthunt",
        ],
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
): Array<RelevantItem & { judge_relevance: number | null }> {
  const minScore = Math.max(0, Math.min(1, params.min_score ?? 0.35));
  const limit = Math.max(1, Math.min(100, params.limit ?? 20));
  const sinceHours = Math.max(1, Math.min(168, params.since_hours ?? 24)); // Max 1 week

  // Try requested window first, then expand progressively
  let items = db.getRelevantContent(minScore, params.source_type, limit, sinceHours);
  if (items.length === 0 && sinceHours < 168) {
    items = db.getRelevantContent(minScore, params.source_type, limit, 168);
  }
  if (items.length === 0) {
    // Deep 30-day fallback at zero floor reaches the stale-epoch tail after a
    // pipeline-version bump — require current-version scores so we never rank
    // numbers the live scoring brain doesn't stand behind.
    items = db.getRelevantContent(0.0, params.source_type, limit, 720, true);
  }
  // Items the relevance judge rejected are not "relevant content", whatever
  // their score: measured 2026-10-01, 151 of 950 feed-relevant items carried
  // a latest judge verdict below 0.5. Unjudged items stay (scored, not yet
  // judged) and carry `judge_relevance: null`.
  const verdicts = latestJudgeVerdicts(db, items.map((item) => item.id));
  return items
    .filter((item) => (verdicts.get(item.id) ?? 1) >= 0.5)
    .map((item) => ({ ...item, judge_relevance: verdicts.get(item.id) ?? null }));
}

/** Latest relevance-judge verdict per item, when the desktop app has judged it. */
function latestJudgeVerdicts(db: FourDADatabase, ids: number[]): Map<number, number> {
  const out = new Map<number, number>();
  if (ids.length === 0) return out;
  try {
    const rows = db
      .getRawDb()
      .prepare(
        `SELECT source_item_id AS id, relevance_score AS v FROM llm_judgments
         WHERE id IN (SELECT MAX(id) FROM llm_judgments WHERE source_item_id IN (${ids.map(() => "?").join(",")}) GROUP BY source_item_id)`,
      )
      .all(...ids) as Array<{ id: number; v: number }>;
    for (const row of rows) out.set(row.id, Math.round(row.v * 100) / 100);
  } catch {
    // No llm_judgments table: nothing judged.
  }
  return out;
}
