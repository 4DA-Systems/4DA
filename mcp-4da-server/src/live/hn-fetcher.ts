// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Hacker News Headline Fetcher
 *
 * Queries HN Algolia API for headlines relevant to the user's tech stack.
 * Free, no auth. Generates search queries from detected technologies.
 *
 * Privacy: sends generic tech keywords (e.g. "react", "rust") — not personal data.
 */

import type { LiveCache } from "./cache.js";
import type { RateLimiter } from "./rate-limiter.js";
import type { LiveHeadline } from "./types.js";

const HN_ALGOLIA_URL = "https://hn.algolia.com/api/v1/search";
const HN_TIMEOUT_MS = 8_000;
const HN_CACHE_TTL = 1800; // 30 minutes
const MAX_QUERIES = 5;
const RESULTS_PER_QUERY = 10;
const MIN_POINTS = 10;

interface HNHit {
  objectID: string;
  title: string;
  url: string | null;
  points: number;
  num_comments: number;
  created_at: string;
  author: string;
}

interface HNResponse {
  hits: HNHit[];
}

export class HNFetcher {
  private cache: LiveCache;
  private rateLimiter: RateLimiter;

  constructor(cache: LiveCache, rateLimiter: RateLimiter) {
    this.cache = cache;
    this.rateLimiter = rateLimiter;
  }

  async fetch(techStack: string[]): Promise<LiveHeadline[]> {
    const cacheKey = `hn:${techStack.slice(0, MAX_QUERIES).sort().join(",")}`;
    const cached = this.cache.get<LiveHeadline[]>(cacheKey);
    if (cached !== null) return cached;

    if (!this.rateLimiter.canProceed("hn")) {
      const stale = this.cache.getStale<LiveHeadline[]>(cacheKey);
      return stale?.data || [];
    }

    const queries = buildQueries(techStack);
    const allHits: HNHit[] = [];
    const seen = new Set<string>();

    for (const query of queries) {
      if (!this.rateLimiter.consume("hn")) break;

      try {
        const hits = await this.searchHN(query);
        for (const hit of hits) {
          if (!seen.has(hit.objectID)) {
            seen.add(hit.objectID);
            allHits.push(hit);
          }
        }
      } catch {
        // Individual query failure — continue with others
      }
    }

    const headlines = scoreAndMap(allHits, techStack);
    this.cache.set(cacheKey, headlines, "hn", HN_CACHE_TTL);
    return headlines;
  }

  private async searchHN(query: string): Promise<HNHit[]> {
    const params = new URLSearchParams({
      query,
      tags: "story",
      numericFilters: `points>${MIN_POINTS}`,
      hitsPerPage: String(RESULTS_PER_QUERY),
    });

    const url = `${HN_ALGOLIA_URL}?${params}`;
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), HN_TIMEOUT_MS);

    try {
      const response = await fetch(url, { signal: controller.signal });
      if (!response.ok) return [];
      const data = (await response.json()) as HNResponse;
      return data.hits || [];
    } finally {
      clearTimeout(timeout);
    }
  }
}

function buildQueries(techStack: string[]): string[] {
  // Take top technologies, prioritize frameworks over languages
  const prioritized = [...techStack]
    .filter((t) => t.length >= 2 && !SKIP_TERMS.has(t))
    .slice(0, MAX_QUERIES);

  return prioritized;
}

function wordBoundaryMatch(text: string, term: string): boolean {
  const idx = text.indexOf(term);
  if (idx === -1) return false;
  const before = idx === 0 || !/[a-zA-Z0-9]/.test(text[idx - 1]);
  const after = idx + term.length >= text.length || !/[a-zA-Z0-9]/.test(text[idx + term.length]);
  return before && after;
}

function scoreAndMap(hits: HNHit[], techStack: string[]): LiveHeadline[] {
  const techLower = techStack.map((t) => t.toLowerCase());

  return hits
    .map((hit) => {
      const titleLower = hit.title.toLowerCase();
      const matchedTech = techLower.filter((t) => wordBoundaryMatch(titleLower, t));
      const techBoost = Math.min(matchedTech.length * 0.2, 0.4);
      const pointsBoost = Math.min(hit.points / 500, 0.3);
      const commentsBoost = Math.min(hit.num_comments / 200, 0.2);
      const score = Math.min(0.1 + techBoost + pointsBoost + commentsBoost, 1.0);

      return {
        id: hit.objectID,
        title: hit.title,
        url: hit.url,
        source: "hacker_news" as const,
        points: hit.points,
        comments: hit.num_comments,
        published: hit.created_at,
        relevanceScore: Math.round(score * 100) / 100,
        relevanceReason: matchedTech.length > 0
          ? `Matches your stack: ${matchedTech.join(", ")}`
          : `Trending (${hit.points} points)`,
      };
    })
    .sort((a, b) => b.relevanceScore - a.relevanceScore)
    .slice(0, 15);
}

const SKIP_TERMS = new Set([
  "javascript", "typescript", // too generic, floods results
  "docker", // too broad
]);
