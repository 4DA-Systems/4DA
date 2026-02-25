// Intelligence Metabolism types — Autophagy + Decision Advantage

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
