/**
 * export_context_packet tool
 *
 * Generate a context packet for handoff to another session or AI agent.
 */

import type { FourDADatabase } from "../db.js";
import type { SimpleTopicRow, SimpleNameRow, SignalEventRow, SourceItemMinimalRow, SavedItemRow } from "../types.js";

export interface ExportContextParams {
  include_signals?: boolean;
  include_saved?: boolean;
}

export const exportContextPacketTool = {
  name: "export_context_packet",
  description: `Generate a portable context packet capturing current work state, open signals, saved items, and active context. Use this to hand off context to another session or AI agent.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      include_signals: {
        type: "boolean",
        description: "Include open signals. Default: true",
        default: true,
      },
      include_saved: {
        type: "boolean",
        description: "Include saved items. Default: true",
        default: true,
      },
    },
  },
};

export interface OpenSignal {
  id: number;
  title: string;
  signal_type: string;
  priority: string;
  source_type: string;
}

export function executeExportContextPacket(
  db: FourDADatabase,
  params: ExportContextParams,
) {
  const rawDb = db.getRawDb();
  const includeSignals = params.include_signals ?? true;
  const includeSaved = params.include_saved ?? true;

  // Get interests
  const interests = rawDb
    .prepare("SELECT topic FROM interests ORDER BY weight DESC LIMIT 20")
    .all() as SimpleTopicRow[];
  const interestTopics = interests.map((r) => r.topic);

  // Get exclusions
  const exclusionRows = rawDb
    .prepare("SELECT topic FROM exclusions")
    .all() as SimpleTopicRow[];
  const exclusions = exclusionRows.map((r) => r.topic);

  // Get detected tech from ACE
  const detectedTechRows = rawDb
    .prepare("SELECT DISTINCT name FROM detected_tech ORDER BY confidence DESC LIMIT 15")
    .all() as SimpleNameRow[];
  const detectedTech = detectedTechRows.map((r) => r.name);

  // Get active topics
  const activeTopicRows = rawDb
    .prepare("SELECT DISTINCT topic FROM active_topics ORDER BY last_seen DESC LIMIT 10")
    .all() as SimpleTopicRow[];
  const activeTopics = activeTopicRows.map((r) => r.topic);

  // Get open signals from temporal events (signals are computed at runtime, not stored in source_items)
  let openSignals: (OpenSignal | SourceItemMinimalRow)[] = [];
  if (includeSignals) {
    try {
      openSignals = (rawDb
        .prepare(`SELECT id, subject as title, data, created_at
          FROM temporal_events
          WHERE event_type = 'signal_emitted'
          ORDER BY created_at DESC LIMIT 20`)
        .all() as SignalEventRow[])
        .map((row) => {
          const data = JSON.parse(row.data || '{}');
          return {
            id: row.id,
            title: row.title,
            signal_type: data.signal_type || 'unknown',
            priority: data.priority || 'medium',
            source_type: data.source_type || 'unknown',
          };
        });
    } catch {
      // temporal_events may not have signal data yet
      openSignals = rawDb
        .prepare(`SELECT id, title, url, source_type
          FROM source_items
          ORDER BY created_at DESC LIMIT 20`)
        .all() as SourceItemMinimalRow[];
    }
  }

  // Get saved items
  let savedItems: SavedItemRow[] = [];
  if (includeSaved) {
    savedItems = rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, i.timestamp as saved_at
        FROM interactions i
        JOIN source_items si ON i.source_item_id = si.id
        WHERE i.action = 'save'
        ORDER BY i.timestamp DESC LIMIT 20`)
      .all() as SavedItemRow[];
  }

  // Trust metrics
  let trustMetrics: {
    precision_30d: number | null;
    action_rate_30d: number | null;
    total_surfaced_30d: number;
    false_positives_30d: number;
  } | null = null;
  try {
    const trustCounts = rawDb
      .prepare(
        `SELECT event_type, COUNT(*) as cnt
       FROM trust_events
       WHERE created_at >= datetime('now', '-30 days')
       GROUP BY event_type`,
      )
      .all() as Array<{ event_type: string; cnt: number }>;

    const getCount = (t: string) =>
      trustCounts.find((c) => c.event_type === t)?.cnt ?? 0;
    const surfaced = getCount("surfaced");
    const acted = getCount("acted_on");
    const fp = getCount("false_positive");
    const validated = getCount("validated");
    const tp = acted + validated;

    trustMetrics = {
      precision_30d:
        tp + fp > 0 ? Math.round((tp / (tp + fp)) * 100) / 100 : null,
      action_rate_30d:
        surfaced > 0
          ? Math.round((acted / surfaced) * 100) / 100
          : null,
      total_surfaced_30d: surfaced,
      false_positives_30d: fp,
    };
  } catch {
    /* trust tables may not exist yet */
  }

  // Recent preemption alerts count
  let preemptionSummary: {
    active_critical_alerts: number;
  } | null = null;
  try {
    const alertCount = rawDb
      .prepare(
        `SELECT COUNT(*) as cnt FROM dependency_alerts
       WHERE created_at >= datetime('now', '-7 days') AND severity IN ('critical', 'high')`,
      )
      .get() as { cnt: number } | undefined;

    preemptionSummary = {
      active_critical_alerts: alertCount?.cnt ?? 0,
    };
  } catch {
    /* table may not exist */
  }

  return {
    generated_at: new Date().toISOString(),
    version: "1.1.0",
    active_context: {
      detected_tech: detectedTech,
      active_topics: activeTopics,
      interests: interestTopics,
      exclusions,
    },
    open_signals: openSignals,
    saved_items: savedItems,
    trust_metrics: trustMetrics,
    preemption_summary: preemptionSummary,
    summary: `${interestTopics.length} interests, ${detectedTech.length} detected tech, ${openSignals.length} open signals, ${savedItems.length} saved items`,
  };
}
