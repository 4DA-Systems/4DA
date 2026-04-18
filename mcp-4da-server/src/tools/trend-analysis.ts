// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Trend Analysis Tool
 *
 * Statistical analysis of patterns and trends over time.
 * This is a SUPERPOWER - it reveals what's rising, falling, and anomalous.
 *
 * With synthesize=true, uses LLM to interpret trends and make predictions.
 */

import type { FourDADatabase } from "../db.js";
import { getLLMConfig, canSynthesize, synthesize, SYNTHESIS_PROMPTS } from "../llm.js";
import { createTrendAnalysisCompact, type CompactResult, type TrendAnalysisKeyData } from "../output-manager.js";

export const trendAnalysisTool = {
  name: "trend_analysis",
  description: `Analyze patterns and trends in 4DA's content over time.

With synthesize=true (recommended), uses AI to interpret trends and predict what matters.

Returns:
- Topic frequency trends (rising/falling)
- Volume anomalies detected
- Week-over-week comparisons
- Source performance trends
- AI predictions (when enabled)

Use this to understand what's changing in your information landscape.`,
  inputSchema: {
    type: "object",
    properties: {
      days: {
        type: "number",
        description: "Analysis period in days (default: 30)",
      },
      topic: {
        type: "string",
        description: "Specific topic to analyze (optional)",
      },
      detect_anomalies: {
        type: "boolean",
        description: "Include anomaly detection (default: true)",
      },
      synthesize: {
        type: "boolean",
        description: "Use AI to interpret trends and predict (default: true if LLM configured)",
      },
      compact: {
        type: "boolean",
        description: "Return compact result with file reference (default: true for ~80% token reduction)",
      },
    },
  },
};

export interface TrendAnalysisParams {
  days?: number;
  topic?: string;
  detect_anomalies?: boolean;
  synthesize?: boolean;
  compact?: boolean;
}

interface TopicTrend {
  topic: string;
  current_week: number;
  previous_week: number;
  change_percent: number;
  trend: "rising" | "falling" | "stable";
  avg_score: number;
}

interface VolumeAnomaly {
  date: string;
  volume: number;
  expected: number;
  z_score: number;
  status: "spike" | "drop" | "normal";
}

interface SourceTrend {
  source: string;
  volume_trend: "up" | "down" | "stable";
  quality_trend: "up" | "down" | "stable";
  avg_score: number;
  item_count: number;
}

interface TrendResult {
  period: {
    start: string;
    end: string;
    days: number;
  };
  summary: {
    total_items: number;
    avg_relevance: number;
    volume_trend: string;
    top_rising_topic: string | null;
    top_falling_topic: string | null;
  };
  topic_trends: TopicTrend[];
  volume_by_day: { date: string; count: number }[];
  anomalies: VolumeAnomaly[];
  source_trends: SourceTrend[];
  correlations: { topic_a: string; topic_b: string; co_occurrences: number }[];
  predictions: string[];
  // AI-powered interpretation
  ai_interpretation?: {
    narrative: string;
    key_signal: string;
    forecast: string;
    recommended_focus: string;
    model_used: string;
  };
}

