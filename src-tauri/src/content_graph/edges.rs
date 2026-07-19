// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Edge computation: semantic similarity, signal chain, and merge logic.

use std::collections::{HashMap, HashSet};

use tracing::debug;

use crate::signal_chains::detect_chains_for_items;
use crate::utils::cosine_similarity;

use super::types::{EdgeType, GraphEdge, RawItem};
use super::{KNN_FLOOR, KNN_K};

/// Mutual k-nearest-neighbor semantic edges.
///
/// Replaces the old global `cosine >= 0.77` gate. Live audit 2026-07-19 across
/// 7/14/30-day windows showed an absolute threshold cannot work: same-source
/// similarity baselines differ by source (crates.io templated titles median
/// 0.733 vs mastodon 0.570), and cross-source pairs about the SAME topic
/// almost never reach 0.77 (1 of 2,536 pairs) — so real cross-source themes
/// were structurally invisible while template-similar registry items over-
/// connected. Rank-based mutuality self-calibrates to each neighborhood's
/// density: an edge exists only when each endpoint ranks the other in its own
/// top-[`KNN_K`], with an absolute floor to keep nonsense pairs out of sparse
/// corners. No per-corpus tuning — this is what makes every user's graph
/// self-optimizing.
pub(super) fn compute_semantic_edges(items: &[RawItem], edges: &mut Vec<GraphEdge>) {
    let n = items.len();
    if n < 2 {
        return;
    }

    // Top-K neighbor lists, deterministic (sim desc, then neighbor id asc).
    let mut top: Vec<Vec<(f32, usize)>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut sims: Vec<(f32, usize)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                (
                    cosine_similarity(&items[i].embedding, &items[j].embedding),
                    j,
                )
            })
            .collect();
        sims.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| items[a.1].id.cmp(&items[b.1].id))
        });
        sims.truncate(KNN_K);
        top.push(sims);
    }

    for i in 0..n {
        for &(sim, j) in &top[i] {
            if i >= j || sim < KNN_FLOOR {
                continue;
            }
            if !top[j].iter().any(|&(_, jj)| jj == i) {
                continue;
            }
            edges.push(GraphEdge {
                source: items[i].id,
                target: items[j].id,
                edge_type: EdgeType::Semantic,
                weight: sim.clamp(0.0, 1.0),
                label: Some(format!("similarity: {:.2}", sim)),
                methods: vec!["semantic".to_string()],
            });
        }
    }
}

/// Chain links reference ORIGINAL item ids; with story aggregation each id
/// maps to its story representative first (`rep_of`), and links that land
/// inside one story collapse away instead of becoming self-loops.
///
/// Chains are detected over the GRAPH'S OWN item set (`rep_of` keys — every
/// loaded member id), not the global recency window: the graph loads by
/// relevance while `detect_chains` reads by recency, and the two sets shared
/// 0 of 150 items live (2026-07-19) — chain edges could never fire.
pub(super) fn compute_chain_edges(
    conn: &rusqlite::Connection,
    rep_of: &HashMap<i64, i64>,
    edges: &mut Vec<GraphEdge>,
) {
    let mut member_ids: Vec<i64> = rep_of.keys().copied().collect();
    member_ids.sort_unstable();
    let chains = match detect_chains_for_items(conn, &member_ids) {
        Ok(c) => c,
        Err(e) => {
            debug!(target: "4da::content_graph", error = %e, "Signal chain detection failed, skipping chain edges");
            return;
        }
    };

    for chain in &chains {
        let mut chain_reps: Vec<i64> = Vec::new();
        for link in &chain.links {
            if let Some(&rep) = rep_of.get(&link.source_item_id) {
                if chain_reps.last() != Some(&rep) {
                    chain_reps.push(rep);
                }
            }
        }

        for window in chain_reps.windows(2) {
            if window[0] == window[1] {
                continue;
            }
            edges.push(GraphEdge {
                source: window[0],
                target: window[1],
                edge_type: EdgeType::Chain,
                weight: (chain.confidence as f32).clamp(0.0, 1.0),
                label: Some(chain.chain_name.clone()),
                methods: vec!["signal_chain".to_string()],
            });
        }
    }
}

