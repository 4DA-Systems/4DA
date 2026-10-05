// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content Graph — relationship visualization for surfaced intelligence.
//!
//! Pipeline: load scored items → collapse near-duplicates into STORIES
//! (story.rs — one node per advisory storm / mirrored announcement / release
//! published to two registries) → mutual-kNN semantic edges between stories
//! → Louvain communities → themes (themes.rs: stack items leave their theme,
//! themes ordered so related ones are adjacent) with c-TF-IDF labels.
//!
//! The frontend renders the result as a theme map — a stack column, a
//! treemap of themes and an unthemed list — so nothing here computes screen
//! positions. Everything is deterministic.

mod category;
mod clustering;
mod detail;
mod edges;
mod labels;
mod loading;
mod story;
mod themes;
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
/// Isolated singletons shown in the map's "not connected to any theme" lane,
/// by relevance; the rest stay in the List view (counted honestly in
/// `meta.hidden_items`).
const SINGLETON_CAP: usize = 40;
/// Per-source composition cap on NOT-yet-judged stories: no single
/// source_type may fill more than this fraction of the node budget with
/// unjudged items (live 2026-07-21: crates.io held 59 of 150 nodes — a
/// registry firehose wallpapering the map). Curated and quota-reserved
/// stories are EXEMPT — corpus parity means the map never overrules the
/// persisted feed verdict; the cap governs only the interim fill, and it
/// relaxes (backfills from the capped source) when nothing else exists so a
/// genuinely single-source corpus still fills the map honestly.
const SOURCE_CAP_FRACTION: f32 = 0.25;

// ============================================================================
// Graph Construction
// ============================================================================

