// Innovation feature types

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
