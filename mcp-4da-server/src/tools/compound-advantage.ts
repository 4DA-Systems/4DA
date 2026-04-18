// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * compound_advantage tool
 *
 * Compound advantage score — measures how effectively you leverage
 * 4DA intelligence for time-sensitive decisions.
 *
 * Now integrates with AWE Wisdom Graph — derives a compound score from
 * decision volume, feedback coverage, principle density, and validation ratio.
 */
import type { FourDADatabase } from "../db.js";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import Database from "better-sqlite3";

interface AweStats {
  decisions: number;
  feedback: number;
  principles: number;
  anti_patterns: number;
  validated: number;
}

function getAweWisdomStats(): AweStats | null {
  try {
    const dbPath = process.platform === "win32"
      ? join(homedir(), "AppData", "Roaming", "awe", "wisdom.db")
      : join(homedir(), ".local", "share", "awe", "wisdom.db");

    if (!existsSync(dbPath)) return null;

    const db = new Database(dbPath, { readonly: true });
    const count = (type: string) =>
      (db.prepare("SELECT COUNT(*) as cnt FROM wisdom_elements WHERE element_type = ?").get(type) as { cnt: number })?.cnt ?? 0;

    // Count validated decisions (HumanConfirmed or HumanEnriched in JSON data)
    let validated = 0;
    try {
      validated = (db.prepare(
        `SELECT COUNT(*) as cnt FROM wisdom_elements
         WHERE element_type = 'decision'
         AND (data LIKE '%HumanConfirmed%' OR data LIKE '%HumanEnriched%')`
      ).get() as { cnt: number })?.cnt ?? 0;
    } catch { /* validation_status may not exist in old data */ }

    const stats = {
      decisions: count("decision"),
      feedback: count("feedback"),
      principles: count("principle"),
      anti_patterns: count("anti_pattern"),
      validated,
    };
    db.close();
    return stats;
  } catch {
    return null;
  }
}

export interface CompoundAdvantageParams {
  period?: string;
}

interface ScoreRow {
  id: number;
  period: string;
  score: number;
  response_rate: number | null;
  avg_lead_time_hours: number | null;
  calibration_accuracy: number | null;
  computed_at: string;
}

export const compoundAdvantageTool = {
  name: "compound_advantage",
  description:
    "Get compound advantage score — measures how effectively you leverage 4DA intelligence for time-sensitive decisions. Tracks response rate, lead time, calibration accuracy.",
  inputSchema: {
    type: "object" as const,
    properties: {
      period: {
        type: "string",
        enum: ["weekly", "monthly"],
        description: "Scoring period. Default: weekly",
        default: "weekly",
      },
    },
  },
};

export function executeCompoundAdvantage(db: FourDADatabase, params: CompoundAdvantageParams): object {
  const rawDb = db.getRawDb();
  const period = params.period ?? "weekly";

  // Latest two scores for trend computation
  let latestScore: ScoreRow | null = null;
  let previousScore: ScoreRow | null = null;
  try {
    const scores = rawDb.prepare(
      `SELECT id, period, score, response_rate, avg_lead_time_hours,
              calibration_accuracy, computed_at
       FROM advantage_score WHERE period = ?
       ORDER BY computed_at DESC LIMIT 2`,
    ).all(period) as ScoreRow[];
    latestScore = scores[0] ?? null;
    previousScore = scores[1] ?? null;
  } catch { /* Table may not exist yet */ }

  // Decision window stats
  let total = 0, acted = 0, expired = 0;
  try {
    const s = rawDb.prepare(
      `SELECT COUNT(*) as total,
              SUM(CASE WHEN status = 'acted' THEN 1 ELSE 0 END) as acted,
              SUM(CASE WHEN status = 'expired' THEN 1 ELSE 0 END) as expired
       FROM decision_windows`,
    ).get() as { total: number; acted: number; expired: number } | undefined;
    if (s) { total = s.total; acted = s.acted; expired = s.expired; }
  } catch { /* Table may not exist yet */ }

  // Trend
  let trend = "no_data";
  let trendDelta: number | null = null;
  if (latestScore && previousScore) {
    trendDelta = latestScore.score - previousScore.score;
    trend = trendDelta > 2 ? "improving" : trendDelta < -2 ? "declining" : "stable";
  } else if (latestScore) {
    trend = "baseline";
  }

  // AWE Wisdom Graph integration — derive compound score from decision quality data
  const aweStats = getAweWisdomStats();
  const aweComponents = aweStats ? {
    decision_volume: Math.min((aweStats.decisions || 0) / 100, 1.0),
    feedback_coverage: aweStats.decisions > 0
      ? (aweStats.feedback || 0) / aweStats.decisions
      : 0,
    principle_density: Math.min((aweStats.principles || 0) / 10, 1.0),
    validated_ratio: aweStats.decisions > 0
      ? Math.min((aweStats.validated || 0) / Math.max(aweStats.decisions, 1), 1.0)
      : 0,
  } : null;

  // Compute AWE-derived score (0-100) if no native score exists
  const aweScore = aweComponents
    ? Math.round(
        ((aweComponents.decision_volume * 0.20) +
         (aweComponents.feedback_coverage * 0.30) +
         (aweComponents.principle_density * 0.25) +
         (aweComponents.validated_ratio * 0.25)) * 100
      )
    : null;

  const effectiveScore = latestScore?.score ?? aweScore;
  const effectiveComponents = latestScore ? {
    response_rate: latestScore.response_rate,
    avg_lead_time_hours: latestScore.avg_lead_time_hours,
    calibration_accuracy: latestScore.calibration_accuracy,
  } : aweComponents;

  return {
    score: effectiveScore,
    period,
    components: effectiveComponents,
    trend: { direction: trend, delta: trendDelta, previous_score: previousScore?.score ?? null },
    decision_windows: {
      total_opened: total,
      total_acted: acted,
      total_expired: expired,
      action_rate: total > 0 ? Math.round((acted / total) * 100) : null,
    },
    awe_stats: aweStats,
    computed_at: latestScore?.computed_at ?? (aweStats ? new Date().toISOString() : null),
    summary: effectiveScore != null
      ? `Compound advantage: ${effectiveScore}/100 (${trend}). ${aweStats ? `${aweStats.decisions} decisions, ${aweStats.principles} principles, ${aweStats.validated} validated.` : `${acted}/${total} windows acted on.`}`
      : "No compound advantage data yet. Record decisions and provide feedback to build your score.",
  };
}
