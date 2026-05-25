// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content Graph — relationship visualization for surfaced intelligence.
//!
//! Assembles edges from four existing relationship systems:
//! - topic_clustering (cosine >= 0.77 semantic similarity)
//! - signal_chains (temporal causal links across days)
//! - concept_graph (topic co-occurrence)
//! - dedup (Jaccard >= 0.65 near-duplicates)
//!
//! Computes a deterministic force-directed layout in Rust and returns
//! positioned nodes + edges for the frontend to render without JS layout.

mod clustering;
mod edges;
mod layout;
mod loading;
mod types;

use std::collections::HashSet;

use tracing::info;

use crate::error::Result;

#[allow(unused_imports)]
pub use types::{ContentGraph, EdgeType, GraphCluster, GraphEdge, GraphMeta, GraphNode};

const DEFAULT_DAYS: u32 = 7;
const DEFAULT_MAX_NODES: usize = 150;
const SIMILARITY_THRESHOLD: f32 = 0.77;
const LEXICAL_FALLBACK_THRESHOLD: f32 = 0.73;
const LEXICAL_OVERLAP_MIN: f32 = 0.60;
const MIN_EDGES_TO_APPEAR: usize = 1;
const LAYOUT_WIDTH: f32 = 1000.0;
const LAYOUT_HEIGHT: f32 = 800.0;
const LAYOUT_ITERATIONS: usize = 80;

// ============================================================================
// Graph Construction
// ============================================================================

