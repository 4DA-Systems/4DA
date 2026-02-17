/**
 * delegation_score tool
 *
 * AI-delegatability assessment. AI agents use this to self-assess:
 * "Should I proceed autonomously or ask the human?"
 *
 * Computes a composite score from pattern stability, security sensitivity,
 * decision density, and AI track record to produce a delegation recommendation.
 */

import type { FourDADatabase } from "../db.js";

// ============================================================================
// Types
// ============================================================================

export interface DelegationScoreParams {
  subject: string;
  task_description?: string;
}

interface TechStackRow {
  technology: string;
}

interface DependencyRow {
  package_name: string;
}

interface DetectedTechRow {
  name: string;
}

interface DecisionRow {
  id: number;
  alternatives_rejected: string;
}

interface MemoryRow {
  id: number;
  memory_type: string;
}

type DelegationRecommendation =
  | "fully_delegate"
  | "delegate_with_review"
  | "collaborate_realtime"
  | "human_only";

// ============================================================================
// Tool Definition
// ============================================================================

export const delegationScoreTool = {
  name: "delegation_score",
  description:
    "AI-delegatability assessment. Computes whether an AI agent should proceed autonomously, request review, collaborate in real-time, or defer to the human. Based on pattern stability, security sensitivity, decision density, and AI track record.",
  inputSchema: {
    type: "object" as const,
    properties: {
      subject: {
        type: "string",
        description:
          "The technology, pattern, or topic to assess for delegation",
      },
      task_description: {
        type: "string",
        description:
          "Optional description of the task being considered for delegation",
      },
    },
    required: ["subject"],
  },
};

// ============================================================================
// Factor Computation
// ============================================================================

function computePatternStability(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  subject: string,
): { score: number; reason: string } {
  const subjectLower = subject.toLowerCase();
  const pattern = `%${subjectLower}%`;

  // Check tech_stack (highest confidence: user explicitly declared this)
  try {
    const techRows = rawDb
      .prepare(`SELECT technology FROM tech_stack WHERE LOWER(technology) LIKE ?`)
      .all(pattern) as TechStackRow[];

    if (techRows.length > 0) {
      return { score: 0.9, reason: `Found in tech stack: ${techRows[0].technology}` };
    }
  } catch {
    // Table may not exist
  }

  // Check project_dependencies (good confidence: detected in projects)
  try {
    const depRows = rawDb
      .prepare(
        `SELECT package_name FROM project_dependencies WHERE LOWER(package_name) LIKE ?`,
      )
      .all(pattern) as DependencyRow[];

    if (depRows.length > 0) {
      return { score: 0.7, reason: `Found in project dependencies: ${depRows[0].package_name}` };
    }
  } catch {
    // Table may not exist
  }

  // Check detected_tech (moderate confidence: auto-detected)
  try {
    const dtRows = rawDb
      .prepare(`SELECT name FROM detected_tech WHERE LOWER(name) LIKE ?`)
      .all(pattern) as DetectedTechRow[];

    if (dtRows.length > 0) {
      return { score: 0.4, reason: `Auto-detected tech: ${dtRows[0].name}` };
    }
  } catch {
    // Table may not exist
  }

  // Check if subject was explicitly rejected in any decision
  try {
    const decisions = rawDb
      .prepare(
        `SELECT id, alternatives_rejected FROM developer_decisions
         WHERE status = 'active'
         AND LOWER(alternatives_rejected) LIKE ?`,
      )
      .all(pattern) as DecisionRow[];

    if (decisions.length > 0) {
      return { score: 0.1, reason: `Subject was rejected in decision #${decisions[0].id}` };
    }
  } catch {
    // Table may not exist
  }

  return { score: 0.5, reason: "No prior pattern data found" };
}

function computeSecuritySensitivity(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  subject: string,
): { score: number; reason: string } {
  const pattern = `%${subject.toLowerCase()}%`;

  try {
    // Count security-related source items for this subject in last 90 days
    const sinceDate = new Date(Date.now() - 90 * 24 * 60 * 60 * 1000).toISOString();

    const rows = rawDb
      .prepare(
        `SELECT COUNT(*) as cnt FROM source_items
         WHERE created_at > ?
         AND LOWER(title) LIKE ?
         AND (
           LOWER(title) LIKE '%security%'
           OR LOWER(title) LIKE '%vulnerability%'
           OR LOWER(title) LIKE '%cve%'
           OR LOWER(title) LIKE '%exploit%'
           OR LOWER(title) LIKE '%advisory%'
         )`,
      )
      .get(sinceDate, pattern) as { cnt: number } | undefined;

    const count = rows?.cnt || 0;

    if (count === 0) {
      return { score: 0.1, reason: "No security signals in last 90 days" };
    } else if (count <= 2) {
      return { score: 0.5, reason: `${count} security signal(s) in last 90 days` };
    } else {
      return { score: 0.9, reason: `${count} security signals in last 90 days — high sensitivity` };
    }
  } catch {
    return { score: 0.3, reason: "Unable to assess security signals" };
  }
}

