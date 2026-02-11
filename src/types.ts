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
