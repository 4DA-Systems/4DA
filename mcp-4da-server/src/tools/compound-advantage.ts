/**
 * compound_advantage tool
 *
 * Compound advantage score — measures how effectively you leverage
 * 4DA intelligence for time-sensitive decisions.
 */
import type { FourDADatabase } from "../db.js";

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

  return {
    score: latestScore?.score ?? null,
    period,
    components: latestScore ? {
      response_rate: latestScore.response_rate,
      avg_lead_time_hours: latestScore.avg_lead_time_hours,
      calibration_accuracy: latestScore.calibration_accuracy,
    } : null,
    trend: { direction: trend, delta: trendDelta, previous_score: previousScore?.score ?? null },
    decision_windows: {
      total_opened: total,
      total_acted: acted,
      total_expired: expired,
      action_rate: total > 0 ? Math.round((acted / total) * 100) : null,
    },
    computed_at: latestScore?.computed_at ?? null,
    summary: latestScore
      ? `Compound advantage: ${latestScore.score}/100 (${trend}). ${acted}/${total} windows acted on.`
      : "No compound advantage scores computed yet. Scores build as you interact with decision windows and intelligence feeds.",
  };
}
