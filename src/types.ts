// Shared types for 4DA application

export interface ContextFile {
  path: string;
  content: string;
  lines: number;
}

export interface RelevanceMatch {
  source_file: string;
  matched_text: string;
  similarity: number;
}

export interface SourceRelevance {
  id: number;
  title: string;
  url: string | null;
  top_score: number;
  matches: RelevanceMatch[];
  relevant: boolean;
  explanation?: string;
  source_type?: string;
  confidence?: number;
  score_breakdown?: ScoreBreakdown;
  signal_type?: string;
  signal_priority?: string;
  signal_action?: string;
  signal_triggers?: string[];
  seen_on?: string[];
  similar_count?: number;
  similar_titles?: string[];
  serendipity?: boolean;
}

export interface ScoreBreakdown {
  context_score: number;
  interest_score: number;
  keyword_score?: number;
  ace_boost: number;
  affinity_mult: number;
  anti_penalty: number;
  freshness_mult?: number;
  feedback_boost?: number;
  source_quality_boost?: number;
  confidence_by_signal: Record<string, number>;
  signal_count?: number;
  confirmed_signals?: string[];
  confirmation_mult?: number;
  /** Dependency match score (0.0-1.0): how strongly content matches installed packages */
  dep_match_score?: number;
  /** Package names from user's dependency graph that matched this content */
  matched_deps?: string[];
  /** Domain relevance (0.15 off-domain to 1.0 primary stack match) */
  domain_relevance?: number;
  /** Content quality multiplier (0.5 clickbait to 1.2 authoritative) */
  content_quality_mult?: number;
  /** Novelty multiplier (0.6 introductory to 1.15 release) */
  novelty_mult?: number;
  /** Intent boost from recent work topics (0.0 to 0.15) */
  intent_boost?: number;
  /** Content type classification (e.g. "security_advisory", "show_and_tell") */
  content_type?: string;
  /** Content DNA utility multiplier (0.3 hiring to 1.3 security) */
  content_dna_mult?: number;
  /** Competing tech penalty multiplier (0.5 or 1.0) */
  competing_mult?: number;
  /** LLM relevance score (1-5 scale) */
  llm_score?: number;
  /** LLM's one-sentence explanation */
  llm_reason?: string;
}

export interface AnalysisProgress {
  stage: string;
  progress: number;
  message: string;
  items_processed: number;
  items_total: number;
}

export interface Settings {
  llm: {
    provider: string;
    model: string;
    has_api_key: boolean;
    base_url: string | null;
  };
  rerank: {
    enabled: boolean;
    max_items_per_batch: number;
    min_embedding_score: number;
    daily_token_limit: number;
    daily_cost_limit_cents: number;
  };
  usage: {
    tokens_today: number;
    cost_today_cents: number;
    tokens_total: number;
    items_reranked: number;
  };
  embedding_threshold: number;
}

export interface MonitoringStatus {
  enabled: boolean;
  interval_minutes: number;
  is_checking: boolean;
  last_check_ago: string | null;
  total_checks: number;
}

export interface UserContext {
  role: string | null;
  tech_stack: string[];
  domains: string[];
  interests: Array<{
    id: number;
    topic: string;
    weight: number;
    source: string;
    has_embedding: boolean;
  }>;
  exclusions: string[];
  stats: {
    interest_count: number;
    exclusion_count: number;
  };
}

export interface Anomaly {
  id: number | null;
  anomaly_type: string;
  topic: string | null;
  description: string;
  confidence: number;
  severity: string;
  evidence: string[];
  detected_at: string;
  resolved: boolean;
}

export interface SystemHealth {
  anomalies: Anomaly[];
  anomalyCount: number;
  embeddingOperational: boolean;
  rateLimitStatus: {
    global_remaining: number;
    source_remaining: number;
    is_limited: boolean;
  } | null;
  accuracyMetrics: {
    precision: number;
    engagement_rate: number;
    calibration_error: number;
  } | null;
}

export interface AppState {
  contextFiles: ContextFile[];
  relevanceResults: SourceRelevance[];
  status: string;
  loading: boolean;
  analysisComplete: boolean;
  progress: number;
  progressMessage: string;
  progressStage: string;
  lastAnalyzedAt: Date | null;
}

export interface LearnedBehavior {
  id: number;
  behavior_type: string;
  pattern: string;
  strength: number;
  last_triggered: string;
}

export type FeedbackAction = 'save' | 'dismiss' | 'mark_irrelevant' | 'click';
export type FeedbackGiven = Record<number, FeedbackAction>;

// Indexed Documents Types
export interface IndexedDocument {
  id: number;
  file_path: string;
  file_name: string;
  file_type: string;
  file_size: number;
  word_count: number;
  extraction_confidence: number;
  indexed_at: string;
}

export interface DocumentChunk {
  index: number;
  content: string;
  word_count: number;
}

export interface IndexedDocumentsResponse {
  documents: IndexedDocument[];
  total: number;
  limit: number;
  offset: number;
}

export interface DocumentContentResponse {
  document: IndexedDocument;
  chunks: DocumentChunk[];
}

export interface DocumentSearchResult {
  id: number;
  file_path: string;
  file_name: string;
  file_type: string;
  word_count: number;
  indexed_at: string;
  preview: string;
}

// Void Engine types
export interface VoidSignal {
  pulse: number;             // 0-1: source fetch activity
  heat: number;              // 0-1: avg relevance from last analysis
  burst: number;             // 0-1: discovery flash (score > 0.7)
  morph: number;             // 0-1: ACE file change activity
  error: number;             // 0-1: recent error indicator
  staleness: number;         // 0-1: hours since last analysis / 24
  item_count: number;        // total cached items
  signal_intensity: number;  // 0-1: highest signal priority / 4
  signal_urgency: number;    // 0-1: weighted urgency from signal types
  critical_count: number;    // count of critical-priority signals
  signal_color_shift: number; // -1 to +1: cool (learning) to warm (alert)
}

