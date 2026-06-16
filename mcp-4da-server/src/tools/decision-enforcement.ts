// SPDX-License-Identifier: Apache-2.0
/**
 * check_decision_alignment tool
 *
 * The key tool AI agents call BEFORE suggesting major changes.
 * Checks if a technology or pattern aligns with the developer's active decisions.
 * Returns alignment status, relevant decisions, and any conflicts.
 *
 * Retrieval of "relevant decisions" is lexical by default and OPTIONALLY hybrid
 * (semantic) when an embedding provider is configured — so a decision that is a
 * paraphrase of the proposal surfaces even with no shared words. Hard conflict
 * detection ("X was explicitly rejected") stays lexical/alias-aware so semantic
 * similarity can never fabricate a grounded conflict; paraphrase risks are
 * reported separately as `possible_conflicts`.
 */

import type { FourDADatabase } from "../db.js";
import { matchesRecallQuery } from "./recall.js";
import { getEmbeddingConfig } from "../embeddings.js";
import {
  rankDecisionsLexical,
  hybridDecisionRecall,
  POSSIBLE_CONFLICT_THRESHOLD,
} from "./decision-recall.js";

// ============================================================================
// Types
// ============================================================================

export interface CheckDecisionAlignmentParams {
  technology: string;
  pattern?: string;
  context?: string;
}

interface DecisionRow {
  id: number;
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string;
  context_tags: string;
  confidence: number;
  status: string;
  superseded_by: number | null;
  created_at: string;
  updated_at: string;
}

interface RelevantDecision {
  id: number;
  subject: string;
  decision: string;
  rationale: string | null;
  confidence: number;
  relationship: "aligned" | "conflict" | "related";
}

interface DecisionConflict {
  decision_id: number;
  subject: string;
  reason: string;
}

interface PossibleConflict {
  decision_id: number;
  subject: string;
  similarity: number;
  reason: string;
}

interface AlignmentResult {
  aligned: boolean;
  technology: string;
  relevant_decisions: RelevantDecision[];
  conflicts: DecisionConflict[];
  /** Paraphrase risks: semantically close to a rejected alternative, not grounded. */
  possible_conflicts: PossibleConflict[];
  confidence: number;
  recommendation: string;
  recall_mode: "hybrid" | "ranked_lexical";
}

// ============================================================================
// Tool Definition
// ============================================================================

export const checkDecisionAlignmentTool = {
  name: "check_decision_alignment",
  description:
    "Check if a technology or pattern aligns with the developer's active decisions. Call BEFORE suggesting major tech changes. Returns alignment status, relevant decisions, and any conflicts.",
  inputSchema: {
    type: "object" as const,
    properties: {
      technology: {
        type: "string",
        description:
          "Technology name to check (e.g., 'postgresql', 'redis', 'graphql')",
      },
      pattern: {
        type: "string",
        description:
          "Architecture pattern to check (e.g., 'microservices', 'event-driven')",
      },
      context: {
        type: "string",
        description: "Additional context about the proposed change",
      },
    },
    required: ["technology"],
  },
};

// ============================================================================
// Helpers
// ============================================================================

/** Load active decisions (unranked); ranking/retrieval is layered on top. */
function loadActiveDecisions(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
): DecisionRow[] {
  return rawDb
    .prepare(
      `SELECT id, decision_type, subject, decision, rationale,
              alternatives_rejected, context_tags, confidence,
              status, superseded_by, created_at, updated_at
       FROM developer_decisions
       WHERE status = 'active'
       ORDER BY updated_at DESC
       LIMIT 500`,
    )
    .all() as DecisionRow[];
}

function parseAlts(row: DecisionRow): string[] {
  try {
    return JSON.parse(row.alternatives_rejected || "[]") as string[];
  } catch {
    return [];
  }
}

/**
 * Classify a decision row's relationship to the queried technology. Hard
 * conflicts require a literal/alias-aware match against rejected alternatives.
 */
function classifyRelationship(
  row: DecisionRow,
  tech: string,
): "aligned" | "conflict" | "related" {
  const techLower = tech.toLowerCase();

  // If the technology appears in rejected alternatives, it's a conflict
  const isRejected = parseAlts(row).some((alt) => matchesRecallQuery(alt, techLower));
  if (isRejected) {
    return "conflict";
  }

  // If the technology matches the decision subject, it's aligned
  if (
    matchesRecallQuery(row.subject, techLower) ||
    matchesRecallQuery(row.decision, techLower)
  ) {
    return "aligned";
  }

  // Otherwise it's tangentially related
  return "related";
}

/** Build a human-readable recommendation string. */
function buildRecommendation(
  technology: string,
  conflicts: DecisionConflict[],
  relevantCount: number,
  possibleCount: number,
): string {
  if (conflicts.length > 0) {
    const first = conflicts[0];
    return (
      `CONFLICT: ${technology} was explicitly rejected. ` +
      `Active decision: '${first.subject}' — ${first.reason}`
    );
  }

  if (possibleCount > 0) {
    return (
      `No explicit conflict, but ${possibleCount} prior decision(s) are semantically ` +
      `close to '${technology}' and rejected alternatives — review before proceeding.`
    );
  }

  if (relevantCount === 0) {
    return `No existing decisions found for '${technology}'. No conflicts detected — proceed with caution.`;
  }
  return `No conflicts found. Proceed with ${technology}.`;
}

