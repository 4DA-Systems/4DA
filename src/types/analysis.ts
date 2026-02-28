// Analysis-related types

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
  /** STREETS revenue engine match (e.g. "Engine 1: Digital Products") */
  streets_engine?: string;
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
  /** Intent boost from recent work topics (0.0 to 0.25) */
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
  /** Decision window boost applied (0.0-0.20) */
  window_boost?: number;
  /** ID of matched decision window */
  matched_window_id?: number;
  /** Skill gap boost from sovereign profile intelligence (0.0-0.20) */
  skill_gap_boost?: number;
}

export interface AnalysisProgress {
  stage: string;
  progress: number;
  message: string;
  items_processed: number;
  items_total: number;
}

export interface ProValueReport {
  period_days: number;
  briefings_generated: number;
  signals_detected: number;
  knowledge_gaps_caught: number;
  predictions_made: number;
  queries_run: number;
  items_surfaced: number;
  attention_insights: number;
  estimated_hours_saved: number;
  data_age_days: number;
  total_feedback_events: number;
  active_since: string | null;
}
