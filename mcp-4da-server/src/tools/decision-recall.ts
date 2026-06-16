// SPDX-License-Identifier: Apache-2.0
/**
 * Shared relevance retrieval for developer_decisions.
 *
 * Both decision tools (check_decision_alignment and decision_memory's
 * check_alignment) retrieve "decisions relevant to X" the same way, so the
 * ranking lives here once. Retrieval is lexical by default and OPTIONALLY hybrid
 * (alias-aware lexical blended with provider-backed embedding similarity) when an
 * embedding provider is configured — letting an agent surface a decision that is
 * a paraphrase of what it is about to propose, even with zero shared words.
 *
 * Note: this affects RETRIEVAL only. Hard "X was rejected" conflict detection
 * stays lexical/alias-aware in the tools, so semantic similarity can never
 * fabricate a grounded conflict claim.
 */

import type { FourDADatabase } from "../db.js";
import { rankRowsByRecall, type RecallField } from "./recall.js";
import { semanticScores, type EmbeddingConfig } from "../embeddings.js";

/** Minimum text fields a decision row must expose for ranking + embedding. */
export interface DecisionLike {
  id: number;
  subject: string;
  decision: string;
  rationale: string | null;
  /** JSON-encoded string[] */
  alternatives_rejected: string;
  /** JSON-encoded string[] */
  context_tags: string;
}

/** Weighted fields for lexical ranking — rejected alternatives matter most. */
export const DECISION_RECALL_FIELDS: RecallField<DecisionLike>[] = [
  { name: "alternatives", weight: 5, value: (row) => row.alternatives_rejected },
  { name: "subject", weight: 4, value: (row) => row.subject },
  { name: "tags", weight: 3, value: (row) => row.context_tags },
  { name: "decision", weight: 2, value: (row) => row.decision },
  { name: "rationale", weight: 1, value: (row) => row.rationale },
];

/** Blend weight for semantic cosine vs normalized lexical score. */
const SEMANTIC_BLEND = 0.65;

/**
 * A decision retrieved with at least this cosine similarity to the query, that
 * also has rejected alternatives, is surfaced as a POSSIBLE (paraphrase) conflict
 * for human review — distinct from a hard, lexically-grounded conflict.
 */
export const POSSIBLE_CONFLICT_THRESHOLD = 0.6;

export interface DecisionRecallResult<T extends DecisionLike> {
  /** Decisions ranked by relevance, capped to `limit`. */
  ranked: T[];
  /** id -> cosine similarity to the query (empty in pure-lexical mode). */
  semanticById: Map<number, number>;
  recall_mode: "hybrid" | "ranked_lexical";
}

/** Pure lexical ranking (the default, provider-free path). */
export function rankDecisionsLexical<T extends DecisionLike>(
  rows: T[],
  query: string,
  limit: number,
): T[] {
  return rankRowsByRecall(rows, query, DECISION_RECALL_FIELDS, limit).map((item) => item.row);
}

/**
 * The text embedded for a decision: everything that carries its meaning.
 * Exported so every surface that embeds a decision row (decision tools AND the
 * what_should_i_know briefing) uses IDENTICAL text — otherwise they would write
 * conflicting vectors into the shared developer_decisions.embedding column.
 */
export function decisionEmbedText(row: DecisionLike): string {
  let alts = "";
  try {
    alts = (JSON.parse(row.alternatives_rejected || "[]") as string[]).join(", ");
  } catch {
    // Malformed JSON — just skip the alternatives in the embed text.
  }
  return [row.subject, row.decision, row.rationale || "", alts].filter(Boolean).join("\n");
}

/**
 * Hybrid retrieval: blends alias-aware lexical scoring with embedding cosine
 * similarity. Falls back to pure lexical (and reports it) when the query cannot
 * be embedded.
 */
export async function hybridDecisionRecall<T extends DecisionLike>(
  db: FourDADatabase,
  query: string,
  rows: T[],
  limit: number,
  config: EmbeddingConfig,
): Promise<DecisionRecallResult<T>> {
  // Lexical baseline over ALL candidates (for blending + fallback).
  const lexical = rankRowsByRecall(rows, query, DECISION_RECALL_FIELDS, rows.length);
  const lexById = new Map<number, number>();
  for (const item of lexical) lexById.set(item.row.id, item.score);
  const maxLex = lexical.length ? lexical[0].score : 0;

  const sem = await semanticScores(
    db,
    "developer_decisions",
    query,
    rows.map((row) => ({ id: row.id, text: decisionEmbedText(row) })),
    config,
  );

  if (!sem.queryEmbedded || sem.embeddedCount === 0) {
    return {
      ranked: rankDecisionsLexical(rows, query, limit),
      semanticById: new Map(),
      recall_mode: "ranked_lexical",
    };
  }

  const ranked = rows
    .map((row) => {
      const semantic = Math.max(0, sem.semanticById.get(row.id) ?? 0);
      const lexNorm = maxLex > 0 ? (lexById.get(row.id) ?? 0) / maxLex : 0;
      const score = SEMANTIC_BLEND * semantic + (1 - SEMANTIC_BLEND) * lexNorm;
      return { row, score };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((x) => x.row);

  return { ranked, semanticById: sem.semanticById, recall_mode: "hybrid" };
}
