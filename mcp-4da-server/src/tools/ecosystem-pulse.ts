// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

export function executeEcosystemPulse(
  _db: FourDADatabase,
  params: EcosystemPulseParams,
  liveIntel: LiveIntelligence | null,
): EcosystemPulseResult {
  if (!liveIntel) {
    return {
      headlines: [],
      total: 0,
      source: "hacker_news",
      note: "Live intelligence not available. Set FOURDA_OFFLINE=false and restart.",
    };
  }

  const headlines = liveIntel.getHeadlines();
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
