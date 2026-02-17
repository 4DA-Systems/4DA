// Settings and monitoring types

export interface LicenseConfig {
  tier: 'free' | 'pro' | 'team';
  has_key: boolean;
  activated_at: string | null;
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
  license: LicenseConfig;
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

export interface LearnedBehavior {
  id: number;
  behavior_type: string;
  pattern: string;
  strength: number;
  last_triggered: string;
}