/// Wall-clock per build phase (ms), in pipeline order. Logged with every
/// build so a slow map is attributable from the log alone (the live 6.5-10.8 s
/// builds of 2026-10-02 had no breakdown anywhere).
pub(crate) type PhaseTimes = Vec<(&'static str, u128)>;

pub fn build_graph(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<ContentGraph> {
    build_graph_timed(conn, days, max_nodes).map(|(graph, _)| graph)
}

pub(crate) fn build_graph_timed(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<(ContentGraph, PhaseTimes)> {
    let mut phases: PhaseTimes = Vec::new();
    let mut clock = std::time::Instant::now();
    let mut mark = |name: &'static str, phases: &mut PhaseTimes| {
        phases.push((name, clock.elapsed().as_millis()));
        clock = std::time::Instant::now();
    };
    let loading::LoadedWindow {
        items,
        window_candidates,
        windows_differ,
    } = loading::load_scored_items(conn, days, max_nodes * RAW_LOAD_FACTOR)?;
    mark("load", &mut phases);
    if items.is_empty() {
        return Ok((
            ContentGraph {
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
            },
            phases,
        ));
    }

    // Collapse near-duplicates into stories FIRST: redundancy becomes one
    // node instead of a rendered clique (86% of live edges before this).
    // The node budget then applies to STORIES (see RAW_LOAD_FACTOR) —
    // curated and quota-reserved stories are exempt from truncation, the
    // rest rank by relevance (matching the load order).
    let mut stories = story::collapse_stories(items);
    mark("collapse", &mut phases);
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
    // Per-source cap (see SOURCE_CAP_FRACTION): stable partition of the
    // sorted stories into kept / same-source overflow, budget filled from
    // kept first, then backfilled from overflow so the map never runs short
    // while items exist. Every dropped story counts into hidden_items.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let source_cap = ((max_nodes as f32) * SOURCE_CAP_FRACTION).ceil() as usize;
    let mut per_source: HashMap<String, usize> = HashMap::new();
    let mut kept: Vec<types::StoryItem> = Vec::with_capacity(stories.len());
    let mut overflow: Vec<types::StoryItem> = Vec::new();
    for s in stories {
        if s.item.curated || s.item.reserved {
            kept.push(s);
            continue;
        }
        let n = per_source.entry(s.item.source_type.clone()).or_insert(0);
        if *n < source_cap {
            *n += 1;
            kept.push(s);
        } else {
            overflow.push(s);
        }
    }
    if kept.len() < max_nodes {
        let deficit = max_nodes - kept.len();
        kept.extend(overflow.drain(..overflow.len().min(deficit)));
        // Backfill breaks the global relevance order; restore it so the node
        // budget still keeps the best stories (same key as the sort above).
        kept.sort_by(|a, b| {
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
    }
    let mut stories = kept;
    let truncated_items: usize = stories
        .iter()
        .skip(max_nodes)
        .map(|s| s.member_count)
        .sum::<usize>()
        + overflow.iter().map(|s| s.member_count).sum::<usize>();
    stories.truncate(max_nodes);

    let story_items: Vec<types::RawItem> =
        stories.iter().map(|s| story::clone_raw(&s.item)).collect();

    // Communities form from semantic edges only: keyword-topic chain edges
    // welded unrelated items into fake themes (live forensics 2026-07-19),
    // and with no edges drawn on screen they no longer earned their query.
    let mut edge_list = Vec::new();
    edges::compute_semantic_edges(&story_items, &mut edge_list);
    let clusters = clustering::compute_clusters(&story_items, &edge_list);
    mark("edges+louvain", &mut phases);

    // Visibility: anything connected or aggregated appears; isolated plain
    // items appear in the unconnected lane up to SINGLETON_CAP by
    // relevance. Curated singletons are EXEMPT from the cap (they carry a
    // persisted feed-curation verdict — the corpus the map claims to show),
    // as are quota-reserved category items (P2.12) — both would otherwise
    // lose their slot to higher-scored not-yet-judged items — and stack
    // items, which the map always shows in their own column.
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
            exempt: s.item.curated || s.item.reserved || s.affects_you,
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
            }
        })
        .collect();

    edge_list.retain(|e| visible_ids.contains(&e.source) && visible_ids.contains(&e.target));
    let clusters: Vec<GraphCluster> = clusters
        .into_iter()
        .map(|mut c| {
            c.node_ids.retain(|id| visible_ids.contains(id));
            c
        })
        .collect();
    // Stack items leave their theme, a theme keeps 2+ remaining members, and
    // every node's cluster_id is rewritten to match (None = stack/unthemed —
    // the frontend partitions on exactly these fields).
    let mut clusters = themes::finalize_themes(clusters, &mut nodes, &story_items);
    labels::assign_cluster_labels(&story_items, &mut clusters);

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

    mark("themes+labels+coherence", &mut phases);

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
        phases_ms = ?phases,
        "Content graph built"
    );

    Ok((
        ContentGraph {
            nodes,
            edges: edge_list,
            clusters,
            meta,
        },
        phases,
    ))
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Async so the build runs on a blocking-pool thread, never the main thread —
/// a non-async Tauri command executes ON the main thread, and a corpus-scale
/// build (60s+ on a multi-GB DB while an analysis run competes for I/O) froze
/// the entire webview and stalled every other sync command behind it.
#[tauri::command]
pub async fn build_content_graph(
    days: Option<u32>,
    max_nodes: Option<usize>,
) -> Result<ContentGraph> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::open_db_connection()?;
        let d = days.unwrap_or(DEFAULT_DAYS);
        let m = max_nodes.unwrap_or(DEFAULT_MAX_NODES);
        build_graph(&conn, d, m)
    })
    .await
    .map_err(|e| crate::error::FourDaError::Internal(format!("graph build task failed: {e}")))?
}

/// Hydrate a selected node's members for the detail panel (keyed lookup of
/// items already surfaced by `build_content_graph` — not a ranked feed).
#[tauri::command]
pub async fn get_graph_node_details(item_ids: Vec<i64>) -> Result<Vec<GraphNodeDetail>> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::open_db_connection()?;
        detail::fetch_node_details(&conn, &item_ids)
    })
    .await
    .map_err(|e| crate::error::FourDaError::Internal(format!("node detail task failed: {e}")))?
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
