/**
 * semantic_shifts tool
 *
 * Detect how topic narratives shift over time.
 */

import type { FourDADatabase } from "../db.js";

export interface SemanticShiftsParams {
  period_days?: number;
  min_magnitude?: number;
}

export const semanticShiftsTool = {
  name: "semantic_shifts",
  description: `Detect narrative shifts in topics you follow. Instead of "new post about X", see "the conversation about X shifted from A to B."`,
  inputSchema: {
    type: "object" as const,
    properties: {
      period_days: {
        type: "number",
        description: "Look-back period in days. Default: 7",
        default: 7,
      },
      min_magnitude: {
        type: "number",
        description: "Minimum drift magnitude (0-1). Default: 0.1",
        default: 0.1,
      },
    },
  },
};

export function executeSemanticShifts(
  db: FourDADatabase,
  params: SemanticShiftsParams,
) {
  const rawDb = db.getRawDb();
  const periodDays = params.period_days || 7;

  const rows = rawDb
    .prepare(
      `SELECT id, subject, data, created_at
       FROM temporal_events
       WHERE event_type = 'topic_centroid'
       AND created_at >= datetime('now', '-' || ? || ' days')
       ORDER BY created_at DESC`,
    )
    .all(periodDays) as any[];

  // Group centroids by subject (topic)
  const byTopic = new Map<string, any[]>();
  for (const row of rows) {
    const existing = byTopic.get(row.subject) || [];
    existing.push({
      data: JSON.parse(row.data),
      created_at: row.created_at,
    });
    byTopic.set(row.subject, existing);
  }

  // Detect shifts where we have at least 2 snapshots
  const shifts: any[] = [];
  for (const [topic, snapshots] of byTopic) {
    if (snapshots.length < 2) continue;
    const latest = snapshots[0];
    const earliest = snapshots[snapshots.length - 1];

    const magnitude = latest.data.drift_magnitude || 0;
    if (magnitude >= (params.min_magnitude || 0.1)) {
      shifts.push({
        topic,
        drift_magnitude: magnitude,
        direction: latest.data.direction || "unknown",
        period: `last ${periodDays} days`,
        snapshots: snapshots.length,
        detected_at: latest.created_at,
      });
    }
  }

  shifts.sort((a, b) => b.drift_magnitude - a.drift_magnitude);

  return {
    shifts,
    total: shifts.length,
    period_days: periodDays,
    topics_tracked: byTopic.size,
    summary: `${shifts.length} narrative shift${shifts.length !== 1 ? "s" : ""} detected across ${byTopic.size} topics`,
  };
}