pub fn build_graph(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<ContentGraph> {
    let items = loading::load_scored_items(conn, days, max_nodes)?;
    if items.is_empty() {
        return Ok(ContentGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            meta: GraphMeta {
                total_items: 0,
                total_edges: 0,
                cluster_count: 0,
                time_window_days: days,
                edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
            },
        });
    }

    let mut edge_list = Vec::new();
    let id_set: HashSet<i64> = items.iter().map(|i| i.id).collect();

    edges::compute_semantic_edges(&items, &mut edge_list);
    edges::compute_chain_edges(conn, &id_set, &mut edge_list);
    edges::merge_duplicate_edges(&mut edge_list);

    let mut clusters = clustering::compute_clusters(&items, &edge_list);
    clustering::assign_cluster_labels(&items, &mut clusters);

    let edge_count_per_node = edges::count_edges_per_node(&edge_list);
    let mut nodes: Vec<GraphNode> = items
        .iter()
        .filter(|item| {
            edge_count_per_node.get(&item.id).copied().unwrap_or(0) >= MIN_EDGES_TO_APPEAR
        })
        .map(|item| {
            let cluster_id = clusters
                .iter()
                .find(|c| c.node_ids.contains(&item.id))
                .map(|c| c.id.clone());
            GraphNode {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                source_type: item.source_type.clone(),
                relevance_score: item.relevance_score,
                signal_type: None,
                signal_priority: None,
                created_at: item.created_at.clone(),
                primary_topic: None,
                cluster_id,
                x: 0.0,
                y: 0.0,
            }
        })
        .collect();

    let visible_ids: HashSet<i64> = nodes.iter().map(|n| n.id).collect();
    edge_list.retain(|e| visible_ids.contains(&e.source) && visible_ids.contains(&e.target));
    let clusters: Vec<GraphCluster> = clusters
        .into_iter()
        .map(|mut c| {
            c.node_ids.retain(|id| visible_ids.contains(id));
            c
        })
        .filter(|c| c.node_ids.len() >= 2)
        .collect();

    let mut clusters = clusters;
    layout::compute_layout(&mut nodes, &edge_list, &mut clusters);

    let meta = GraphMeta {
        total_items: nodes.len(),
        total_edges: edge_list.len(),
        cluster_count: clusters.len(),
        time_window_days: days,
        edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
    };

    info!(
        target: "4da::content_graph",
        nodes = nodes.len(),
        edges = edge_list.len(),
        clusters = clusters.len(),
        "Content graph built"
    );

    Ok(ContentGraph {
        nodes,
        edges: edge_list,
        clusters,
        meta,
    })
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub fn build_content_graph(days: Option<u32>, max_nodes: Option<usize>) -> Result<ContentGraph> {
    let conn = crate::open_db_connection()?;
    let d = days.unwrap_or(DEFAULT_DAYS);
    let m = max_nodes.unwrap_or(DEFAULT_MAX_NODES);
    build_graph(&conn, d, m)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use types::RawItem;

    #[test]
    fn test_empty_graph() {
        let graph = ContentGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            meta: GraphMeta {
                total_items: 0,
                total_edges: 0,
                cluster_count: 0,
                time_window_days: 7,
                edge_threshold: "cosine >= 0.77".to_string(),
            },
        };
        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_semantic_edge_above_threshold() {
        let items = vec![
            RawItem {
                id: 1,
                title: "Rust async runtime".to_string(),
                url: None,
                source_type: "hackernews".to_string(),
                relevance_score: 0.8,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "Rust async runtime update".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
        ];

        let mut edge_list = Vec::new();
        edges::compute_semantic_edges(&items, &mut edge_list);

        assert_eq!(
            edge_list.len(),
            1,
            "identical embeddings should create an edge"
        );
        assert_eq!(edge_list[0].edge_type, EdgeType::Semantic);
        assert!((edge_list[0].weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_no_edge_below_threshold() {
        let items = vec![
            RawItem {
                id: 1,
                title: "Rust async".to_string(),
                url: None,
                source_type: "hackernews".to_string(),
                relevance_score: 0.8,
                created_at: "2026-05-24".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "Python web framework".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "2026-05-24".to_string(),
                embedding: vec![0.0, 1.0, 0.0],
            },
        ];

        let mut edge_list = Vec::new();
        edges::compute_semantic_edges(&items, &mut edge_list);

        assert!(
            edge_list.is_empty(),
            "orthogonal embeddings should create no edge"
        );
    }

    #[test]
    fn test_merge_duplicate_edges() {
        let mut edge_list = vec![
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Semantic,
                weight: 0.85,
                label: Some("similarity: 0.85".to_string()),
                methods: vec!["semantic".to_string()],
            },
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Chain,
                weight: 0.70,
                label: Some("chain: tokio".to_string()),
                methods: vec!["signal_chain".to_string()],
            },
        ];

        edges::merge_duplicate_edges(&mut edge_list);

        assert_eq!(edge_list.len(), 1, "duplicate edges should merge");
        assert_eq!(edge_list[0].edge_type, EdgeType::Convergence);
        assert_eq!(edge_list[0].methods.len(), 2);
        assert!((edge_list[0].weight - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cluster_formation() {
        let items = vec![
            RawItem {
                id: 1,
                title: "A".to_string(),
                url: None,
                source_type: "hn".to_string(),
                relevance_score: 0.8,
                created_at: "".to_string(),
                embedding: vec![1.0, 0.0],
            },
            RawItem {
                id: 2,
                title: "B".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                created_at: "".to_string(),
                embedding: vec![1.0, 0.0],
            },
            RawItem {
                id: 3,
                title: "C".to_string(),
                url: None,
                source_type: "github".to_string(),
                relevance_score: 0.6,
                created_at: "".to_string(),
                embedding: vec![0.0, 1.0],
            },
        ];

        let edge_list = vec![GraphEdge {
            source: 1,
            target: 2,
            edge_type: EdgeType::Semantic,
            weight: 0.9,
            label: None,
            methods: vec!["semantic".to_string()],
        }];

        let clusters = clustering::compute_clusters(&items, &edge_list);
        assert_eq!(clusters.len(), 1, "connected items should form one cluster");
        assert_eq!(clusters[0].node_ids.len(), 2);
        assert_eq!(clusters[0].source_count, 2);
    }

    #[test]
    fn test_layout_positions_in_bounds() {
        let mut nodes = vec![
            GraphNode {
                id: 1,
                title: "A".to_string(),
                url: None,
                source_type: "hn".to_string(),
                relevance_score: 0.8,
                signal_type: None,
                signal_priority: None,
                created_at: "".to_string(),
                primary_topic: None,
                cluster_id: None,
                x: 0.0,
                y: 0.0,
            },
            GraphNode {
                id: 2,
                title: "B".to_string(),
                url: None,
                source_type: "reddit".to_string(),
                relevance_score: 0.7,
                signal_type: None,
                signal_priority: None,
                created_at: "".to_string(),
                primary_topic: None,
                cluster_id: None,
                x: 0.0,
                y: 0.0,
            },
        ];

        let edge_list = vec![GraphEdge {
            source: 1,
            target: 2,
            edge_type: EdgeType::Semantic,
            weight: 0.9,
            label: None,
            methods: vec![],
        }];

        layout::compute_layout(&mut nodes, &edge_list, &mut []);

        for node in &nodes {
            assert!(
                node.x >= 0.0 && node.x <= LAYOUT_WIDTH,
                "x out of bounds: {}",
                node.x
            );
            assert!(
                node.y >= 0.0 && node.y <= LAYOUT_HEIGHT,
                "y out of bounds: {}",
                node.y
            );
        }
    }

    #[test]
    fn test_title_word_overlap_high() {
        let overlap = edges::title_word_overlap(
            "React 19 server components released",
            "React 19 server components update",
        );
        assert!(
            overlap > LEXICAL_OVERLAP_MIN,
            "similar titles should overlap >{LEXICAL_OVERLAP_MIN}, got {overlap}"
        );
    }

    #[test]
    fn test_title_word_overlap_low() {
        let overlap = edges::title_word_overlap("Rust async runtime", "Python web framework");
        assert!(overlap < LEXICAL_OVERLAP_MIN);
    }

    #[test]
    fn test_extract_title_keywords() {
        let keywords = clustering::extract_title_keywords("Show HN: A New Rust Web Framework");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"web".to_string()));
        assert!(keywords.contains(&"framework".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
        assert!(!keywords.contains(&"hn".to_string()));
    }

    #[test]
    fn test_edge_count_per_node() {
        let edge_list = vec![
            GraphEdge {
                source: 1,
                target: 2,
                edge_type: EdgeType::Semantic,
                weight: 0.9,
                label: None,
                methods: vec![],
            },
            GraphEdge {
                source: 1,
                target: 3,
                edge_type: EdgeType::Chain,
                weight: 0.8,
                label: None,
                methods: vec![],
            },
        ];

        let counts = edges::count_edges_per_node(&edge_list);
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&2], 1);
        assert_eq!(counts[&3], 1);
    }
}
