// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Types for the Sovereign Content Engine (content personalization)

export interface PersonalizedLesson {
  content: string;
  insight_blocks: InsightBlock[];
  mirror_blocks: MirrorBlock[];
  temporal_blocks: TemporalBlock[];
  depth: PersonalizationDepth;
  context_hash: string;
}

export interface PersonalizationDepth {
  l1_resolved: number;
  l1_fallbacks: number;
  l2_evaluated: number;
  l3_cards: number;
  l4_connections: number;
  l5_temporal: number;
  llm_pending: boolean;
}

export interface InsightBlock {
  block_id: string;
  position: BlockPosition;
  content: InsightContent;
  source_labels: string[];
  confidence: number;
}

export type BlockPosition =
  | { type: 'injection'; marker_id: string }
  | { type: 'end' };

export type InsightContent =
  | { type: 'card' } & SovereignInsightCard
  | { type: 'prose'; text: string; model: string };

export interface SovereignInsightCard {
  card_type: CardType;
  title: string;
  data_points: DataPoint[];
  visualization: Visualization | null;
}

export type CardType =
  | 'hardware_benchmark'
  | 'stack_fit'
  | 'cost_projection'
  | 't_shape_diagram'
  | 'engine_ranking'
  | 'blind_spot_alert'
  | 'competitive_position'
  | 'temporal_delta'
  | 'feed_echo'
  | 'progress_gate';

export interface DataPoint {
  label: string;
  value: string;
  context: string | null;
  highlight: boolean;
}

export type Visualization =
  | { type: 'bar_chart'; bars: BarEntry[]; max_value: number; unit: string }
  | { type: 'rank_list'; items: RankItem[] }
  | { type: 't_shape'; primary: string; depth_label: string; adjacent: string[]; breadth_label: string }
  | { type: 'rate_table'; headers: string[]; rows: TableRow[] }
  | { type: 'diff_ribbon'; added: string[]; removed: string[]; changed: DiffChange[] };

export interface BarEntry {
  label: string;
  value: number;
  highlight: boolean;
}

export interface RankItem {
  rank: number;
  name: string;
  score: number;
  matches_stack: boolean;
}

export interface TableRow {
  cells: string[];
  highlight: boolean;
}

export interface DiffChange {
  field: string;
  old_value: string;
  new_value: string;
}

export interface MirrorBlock {
  block_id: string;
  connection_type: 'blind_spot_moat' | 'feed_predicts_engine' | 'radar_momentum';
  headline: string;
  insight: string;
  data_sources: string[];
  content: InsightContent | null;
}

export interface TemporalBlock {
  block_id: string;
  block_type: TemporalBlockType;
}

export type TemporalBlockType =
  | { type: 'diff_ribbon'; added: string[]; removed: string[]; changed: DiffChange[] }
  | { type: 'progressive_reveal'; newly_completed: string[]; unlocked_content: string[] }
  | { type: 'feed_echo'; items: FeedEchoItem[] };

export interface FeedEchoItem {
  title: string;
  source: string;
  url: string | null;
  matched_topic: string;
  fetched_at: string;
}
