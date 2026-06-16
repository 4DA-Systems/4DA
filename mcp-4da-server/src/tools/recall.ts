// SPDX-License-Identifier: Apache-2.0
/**
 * Ranked lexical recall for small local knowledge tables.
 *
 * This is not embedding search. It is a deterministic bridge that searches
 * all meaningful text fields and ranks matches until decision/memory
 * embeddings get their own schema and vec index.
 */

export interface RecallField<T> {
  name: string;
  weight: number;
  value: (row: T) => unknown;
}

export interface RankedRecall<T> {
  row: T;
  score: number;
  matched_fields: string[];
}

const STOP_WORDS = new Set([
  "about",
  "after",
  "again",
  "against",
  "before",
  "between",
  "from",
  "into",
  "should",
  "that",
  "their",
  "there",
  "this",
  "with",
  "would",
]);

const TERM_ALIASES: Record<string, string[]> = {
  auth: ["authentication", "authorization", "jwt", "oauth"],
  authentication: ["auth"],
  authorization: ["auth", "authz"],
  authz: ["authorization"],
  js: ["javascript"],
  javascript: ["js"],
  postgres: ["postgresql"],
  postgresql: ["postgres"],
  py: ["python"],
  python: ["py"],
  sqlite: ["sqlite3"],
  sqlite3: ["sqlite"],
  ts: ["typescript"],
  typescript: ["ts"],
};

export function normalizeRecallText(input: unknown): string {
  if (input === null || input === undefined) return "";
  const text = Array.isArray(input) ? input.join(" ") : String(input);
  return text
    .toLowerCase()
    .replace(/[^a-z0-9+#]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export function recallTerms(query: string): string[] {
  const terms = new Set<string>();
  const normalized = normalizeRecallText(query);

  for (const token of normalized.split(" ")) {
    if (token.length < 3 || STOP_WORDS.has(token)) continue;
    terms.add(token);
    for (const alias of TERM_ALIASES[token] || []) {
      terms.add(alias);
    }
  }

  return [...terms];
}

export function matchesRecallQuery(text: unknown, query: string): boolean {
  return scoreRecallText(text, query) > 0;
}

export function rankRowsByRecall<T>(
  rows: T[],
  query: string,
  fields: RecallField<T>[],
  limit: number,
): RankedRecall<T>[] {
  const scoreText = createRelevanceScorer(query);
  const ranked = rows
    .map((row) => {
      let score = 0;
      const matched = new Set<string>();

      for (const field of fields) {
        const fieldScore = scoreText(safeValue(field, row));
        if (fieldScore > 0) {
          score += fieldScore * field.weight;
          matched.add(field.name);
        }
      }

      return { row, score, matched_fields: [...matched] };
    })
    .filter((item) => item.score > 0)
    .sort((a, b) => b.score - a.score);

  return ranked.slice(0, Math.max(1, limit));
}

function safeValue<T>(field: RecallField<T>, row: T): unknown {
  try {
    return field.value(row);
  } catch {
    return "";
  }
}

/**
 * Build a reusable relevance scorer for a single query. The query's normalized
 * phrase and alias-expanded terms are computed ONCE, so callers that score many
 * texts against the same query (ranking 300+ rows x 5 fields, or filtering a
 * briefing's advisories/windows/news) do not re-tokenize the query per text.
 *
 * Exported so every relevance check in the server shares one alias-aware scorer
 * instead of re-implementing substring matching (which silently drops aliases
 * like auth -> jwt).
 */
export function createRelevanceScorer(query: string): (text: unknown) => number {
  const phrase = normalizeRecallText(query);
  const hasPhrase = phrase.length >= 3;
  const terms = recallTerms(query);

  return (text: unknown): number => {
    if (!hasPhrase && terms.length === 0) return 0;

    const haystack = normalizeRecallText(text);
    if (!haystack) return 0;

    let score = 0;
    if (hasPhrase && haystack.includes(phrase)) {
      score += 4;
    }

    const haystackTerms = new Set(haystack.split(" "));
    for (const term of terms) {
      if (haystackTerms.has(term)) {
        score += 2;
      } else if (haystack.includes(term)) {
        score += 1;
      }
    }

    return score;
  };
}

function scoreRecallText(text: unknown, query: string): number {
  return createRelevanceScorer(query)(text);
}
