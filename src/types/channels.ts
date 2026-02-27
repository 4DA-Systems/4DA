// Information Channels types — matches Rust structs from channels.rs

export type ChannelStatus = 'active' | 'paused' | 'archived';
export type ChannelFreshness = 'fresh' | 'stale' | 'never_rendered';

export interface Channel {
  id: number;
  slug: string;
  title: string;
  description: string;
  topic_query: string[];
  status: ChannelStatus;
  source_count: number;
  render_count: number;
  last_rendered_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ChannelSummary {
  id: number;
  slug: string;
  title: string;
  description: string;
  source_count: number;
  render_count: number;
  freshness: ChannelFreshness;
  last_rendered_at: string | null;
}

export interface ChannelRender {
  id: number;
  channel_id: number;
  version: number;
  content_markdown: string;
  content_hash: string;
  source_item_ids: number[];
  model: string | null;
  tokens_used: number | null;
  latency_ms: number | null;
  rendered_at: string;
}

export interface RenderProvenance {
  render_id: number;
  claim_index: number;
  claim_text: string;
  source_item_ids: number[];
  source_titles: string[];
  source_urls: string[];
}

export interface ChannelChangelog {
  channel_id: number;
  from_version: number;
  to_version: number;
  summary: string;
  added_lines: string[];
  removed_lines: string[];
  changed_at: string;
}

export interface ChannelSourceMatch {
  channel_id: number;
  source_item_id: number;
  title: string;
  url: string | null;
  source_type: string;
  match_score: number;
  matched_at: string;
}
