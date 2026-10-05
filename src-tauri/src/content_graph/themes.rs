// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Themes for the map: the Louvain communities, partitioned against the
//! user's stack and ordered so related themes sit side by side.
//!
//! The map shows three disjoint groups — "your stack", themes, unthemed — so
//! every item appears exactly once:
//! - A stack item (`affects_you`) leaves its theme. It is the most actionable
//!   thing on the map and gets its own column; live 2026-10-05, 4 of 16 stack
//!   items had sat in the "not connected" lane and 10 were spread over three
//!   release-only themes. The theme remembers them in `stack_node_ids`, so
//!   the link between a theme and the user's packages survives the move.
//! - A theme left with fewer than 2 members is no theme; its last member is
//!   unthemed.
//!
//! Themes are NOT merged by similarity. Measured on the live corpus
//! 2026-10-05 (190 theme pairs, average-linkage cosine): median 0.568, top
//! 0.672 — and the top band mixed true pairs ("security · agents" ~ "openai ·
//! agents", 0.672) with false ones ("production" ~ "how an agent verifies",
//! also 0.672). The 2026-07-19 audit refuted centroid merging the same way.
//! Similarity is used only for ORDER, where a wrong neighbour costs little:
//! the frontend's order-preserving treemap turns list adjacency into screen
//! adjacency.

use std::collections::{HashMap, HashSet};

use super::edges::{dot, unit_vectors};
use super::types::{GraphCluster, GraphNode, RawItem};

/// Partition `clusters` against the stack and order them by similarity.
/// Updates every node's `cluster_id` to match (stack + orphans → `None`).
pub(super) fn finalize_themes(
    clusters: Vec<GraphCluster>,
    nodes: &mut [GraphNode],
    items: &[RawItem],
) -> Vec<GraphCluster> {
    let stack: HashSet<i64> = nodes
        .iter()
        .filter(|n| n.affects_you)
        .map(|n| n.id)
        .collect();
    let themes: Vec<GraphCluster> = clusters
        .into_iter()
        .map(|mut c| {
            c.stack_node_ids = c
                .node_ids
                .iter()
                .copied()
                .filter(|id| stack.contains(id))
                .collect();
            c.node_ids.retain(|id| !stack.contains(id));
            c
        })
        .filter(|c| c.node_ids.len() >= 2)
        .collect();

    let theme_of: HashMap<i64, &str> = themes
        .iter()
        .flat_map(|c| c.node_ids.iter().map(move |&id| (id, c.id.as_str())))
        .collect();
    for node in nodes.iter_mut() {
        node.cluster_id = theme_of.get(&node.id).map(|id| (*id).to_string());
    }

    order_by_similarity(themes, items)
}

/// Mean pairwise cosine between two member sets (average linkage).
fn average_linkage(a: &[usize], b: &[usize], unit: &[Vec<f32>]) -> f32 {
    let mut sum = 0.0f32;
    let mut pairs = 0usize;
    for &i in a {
        for &j in b {
            sum += dot(&unit[i], &unit[j]);
            pairs += 1;
        }
    }
    if pairs == 0 {
        0.0
    } else {
        sum / pairs as f32
    }
}

/// Greedy chain seriation: start from the largest theme, then repeatedly
/// attach the remaining theme most similar to either END of the chain, at
/// that end. Neighbours in the result are each other's best available match.
/// Deterministic: ties go to the larger theme, then the smaller id.
fn order_by_similarity(themes: Vec<GraphCluster>, items: &[RawItem]) -> Vec<GraphCluster> {
    let k = themes.len();
    if k < 3 {
        return themes;
    }
    let idx_of: HashMap<i64, usize> = items.iter().enumerate().map(|(i, it)| (it.id, i)).collect();
    let unit = unit_vectors(items);
    let members: Vec<Vec<usize>> = themes
        .iter()
        .map(|c| {
            c.node_ids
                .iter()
                .filter_map(|id| idx_of.get(id).copied())
                .collect()
        })
        .collect();
    let mut sim = vec![vec![0.0f32; k]; k];
    for a in 0..k {
        for b in (a + 1)..k {
            let s = average_linkage(&members[a], &members[b], &unit);
            sim[a][b] = s;
            sim[b][a] = s;
        }
    }

    // Larger first, then id: the tie-break order for every choice below.
    let rank = |a: usize, b: usize| {
        themes[b]
            .node_ids
            .len()
            .cmp(&themes[a].node_ids.len())
            .then_with(|| themes[a].id.cmp(&themes[b].id))
    };
    let start = (0..k).min_by(|&a, &b| rank(a, b)).unwrap_or(0);
    let mut chain: std::collections::VecDeque<usize> = std::collections::VecDeque::from([start]);
    let mut left: Vec<usize> = (0..k).filter(|&i| i != start).collect();
    while !left.is_empty() {
        let head = chain[0];
        let tail = chain[chain.len() - 1];
        let mut best: Option<(usize, f32, bool)> = None; // (pos in left, score, at_head)
        for (pos, &t) in left.iter().enumerate() {
            let (score, at_head) = if sim[t][head] > sim[t][tail] {
                (sim[t][head], true)
            } else {
                (sim[t][tail], false)
            };
            let better = match best {
                None => true,
                Some((bp, bs, _)) => {
                    score > bs || (score == bs && rank(t, left[bp]) == std::cmp::Ordering::Less)
                }
            };
            if better {
                best = Some((pos, score, at_head));
            }
        }
        let Some((pos, _, at_head)) = best else { break };
        let t = left.remove(pos);
        if at_head {
            chain.push_front(t);
        } else {
            chain.push_back(t);
        }
    }

    let mut slots: Vec<Option<GraphCluster>> = themes.into_iter().map(Some).collect();
    chain.into_iter().filter_map(|i| slots[i].take()).collect()
}
