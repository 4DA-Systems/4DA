// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * preemption_feed tool
 *
 * Returns forward-looking intelligence alerts — what's likely to matter
 * soon based on signal chains, dependency health, and knowledge gaps.
 * Use before starting work to check for upcoming risks.
 */

import type { FourDADatabase } from "../db.js";

export interface PreemptionFeedParams {
  urgency_filter?: string; // "critical", "high", "medium", "watch"
  limit?: number;
}

export const preemptionFeedTool = {
  name: "preemption_feed",
  description:
    "Get forward-looking intelligence alerts about upcoming risks, breaking changes, and ecosystem shifts affecting your stack. Use before dependency updates or when planning work.",
  inputSchema: {
    type: "object" as const,
    properties: {
      urgency_filter: {
        type: "string",
        description:
          "Filter by minimum urgency: critical, high, medium, watch. Default: all",
        enum: ["critical", "high", "medium", "watch"],
      },
      limit: {
        type: "number",
        description: "Maximum alerts to return. Default: 10",
        default: 10,
      },
    },
  },
};

export function executePreemptionFeed(
  db: FourDADatabase,
  params: PreemptionFeedParams,
) {
  // Preemption data comes from the Tauri backend (signal chains, project health, etc.)
  // The MCP server reads from the same DB. We synthesize from available tables.
  const rawDb = db.getRawDb();
  const limit = params.limit ?? 10;

  // Get recent signal chains (open, with high confidence)
  let chains: Array<{
    id: string;
    chain_name: string;
    overall_priority: string;
    confidence: number;
    suggested_action: string;
    link_count: number;
  }> = [];
  try {
    // signal_chains might be stored as temporal events
    const rows = rawDb
      .prepare(
        `SELECT te.id, te.subject, te.data, te.created_at
       FROM temporal_events te
       WHERE te.event_type = 'signal_chain'
       AND te.created_at >= datetime('now', '-14 days')
       ORDER BY te.created_at DESC
       LIMIT ?`,
      )
      .all(limit) as Array<{
      id: number;
      subject: string;
      data: string;
      created_at: string;
    }>;

    chains = rows
      .map((r) => {
        try {
          const data = JSON.parse(r.data);
          return {
            id: String(r.id),
            chain_name: r.subject || data.chain_name || "Signal chain",
            overall_priority: data.overall_priority || "medium",
            confidence: data.confidence || 0.5,
            suggested_action: data.suggested_action || "Investigate",
            link_count: data.links?.length || 0,
          };
        } catch {
          return null;
        }
      })
      .filter(Boolean) as typeof chains;
  } catch {
    /* table may not exist */
  }

  // Get critical dependency alerts from project health
  let health_alerts: Array<{
    project: string;
    severity: string;
    message: string;
    dep: string | null;
  }> = [];
  try {
    const alerts = rawDb
      .prepare(
        `SELECT da.package_name, da.alert_type, da.severity, da.message, pd.project_path
       FROM dependency_alerts da
       LEFT JOIN project_dependencies pd ON pd.package_name = da.package_name
       WHERE da.created_at >= datetime('now', '-7 days')
       ORDER BY da.severity DESC
       LIMIT ?`,
      )
      .all(limit) as Array<{
      package_name: string;
      alert_type: string;
      severity: string;
      message: string;
      project_path: string | null;
    }>;

    health_alerts = alerts.map((a) => ({
      project: a.project_path || "unknown",
      severity: a.severity,
      message: a.message,
      dep: a.package_name,
    }));
  } catch {
    /* table may not exist */
  }

  // Get knowledge gaps (blind spots)
  let blind_spots: Array<{
    dep: string;
    days_stale: number;
    missed_count: number;
  }> = [];
  try {
    // Find deps with no recent interactions
    const gaps = rawDb
      .prepare(
        `SELECT ud.name, ud.dep_type,
             COALESCE(
               CAST((julianday('now') - julianday(MAX(i.created_at))) AS INTEGER),
               999
             ) as days_since
      FROM user_dependencies ud
      LEFT JOIN interactions i ON LOWER(i.action) LIKE '%' || LOWER(ud.name) || '%'
      GROUP BY ud.name
      HAVING days_since > 14
      ORDER BY days_since DESC
      LIMIT ?`,
      )
      .all(limit) as Array<{
      name: string;
      dep_type: string;
      days_since: number;
    }>;

    blind_spots = gaps.map((g) => ({
      dep: g.name,
      days_stale: g.days_since,
      missed_count: 0,
    }));
  } catch {
    /* table may not exist */
  }

  return {
    signal_chains: chains,
    health_alerts,
    blind_spots,
    total_alerts: chains.length + health_alerts.length + blind_spots.length,
    recommendation:
      chains.length + health_alerts.length > 0
        ? "Active risks detected. Review before proceeding with dependency changes."
        : "No urgent risks detected. Safe to proceed.",
  };
}