function computeDecisionDensity(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  subject: string,
): { score: number; reason: string } {
  const pattern = `%${subject.toLowerCase()}%`;

  try {
    const rows = rawDb
      .prepare(
        `SELECT COUNT(*) as cnt FROM developer_decisions
         WHERE status = 'active'
         AND (LOWER(subject) LIKE ? OR LOWER(context_tags) LIKE ?)`,
      )
      .get(pattern, pattern) as { cnt: number } | undefined;

    const count = rows?.cnt || 0;

    if (count === 0) {
      return { score: 0.1, reason: "No active decisions for this subject" };
    } else if (count === 1) {
      return { score: 0.3, reason: "1 active decision — some constraints exist" };
    } else if (count <= 3) {
      return { score: 0.6, reason: `${count} active decisions — moderate constraint density` };
    } else {
      return { score: 0.9, reason: `${count} active decisions — heavily constrained area` };
    }
  } catch {
    return { score: 0.3, reason: "Unable to assess decision density" };
  }
}

function computeAITrackRecord(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  subject: string,
): { score: number; reason: string } {
  const pattern = `%${subject.toLowerCase()}%`;

  try {
    const rows = rawDb
      .prepare(
        `SELECT COUNT(*) as cnt FROM agent_memory
         WHERE memory_type = 'warning'
         AND (LOWER(subject) LIKE ? OR LOWER(content) LIKE ?)`,
      )
      .get(pattern, pattern) as { cnt: number } | undefined;

    const warningCount = rows?.cnt || 0;

    if (warningCount === 0) {
      return { score: 0.9, reason: "No prior warnings — clean track record" };
    } else if (warningCount === 1) {
      return { score: 0.6, reason: "1 prior warning — proceed with caution" };
    } else {
      return { score: 0.3, reason: `${warningCount} prior warnings — needs human oversight` };
    }
  } catch {
    return { score: 0.7, reason: "Unable to assess track record (no memory table)" };
  }
}

// ============================================================================
// Execute
// ============================================================================

export function executeDelegationScore(
  db: FourDADatabase,
  params: DelegationScoreParams,
): object {
  const rawDb = db.getRawDb();
  const subject = params.subject;

  // Compute factors
  const patternStability = computePatternStability(rawDb, subject);
  const securitySensitivity = computeSecuritySensitivity(rawDb, subject);
  const decisionDensity = computeDecisionDensity(rawDb, subject);
  const aiTrackRecord = computeAITrackRecord(rawDb, subject);

  // Composite score
  // Higher = more delegatable
  // pattern_stability: higher is better (known pattern)
  // security_sensitivity: INVERT (lower security risk = more delegatable)
  // decision_density: INVERT (fewer constraints = more delegatable)
  // ai_track_record: higher is better (fewer warnings)
  const composite =
    patternStability.score * 0.30 +
    (1 - securitySensitivity.score) * 0.25 +
    (1 - decisionDensity.score) * 0.25 +
    aiTrackRecord.score * 0.20;

  const overallScore = Math.max(0, Math.min(1, Math.round(composite * 100) / 100));

  // Map to recommendation
  let recommendation: DelegationRecommendation;
  if (overallScore >= 0.8) {
    recommendation = "fully_delegate";
  } else if (overallScore >= 0.5) {
    recommendation = "delegate_with_review";
  } else if (overallScore >= 0.3) {
    recommendation = "collaborate_realtime";
  } else {
    recommendation = "human_only";
  }

  // Build caveats
  const caveats: string[] = [];
  if (securitySensitivity.score >= 0.5) {
    caveats.push("Security signals detected — review security implications");
  }
  if (decisionDensity.score >= 0.6) {
    caveats.push("Multiple active decisions constrain this area — check alignment");
  }
  if (patternStability.score <= 0.3) {
    caveats.push("Subject was previously rejected or is unfamiliar — verify approach");
  }
  if (aiTrackRecord.score <= 0.5) {
    caveats.push("Prior warnings exist for this subject — exercise caution");
  }

  return {
    subject,
    task_description: params.task_description || null,
    overall_score: overallScore,
    factors: {
      pattern_stability: {
        score: patternStability.score,
        weight: 0.30,
        reason: patternStability.reason,
      },
      security_sensitivity: {
        score: securitySensitivity.score,
        weight: 0.25,
        reason: securitySensitivity.reason,
      },
      decision_density: {
        score: decisionDensity.score,
        weight: 0.25,
        reason: decisionDensity.reason,
      },
      ai_track_record: {
        score: aiTrackRecord.score,
        weight: 0.20,
        reason: aiTrackRecord.reason,
      },
    },
    recommendation,
    caveats,
  };
}