export async function executeTrendAnalysis(
  db: FourDADatabase,
  params: TrendAnalysisParams
): Promise<TrendResult | CompactResult<TrendAnalysisKeyData>> {
  const { days = 30, topic, detect_anomalies = true } = params;
  const useCompact = params.compact !== false; // Default to compact=true

  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[]; get: (...args: unknown[]) => unknown } } }).db;

  // Check LLM availability
  const llmConfig = getLLMConfig(dbInstance);
  const shouldSynthesize = params.synthesize ?? canSynthesize(llmConfig);

  const endDate = new Date();
  const startDate = new Date(endDate.getTime() - days * 24 * 60 * 60 * 1000);

  // Volume by day
  const volumeByDayStmt = dbInstance.prepare(`
    SELECT date(created_at) as date, COUNT(*) as count
    FROM source_items
    WHERE datetime(created_at) >= datetime(?)
    GROUP BY date(created_at)
    ORDER BY date
  `);
  const volumeByDay = volumeByDayStmt.all(startDate.toISOString()) as { date: string; count: number }[];

  // Calculate statistics for anomaly detection
  const volumes = volumeByDay.map(v => v.count);
  const mean = volumes.reduce((a, b) => a + b, 0) / volumes.length || 0;
  const stdDev = Math.sqrt(
    volumes.reduce((sum, v) => sum + Math.pow(v - mean, 2), 0) / volumes.length
  ) || 1;

  // Detect anomalies
  const anomalies: VolumeAnomaly[] = detect_anomalies
    ? volumeByDay
        .map(v => ({
          date: v.date,
          volume: v.count,
          expected: Math.round(mean),
          z_score: Math.round(((v.count - mean) / stdDev) * 100) / 100,
          status: (v.count - mean) / stdDev > 2 ? "spike" as const :
                  (v.count - mean) / stdDev < -2 ? "drop" as const : "normal" as const,
        }))
        .filter(a => a.status !== "normal")
    : [];

  // Topic trends - compare this week vs last week
  const topicTrends = getTopicTrends(dbInstance, topic);

  // Source trends
  const sourceTrends = getSourceTrends(dbInstance, days);

  // Topic correlations (co-occurrence)
  const correlations = getTopicCorrelations(dbInstance, days);

  // Overall stats
  const totalStmt = dbInstance.prepare(`
    SELECT COUNT(*) as total, AVG(CASE WHEN url IS NOT NULL THEN 1 ELSE 0 END) as avg_has_url
    FROM source_items
    WHERE datetime(created_at) >= datetime(?)
  `);
  const totalStats = totalStmt.get(startDate.toISOString()) as { total: number };

  // Get relevant items for avg score
  const relevantItems = db.getRelevantContent(0, undefined, 1000, days * 24);
  const avgRelevance = relevantItems.length > 0
    ? relevantItems.reduce((sum, i) => sum + i.relevance_score, 0) / relevantItems.length
    : 0;

  // Volume trend
  const recentVolume = volumeByDay.slice(-7).reduce((sum, v) => sum + v.count, 0);
  const olderVolume = volumeByDay.slice(-14, -7).reduce((sum, v) => sum + v.count, 0);
  const volumeTrend = recentVolume > olderVolume * 1.1 ? "increasing" :
                      recentVolume < olderVolume * 0.9 ? "decreasing" : "stable";

  // Generate predictions
  const predictions = generatePredictions(topicTrends, volumeTrend, anomalies);

  const result: TrendResult = {
    period: {
      start: startDate.toISOString().split("T")[0],
      end: endDate.toISOString().split("T")[0],
      days,
    },
    summary: {
      total_items: totalStats.total,
      avg_relevance: Math.round(avgRelevance * 100) / 100,
      volume_trend: volumeTrend,
      top_rising_topic: topicTrends.find(t => t.trend === "rising")?.topic || null,
      top_falling_topic: topicTrends.find(t => t.trend === "falling")?.topic || null,
    },
    topic_trends: topicTrends,
    volume_by_day: volumeByDay,
    anomalies,
    source_trends: sourceTrends,
    correlations,
    predictions,
  };

  // AI Interpretation - the actual superpower
  if (shouldSynthesize && canSynthesize(llmConfig)) {
    try {
      const context = db.getUserContext(true, true);
      const trendData = {
        period: result.period,
        summary: result.summary,
        topic_trends: result.topic_trends.slice(0, 8),
        anomalies: result.anomalies,
        correlations: result.correlations,
      };

      const contextData = {
        interests: context.interests.slice(0, 10),
        tech_stack: context.tech_stack,
        role: context.role,
      };

      const synthesis = await synthesize(llmConfig, {
        system: SYNTHESIS_PROMPTS.trendAnalysis.system,
        prompt: SYNTHESIS_PROMPTS.trendAnalysis.buildPrompt(trendData, contextData),
        max_tokens: 400,
        complexity: SYNTHESIS_PROMPTS.trendAnalysis.complexity,
      });

      // Parse the synthesis
      const lines = synthesis.synthesis.split("\n").filter(l => l.trim());
      const keySignal = lines[0] || "";
      const forecast = lines.find(l => l.toLowerCase().includes("week") || l.toLowerCase().includes("expect") || l.toLowerCase().includes("predict")) || lines[1] || "";
      const focus = lines.find(l => l.toLowerCase().includes("focus") || l.toLowerCase().includes("attention") || l.toLowerCase().includes("should")) || "";

      result.ai_interpretation = {
        narrative: synthesis.synthesis,
        key_signal: keySignal,
        forecast: forecast,
        recommended_focus: focus || "Continue monitoring trends",
        model_used: synthesis.model_used,
      };
    } catch (error) {
      console.error("AI interpretation failed:", error);
    }
  }

  // Return compact or full result based on parameter
  if (useCompact) {
    return createTrendAnalysisCompact(result);
  }

  return result;
}

