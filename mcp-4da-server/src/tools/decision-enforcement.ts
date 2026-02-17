/**
 * check_decision_alignment tool
 *
 * The key tool AI agents call BEFORE suggesting major changes.
 * Checks if a technology or pattern aligns with the developer's active decisions.
 * Returns alignment status, relevant decisions, and any conflicts.
 */

import type { FourDADatabase } from "../db.js";

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

interface AlignmentResult {
  aligned: boolean;
  technology: string;
  relevant_decisions: RelevantDecision[];
  conflicts: DecisionConflict[];
  confidence: number;
  recommendation: string;
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

/**
 * Search active decisions matching a given term against subject,
 * context_tags, and alternatives_rejected columns.
 */
function searchActiveDecisions(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  searchTerm: string,
): DecisionRow[] {
  const pattern = `%${searchTerm.toLowerCase()}%`;
  return rawDb
    .prepare(
      `SELECT id, decision_type, subject, decision, rationale,
              alternatives_rejected, context_tags, confidence,
              status, superseded_by, created_at, updated_at
       FROM developer_decisions
       WHERE status = 'active'
         AND (LOWER(subject) LIKE ?
           OR LOWER(context_tags) LIKE ?
           OR LOWER(alternatives_rejected) LIKE ?)`,
    )
    .all(pattern, pattern, pattern) as DecisionRow[];
}

/**
 * Classify a decision row's relationship to the queried technology.
 */
function classifyRelationship(
  row: DecisionRow,
  tech: string,
): "aligned" | "conflict" | "related" {
  const techLower = tech.toLowerCase();
  const alts: string[] = JSON.parse(row.alternatives_rejected || "[]");

  // If the technology appears in rejected alternatives, it's a conflict
  const isRejected = alts.some((alt) =>
    alt.toLowerCase().includes(techLower),
  );
  if (isRejected) {
    return "conflict";
  }

  // If the technology matches the decision subject, it's aligned
  if (row.subject.toLowerCase().includes(techLower)) {
    return "aligned";
  }

  // Otherwise it's tangentially related
  return "related";
}

/**
 * Build a human-readable recommendation string.
 */
function buildRecommendation(
  technology: string,
  conflicts: DecisionConflict[],
  relevantCount: number,
): string {
  if (conflicts.length === 0) {
    if (relevantCount === 0) {
      return `No existing decisions found for '${technology}'. No conflicts detected — proceed with caution.`;
    }
    return `No conflicts found. Proceed with ${technology}.`;
  }

  // Report the first (most relevant) conflict
  const first = conflicts[0];
  return (
    `CONFLICT: ${technology} was explicitly rejected. ` +
    `Active decision: '${first.subject}' — ${first.reason}`
  );
}

// ============================================================================
// Execute
// ============================================================================

export function executeCheckDecisionAlignment(
  db: FourDADatabase,
  params: CheckDecisionAlignmentParams,
): AlignmentResult {
  const rawDb = db.getRawDb();
  const { technology, pattern, context } = params;

  // Collect all matching decision rows (deduplicated by id)
  const seenIds = new Set<number>();
  const allRows: DecisionRow[] = [];

  try {
    // Primary search: technology name
    const techRows = searchActiveDecisions(rawDb, technology);
    for (const row of techRows) {
      if (!seenIds.has(row.id)) {
        seenIds.add(row.id);
        allRows.push(row);
      }
    }

    // Secondary search: pattern (if provided)
    if (pattern) {
      const patternRows = searchActiveDecisions(rawDb, pattern);
      for (const row of patternRows) {
        if (!seenIds.has(row.id)) {
          seenIds.add(row.id);
          allRows.push(row);
        }
      }
    }
  } catch {
    // Table may not exist yet — return a safe default
    return {
      aligned: true,
      technology,
      relevant_decisions: [],
      conflicts: [],
      confidence: 0.5,
      recommendation: `No decision history available. The developer_decisions table may not exist yet. Proceed with ${technology}.`,
    };
  }

  // Classify each row
  const relevantDecisions: RelevantDecision[] = [];
  const conflicts: DecisionConflict[] = [];

  for (const row of allRows) {
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
      const alts: string[] = JSON.parse(row.alternatives_rejected || "[]");
      const rejectedAlt = alts.find((alt) =>
        alt.toLowerCase().includes(technology.toLowerCase()),
      );

      conflicts.push({
        decision_id: row.id,
        subject: row.subject,
        reason: `'${rejectedAlt || technology}' was rejected in favor of '${row.decision}' (rationale: ${row.rationale || "none"})`,
      });
    }
  }

  // If pattern was provided but didn't surface via technology search,
  // also check pattern against decision relationships
  if (pattern) {
    for (const row of allRows) {
      const patternLower = pattern.toLowerCase();
      const alts: string[] = JSON.parse(row.alternatives_rejected || "[]");
      const patternRejected = alts.some((alt) =>
        alt.toLowerCase().includes(patternLower),
      );

      if (
        patternRejected &&
        !conflicts.some((c) => c.decision_id === row.id)
      ) {
        conflicts.push({
          decision_id: row.id,
          subject: row.subject,
          reason: `Pattern '${pattern}' was rejected in favor of '${row.decision}' (rationale: ${row.rationale || "none"})`,
        });

        // Update the relationship for this decision
        const existing = relevantDecisions.find((d) => d.id === row.id);
        if (existing && existing.relationship !== "conflict") {
          existing.relationship = "conflict";
        }
      }
    }
  }

  const maxConfidence =
    relevantDecisions.length > 0
      ? Math.max(...relevantDecisions.map((d) => d.confidence))
      : 0.5;

  const recommendation = buildRecommendation(
    technology,
    conflicts,
    relevantDecisions.length,
  );

  return {
    aligned: conflicts.length === 0,
    technology,
    relevant_decisions: relevantDecisions,
    conflicts,
    confidence: maxConfidence,
    recommendation,
  };
}