export interface SuggestedInterest {
  topic: string;
  source: string;
  confidence: number;
  already_declared: boolean;
}

export interface IndexedStats {
  total_documents: number;
  total_chunks: number;
  total_words: number;
  by_type: Array<{ file_type: string; count: number }>;
}

// ============================================================================
// Innovation Feature Types
// ============================================================================

// Predictive Context
export interface PredictedContext {
  predicted_topics: [string, number][];
  predicted_at: string;
  reasoning: string;
  confidence: number;
}

// Knowledge Decay
export interface KnowledgeGap {
  dependency: string;
  version: string | null;
  project_path: string;
  missed_items: MissedItem[];
  gap_severity: 'critical' | 'high' | 'medium' | 'low';
  days_since_last_engagement: number;
}

export interface MissedItem {
  item_id: number;
  title: string;
  url: string | null;
  source_type: string;
  created_at: string;
}

// Signal Chains
export interface SignalChain {
  id: string;
  chain_name: string;
  links: ChainLink[];
  overall_priority: string;
  resolution: 'open' | 'resolved' | 'expired' | 'snoozed';
  suggested_action: string;
  created_at: string;
  updated_at: string;
}

export interface ChainLink {
  signal_type: string;
  source_item_id: number;
  title: string;
  timestamp: string;
  description: string;
}

// Semantic Diff
export interface SemanticShift {
  topic: string;
  drift_magnitude: number;
  direction: string;
  representative_items: number[];
  period: string;
  detected_at: string;
}

// Reverse Relevance
export interface ReverseMention {
  source_item_id: number;
  title: string;
  url: string | null;
  mentioned_project: string;
  mention_context: string;
  source_type: string;
  discovered_at: string;
}

// Project Health
export interface ProjectHealth {
  project_path: string;
  project_name: string;
  overall_score: number;
  freshness: HealthDimension;
  security: HealthDimension;
  momentum: HealthDimension;
  community: HealthDimension;
  alerts: HealthAlert[];
  last_checked: string;
  dependency_count: number;
}

export interface HealthDimension {
  score: number;
  label: string;
  details: string;
}

export interface HealthAlert {
  severity: string;
  message: string;
  dependency: string | null;
}

// Attention Dashboard
export interface AttentionReport {
  period_days: number;
  topic_engagement: TopicEngagement[];
  codebase_topics: CodebaseTopic[];
  blind_spots: BlindSpot[];
  attention_trend: TrendPoint[];
}

export interface TopicEngagement {
  topic: string;
  interactions: number;
  percent_of_total: number;
  sentiment: number;
}

export interface CodebaseTopic {
  topic: string;
  file_count: number;
  source: string;
}

export interface BlindSpot {
  topic: string;
  in_codebase: boolean;
  engagement_level: number;
  gap_description: string;
  risk_level: string;
}

export interface TrendPoint {
  date: string;
  topic: string;
  engagement_level: number;
}

// Audio Briefing
export interface AudioBriefingStatus {
  available: boolean;
  file_path: string | null;
  duration_seconds: number | null;
  generated_at: string | null;
  tts_engine: string;
}

// Context Handoff
export interface ContextPacket {
  generated_at: string;
  version: string;
  active_context: {
    detected_tech: string[];
    active_topics: string[];
    interests: string[];
    exclusions: string[];
    context_dirs: string[];
    recent_work_topics: string[];
  };
  open_signals: Array<{
    item_id: number;
    title: string;
    signal_type: string;
    priority: string;
    action: string | null;
    source_type: string;
  }>;
  saved_items: Array<{
    item_id: number;
    title: string;
    url: string | null;
    source_type: string;
    saved_at: string;
  }>;
  recent_briefing: string | null;
  attention_state: {
    top_topics: [string, number][];
    topic_count: number;
    total_interactions: number;
  };
  suggested_actions: string[];
}

// Developer DNA
export interface DeveloperDna {
  generated_at: string;
  primary_stack: string[];
  adjacent_tech: string[];
  top_dependencies: DependencyEntry[];
  interests: string[];
  top_engaged_topics: EngagedTopic[];
  blind_spots: BlindSpotEntry[];
  source_engagement: SourceEngagement[];
  stats: DnaStats;
  identity_summary: string;
}

export interface DependencyEntry {
  name: string;
  project_path: string;
}

export interface EngagedTopic {
  topic: string;
  interactions: number;
  percent_of_total: number;
}

export interface BlindSpotEntry {
  dependency: string;
  severity: string;
  days_stale: number;
}

export interface SourceEngagement {
  source_type: string;
  items_seen: number;
  items_saved: number;
  engagement_rate: number;
}

export interface DnaStats {
  total_items_processed: number;
  total_relevant: number;
  rejection_rate: number;
  project_count: number;
  dependency_count: number;
  days_active: number;
}

// ============================================================================
// Content Reader & AI Summary Types
// ============================================================================

export interface ItemContent {
  content: string;
  source_type: string;
  word_count: number;
  has_summary: boolean;
  summary: string | null;
}

export interface ItemSummary {
  summary: string;
  cached: boolean;
}

export interface SavedItem {
  item_id: number;
  title: string;
  url: string | null;
  source_type: string;
  saved_at: string;
  summary: string | null;
  content_preview: string | null;
}

export interface SourceHealthStatus {
  source_type: string;
  status: 'healthy' | 'error' | 'circuit_open' | 'unknown';
  last_success_relative: string | null;
  items_fetched: number;
  gap_message: string | null;
}