function getTopicTrends(
  db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[] } },
  filterTopic?: string
): TopicTrend[] {
  // Define topics to track
  const topics = filterTopic
    ? [filterTopic]
    : ["rust", "typescript", "python", "ai", "llm", "database", "security", "async", "wasm", "react"];

  const trends: TopicTrend[] = [];

  for (const topic of topics) {
    // This week
    const thisWeekStmt = db.prepare(`
      SELECT COUNT(*) as count, AVG(CASE WHEN url IS NOT NULL THEN 1 ELSE 0 END) as quality
      FROM source_items
      WHERE datetime(created_at) >= datetime('now', '-7 days')
      AND (lower(title) LIKE ? OR lower(content) LIKE ?)
    `);
    const thisWeek = thisWeekStmt.all(`%${topic}%`, `%${topic}%`) as { count: number; quality: number }[];

    // Last week
    const lastWeekStmt = db.prepare(`
      SELECT COUNT(*) as count
      FROM source_items
      WHERE datetime(created_at) >= datetime('now', '-14 days')
      AND datetime(created_at) < datetime('now', '-7 days')
      AND (lower(title) LIKE ? OR lower(content) LIKE ?)
    `);
    const lastWeek = lastWeekStmt.all(`%${topic}%`, `%${topic}%`) as { count: number }[];

    const current = thisWeek[0]?.count || 0;
    const previous = lastWeek[0]?.count || 0;
    const changePercent = previous > 0
      ? Math.round(((current - previous) / previous) * 100)
      : current > 0 ? 100 : 0;

    trends.push({
      topic,
      current_week: current,
      previous_week: previous,
      change_percent: changePercent,
      trend: changePercent > 15 ? "rising" : changePercent < -15 ? "falling" : "stable",
      avg_score: thisWeek[0]?.quality || 0,
    });
  }

  // Sort by absolute change
  return trends.sort((a, b) => Math.abs(b.change_percent) - Math.abs(a.change_percent));
}

function getSourceTrends(
  db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[] } },
  days: number
): SourceTrend[] {
  const stmt = db.prepare(`
    SELECT
      source_type,
      COUNT(*) as total,
      SUM(CASE WHEN datetime(created_at) >= datetime('now', '-7 days') THEN 1 ELSE 0 END) as recent,
      SUM(CASE WHEN datetime(created_at) >= datetime('now', '-14 days')
               AND datetime(created_at) < datetime('now', '-7 days') THEN 1 ELSE 0 END) as older
    FROM source_items
    WHERE datetime(created_at) >= datetime('now', '-' || ? || ' days')
    GROUP BY source_type
  `);

  const sources = stmt.all(days) as {
    source_type: string;
    total: number;
    recent: number;
    older: number;
  }[];

  return sources.map(s => ({
    source: s.source_type,
    volume_trend: s.recent > s.older * 1.1 ? "up" as const :
                  s.recent < s.older * 0.9 ? "down" as const : "stable" as const,
    quality_trend: "stable" as const, // Would need more data to compute
    avg_score: 0.6, // Placeholder
    item_count: s.total,
  }));
}

function getTopicCorrelations(
  db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[] } },
  days: number
): { topic_a: string; topic_b: string; co_occurrences: number }[] {
  const topicPairs = [
    ["rust", "async"],
    ["ai", "llm"],
    ["typescript", "react"],
    ["database", "sql"],
    ["security", "auth"],
  ];

  const correlations: { topic_a: string; topic_b: string; co_occurrences: number }[] = [];

  for (const [a, b] of topicPairs) {
    const stmt = db.prepare(`
      SELECT COUNT(*) as count
      FROM source_items
      WHERE datetime(created_at) >= datetime('now', '-' || ? || ' days')
      AND (lower(title) LIKE ? OR lower(content) LIKE ?)
      AND (lower(title) LIKE ? OR lower(content) LIKE ?)
    `);
    const result = stmt.all(days, `%${a}%`, `%${a}%`, `%${b}%`, `%${b}%`) as { count: number }[];

    if (result[0]?.count > 0) {
      correlations.push({
        topic_a: a,
        topic_b: b,
        co_occurrences: result[0].count,
      });
    }
  }

  return correlations.sort((a, b) => b.co_occurrences - a.co_occurrences);
}

function generatePredictions(
  trends: TopicTrend[],
  volumeTrend: string,
  anomalies: VolumeAnomaly[]
): string[] {
  const predictions: string[] = [];

  // Rising topics
  const rising = trends.filter(t => t.trend === "rising");
  if (rising.length > 0) {
    predictions.push(
      `"${rising[0].topic}" is trending up (+${rising[0].change_percent}%) - expect more content soon`
    );
  }

  // Falling topics
  const falling = trends.filter(t => t.trend === "falling");
  if (falling.length > 0) {
    predictions.push(
      `"${falling[0].topic}" is declining (${falling[0].change_percent}%) - may stabilize or continue dropping`
    );
  }

  // Volume prediction
  if (volumeTrend === "increasing") {
    predictions.push("Overall content volume is increasing - more items to filter");
  } else if (volumeTrend === "decreasing") {
    predictions.push("Content volume is decreasing - sources may need attention");
  }

  // Anomaly-based prediction
  if (anomalies.filter(a => a.status === "spike").length > 2) {
    predictions.push("Multiple volume spikes detected - possibly major news events");
  }

  if (predictions.length === 0) {
    predictions.push("Patterns are stable - no significant changes predicted");
  }

  return predictions;
}