/**
 * Classify the retrieved decisions into aligned/conflict/related, derive hard and
 * possible (paraphrase) conflicts, and assemble the alignment result. Shared by
 * the lexical and hybrid retrieval paths.
 */
function assembleAlignment(
  params: CheckDecisionAlignmentParams,
  rows: DecisionRow[],
  semanticById: Map<number, number>,
  recallMode: "hybrid" | "ranked_lexical",
): AlignmentResult {
  const { technology, pattern } = params;

  const relevantDecisions: RelevantDecision[] = [];
  const conflicts: DecisionConflict[] = [];

  for (const row of rows) {
    const relationship = classifyRelationship(row, technology);

    relevantDecisions.push({
      id: row.id,
      subject: row.subject,
      decision: row.decision,
      rationale: row.rationale,
      confidence: row.confidence,
      relationship,
    });

    if (relationship === "conflict") {
      const rejectedAlt = parseAlts(row).find((alt) => matchesRecallQuery(alt, technology));
      conflicts.push({
        decision_id: row.id,
        subject: row.subject,
        reason: `'${rejectedAlt || technology}' was rejected in favor of '${row.decision}' (rationale: ${row.rationale || "none"})`,
      });
    }
  }

  // Pattern rejected-alternative check (if a pattern was supplied separately).
  if (pattern) {
    const patternLower = pattern.toLowerCase();
    for (const row of rows) {
      const patternRejected = parseAlts(row).some((alt) =>
        matchesRecallQuery(alt, patternLower),
      );
      if (patternRejected && !conflicts.some((c) => c.decision_id === row.id)) {
        conflicts.push({
          decision_id: row.id,
          subject: row.subject,
          reason: `Pattern '${pattern}' was rejected in favor of '${row.decision}' (rationale: ${row.rationale || "none"})`,
        });
        const existing = relevantDecisions.find((d) => d.id === row.id);
        if (existing && existing.relationship !== "conflict") {
          existing.relationship = "conflict";
        }
      }
    }
  }

  // Possible (paraphrase) conflicts: semantically close to a decision that rejected
  // alternatives, but NOT a literal match. Advisory only — never flips `aligned`.
  const possibleConflicts: PossibleConflict[] = [];
  for (const row of rows) {
    const sim = semanticById.get(row.id) ?? 0;
    if (sim < POSSIBLE_CONFLICT_THRESHOLD) continue;
    const alts = parseAlts(row);
    if (alts.length === 0) continue;
    if (conflicts.some((c) => c.decision_id === row.id)) continue;
    possibleConflicts.push({
      decision_id: row.id,
      subject: row.subject,
      similarity: Math.round(sim * 1000) / 1000,
      reason: `'${technology}' is semantically close (${Math.round(sim * 100)}%) to a decision that rejected: ${alts.join(", ")}. Review before proceeding.`,
    });
  }

  const maxConfidence =
    relevantDecisions.length > 0
      ? Math.max(...relevantDecisions.map((d) => d.confidence))
      : 0.5;

  return {
    aligned: conflicts.length === 0,
    technology,
    relevant_decisions: relevantDecisions,
    conflicts,
    possible_conflicts: possibleConflicts,
    confidence: maxConfidence,
    recommendation: buildRecommendation(
      technology,
      conflicts,
      relevantDecisions.length,
      possibleConflicts.length,
    ),
    recall_mode: recallMode,
  };
}

// ============================================================================
// Execute
// ============================================================================

export function executeCheckDecisionAlignment(
  db: FourDADatabase,
  params: CheckDecisionAlignmentParams,
): AlignmentResult | Promise<AlignmentResult> {
  const rawDb = db.getRawDb();

  let candidates: DecisionRow[];
  try {
    candidates = loadActiveDecisions(rawDb);
  } catch {
    // developer_decisions table may not exist yet — safe default.
    return {
      aligned: true,
      technology: params.technology,
      relevant_decisions: [],
      conflicts: [],
      possible_conflicts: [],
      confidence: 0.5,
      recommendation: `No decision history available. The developer_decisions table may not exist yet. Proceed with ${params.technology}.`,
      recall_mode: "ranked_lexical",
    };
  }

  const query = [params.technology, params.pattern].filter(Boolean).join(" ");
  const config = getEmbeddingConfig();

  // No provider -> synchronous lexical retrieval (classic behaviour).
  if (!config) {
    const rows = rankDecisionsLexical(candidates, query, 50);
    return assembleAlignment(params, rows, new Map(), "ranked_lexical");
  }

  // Provider configured -> hybrid retrieval (returns a Promise; dispatch awaits).
  return hybridDecisionRecall(db, query, candidates, 50, config).then(
    ({ ranked, semanticById, recall_mode }) =>
      assembleAlignment(params, ranked, semanticById, recall_mode),
  );
}
