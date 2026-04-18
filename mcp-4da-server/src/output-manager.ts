// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Output Manager for Dynamic Context Discovery
 *
 * Reduces token usage by ~80% through:
 * - Writing full results to files (referenced by path)
 * - Returning compact summaries inline
 * - Providing retrieval hints for common queries
 *
 * Full results stored at: ~/.local/share/4da/context/results/
 */

import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

// Base directory for 4DA context files
const CONTEXT_DIR = join(homedir(), ".local", "share", "4da", "context");
const RESULTS_DIR = join(CONTEXT_DIR, "results");

/**
 * Compact result format - what gets returned inline to Claude
 */
export interface CompactResult<T> {
  /** 2-3 sentence human-readable summary */
  summary: string;
  /** Essential 20% of data inline for immediate use */
  key_data: T;
  /** Path to complete JSON file */
  full_result_path: string;
  /** Hints for retrieving specific data (jq patterns, grep patterns) */
  retrieval_hints: string[];
}

/**
 * Ensure the context directories exist
 */
function ensureDirectories(): void {
  if (!existsSync(CONTEXT_DIR)) {
    mkdirSync(CONTEXT_DIR, { recursive: true });
  }
  if (!existsSync(RESULTS_DIR)) {
    mkdirSync(RESULTS_DIR, { recursive: true });
  }
}

/**
 * Write full result to file and return the file path
 */
function writeResultFile<T>(toolName: string, fullResult: T): string {
  ensureDirectories();

  // Generate timestamped filename
  const timestamp = Math.floor(Date.now() / 1000);
  const filename = `${toolName}_${timestamp}.json`;
  const filePath = join(RESULTS_DIR, filename);

  // Write full result to file
  writeFileSync(filePath, JSON.stringify(fullResult, null, 2), "utf-8");

  return filePath;
}

/**
 * Get the results directory path (for documentation)
 */
export function getResultsDir(): string {
  return RESULTS_DIR;
}

// =============================================================================
// Tool-specific compact output helpers
// =============================================================================

/**
 * Daily Briefing compact output
 */
export interface DailyBriefingKeyData {
  total_items: number;
  high_priority: number;
  top_theme: string;
  tldr: string;
  executive_brief?: string;
}

export function createDailyBriefingCompact(result: {
  summary: { total_items_analyzed: number; high_relevance_count: number; tldr: string };
  themes: { name: string; item_count: number }[];
  ai_synthesis?: { executive_brief: string };
}): CompactResult<DailyBriefingKeyData> {
  const topTheme = result.themes[0]?.name || "general";

  return {
    summary: `${result.summary.total_items_analyzed} items analyzed, ${result.summary.high_relevance_count} high-priority. Top theme: ${topTheme}. ${result.ai_synthesis ? "AI synthesis included." : ""}`,
    key_data: {
      total_items: result.summary.total_items_analyzed,
      high_priority: result.summary.high_relevance_count,
      top_theme: result.themes[0]?.name || "none",
      tldr: result.summary.tldr,
      executive_brief: result.ai_synthesis?.executive_brief,
    },
    full_result_path: writeResultFile("daily_briefing", result),
    retrieval_hints: [
      "jq '.notable_items' - get high-priority items",
      "jq '.themes' - get topic breakdown",
      "jq '.recommendations' - get action items",
      "jq '.ai_synthesis.executive_brief' - get AI summary",
    ],
  };
}

/**
 * Trend Analysis compact output
 */
export interface TrendAnalysisKeyData {
  total_items: number;
  volume_trend: string;
  top_rising: string | null;
  top_falling: string | null;
  anomaly_count: number;
  narrative?: string;
}

export function createTrendAnalysisCompact(result: {
  summary: {
    total_items: number;
    volume_trend: string;
    top_rising_topic: string | null;
    top_falling_topic: string | null;
  };
  anomalies: unknown[];
  ai_interpretation?: { narrative: string };
}): CompactResult<TrendAnalysisKeyData> {
  const anomalyNote = result.anomalies.length > 0 ? ` ${result.anomalies.length} anomalies detected.` : "";

  return {
    summary: `Volume ${result.summary.volume_trend}. Rising: ${result.summary.top_rising_topic || "none"}. Falling: ${result.summary.top_falling_topic || "none"}.${anomalyNote}`,
    key_data: {
      total_items: result.summary.total_items,
      volume_trend: result.summary.volume_trend,
      top_rising: result.summary.top_rising_topic,
      top_falling: result.summary.top_falling_topic,
      anomaly_count: result.anomalies.length,
      narrative: result.ai_interpretation?.narrative,
    },
    full_result_path: writeResultFile("trend_analysis", result),
    retrieval_hints: [
      "jq '.topic_trends' - get all topic trends",
      "jq '.volume_by_day' - get daily volume data",
      "jq '.anomalies' - get detected anomalies",
      "jq '.ai_interpretation.forecast' - get predictions",
    ],
  };
}

