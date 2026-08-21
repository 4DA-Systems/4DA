// SPDX-License-Identifier: Apache-2.0
/**
 * ecosystem_pulse tool
 *
 * Surfaces live ecosystem news relevant to the user's tech stack.
 * Data is already fetched on server startup from Hacker News via Algolia API.
 * This tool makes it queryable.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";

export interface EcosystemPulseParams {
  min_points?: number;
  limit?: number;
}

interface EcosystemPulseResult {
  headlines: Array<{
    title: string;
    url: string | null;
    points: number;
    comments: number;
    published: string;
    relevance_score: number;
    relevance_reason: string;
    hn_discussion: string;
  }>;
  total: number;
  source: string;
  note: string;
}

export const ecosystemPulseTool = {
  name: "ecosystem_pulse",
  description:
    "Live ecosystem news relevant to your tech stack. Surfaces trending Hacker News discussions filtered by your detected technologies. Updated on server startup.",
  inputSchema: {
    type: "object" as const,
    properties: {
      min_points: {
        type: "number",
        description: "Minimum HN points to include. Default: 0",
      },
      limit: {
        type: "number",
        description: "Maximum headlines to return. Default: 15",
      },
    },
  },
};

/**
 * Derive the tech stack used to filter HN headlines from the database:
 * explicit tech_stack entries plus the languages of tracked dependencies.
 * Works on both the desktop DB and the standalone minimal schema.
 */
export function deriveTechStackForHeadlines(db: FourDADatabase): string[] {
  const stack = new Set<string>();
  const rawDb = db.getRawDb();
  try {
    for (const row of rawDb.prepare("SELECT technology FROM tech_stack").all() as Array<{ technology: string }>) {
      if (row.technology) stack.add(row.technology.toLowerCase());
    }
  } catch {
    // tech_stack may not exist on exotic DBs — languages below still apply.
  }
  try {
    for (const row of rawDb.prepare("SELECT DISTINCT language FROM project_dependencies").all() as Array<{ language: string }>) {
      if (row.language) stack.add(row.language.toLowerCase());
    }
  } catch {
    // project_dependencies may not exist — an empty stack yields an honest empty result.
  }
  return [...stack];
}

export async function executeEcosystemPulse(
  db: FourDADatabase,
  params: EcosystemPulseParams,
  liveIntel: LiveIntelligence | null,
): Promise<EcosystemPulseResult> {
  if (!liveIntel) {
    return {
      headlines: [],
      total: 0,
      source: "hacker_news",
      note: "Live intelligence not available. Set FOURDA_OFFLINE=false and restart.",
    };
  }

  // The startup prefetch only ever ran in standalone mode, so full-DB servers
  // returned an empty cache forever. Fetch on demand when the cache is empty —
  // the cache still serves warm repeat calls.
  let headlines = liveIntel.getHeadlines();
  if (headlines.length === 0 && liveIntel.isEnabled()) {
    const techStack = deriveTechStackForHeadlines(db);
    if (techStack.length > 0) {
      headlines = await liveIntel.fetchHeadlines(techStack);
    }
  }
  const minPoints = params.min_points ?? 0;
  const limit = params.limit ?? 15;

  const filtered = headlines
    .filter((h) => h.points >= minPoints)
    .slice(0, limit)
    .map((h) => ({
      title: h.title,
      url: h.url,
      points: h.points,
      comments: h.comments,
      published: h.published,
      relevance_score: h.relevanceScore,
      relevance_reason: h.relevanceReason,
      hn_discussion: `https://news.ycombinator.com/item?id=${h.id}`,
    }));

  return {
    headlines: filtered,
    total: filtered.length,
    source: "hacker_news",
    note: filtered.length === 0
      ? "No relevant headlines found for your tech stack. Headlines are filtered by detected technologies."
      : `${filtered.length} headline${filtered.length !== 1 ? "s" : ""} relevant to your tech stack.`,
  };
}
