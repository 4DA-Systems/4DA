// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content Graph — relationship visualization for surfaced intelligence.
//!
//! Pipeline: load scored items → collapse near-duplicates into STORIES
//! (story.rs — one node per advisory storm / mirrored announcement) → compute
//! semantic + signal-chain edges between stories → connected-component
//! clusters with c-TF-IDF labels → two-phase cluster-first layout →
//! sparsify display edges to backbone + top-K.
//!
//! Everything is computed deterministically in Rust; the frontend renders
//! positioned nodes without any JS layout.

mod clustering;
mod edges;
mod layout;
mod loading;
mod story;
mod types;

use std::collections::{HashMap, HashSet};

use tracing::info;

use crate::error::Result;

#[allow(unused_imports)]
pub use types::{ContentGraph, EdgeType, GraphCluster, GraphEdge, GraphMeta, GraphNode};

const DEFAULT_DAYS: u32 = 7;
const DEFAULT_MAX_NODES: usize = 150;
const SIMILARITY_THRESHOLD: f32 = 0.77;
const LEXICAL_FALLBACK_THRESHOLD: f32 = 0.73;
const LEXICAL_OVERLAP_MIN: f32 = 0.60;
/// Isolated singletons shown on the orbit ring, by relevance; the rest stay
/// in the List view (counted honestly in `meta.hidden_items`).
const RING_CAP: usize = 40;
/// Per-node top-K edges kept for display (plus the spanning backbone).
const TOP_K_EDGES: usize = 4;

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
                story_count: 0,
                collapsed_items: 0,
                hidden_items: 0,
                time_window_days: days,
                edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
            },
        });
    }

    // Collapse near-duplicates into stories FIRST: redundancy becomes one
    // node instead of a rendered clique (86% of live edges before this).
    let stories = story::collapse_stories(items);
    let mut rep_of: HashMap<i64, i64> = HashMap::new();
    for s in &stories {
        for &member in &s.member_ids {
            rep_of.insert(member, s.item.id);
        }
    }
    let story_items: Vec<types::RawItem> =
        stories.iter().map(|s| story::clone_raw(&s.item)).collect();

    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&story_items, &mut edge_list);
    edges::compute_chain_edges(conn, &rep_of, &mut edge_list);
    edges::merge_duplicate_edges(&mut edge_list);

    let clusters = clustering::compute_clusters(&story_items, &edge_list);

    // Visibility: anything connected or aggregated appears; isolated plain
    // items appear on the orbit ring up to RING_CAP by relevance.
    struct Vis {
        id: i64,
        relevance: f32,
        connected: bool,
        is_story: bool,
    }
    let degree = edges::count_edges_per_node(&edge_list);
    let mut handles: Vec<Vis> = stories
        .iter()
        .map(|s| Vis {
            id: s.item.id,
            relevance: s.item.relevance_score,
            connected: degree.get(&s.item.id).copied().unwrap_or(0) > 0,
            is_story: s.member_count > 1,
        })
        .collect();
    handles.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    let mut visible_ids: HashSet<i64> = HashSet::new();
    let mut ring_candidates: Vec<&Vis> = Vec::new();
    for handle in &handles {
        if handle.connected || handle.is_story {
            visible_ids.insert(handle.id);
        } else {
            ring_candidates.push(handle);
        }
    }
    let hidden_items = ring_candidates.len().saturating_sub(RING_CAP);
    for handle in ring_candidates.iter().take(RING_CAP) {
        visible_ids.insert(handle.id);
    }

    let mut nodes: Vec<GraphNode> = stories
        .iter()
        .filter(|s| visible_ids.contains(&s.item.id))
        .map(|s| {
            let cluster_id = clusters
                .iter()
                .find(|c| c.node_ids.contains(&s.item.id))
                .map(|c| c.id.clone());
            GraphNode {
                id: s.item.id,
                title: s.item.title.clone(),
                url: s.item.url.clone(),
                source_type: s.item.source_type.clone(),
                relevance_score: s.item.relevance_score,
                signal_type: s.item.signal_type.clone(),
                signal_priority: s.item.signal_priority.clone(),
                created_at: s.item.created_at.clone(),
                primary_topic: None,
                cluster_id,
                member_count: s.member_count,
                member_titles: s.member_titles.clone(),
                member_ids: s.member_ids.clone(),
                x: 0.0,
                y: 0.0,
            }
        })
        .collect();

    edge_list.retain(|e| visible_ids.contains(&e.source) && visible_ids.contains(&e.target));
    let mut clusters: Vec<GraphCluster> = clusters
        .into_iter()
        .map(|mut c| {
            c.node_ids.retain(|id| visible_ids.contains(id));
            c
        })
        .filter(|c| c.node_ids.len() >= 2)
        .collect();
    clustering::assign_cluster_labels(&story_items, &mut clusters);

    // Layout sees the full retained edge set (affinity fidelity); display
    // gets the sparsified backbone + top-K.
    layout::compute_layout(&mut nodes, &edge_list, &mut clusters);
    edges::sparsify_edges(&mut edge_list, TOP_K_EDGES);

    let story_count = nodes.iter().filter(|n| n.member_count > 1).count();
    let collapsed_items: usize = nodes.iter().map(|n| n.member_count.saturating_sub(1)).sum();

    let meta = GraphMeta {
        total_items: nodes.len(),
        total_edges: edge_list.len(),
        cluster_count: clusters.len(),
        story_count,
        collapsed_items,
        hidden_items,
        time_window_days: days,
        edge_threshold: format!("cosine >= {SIMILARITY_THRESHOLD}"),
    };

    info!(
        target: "4da::content_graph",
        nodes = nodes.len(),
        edges = edge_list.len(),
        clusters = clusters.len(),
        stories = story_count,
        collapsed = collapsed_items,
        hidden = hidden_items,
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

    fn raw(id: i64, title: &str, source_type: &str, score: f32, embedding: Vec<f32>) -> RawItem {
        RawItem {
            id,
            title: title.to_string(),
            url: None,
            source_type: source_type.to_string(),
            relevance_score: score,
            signal_type: None,
            signal_priority: None,
            matched_package: None,
            created_at: "2026-05-24".to_string(),
            embedding,
        }
    }

    fn edge(source: i64, target: i64, weight: f32) -> GraphEdge {
        GraphEdge {
            source,
            target,
            edge_type: EdgeType::Semantic,
            weight,
            label: None,
            methods: vec!["semantic".to_string()],
        }
    }

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
                story_count: 0,
                collapsed_items: 0,
                hidden_items: 0,
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
            raw(
                1,
                "Rust async runtime",
                "hackernews",
                0.8,
                vec![1.0, 0.0, 0.0],
            ),
            raw(
                2,
                "Rust async runtime update",
                "reddit",
                0.7,
                vec![1.0, 0.0, 0.0],
            ),
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
            raw(1, "Rust async", "hackernews", 0.8, vec![1.0, 0.0, 0.0]),
            raw(
                2,
                "Python web framework",
                "reddit",
                0.7,
                vec![0.0, 1.0, 0.0],
            ),
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
            raw(1, "A", "hn", 0.8, vec![1.0, 0.0]),
            raw(2, "B", "reddit", 0.7, vec![1.0, 0.0]),
            raw(3, "C", "github", 0.6, vec![0.0, 1.0]),
        ];

        let edge_list = vec![edge(1, 2, 0.9)];

        let clusters = clustering::compute_clusters(&items, &edge_list);
        assert_eq!(clusters.len(), 1, "connected items should form one cluster");
        assert_eq!(clusters[0].node_ids.len(), 2);
        assert_eq!(clusters[0].source_count, 2);
    }

    #[test]
    fn test_cluster_labels_prefer_distinctive_terms() {
        // "release" appears in both clusters; the distinctive terms must win.
        let items = vec![
            raw(
                1,
                "React server components release",
                "hn",
                0.8,
                vec![1.0, 0.0],
            ),
            raw(
                2,
                "React server actions release",
                "reddit",
                0.7,
                vec![1.0, 0.0],
            ),
            raw(3, "Tokio scheduler release", "hn", 0.8, vec![0.0, 1.0]),
            raw(
                4,
                "Tokio runtime internals release",
                "reddit",
                0.7,
                vec![0.0, 1.0],
            ),
        ];
        let edge_list = vec![edge(1, 2, 0.9), edge(3, 4, 0.9)];

        let mut clusters = clustering::compute_clusters(&items, &edge_list);
        clustering::assign_cluster_labels(&items, &mut clusters);

        for cluster in &clusters {
            let first = cluster.label.split(" · ").next().unwrap_or("");
            assert_ne!(
                first, "release",
                "shared term must not lead a label; got '{}'",
                cluster.label
            );
        }
    }

    #[test]
    fn test_sparsify_keeps_backbone_connectivity() {
        // A 10-node clique: sparsify must keep every node reachable and cut
        // well below the full 45 edges.
        let mut edge_list = Vec::new();
        for i in 1..=10i64 {
            for j in (i + 1)..=10 {
                edge_list.push(edge(i, j, 0.8 + (i as f32) * 0.001));
            }
        }
        edges::sparsify_edges(&mut edge_list, 3);

        assert!(
            edge_list.len() < 45,
            "clique should shrink, kept {}",
            edge_list.len()
        );
        // Connectivity: union everything and confirm one component.
        let mut parent: std::collections::HashMap<i64, i64> = (1..=10).map(|i| (i, i)).collect();
        fn find(parent: &std::collections::HashMap<i64, i64>, mut x: i64) -> i64 {
            while parent[&x] != x {
                x = parent[&x];
            }
            x
        }
        for e in &edge_list {
            let (a, b) = (find(&parent, e.source), find(&parent, e.target));
            if a != b {
                parent.insert(a.max(b), a.min(b));
            }
        }
        let roots: std::collections::HashSet<i64> = (1..=10).map(|i| find(&parent, i)).collect();
        assert_eq!(roots.len(), 1, "sparsified graph must stay connected");
    }

    #[test]
    fn test_sparsify_leaves_sparse_graphs_alone() {
        let mut edge_list = vec![edge(1, 2, 0.9), edge(2, 3, 0.8)];
        edges::sparsify_edges(&mut edge_list, 4);
        assert_eq!(edge_list.len(), 2);
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
    fn test_extract_title_keywords_drops_numeric_ids() {
        // Live label leak: a mastodon status URL glued to text tokenized as
        // "116885294589687234here".
        let keywords = clustering::extract_title_keywords(
            "RE: fosstodon 116885294589687234Here TypeScript 7.0 released v5-9",
        );
        assert!(!keywords.iter().any(|k| k.contains("116885294589687234")));
        assert!(keywords.contains(&"typescript".to_string()));
        assert!(
            keywords.contains(&"v5-9".to_string()),
            "short numerics stay"
        );
    }

    #[test]
    fn test_edge_count_per_node() {
        let edge_list = vec![edge(1, 2, 0.9), edge(1, 3, 0.8)];

        let counts = edges::count_edges_per_node(&edge_list);
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&2], 1);
        assert_eq!(counts[&3], 1);
    }
}
