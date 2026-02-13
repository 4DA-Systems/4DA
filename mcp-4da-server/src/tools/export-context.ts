/**
 * export_context_packet tool
 *
 * Generate a context packet for handoff to another session or AI agent.
 */

import type { FourDADatabase } from "../db.js";

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
    .all()
    .map((r: any) => r.topic);

  // Get exclusions
  const exclusions = rawDb
    .prepare("SELECT topic FROM exclusions")
    .all()
    .map((r: any) => r.topic);

  // Get detected tech from ACE
  const detectedTech = rawDb
    .prepare("SELECT DISTINCT name FROM detected_tech ORDER BY confidence DESC LIMIT 15")
    .all()
    .map((r: any) => r.name);

  // Get active topics
  const activeTopics = rawDb
    .prepare("SELECT DISTINCT topic FROM active_topics ORDER BY last_seen DESC LIMIT 10")
    .all()
    .map((r: any) => r.topic);

  // Get open signals from temporal events (signals are computed at runtime, not stored in source_items)
  let openSignals: any[] = [];
  if (includeSignals) {
    try {
      openSignals = rawDb
        .prepare(`SELECT id, subject as title, data, created_at
          FROM temporal_events
          WHERE event_type = 'signal_emitted'
          ORDER BY created_at DESC LIMIT 20`)
        .all()
        .map((row: any) => {
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
        .all();
    }
  }

  // Get saved items
  let savedItems: any[] = [];
  if (includeSaved) {
    savedItems = rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, i.timestamp as saved_at
        FROM interactions i
        JOIN source_items si ON i.source_item_id = si.id
        WHERE i.action = 'save'
        ORDER BY i.timestamp DESC LIMIT 20`)
      .all();
  }

  return {
    generated_at: new Date().toISOString(),
    version: "1.0.0",
    active_context: {
      detected_tech: detectedTech,
      active_topics: activeTopics,
      interests,
      exclusions,
    },
    open_signals: openSignals,
    saved_items: savedItems,
    summary: `${interests.length} interests, ${detectedTech.length} detected tech, ${openSignals.length} open signals, ${savedItems.length} saved items`,
  };
}
