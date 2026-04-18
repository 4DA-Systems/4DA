// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Type definitions for 4DA MCP Server
 *
 * These types mirror the 4DA database schema for read operations.
 */

// =============================================================================
// Source Items
// =============================================================================

/**
 * A content item from any source (HN, arXiv, Reddit, etc.)
 */
export interface SourceItem {
  id: number;
  source_type: string;
  source_id: string;
  url: string | null;
  title: string;
  content: string;
  content_hash: string;
  created_at: string;
  last_seen: string;
}

/**
 * A relevant content item with computed score
 */
export interface RelevantItem {
  id: number;
  source_type: string;
  source_id: string;
  url: string | null;
  title: string;
  content: string;
  relevance_score: number;
  created_at: string;
  discovered_ago: string;
  /** Necessity score: "what you'd regret missing" (0.0-1.0). Populated from Rust analysis pipeline when available. */
  necessity_score: number;
  /** One-line explanation of why this item is necessary. Null when necessity_score is 0. */
  necessity_reason: string | null;
  /** Necessity category: security_vulnerability, breaking_change, deprecation_notice, blind_spot, decision_relevant, none */
  necessity_category: string | null;
  /** Necessity urgency: immediate, this_week, awareness, none */
  necessity_urgency: string | null;
}

// =============================================================================
// User Context
// =============================================================================

/**
 * Explicit user interest
 */
export interface Interest {
  id: number;
  topic: string;
  weight: number;
  source: "explicit" | "github" | "import" | "inferred";
}

/**
 * Detected technology from ACE
 */
export interface DetectedTech {
  name: string;
  category: string;
  confidence: number;
  source: string;
}

/**
 * Active topic from ACE
 */
export interface ActiveTopic {
  topic: string;
  weight: number;
  confidence: number;
  source: string;
  last_seen: string;
}

/**
 * Learned topic affinity
 */
export interface TopicAffinity {
  topic: string;
  affinity_score: number;
  confidence: number;
  positive_signals: number;
  negative_signals: number;
  total_exposures: number;
}

/**
 * Anti-topic (learned exclusion)
 */
export interface AntiTopic {
  topic: string;
  rejection_count: number;
  confidence: number;
  auto_detected: boolean;
  user_confirmed: boolean;
}

/**
 * Complete user context returned by get_context
 */
export interface UserContext {
  // Static identity (user-declared)
  role: string | null;
  tech_stack: string[];
  domains: string[];
  interests: Interest[];
  exclusions: string[];

  // ACE-detected context (optional)
  ace?: {
    detected_tech: DetectedTech[];
    active_topics: ActiveTopic[];
  };

  // Learned preferences (optional)
  learned?: {
    topic_affinities: TopicAffinity[];
    anti_topics: AntiTopic[];
  };
}

// =============================================================================
// Relevance Explanation
// =============================================================================

/**
 * Score breakdown for explain_relevance
 */
export interface ScoreBreakdown {
  embedding_similarity: number | null;
  static_match_score: number;
  ace_match_score: number;
  learned_affinity_score: number;
  anti_penalty: number;
  final_score: number;
}

/**
 * Matching context elements
 */
export interface MatchingContext {
  matching_interests: string[];
  matching_tech: string[];
  matching_topics: string[];
  matching_affinities: string[];
}

/**
 * Full relevance explanation
 */
export interface RelevanceExplanation {
  item_id: number;
  source_type: string;
  title: string;
  score_breakdown: ScoreBreakdown;
  matching_context: MatchingContext;
  explanation: string;
}

// =============================================================================
// Feedback
// =============================================================================

/**
 * Valid feedback action types
 */
export type FeedbackAction = "click" | "save" | "dismiss" | "mark_irrelevant";

/**
 * Feedback recording result
 */
export interface FeedbackResult {
  success: boolean;
  message: string;
  interaction_id?: number;
}

// =============================================================================
// Tool Parameters
// =============================================================================

export interface GetRelevantContentParams {
  min_score?: number;
  source_type?: string;
  limit?: number;
  since_hours?: number;
}

export interface GetContextParams {
  include_ace?: boolean;
  include_learned?: boolean;
}

export interface ExplainRelevanceParams {
  item_id: number;
  source_type: string;
}

export interface RecordFeedbackParams {
  item_id: number;
  source_type: string;
  action: FeedbackAction;
}

// =============================================================================
// Database Row Types (for MCP tool queries)
// =============================================================================

/** Row from: SELECT si.source_type, COUNT(*) as interactions FROM interactions ... GROUP BY si.source_type */
export interface EngagementRow {
  source_type: string;
  interactions: number;
}

/** Row from: SELECT topic, positive_signals, negative_signals, (computed) as attention_score FROM topic_affinities */
export interface TopicAffinityRow {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  attention_score: number;
}

/** Row from: SELECT name as topic, category, confidence FROM detected_tech */
export interface CodebaseTopicRow {
  topic: string;
  category: string;
  confidence: number;
}

/** Row from: SELECT topic FROM interests / exclusions */
export interface SimpleTopicRow {
  topic: string;
}

/** Row from: SELECT DISTINCT name FROM detected_tech */
export interface SimpleNameRow {
  name: string;
}

