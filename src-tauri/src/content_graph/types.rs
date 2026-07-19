// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Type definitions for the content graph.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ContentGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub clusters: Vec<GraphCluster>,
    pub meta: GraphMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphNode {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub signal_type: Option<String>,
    pub signal_priority: Option<String>,
    pub created_at: String,
    pub primary_topic: Option<String>,
    pub cluster_id: Option<String>,
    /// Total items this node represents (1 = a plain item; >1 = a story that
    /// collapsed near-duplicate items behind one representative).
    pub member_count: usize,
    /// Item ids of all members including the representative — the detail
    /// panel hydrates member rows from these via `get_graph_node_details`.
    pub member_ids: Vec<i64>,
    /// Content category: "security" | "release" | "discussion" | "research".
    /// The primary color channel — source identity moved to the tooltip.
    pub category: String,
    /// Any member is linked to one of the user's declared dependencies
    /// (dep_linker) — rendered as the gold "touches your stack" ring.
    pub affects_you: bool,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub edge_type: EdgeType,
    pub weight: f32,
    pub label: Option<String>,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Semantic,
    Chain,
    Convergence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphCluster {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<i64>,
    pub source_count: usize,
    /// Mean pairwise embedding cosine among members — how tight this theme
    /// really is. Emitted so coherence is measurable on every corpus, not
    /// asserted (Wave 4 self-measurement).
    pub coherence: f32,
    pub centroid_x: f32,
    pub centroid_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphMeta {
    pub total_items: usize,
    pub total_edges: usize,
    pub cluster_count: usize,
    /// Nodes that represent 2+ collapsed items.
    pub story_count: usize,
    /// Items folded behind story representatives (not separately visible).
    pub collapsed_items: usize,
    /// Items selected for the window but not on the map: non-curated isolated
    /// singletons beyond the shelf cap, plus items behind stories truncated by
    /// the node budget. Curated items are never hidden (corpus parity).
    pub hidden_items: usize,
    /// Total items matching the corpus selection in this window, independent
    /// of the node budget — lets the UI state real coverage ("top N of M")
    /// instead of implying the map is exhaustive.
    pub window_candidates: usize,
    pub time_window_days: u32,
    pub edge_threshold: String,
    /// Pair-count-weighted mean of per-cluster coherence — the graph's own
    /// quality gauge, comparable across corpora and windows. `None` when no
    /// cluster has 2+ members.
    pub mean_cluster_coherence: Option<f32>,
    /// Nodes whose story carries a persisted feed-curation verdict
    /// (corpus parity, Phase 95). The remainder are young not-yet-judged
    /// items; this count makes the curation ramp measurable, not hidden.
    pub curated_items: usize,
}

/// Internal raw item loaded from the database (not exported).
pub(super) struct RawItem {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub signal_type: Option<String>,
    pub signal_priority: Option<String>,
    /// Highest-confidence dependency this item was linked to (dep_linker),
    /// used as the exact grouping key for security-advisory stories.
    pub matched_package: Option<String>,
    pub created_at: String,
    /// The analysis run's persisted curation verdict says this item is in
    /// the feed corpus (`feed_relevant = 1`, Phase 95). False = not yet
    /// judged (young interim items) — judged-and-rejected items never load.
    pub curated: bool,
    pub embedding: Vec<f32>,
}

/// A story: one representative item standing for a set of near-duplicate
/// members (a package's advisory storm, the same announcement from N mirrors).
pub(super) struct StoryItem {
    /// Representative item; `embedding` is the normalized member centroid,
    /// `relevance_score` the member max.
    pub item: RawItem,
    pub member_ids: Vec<i64>,
    pub member_count: usize,
    /// Any member carries a dep_linker match to the user's declared stack.
    pub affects_you: bool,
}
