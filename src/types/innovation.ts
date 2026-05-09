// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Innovation feature types

// Signal Chains
export interface SignalChain {
  id: string;
  chain_name: string;
  links: ChainLink[];
  overall_priority: string;
  resolution: 'open' | 'resolved' | 'expired' | 'snoozed';
  suggested_action: string;
  confidence: number;
  created_at: string;
  updated_at: string;
}

interface ChainLink {
  signal_type: string;
  source_item_id: number;
  title: string;
  timestamp: string;
  description: string;
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

interface HealthDimension {
  score: number;
  label: string;
  details: string;
}

interface HealthAlert {
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

interface TopicEngagement {
  topic: string;
  interactions: number;
  percent_of_total: number;
  sentiment: number;
}

interface CodebaseTopic {
  topic: string;
  file_count: number;
  source: string;
}

interface BlindSpot {
  topic: string;
  in_codebase: boolean;
  engagement_level: number;
  gap_description: string;
  risk_level: string;
}

interface TrendPoint {
  date: string;
  topic: string;
  engagement_level: number;
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

interface DependencyEntry {
  name: string;
  project_path: string;
}

interface EngagedTopic {
  topic: string;
  interactions: number;
  percent_of_total: number;
}

interface BlindSpotEntry {
  dependency: string;
  severity: string;
  days_stale: number;
}

interface SourceEngagement {
  source_type: string;
  items_seen: number;
  items_saved: number;
  engagement_rate: number;
}

interface DnaStats {
  total_items_processed: number;
  total_relevant: number;
  rejection_rate: number;
  project_count: number;
  dependency_count: number;
  days_active: number;
}
