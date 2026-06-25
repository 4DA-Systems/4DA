// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Community detection: connected-component clustering and label extraction.

use std::collections::{HashMap, HashSet};

use super::types::{GraphCluster, GraphEdge, RawItem};
use super::SIMILARITY_THRESHOLD;

pub(super) fn compute_clusters(items: &[RawItem], edges: &[GraphEdge]) -> Vec<GraphCluster> {
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    for edge in edges {
        if edge.weight >= SIMILARITY_THRESHOLD * 0.9 {
            adjacency.entry(edge.source).or_default().push(edge.target);
            adjacency.entry(edge.target).or_default().push(edge.source);
        }
    }

    let mut visited: HashSet<i64> = HashSet::new();
    let mut clusters = Vec::new();
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();

    for item in items {
        if visited.contains(&item.id) {
            continue;
        }

        let mut component = Vec::new();
        let mut stack = vec![item.id];

        while let Some(node_id) = stack.pop() {
            if !visited.insert(node_id) {
                continue;
            }
            component.push(node_id);
            if let Some(neighbors) = adjacency.get(&node_id) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }

        if component.len() < 2 {
            continue;
        }

        let sources: HashSet<&str> = component
            .iter()
            .filter_map(|id| item_map.get(id))
            .map(|i| i.source_type.as_str())
            .collect();

        let cluster_id = format!("cluster_{}", component.iter().min().unwrap_or(&0));

        clusters.push(GraphCluster {
            id: cluster_id,
            label: String::new(),
            node_ids: component,
            source_count: sources.len(),
            centroid_x: 0.0,
            centroid_y: 0.0,
        });
    }

    clusters
}

pub(super) fn assign_cluster_labels(items: &[RawItem], clusters: &mut [GraphCluster]) {
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();

    for cluster in clusters.iter_mut() {
        let mut word_freq: HashMap<String, usize> = HashMap::new();
        for &id in &cluster.node_ids {
            if let Some(item) = item_map.get(&id) {
                for word in extract_title_keywords(&item.title) {
                    *word_freq.entry(word).or_insert(0) += 1;
                }
            }
        }

        let mut words: Vec<(String, usize)> = word_freq.into_iter().collect();
        words.sort_by_key(|b| std::cmp::Reverse(b.1));
        cluster.label = words
            .iter()
            .take(3)
            .map(|(w, _)| w.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
    }
}

pub(super) fn extract_title_keywords(title: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "in", "of", "for", "to", "and", "is", "new", "on", "at", "by", "with",
        "from", "this", "that", "it", "its", "has", "have", "are", "was", "were", "been", "be",
        "do", "does", "did", "will", "would", "could", "should", "may", "can", "not", "no", "but",
        "or", "if", "how", "what", "when", "where", "who", "why", "which", "all", "each", "every",
        "both", "more", "most", "other", "some", "such", "than", "too", "very", "just", "about",
        "up", "out", "so", "show", "hn", "ask",
    ];

    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(String::from)
        .collect()
}
