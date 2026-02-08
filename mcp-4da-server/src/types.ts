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
