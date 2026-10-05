// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Edge computation: mutual-kNN semantic similarity — the relation themes
//! are built from — plus the vector helpers the similarity passes share.

use std::collections::{HashMap, HashSet};

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
    let unit = unit_vectors(items);
    let mut top: Vec<Vec<(f32, usize)>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut sims: Vec<(f32, usize)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (dot(&unit[i], &unit[j]), j))
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
            let (a, b) = (items[i].id, items[j].id);
            edges.push(GraphEdge {
                source: a.min(b),
                target: a.max(b),
                edge_type: EdgeType::Semantic,
                weight: sim.clamp(0.0, 1.0),
                label: Some(format!("similarity: {:.2}", sim)),
                methods: vec!["semantic".to_string()],
            });
        }
    }
    // Canonical order: edge order feeds f32 accumulations in Louvain, where
    // addition is non-associative — a fixed order keeps builds deterministic
    // (two same-corpus builds once diverged on 104/139 nodes, 2026-07-19).
    edges.sort_by_key(|e| (e.source, e.target));
}

/// Unit-normalized copies of every item embedding, so the O(n²) similarity
/// passes (story collapse over ~450 raw items, kNN over the node budget) cost
/// one dot product per pair instead of a dot product plus two norms. Measured
/// on the live corpus 2026-10-02 (debug build, the one the dev app runs):
/// story collapse was 2.4-2.5 s of a 4.0-4.3 s build. A zero vector stays
/// zero, so its similarity to anything is 0 — the same as
/// `cosine_similarity`'s zero-norm guard.
pub(super) fn unit_vectors(items: &[RawItem]) -> Vec<Vec<f32>> {
    items
        .iter()
        .map(|item| {
            let norm = item.embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                item.embedding.iter().map(|v| v / norm).collect()
            } else {
                vec![0.0; item.embedding.len()]
            }
        })
        .collect()
}

/// Dot product of two unit vectors = their cosine. Mismatched lengths are
/// unrelated (0.0), as in `cosine_similarity`.
pub(super) fn dot(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
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