/** Row from: SELECT id, subject, data, created_at FROM temporal_events */
export interface TemporalEventRow {
  id: number;
  subject: string;
  data: string;
  created_at: string;
}

/** Row from: SELECT id, event_type, subject, data, created_at FROM temporal_events WHERE event_type = 'signal_chain' */
export interface SignalChainRow {
  id: number;
  event_type: string;
  subject: string;
  data: string;
  created_at: string;
}

/** Row from: SELECT ir.source_item_id, ir.related_item_id, ir.metadata, ir.created_at, si.title, si.url, si.source_type FROM item_relationships ir JOIN source_items si ... */
export interface MentionRow {
  source_item_id: number;
  related_item_id: number;
  metadata: string;
  created_at: string;
  title: string;
  url: string;
  source_type: string;
}

/** Row from: SELECT DISTINCT package_name FROM project_dependencies */
export interface PackageNameRow {
  package_name: string;
}

/** Row from: SELECT project_path, COUNT(*) as dep_count, GROUP_CONCAT(package_name, ', ') as packages FROM project_dependencies ... GROUP BY project_path */
export interface ProjectSummaryRow {
  project_path: string;
  dep_count: number;
  packages: string;
}

/** Row from: SELECT package_name, version, language, is_dev FROM project_dependencies WHERE project_path = ? */
export interface DependencyRow {
  package_name: string;
  version: string;
  language: string;
  is_dev: number | null;
}

/** Row from: SELECT package_name, version, project_path, language FROM project_dependencies */
export interface DependencyWithProjectRow {
  package_name: string;
  version: string;
  project_path: string;
  language: string;
}

/** Row from: SELECT COUNT(*) as cnt FROM source_items ... */
export interface CountRow {
  cnt: number;
}

/** Row from: SELECT si.id, si.title, si.url, si.source_type, si.created_at FROM source_items si ... */
export interface SourceItemBriefRow {
  id: number;
  title: string;
  url: string | null;
  source_type: string;
  created_at: string;
}

/** Row from: SELECT id, title, url, source_type FROM source_items (fallback in export-context) */
export interface SourceItemMinimalRow {
  id: number;
  title: string;
  url: string | null;
  source_type: string;
}

/** Row from: SELECT si.id, si.title, si.url, si.source_type, i.timestamp as saved_at FROM interactions i JOIN source_items si ... */
export interface SavedItemRow {
  id: number;
  title: string;
  url: string | null;
  source_type: string;
  saved_at: string;
}

/** Row from: SELECT id, subject as title, data, created_at FROM temporal_events WHERE event_type = 'signal_emitted' */
export interface SignalEventRow {
  id: number;
  title: string;
  data: string;
  created_at: string;
}

// =============================================================================
// Agent Feedback
// =============================================================================

/**
 * Valid agent feedback outcome types
 */
export type AgentFeedbackOutcome = "used" | "rejected" | "partially_used";

/**
 * Parameters for recording agent feedback
 */
export interface RecordAgentFeedbackParams {
  item_ids: string[];
  outcome: AgentFeedbackOutcome;
  context?: string;
  session_task?: string;
}

/**
 * Result from recording agent feedback
 */
export interface AgentFeedbackResult {
  success: boolean;
  message: string;
  recorded_count: number;
}

/**
 * Parameters for getting agent feedback stats
 */
export interface GetAgentFeedbackStatsParams {
  days?: number;
}

/**
 * Outcome breakdown in agent feedback stats
 */
export interface AgentFeedbackOutcomeStats {
  used: number;
  rejected: number;
  partially_used: number;
  total: number;
}

/**
 * Per-source feedback stats
 */
export interface AgentFeedbackSourceStats {
  source_type: string;
  used: number;
  rejected: number;
  partially_used: number;
  total: number;
  usefulness_rate: number;
}

/**
 * A frequently used item
 */
export interface AgentFeedbackTopItem {
  item_id: string;
  title: string | null;
  source_type: string | null;
  used_count: number;
  rejected_count: number;
}

/**
 * A recent feedback entry
 */
export interface AgentFeedbackRecentEntry {
  item_id: string;
  outcome: string;
  context: string | null;
  session_task: string | null;
  recorded_at: string;
}

/**
 * Full agent feedback stats response
 */
export interface AgentFeedbackStats {
  period_days: number;
  outcomes: AgentFeedbackOutcomeStats;
  by_source: AgentFeedbackSourceStats[];
  top_used_items: AgentFeedbackTopItem[];
  recent_feedback: AgentFeedbackRecentEntry[];
}

/** Row from agent_feedback table */
export interface AgentFeedbackRow {
  item_id: string;
  outcome: string;
  context: string | null;
  session_task: string | null;
  recorded_at: string;
}

/** Row from agent_feedback stats query — outcome counts */
export interface AgentFeedbackOutcomeRow {
  outcome: string;
  cnt: number;
}

/** Row from agent_feedback stats query — source breakdown */
export interface AgentFeedbackSourceRow {
  source_type: string;
  outcome: string;
  cnt: number;
}

/** Row from agent_feedback stats query — top items */
export interface AgentFeedbackTopItemRow {
  item_id: string;
  title: string | null;
  source_type: string | null;
  used_count: number;
  rejected_count: number;
}
