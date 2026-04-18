// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * attention_report tool
 *
 * Analyze where your attention goes vs. where your code needs it.
 */

import type { FourDADatabase } from "../db.js";
import type { EngagementRow, TopicAffinityRow, CodebaseTopicRow } from "../types.js";

export interface AttentionReportParams {
  period_days?: number;
}

export const attentionReportTool = {
  name: "attention_report",
  description: `Analyze your attention allocation - where you focus vs. where your codebase needs attention. Identifies blind spots and engagement patterns.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      period_days: {
        type: "number",
        description: "Analysis period in days. Default: 7",
        default: 7,
      },
    },
  },
};

export function executeAttentionReport(
  db: FourDADatabase,
  params: AttentionReportParams,
) {
  const rawDb = db.getRawDb();
  const days = params.period_days || 7;

  // Get engagement by topic from interactions + source items
  const engagementRows = rawDb
    .prepare(
      `SELECT si.source_type, COUNT(*) as interactions
       FROM interactions i
       JOIN source_items si ON i.source_item_id = si.id
       WHERE i.timestamp >= datetime('now', '-' || ? || ' days')
       GROUP BY si.source_type
       ORDER BY interactions DESC`,
    )
    .all(days) as EngagementRow[];

  // Get topic affinities (learned attention)
  const topicAffinities = rawDb
    .prepare(
      `SELECT topic, positive_signals, negative_signals,
              (positive_signals - negative_signals * 0.5) as attention_score
       FROM topic_affinities
       WHERE total_exposures > 0
       ORDER BY attention_score DESC
       LIMIT 15`,
    )
    .all() as TopicAffinityRow[];

  // Get codebase topics from detected tech
  const codebaseTopics = rawDb
    .prepare(
      `SELECT name as topic, category, confidence
       FROM detected_tech
       ORDER BY confidence DESC
       LIMIT 15`,
    )
    .all() as CodebaseTopicRow[];

  // Identify blind spots: in codebase but low engagement
  const engagedTopics = new Set(
    topicAffinities
      .filter((t) => t.attention_score > 0)
      .map((t) => t.topic.toLowerCase()),
  );

  const blindSpots = codebaseTopics
    .filter((ct) => !engagedTopics.has(ct.topic.toLowerCase()))
    .map((ct) => ({
      topic: ct.topic,
      in_codebase: true,
      engagement_level: 0,
      risk_level: ct.confidence > 0.8 ? "high" : "medium",
      gap_description: `${ct.topic} is in your codebase (${ct.category}) but has low engagement`,
    }));

  const totalInteractions = engagementRows.reduce(
    (sum, r) => sum + r.interactions,
    0,
  );

  return {
    period_days: days,
    total_interactions: totalInteractions,
    engagement_by_source: engagementRows,
    topic_affinities: topicAffinities.slice(0, 10),
    codebase_topics: codebaseTopics,
    blind_spots: blindSpots,
    summary: `${totalInteractions} interactions over ${days} days, ${blindSpots.length} blind spot${blindSpots.length !== 1 ? "s" : ""} detected`,
  };
}
