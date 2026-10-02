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
import { COMMON_WORDS } from "./briefing-task-scope.js";

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

/** Languages: matching a headline on one is matching the whole language's news. */
const LANGUAGES = new Set(["rust", "javascript", "typescript", "python", "go", "golang", "java", "kotlin", "ruby", "php", "c", "c++", "c#", "swift", "dart", "npm"]);
const MAX_TERMS = 8;

/**
 * The terms HN headlines are searched and matched on: the frameworks the
 * project uses and its most-used direct dependencies — names specific enough
 * that a headline naming one is about something this project runs. Language
 * names are the fallback only: matched on "rust", the pulse was "What Zig felt
 * like, coming from Rust" and an Emacs fork (live 2026-10-01). Package names
 * that are everyday words are left out (they match unrelated titles).
 * Works on both the desktop DB and the standalone minimal schema.
 */
export function deriveTechStackForHeadlines(db: FourDADatabase): string[] {
  const terms: string[] = [];
  const add = (term: string | null | undefined) => {
    const t = (term ?? "").toLowerCase().trim();
    if (t.length >= 3 && !LANGUAGES.has(t) && !COMMON_WORDS.has(t) && !terms.includes(t) && !t.includes("/")) terms.push(t);
  };
  const rawDb = db.getRawDb();
  try {
    for (const row of rawDb
      .prepare("SELECT name FROM detected_tech WHERE category IN ('framework', 'library') ORDER BY confidence DESC LIMIT 20")
      .all() as Array<{ name: string }>) {
      add(row.name);
    }
  } catch {
    // detected_tech may not exist on exotic DBs.
  }
  try {
    const direct = db.hasColumn("project_dependencies", "is_direct") ? "WHERE is_direct = 1" : "";
    for (const row of rawDb
      .prepare(
        `SELECT package_name FROM project_dependencies ${direct}
         GROUP BY LOWER(package_name) ORDER BY COUNT(DISTINCT project_path) DESC, package_name LIMIT 40`,
      )
      .all() as Array<{ package_name: string }>) {
      if (terms.length >= MAX_TERMS) break;
      add(row.package_name);
    }
  } catch {
    // project_dependencies may not exist.
  }
  // Then the project's languages, as language-level news (labelled so by the
  // fetcher): HN titles rarely name a library in a two-week window, and an
  // asked-for ecosystem pulse should still say what moved in the language.
  const specific = terms.slice(0, MAX_TERMS - 2);
  const languages: string[] = [];
  try {
    for (const row of rawDb.prepare("SELECT DISTINCT language FROM project_dependencies").all() as Array<{ language: string }>) {
      const term = (row.language ?? "").toLowerCase();
      if (term && PULSE_LANGUAGES.has(term) && !languages.includes(term)) languages.push(term);
    }
  } catch {
    // An empty stack yields an honest empty result.
  }
  return [...specific, ...languages.slice(0, MAX_TERMS - specific.length)];
}

/**
 * Languages with distinctive enough names to search HN for: not
 * "javascript"/"typescript" (they flood), not "go" (an everyday word in titles).
 */
const PULSE_LANGUAGES = new Set(["rust", "python", "zig", "kotlin", "swift", "ruby", "elixir"]);

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

  // Fetched when asked for, never at startup: the search terms are names from
  // the user's projects, and HN should see them only when the user asks for
  // the pulse. The cache serves warm repeat calls.
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
