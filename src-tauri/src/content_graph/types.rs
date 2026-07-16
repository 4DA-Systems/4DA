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
    /// Titles of the collapsed siblings (representative excluded), capped.
    pub member_titles: Vec<String>,
    /// Item ids of all members including the representative.
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
    Concept,
    Convergence,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphCluster {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<i64>,
    pub source_count: usize,
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
    /// Low-signal isolated items dropped beyond the orbit-ring cap.
    pub hidden_items: usize,
    pub time_window_days: u32,
    pub edge_threshold: String,
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
    pub embedding: Vec<f32>,
}

/// A story: one representative item standing for a set of near-duplicate
/// members (a package's advisory storm, the same announcement from N mirrors).
pub(super) struct StoryItem {
    /// Representative item; `embedding` is the normalized member centroid,
    /// `relevance_score` the member max.
    pub item: RawItem,
    pub member_ids: Vec<i64>,
    pub member_titles: Vec<String>,
    pub member_count: usize,
    /// Any member carries a dep_linker match to the user's declared stack.
    pub affects_you: bool,
}
