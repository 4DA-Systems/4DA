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
    pub created_at: String,
    pub embedding: Vec<f32>,
}
