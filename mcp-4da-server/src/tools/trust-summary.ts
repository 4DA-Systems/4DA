// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * trust_summary tool
 *
 * Returns intelligence quality metrics from the trust ledger —
 * precision, action rate, false positive rate, preemption wins.
 * Helps agents assess how reliable 4DA's intelligence is.
 */

import type { FourDADatabase } from "../db.js";

export interface TrustSummaryParams {
  days?: number;
}

export const trustSummaryTool = {
  name: "trust_summary",
  description:
    "Get intelligence quality metrics — precision, action conversion rate, false positive rate, and preemption wins. Use this to assess how reliable 4DA's signals are before acting on them.",
  inputSchema: {
    type: "object" as const,
    properties: {
      days: {
        type: "number",
        description: "Number of days to analyze. Default: 30",
        default: 30,
      },
    },
  },
};

export function executeTrustSummary(
  db: FourDADatabase,
  params: TrustSummaryParams,
) {
  const rawDb = db.getRawDb();
  const days = params.days ?? 30;
  const offset = `-${days} days`;

  // Count events by type
  let counts: Array<{ event_type: string; cnt: number }> = [];
  try {
    counts = rawDb
      .prepare(
        `SELECT event_type, COUNT(*) as cnt
       FROM trust_events
       WHERE created_at >= datetime('now', ?)
       GROUP BY event_type`,
      )
      .all(offset) as Array<{ event_type: string; cnt: number }>;
  } catch {
    /* trust_events table may not exist yet */
  }

  const getCount = (type: string) =>
    counts.find((c) => c.event_type === type)?.cnt ?? 0;

  const surfaced = getCount("surfaced");
  const acted_on = getCount("acted_on");
  const dismissed = getCount("dismissed");
  const false_positives = getCount("false_positive");
  const validated = getCount("validated");

  const true_positives = acted_on + validated;
  const precision =
    true_positives + false_positives > 0
      ? true_positives / (true_positives + false_positives)
      : 1.0;

  const action_rate = surfaced > 0 ? acted_on / surfaced : 0;

  // Preemption wins
  let preemption_wins = 0;
  let avg_lead_time_hours: number | null = null;
  try {
    const winRow = rawDb
      .prepare(
        `SELECT COUNT(*) as wins, AVG(lead_time_hours) as avg_lead
       FROM preemption_wins
       WHERE verified = 1 AND created_at >= datetime('now', ?)`,
      )
      .get(offset) as { wins: number; avg_lead: number | null } | undefined;
    if (winRow) {
      preemption_wins = winRow.wins;
      avg_lead_time_hours = winRow.avg_lead;
    }
  } catch {
    /* table may not exist yet */
  }

  return {
    period_days: days,
    total_surfaced: surfaced,
    acted_on,
    dismissed,
    false_positives,
    precision: Math.round(precision * 1000) / 1000,
    action_conversion_rate: Math.round(action_rate * 1000) / 1000,
    preemption_wins,
    avg_lead_time_hours: avg_lead_time_hours
      ? Math.round(avg_lead_time_hours * 10) / 10
      : null,
    reliability_assessment:
      precision >= 0.8
        ? "high"
        : precision >= 0.6
          ? "moderate"
          : precision >= 0.3
            ? "developing"
            : "insufficient_data",
  };
}
