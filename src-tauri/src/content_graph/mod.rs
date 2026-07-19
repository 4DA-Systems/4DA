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

mod anchors;
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
/// Raw items loaded per node of budget. The node budget must apply AFTER
/// story collapse: with `LIMIT max_nodes` on raw rows, one advisory storm
/// (26 axios advisories, 2026-07-16) eats a sixth of the load and then folds
/// into a single node — the map silently shrinks while presenting itself as
/// the full picture. Loading a multiple and capping post-collapse keeps the
/// map full under redundancy bursts. O(n²) passes at 3×150 = 450 raw items
/// remain sub-100ms.
const RAW_LOAD_FACTOR: usize = 3;
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
    let loading::LoadedWindow {
        items,
        window_candidates,
        windows_differ,
    } = loading::load_scored_items(conn, days, max_nodes * RAW_LOAD_FACTOR)?;
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
                window_candidates,
                time_window_days: days,
                edge_threshold: format!("mutual top-{KNN_K} nearest neighbors"),
                mean_cluster_coherence: None,
                curated_items: 0,
                windows_differ,
            },
        });
    }

    // Collapse near-duplicates into stories FIRST: redundancy becomes one
    // node instead of a rendered clique (86% of live edges before this).
    // The node budget then applies to STORIES (see RAW_LOAD_FACTOR) —
    // curated and quota-reserved stories are exempt from truncation, the
    // rest rank by relevance (matching the load order).
    let mut stories = story::collapse_stories(items);
    stories.sort_by(|a, b| {
        (b.item.curated || b.item.reserved)
            .cmp(&(a.item.curated || a.item.reserved))
            .then(
                b.item
                    .relevance_score
                    .partial_cmp(&a.item.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.item.id.cmp(&b.item.id))
    });
    let truncated_items: usize = stories.iter().skip(max_nodes).map(|s| s.member_count).sum();
    stories.truncate(max_nodes);

    let mut rep_of: HashMap<i64, i64> = HashMap::new();
    for s in &stories {
        for &member in &s.member_ids {
            rep_of.insert(member, s.item.id);
        }
    }
    let story_items: Vec<types::RawItem> =
        stories.iter().map(|s| story::clone_raw(&s.item)).collect();

    // Semantic edges first; communities form from THESE ONLY. Chain edges are
    // keyword-topic paths (rendered context) — feeding them into community
    // detection welded semantically unrelated items into fake themes (live
    // forensics 2026-07-19: 2 of 30 clusters existed solely on chain edges,
    // 5 more part-welded, one politics item chained into an "api" cluster).
    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&story_items, &mut edge_list);
    let clusters = clustering::compute_clusters(&story_items, &edge_list);

    edges::compute_chain_edges(conn, &rep_of, &mut edge_list);
    edges::merge_duplicate_edges(&mut edge_list);

    // Visibility: anything connected or aggregated appears; isolated plain
    // items appear as semantic satellites (or shelf) up to SINGLETON_CAP by
    // relevance. Curated singletons are EXEMPT from the cap (they carry a
    // persisted feed-curation verdict — the corpus the map claims to show),
    // as are quota-reserved category items (P2.12) — both would otherwise
    // lose their slot to higher-scored not-yet-judged items.
    struct Vis {
        id: i64,
        relevance: f32,
        connected: bool,
        is_story: bool,
        exempt: bool,
    }
    let degree = edges::count_edges_per_node(&edge_list);
    let mut handles: Vec<Vis> = stories
        .iter()
        .map(|s| Vis {
            id: s.item.id,
            relevance: s.item.relevance_score,
            connected: degree.get(&s.item.id).copied().unwrap_or(0) > 0,
            is_story: s.member_count > 1,
            exempt: s.item.curated || s.item.reserved,
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
        if handle.connected || handle.is_story || handle.exempt {
            visible_ids.insert(handle.id);
        } else {
            ring_candidates.push(handle);
        }
    }
    let hidden_singletons = ring_candidates.len().saturating_sub(SINGLETON_CAP);
    for handle in ring_candidates.iter().take(SINGLETON_CAP) {
        visible_ids.insert(handle.id);
    }
    let hidden_items = hidden_singletons + truncated_items;

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

    // Temporal layout anchors (P2.11): clusters overlapping a persisted
    // anchor's member set seed at the remembered position, so day-over-day
    // maps stay spatially recognizable. Read-only here — persisting is the
    // Tauri command's side effect, keeping build_graph a pure function of
    // (corpus, anchor state) and the determinism contract intact.
    let stored_anchors = anchors::load_anchors(conn, days);
    let cluster_members: Vec<(String, HashSet<i64>)> = clusters
        .iter()
        .map(|c| {
            let members: HashSet<i64> = c
                .node_ids
                .iter()
                .filter_map(|id| stories.iter().find(|s| s.item.id == *id))
                .flat_map(|s| s.member_ids.iter().copied())
                .collect();
            (c.id.clone(), members)
        })
        .collect();
    let anchor_seeds = anchors::match_anchors(&cluster_members, &stored_anchors);

    // Layout sees the full retained edge set (affinity fidelity); display
    // gets the sparsified backbone + top-K.
    layout::compute_layout(
        &mut nodes,
        &edge_list,
        &mut clusters,
        &satellite_of,
        &anchor_seeds,
    );
    edges::sparsify_edges(&mut edge_list, TOP_K_EDGES);

    let story_count = nodes.iter().filter(|n| n.member_count > 1).count();
    let collapsed_items: usize = nodes.iter().map(|n| n.member_count.saturating_sub(1)).sum();
    // Item-level count (P2.14): a story contributes its curated MEMBER count,
    // so near-duplicate collapse can't launder unjudged items into the ramp.
    let curated_items: usize = stories
        .iter()
        .filter(|s| visible_ids.contains(&s.item.id))
        .map(|s| s.curated_count)
        .sum();

    let meta = GraphMeta {
        total_items: nodes.len(),
        total_edges: edge_list.len(),
        cluster_count: clusters.len(),
        story_count,
        collapsed_items,
        hidden_items,
        window_candidates,
        time_window_days: days,
        edge_threshold: format!("mutual top-{KNN_K} nearest neighbors"),
        mean_cluster_coherence,
        curated_items,
        windows_differ,
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
    let graph = build_graph(&conn, d, m)?;
    // Persist this build's cluster geometry as the next build's layout
    // anchors (P2.11). Deliberately OUTSIDE build_graph: builds stay pure.
    anchors::persist_layout_anchors(&conn, d, &graph);
    Ok(graph)
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
#[path = "graph_tests.rs"]
mod tests;
