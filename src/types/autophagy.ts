// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Intelligence Metabolism types — Autophagy + Decision Advantage + Intelligence Pulse

export interface AutophagyCycleResult {
  items_analyzed: number;
  items_pruned: number;
  calibrations_produced: number;
  topic_decay_rates_updated: number;
  source_autopsies_produced: number;
  anti_patterns_detected: number;
  duration_ms: number;
}

export interface AutophagyStatus {
  last_cycle: AutophagyCycleResult | null;
  total_cycles: number;
  total_calibrations: number;
  total_anti_patterns: number;
}

export interface CalibrationDelta {
  topic: string;
  scored_avg: number;
  engaged_avg: number;
  delta: number;
  sample_size: number;
  confidence: number;
}

export interface TopicDecayProfile {
  topic: string;
  half_life_hours: number;
  peak_relevance_age_hours: number;
}

export interface DecisionWindow {
  id: number;
  window_type: 'security_patch' | 'migration' | 'adoption' | 'knowledge';
  title: string;
  description: string;
  urgency: number;
  relevance: number;
  dependency: string | null;
  status: 'open' | 'acted' | 'expired' | 'closed';
  opened_at: string;
  expires_at: string | null;
  lead_time_hours: number | null;
  streets_engine: string | null;
}

// Intelligence Pulse types (shared between IntelligencePulse + BriefingView via store)

export interface CalibrationInsight {
  topic: string;
  delta: number;
  sample_size: number;
  confidence: number;
}

export interface SourceQuality {
  source_type: string;
  items_surfaced: number;
  items_engaged: number;
  engagement_rate: number;
}

export interface IntelligencePulseData {
  items_analyzed_7d: number;
  items_surfaced_7d: number;
  rejection_rate: number;
  calibration_accuracy: number;
  top_calibrations: CalibrationInsight[];
  source_quality: SourceQuality[];
  anti_patterns_detected: number;
  total_cycles: number;
  learning_narratives: string[];
}

// Data Health types (database maintenance + cleanup)

export interface DbStats {
  source_items: number;
  context_chunks: number;
  feedback_count: number;
  sources_count: number;
  embeddings_count: number;
  digested_intelligence: number;
  decision_windows: number;
  autophagy_cycles: number;
  necessity_scores: number;
  db_size_bytes: number;
  oldest_item_date: string | null;
}

export interface DataHealth {
  stats: DbStats;
  retention_days: number;
  db_size_mb: number;
  health_status: 'healthy' | 'growing' | 'needs_attention';
}

export interface MaintenanceResult {
  deleted_items: number;
  deleted_feedback: number;
  deleted_void: number;
  deleted_intelligence: number;
  deleted_windows: number;
  deleted_cycles: number;
  deleted_necessity: number;
  vacuumed: boolean;
}

export interface CompoundAdvantageScore {
  score: number;
  period: string;
  items_surfaced: number;
  avg_lead_time_hours: number;
  windows_opened: number;
  windows_acted: number;
  windows_expired: number;
  knowledge_gaps_closed: number;
  calibration_accuracy: number;
  trend: number;
}