pub(super) fn merge_duplicate_edges(edges: &mut Vec<GraphEdge>) {
    let mut merged: HashMap<(i64, i64), GraphEdge> = HashMap::new();

    for edge in edges.drain(..) {
        let key = if edge.source <= edge.target {
            (edge.source, edge.target)
        } else {
            (edge.target, edge.source)
        };

        merged
            .entry(key)
            .and_modify(|existing| {
                if edge.weight > existing.weight {
                    existing.weight = edge.weight;
                    existing.label = edge.label.clone();
                }
                for method in &edge.methods {
                    if !existing.methods.contains(method) {
                        existing.methods.push(method.clone());
                    }
                }
                if existing.edge_type != edge.edge_type {
                    existing.edge_type = EdgeType::Convergence;
                }
            })
            .or_insert(GraphEdge {
                source: key.0,
                target: key.1,
                ..edge
            });
    }

    *edges = merged.into_values().collect();
    // HashMap iteration order varies per instance (random hash seed), and the
    // edge ORDER feeds f32 accumulations downstream (Louvain link sums, disc
    // affinity forces) where float addition is non-associative — live measure
    // 2026-07-19: two same-corpus builds diverged on 104/139 node positions.
    // A canonical order makes the whole pipeline deterministic again.
    edges.sort_by_key(|e| (e.source, e.target));
}

/// Keep the readable backbone of a dense edge set: a maximum-spanning forest
/// (connectivity is never lost) plus each node's top-`k` edges by weight.
/// Everything else is display noise — cluster membership is computed from the
/// FULL edge list before this runs, so sparsification only affects rendering.
pub(super) fn sparsify_edges(edges: &mut Vec<GraphEdge>, k: usize) {
    if edges.len() <= k {
        return;
    }

    // Deterministic order: weight desc, then endpoint ids.
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by(|&a, &b| {
        edges[b]
            .weight
            .partial_cmp(&edges[a].weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                (edges[a].source, edges[a].target).cmp(&(edges[b].source, edges[b].target))
            })
    });

    // Union-find over node ids for the spanning forest.
    let mut parent: HashMap<i64, i64> = HashMap::new();
    fn find(parent: &mut HashMap<i64, i64>, mut x: i64) -> i64 {
        loop {
            let p = *parent.entry(x).or_insert(x);
            if p == x {
                return x;
            }
            let gp = *parent.entry(p).or_insert(p);
            parent.insert(x, gp);
            x = gp;
        }
    }

    let mut kept_count: HashMap<i64, usize> = HashMap::new();
    let mut keep = vec![false; edges.len()];
    for &i in &order {
        let (s, t) = (edges[i].source, edges[i].target);
        let (rs, rt) = (find(&mut parent, s), find(&mut parent, t));
        let is_backbone = rs != rt;
        if is_backbone {
            // Deterministic union: larger root id points at the smaller.
            parent.insert(rs.max(rt), rs.min(rt));
        }
        let s_wants = kept_count.get(&s).copied().unwrap_or(0) < k;
        let t_wants = kept_count.get(&t).copied().unwrap_or(0) < k;
        if is_backbone || s_wants || t_wants {
            keep[i] = true;
            *kept_count.entry(s).or_insert(0) += 1;
            *kept_count.entry(t).or_insert(0) += 1;
        }
    }

    let mut idx = 0;
    edges.retain(|_| {
        let kept = keep[idx];
        idx += 1;
        kept
    });
}

pub(super) fn count_edges_per_node(edges: &[GraphEdge]) -> HashMap<i64, usize> {
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for edge in edges {
        *counts.entry(edge.source).or_insert(0) += 1;
        *counts.entry(edge.target).or_insert(0) += 1;
    }
    counts
}

pub(super) fn title_word_overlap(a: &str, b: &str) -> f32 {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "in", "of", "for", "to", "and", "is", "new",
    ];

    let set_a: HashSet<String> = a
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .map(String::from)
        .collect();
    let set_b: HashSet<String> = b
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOPWORDS.contains(w))
        .map(String::from)
        .collect();

    if set_a.is_empty() && set_b.is_empty() {
        return 0.0;
    }

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}
