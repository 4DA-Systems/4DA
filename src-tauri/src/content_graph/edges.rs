// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Edge computation: semantic similarity, signal chain, and merge logic.

use std::collections::{HashMap, HashSet};

use tracing::debug;

use crate::signal_chains::detect_chains;
use crate::utils::cosine_similarity;

use super::types::{EdgeType, GraphEdge, RawItem};
use super::{LEXICAL_FALLBACK_THRESHOLD, LEXICAL_OVERLAP_MIN, SIMILARITY_THRESHOLD};

pub(super) fn compute_semantic_edges(items: &[RawItem], edges: &mut Vec<GraphEdge>) {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            let sim = cosine_similarity(&items[i].embedding, &items[j].embedding);
            let should_connect = sim >= SIMILARITY_THRESHOLD
                || (sim >= LEXICAL_FALLBACK_THRESHOLD
                    && title_word_overlap(&items[i].title, &items[j].title) >= LEXICAL_OVERLAP_MIN);

            if should_connect {
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
}

pub(super) fn compute_chain_edges(
    conn: &rusqlite::Connection,
    id_set: &HashSet<i64>,
    edges: &mut Vec<GraphEdge>,
) {
    let chains = match detect_chains(conn) {
        Ok(c) => c,
        Err(e) => {
            debug!(target: "4da::content_graph", error = %e, "Signal chain detection failed, skipping chain edges");
            return;
        }
    };

    for chain in &chains {
        let chain_item_ids: Vec<i64> = chain
            .links
            .iter()
            .map(|l| l.source_item_id)
            .filter(|id| id_set.contains(id))
            .collect();

        for window in chain_item_ids.windows(2) {
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
