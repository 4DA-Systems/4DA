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

mod category;
mod clustering;
mod detail;
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

pub use detail::GraphNodeDetail;

const DEFAULT_DAYS: u32 = 7;
const DEFAULT_MAX_NODES: usize = 150;
/// Mutual k-nearest-neighbor edge construction: each endpoint must rank the
/// other in its own top-K by cosine. Rank-based, so it self-calibrates to
/// every corpus — validated across 7/14/30-day windows 2026-07-19 (k=3 beat
/// k=4/5 on theme purity; results insensitive to the floor within 0.50–0.60).
const KNN_K: usize = 3;
/// Absolute floor under which even a mutual nearest neighbor is noise.
const KNN_FLOOR: f32 = 0.55;
/// Isolated singletons shown on the map (as semantic satellites or on the
/// shelf), by relevance; the rest stay in the List view (counted honestly in
/// `meta.hidden_items`).
const SINGLETON_CAP: usize = 40;
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
                edge_threshold: format!("mutual top-{KNN_K} nearest neighbors"),
                mean_cluster_coherence: None,
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
    // items appear as semantic satellites (or shelf) up to SINGLETON_CAP by
    // relevance.
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
    let hidden_items = ring_candidates.len().saturating_sub(SINGLETON_CAP);
    for handle in ring_candidates.iter().take(SINGLETON_CAP) {
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
                member_ids: s.member_ids.clone(),
                category: category::category_for(
                    &s.item.source_type,
                    s.item.signal_type.as_deref(),
                )
                .to_string(),
                affects_you: s.affects_you,
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

    // Coherence: mean pairwise member cosine per cluster, and the pair-count
    // weighted mean across clusters — the graph measures its own theme
    // tightness on every corpus instead of asserting it.
    let embedding_of: HashMap<i64, &[f32]> = story_items
        .iter()
        .map(|i| (i.id, i.embedding.as_slice()))
        .collect();
    let mut coherence_sum = 0.0f64;
    let mut coherence_pairs = 0usize;
    for cluster in &mut clusters {
        let mut sum = 0.0f64;
        let mut pairs = 0usize;
        for (ai, a) in cluster.node_ids.iter().enumerate() {
            for b in &cluster.node_ids[ai + 1..] {
                if let (Some(ea), Some(eb)) = (embedding_of.get(a), embedding_of.get(b)) {
                    sum += f64::from(crate::utils::cosine_similarity(ea, eb));
                    pairs += 1;
                }
            }
        }
        cluster.coherence = if pairs > 0 {
            (sum / pairs as f64) as f32
        } else {
            0.0
        };
        coherence_sum += sum;
        coherence_pairs += pairs;
    }
    let mean_cluster_coherence = if coherence_pairs > 0 {
        Some((coherence_sum / coherence_pairs as f64) as f32)
    } else {
        None
    };

    // Semantic satellite assignment: each visible singleton attaches to its
    // nearest clustered story (max member cosine). Below the floor it goes
    // to the shelf — genuinely unrelated. Live evidence for this design:
    // 90 of 93 window singletons sat at cosine 0.45-0.77 from a cluster.
    let clustered_visible: Vec<&types::RawItem> = story_items
        .iter()
        .filter(|item| {
            visible_ids.contains(&item.id) && clusters.iter().any(|c| c.node_ids.contains(&item.id))
        })
        .collect();
    let mut satellite_of: HashMap<i64, layout::SatelliteAssign> = HashMap::new();
    for node in &nodes {
        if node.cluster_id.is_some() {
            continue;
        }
        let Some(item) = story_items.iter().find(|i| i.id == node.id) else {
            continue;
        };
        let best = clustered_visible
            .iter()
            .map(|c| {
                (
                    c.id,
                    crate::utils::cosine_similarity(&item.embedding, &c.embedding),
                )
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((nearest_id, sim)) = best {
            if sim >= layout::SATELLITE_MIN_SIM {
                if let Some(cluster) = clusters.iter().find(|c| c.node_ids.contains(&nearest_id)) {
                    satellite_of.insert(
                        node.id,
                        layout::SatelliteAssign {
                            cluster_id: cluster.id.clone(),
                            sim,
                        },
                    );
                }
            }
        }
    }

    // Layout sees the full retained edge set (affinity fidelity); display
    // gets the sparsified backbone + top-K.
    layout::compute_layout(&mut nodes, &edge_list, &mut clusters, &satellite_of);
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
        edge_threshold: format!("mutual top-{KNN_K} nearest neighbors"),
        mean_cluster_coherence,
    };

    info!(
        target: "4da::content_graph",
        nodes = nodes.len(),
        edges = edge_list.len(),
        clusters = clusters.len(),
        stories = story_count,
        collapsed = collapsed_items,
        hidden = hidden_items,
        coherence = mean_cluster_coherence.unwrap_or(0.0),
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

/// Hydrate a selected node's members for the detail panel (keyed lookup of
/// items already surfaced by `build_content_graph` — not a ranked feed).
#[tauri::command]
pub fn get_graph_node_details(item_ids: Vec<i64>) -> Result<Vec<GraphNodeDetail>> {
    let conn = crate::open_db_connection()?;
    detail::fetch_node_details(&conn, &item_ids)
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
                edge_threshold: "mutual top-3 nearest neighbors".to_string(),
                mean_cluster_coherence: None,
            },
        };
        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_mutual_neighbors_create_edge() {
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
            "identical embeddings are mutual nearest neighbors"
        );
        assert_eq!(edge_list[0].edge_type, EdgeType::Semantic);
        assert!((edge_list[0].weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_knn_floor_blocks_nonsense_mutual_pairs() {
        // Two orthogonal items are each other's ONLY neighbor — mutual by
        // construction — but similarity 0 sits under KNN_FLOOR, so no edge.
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
            "sub-floor mutual neighbors must not connect"
        );
    }

    #[test]
    fn test_knn_mutuality_required() {
        // Hub h is near a, b, c, d (its top-3 covers only 3 of them), while
        // e is far from everything except h. e ranks h first, but h's top-3
        // never includes e -> no h–e edge (this asymmetry-cut is what stops
        // one promiscuous hub from wiring the whole corpus together).
        let items = vec![
            raw(1, "hub", "hn", 0.9, vec![1.0, 0.0, 0.0]),
            raw(2, "a", "hn", 0.9, vec![0.99, 0.14, 0.0]),
            raw(3, "b", "hn", 0.9, vec![0.99, 0.0, 0.14]),
            raw(4, "c", "hn", 0.9, vec![0.99, 0.10, 0.10]),
            raw(5, "e", "hn", 0.9, vec![0.75, -0.66, 0.0]),
        ];

        let mut edge_list = Vec::new();
        edges::compute_semantic_edges(&items, &mut edge_list);

        assert!(
            !edge_list
                .iter()
                .any(|e| (e.source == 1 && e.target == 5) || (e.source == 5 && e.target == 1)),
            "one-sided nearest-neighbor pairs must not connect"
        );
    }

    #[test]
    fn test_knn_edges_deterministic() {
        let build = || {
            let items: Vec<RawItem> = (0..20)
                .map(|i| {
                    let angle = i as f32 * 0.3;
                    raw(
                        i64::from(i),
                        "t",
                        "hn",
                        0.5,
                        vec![angle.cos(), angle.sin(), 0.2],
                    )
                })
                .collect();
            let mut edge_list = Vec::new();
            edges::compute_semantic_edges(&items, &mut edge_list);
            edge_list
                .iter()
                .map(|e| (e.source, e.target))
                .collect::<Vec<_>>()
        };
        assert_eq!(build(), build());
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
        // 0.5 = STORY_OVERLAP_MIN in story.rs, title_word_overlap's only consumer.
        let overlap = edges::title_word_overlap(
            "React 19 server components released",
            "React 19 server components update",
        );
        assert!(
            overlap > 0.5,
            "similar titles should overlap >0.5, got {overlap}"
        );
    }

    #[test]
    fn test_title_word_overlap_low() {
        let overlap = edges::title_word_overlap("Rust async runtime", "Python web framework");
        assert!(overlap < 0.5);
    }

    #[test]
    fn test_louvain_cuts_hub_chains() {
        // Two dense themes bridged by one promiscuous hub. Connected
        // components fused all 9 nodes into one cluster (the live 23-node
        // crates blob); modularity must keep the themes separate.
        let items: Vec<RawItem> = (1..=9)
            .map(|i| raw(i, "t", "src", 0.5, vec![1.0, 0.0]))
            .collect();
        let mut edge_list = Vec::new();
        // Theme A: 1-2-3-4 clique.
        for i in 1..=4i64 {
            for j in (i + 1)..=4 {
                edge_list.push(edge(i, j, 0.9));
            }
        }
        // Theme B: 5-6-7-8 clique.
        for i in 5..=8i64 {
            for j in (i + 1)..=8 {
                edge_list.push(edge(i, j, 0.9));
            }
        }
        // Hub 9 weakly touches both themes.
        edge_list.push(edge(9, 1, 0.6));
        edge_list.push(edge(9, 5, 0.6));

        let clusters = clustering::compute_clusters(&items, &edge_list);

        let of = |id: i64| {
            clusters
                .iter()
                .position(|c| c.node_ids.contains(&id))
                .expect("clustered")
        };
        assert_eq!(of(1), of(4), "theme A stays together");
        assert_eq!(of(5), of(8), "theme B stays together");
        assert_ne!(of(1), of(5), "hub must not weld the two themes");
    }

    #[test]
    fn test_louvain_deterministic() {
        let build = || {
            let items: Vec<RawItem> = (1..=12)
                .map(|i| raw(i, "t", "src", 0.5, vec![1.0, 0.0]))
                .collect();
            let mut edge_list = Vec::new();
            for i in 1..=6i64 {
                for j in (i + 1)..=6 {
                    edge_list.push(edge(i, j, 0.8));
                }
            }
            for i in 7..=12i64 {
                for j in (i + 1)..=12 {
                    edge_list.push(edge(i, j, 0.8));
                }
            }
            edge_list.push(edge(3, 9, 0.55));
            clustering::compute_clusters(&items, &edge_list)
                .into_iter()
                .map(|c| c.node_ids)
                .collect::<Vec<_>>()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn test_label_falls_back_to_source_digest_when_no_shared_topic() {
        // Six crates.io releases with NOTHING in common except the source
        // template — the live case whose label degenerated to
        // "crates · agentmail-rs · alpacars". "crates" is source boilerplate
        // (in 100% of titles) and no other term covers 30%, so the honest
        // digest label must win.
        let titles = [
            "crates.io: agentmail-rs v0.2.0",
            "crates.io: alpacars v0.3.0",
            "crates.io: krb5-gss v0.1.0",
            "crates.io: rust-ynab v0.5.6",
            "crates.io: oganesson-rs v0.2.1",
            "crates.io: nidus-sqs v1.0.12",
        ];
        let items: Vec<RawItem> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| raw(i as i64 + 1, t, "crates_io", 0.5, vec![1.0, 0.0]))
            .collect();
        let mut edge_list = Vec::new();
        for i in 1..=6i64 {
            for j in (i + 1)..=6 {
                edge_list.push(edge(i, j, 0.8));
            }
        }

        let mut clusters = clustering::compute_clusters(&items, &edge_list);
        assert_eq!(clusters.len(), 1);
        clustering::assign_cluster_labels(&items, &mut clusters);

        assert_eq!(
            clusters[0].label, "crates.io · assorted",
            "template-only cohesion must get an honest digest label"
        );
    }

    #[test]
    fn test_label_uses_real_topic_despite_source_boilerplate() {
        // Boilerplate-heavy source ("crates" in every title) plus a REAL
        // shared theme inside compound names — the live tauri sub-theme. The
        // item universe includes unrelated crates so "tauri" is a topic
        // (5 of 11 items), not source boilerplate. Sub-token extraction must
        // surface it and the label must not be the digest fallback.
        let titles = [
            "crates.io: tauri-browser v0.5.0",
            "crates.io: tauri-plugin-syncular v0.3.1",
            "crates.io: tauri-plugin-hdiff-update v0.3.0",
            "crates.io: tauri-plugin-thermal-printer v2.1.0",
            "crates.io: tauri-plugin-serialplugin v3.0.1",
            "crates.io: agentmail-rs v0.2.0",
            "crates.io: alpacars v0.3.0",
            "crates.io: krb5-gss v0.1.0",
            "crates.io: rust-ynab v0.5.6",
            "crates.io: oganesson-rs v0.2.1",
            "crates.io: nidus-sqs v1.0.12",
        ];
        let items: Vec<RawItem> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| raw(i as i64 + 1, t, "crates_io", 0.5, vec![1.0, 0.0]))
            .collect();
        // Only the five tauri crates form a cluster.
        let mut edge_list = Vec::new();
        for i in 1..=5i64 {
            for j in (i + 1)..=5 {
                edge_list.push(edge(i, j, 0.85));
            }
        }

        let mut clusters = clustering::compute_clusters(&items, &edge_list);
        assert_eq!(clusters.len(), 1);
        clustering::assign_cluster_labels(&items, &mut clusters);

        assert!(
            clusters[0].label.split(" · ").any(|w| w == "tauri"),
            "the shared 'tauri' theme must be labelable; got '{}'",
            clusters[0].label
        );
        assert!(
            !clusters[0].label.split(" · ").any(|w| w == "crates"),
            "source boilerplate must not enter labels; got '{}'",
            clusters[0].label
        );
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
    fn test_extract_title_keywords_drops_numeric_noise() {
        // Two live label leaks: a mastodon status URL glued to text tokenized
        // as "116885294589687234Here" (2026-07-16), and short version/count
        // tokens crowning labels ("152", "8th", "191k", "160-post",
        // 2026-07-19). Digit-dominated tokens never label; real names with
        // incidental digits survive.
        let keywords = clustering::extract_title_keywords(
            "RE: fosstodon 116885294589687234Here TypeScript 7.0 released v5-9 8th 191k sqlite3",
        );
        assert!(!keywords.iter().any(|k| k.contains("116885294589687234")));
        assert!(keywords.contains(&"typescript".to_string()));
        assert!(
            keywords.contains(&"sqlite3".to_string()),
            "incidental digit stays"
        );
        for noise in ["v5-9", "8th", "191k"] {
            assert!(
                !keywords.contains(&noise.to_string()),
                "digit-dominated token '{noise}' must not label"
            );
        }
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