/**
 * Score Autopsy compact output
 */
export interface ScoreAutopsyKeyData {
  item_title: string;
  final_score: number;
  score_assessment?: "accurate" | "too_high" | "too_low" | "uncertain";
  top_contributor: string;
  verdict?: string;
}

export function createScoreAutopsyCompact(result: {
  item: { title: string };
  final_score: number;
  components: { name: string; contribution: number }[];
  ai_analysis?: { score_assessment: "accurate" | "too_high" | "too_low" | "uncertain"; verdict: string };
}): CompactResult<ScoreAutopsyKeyData> {
  const topComponent = result.components.sort((a, b) => b.contribution - a.contribution)[0];
  const assessment = result.ai_analysis?.score_assessment || "unanalyzed";

  return {
    summary: `"${result.item.title.substring(0, 40)}..." scored ${Math.round(result.final_score * 100)}%. Assessment: ${assessment}. Top contributor: ${topComponent?.name || "unknown"}.`,
    key_data: {
      item_title: result.item.title,
      final_score: result.final_score,
      score_assessment: result.ai_analysis?.score_assessment,
      top_contributor: topComponent?.name || "unknown",
      verdict: result.ai_analysis?.verdict,
    },
    full_result_path: writeResultFile("score_autopsy", result),
    retrieval_hints: [
      "jq '.components' - get score breakdown",
      "jq '.matching_context' - get what matched",
      "jq '.similar_items' - get comparison items",
      "jq '.recommendations' - get improvement suggestions",
    ],
  };
}

/**
 * Context Analysis compact output
 */
export interface ContextAnalysisKeyData {
  quality_score: number;
  interests_count: number;
  tech_count: number;
  gap_count: number;
  top_recommendation: string;
  personalized_advice?: string;
}

export function createContextAnalysisCompact(result: {
  context_summary: { interests_count: number; tech_stack_count: number };
  quality_score: { overall: number };
  coverage_gaps: unknown[];
  recommendations: { action: string }[];
  ai_advice?: { personalized_recommendations: string };
}): CompactResult<ContextAnalysisKeyData> {
  const quality = Math.round(result.quality_score.overall * 100);

  return {
    summary: `Context quality: ${quality}%. ${result.context_summary.interests_count} interests, ${result.context_summary.tech_stack_count} tech. ${result.coverage_gaps.length} gaps found.`,
    key_data: {
      quality_score: result.quality_score.overall,
      interests_count: result.context_summary.interests_count,
      tech_count: result.context_summary.tech_stack_count,
      gap_count: result.coverage_gaps.length,
      top_recommendation: result.recommendations[0]?.action || "No recommendations",
      personalized_advice: result.ai_advice?.personalized_recommendations,
    },
    full_result_path: writeResultFile("context_analysis", result),
    retrieval_hints: [
      "jq '.quality_score.dimensions' - get quality breakdown",
      "jq '.coverage_gaps' - get what's missing",
      "jq '.recommendations' - get all suggestions",
      "jq '.top_affinities' - get learned preferences",
    ],
  };
}

/**
 * Topic Connections compact output
 */
export interface TopicConnectionsKeyData {
  items_analyzed: number;
  node_count: number;
  edge_count: number;
  central_topics: string[];
  top_connection: { from: string; to: string; strength: number } | null;
  graph_narrative?: string;
}

export function createTopicConnectionsCompact(result: {
  analysis_period: { items_analyzed: number };
  nodes: unknown[];
  edges: { from: string; to: string; co_occurrences: number }[];
  central_topics: string[];
  ai_insights?: { graph_narrative: string };
}): CompactResult<TopicConnectionsKeyData> {
  const topEdge = result.edges[0];
  const connectionNote = topEdge
    ? `Strongest: ${topEdge.from} ↔ ${topEdge.to}.`
    : "No strong connections.";

  return {
    summary: `${result.nodes.length} topics, ${result.edges.length} connections. Central: ${result.central_topics.slice(0, 3).join(", ")}. ${connectionNote}`,
    key_data: {
      items_analyzed: result.analysis_period.items_analyzed,
      node_count: result.nodes.length,
      edge_count: result.edges.length,
      central_topics: result.central_topics.slice(0, 5),
      top_connection: topEdge
        ? { from: topEdge.from, to: topEdge.to, strength: topEdge.co_occurrences }
        : null,
      graph_narrative: result.ai_insights?.graph_narrative,
    },
    full_result_path: writeResultFile("topic_connections", result),
    retrieval_hints: [
      "jq '.nodes' - get all topic nodes",
      "jq '.edges' - get all connections",
      "jq '.clusters' - get topic clusters",
      "jq '.path' - get path between topics (if requested)",
    ],
  };
}
