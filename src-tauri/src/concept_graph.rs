// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Concept Graph — Phase 5 of the Six-Layer Intelligence Architecture
//!
//! Builds a weighted topic co-occurrence graph from recent relevant content.
//! Discovers conceptual neighbors at increasing hop distances to surface
//! serendipitous content the user wouldn't find through direct interest matching.
//!
//! The graph is computed on-demand and cached in memory — no persistent table needed.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing::debug;

use crate::extract_topics;

// ============================================================================
// Types
// ============================================================================

/// A weighted edge between two co-occurring topics in the concept graph.
#[derive(Debug, Clone)]
// REMOVE BY 2026-11-10: struct fields set but not yet read — wire into concept map UI or drop
#[allow(dead_code)]
pub struct ConceptEdge {
    pub topic_a: String,
    pub topic_b: String,
    pub co_occurrence_count: u32,
    pub avg_quality: f32,
    pub weight: f32,
}

/// Maximum number of edges to retain (prevents explosion on large corpora).
const MAX_EDGES: usize = 500;

/// Minimum item count for a topic to be included in neighbor results.
/// Singletons are noise — require 3+ items mentioning a topic.
const MIN_TOPIC_ITEMS: u32 = 3;

// ============================================================================
// Graph Construction
// ============================================================================

/// Build a concept co-occurrence graph from recent relevant source items.
///
/// Reads items from the last 30 days that received positive user feedback.
/// For each item, extracts topics and records pairwise co-occurrences.
/// Edge weight = co_occurrence_count * avg_quality (feedback-based).
///
/// Returns edges sorted by weight descending, capped at [`MAX_EDGES`].
pub fn build_concept_graph(conn: &Connection) -> Result<Vec<ConceptEdge>> {
    // Query recent items that have at least one positive feedback signal.
    // We join source_items with feedback to find "relevant" items,
    // and compute a quality score per item based on feedback ratio.
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, COALESCE(si.content, '') as content,
                    CAST(SUM(f.relevant) AS REAL) / COUNT(f.id) AS quality
             FROM source_items si
             INNER JOIN feedback f ON f.source_item_id = si.id
             WHERE si.created_at >= datetime('now', '-30 days')
             GROUP BY si.id
             HAVING quality > 0.0
             ORDER BY si.created_at DESC
             LIMIT 5000",
        )
        .context("Failed to prepare concept graph query")?;

    // Collect items with their topics and quality scores
    struct ItemTopics {
        topics: Vec<String>,
        quality: f32,
    }

    let items: Vec<ItemTopics> = stmt
        .query_map([], |row| {
            let title: String = row.get(1)?;
            let content: String = row.get(2)?;
            let quality: f64 = row.get(3)?;
            Ok((title, content, quality as f32))
        })
        .context("Failed to execute concept graph query")?
        .filter_map(|r| r.ok())
        .map(|(title, content, quality)| {
            let topics = extract_topics(&title, &content, &[]);
            ItemTopics { topics, quality }
        })
        .collect();

    debug!(
        target: "4da::concept_graph",
        items = items.len(),
        "Building concept graph from recent relevant items"
    );

    // Count pairwise co-occurrences and accumulate quality scores.
    // Key: (topic_a, topic_b) in sorted order to avoid (A,B) vs (B,A) duplication.
    let mut edge_data: HashMap<(String, String), (u32, f32)> = HashMap::new();
    // Track per-topic item count for the singleton filter
    let mut topic_item_counts: HashMap<String, u32> = HashMap::new();

    for item in &items {
        // Deduplicate topics within a single item
        let unique_topics: Vec<&String> = {
            let mut seen = HashSet::new();
            item.topics
                .iter()
                .filter(|t| seen.insert(t.as_str()))
                .collect()
        };

        // Count per-topic appearances
        for topic in &unique_topics {
            *topic_item_counts.entry((*topic).clone()).or_insert(0) += 1;
        }

        // Record all pairs (sorted order for canonical key)
        for i in 0..unique_topics.len() {
            for j in (i + 1)..unique_topics.len() {
                let (a, b) = if unique_topics[i] <= unique_topics[j] {
                    (unique_topics[i].clone(), unique_topics[j].clone())
                } else {
                    (unique_topics[j].clone(), unique_topics[i].clone())
                };
                let entry = edge_data.entry((a, b)).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += item.quality;
            }
        }
    }

    // Build edges: weight = co_occurrence_count * avg_quality
    let mut edges: Vec<ConceptEdge> = edge_data
        .into_iter()
        .filter(|((a, b), _)| {
            // Only keep edges where both topics meet the minimum item threshold
            topic_item_counts.get(a).copied().unwrap_or(0) >= MIN_TOPIC_ITEMS
                && topic_item_counts.get(b).copied().unwrap_or(0) >= MIN_TOPIC_ITEMS
        })
        .map(|((topic_a, topic_b), (count, total_quality))| {
            let avg_quality = if count > 0 {
                total_quality / count as f32
            } else {
                0.0
            };
            ConceptEdge {
                topic_a,
                topic_b,
                co_occurrence_count: count,
                avg_quality,
                weight: count as f32 * avg_quality,
            }
        })
        .collect();

    // Sort by weight descending and cap
    edges.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    edges.truncate(MAX_EDGES);

    debug!(
        target: "4da::concept_graph",
        edges = edges.len(),
        "Concept graph built"
    );

    Ok(edges)
}

// ============================================================================
// Neighbor Discovery
// ============================================================================

/// Find conceptual neighbors at increasing hop distances from user topics.
///
/// - Hop 0: `user_topics` themselves (not returned)
/// - Hop 1: topics directly connected to user_topics via strong edges (weight > median)
/// - Hop 2: topics connected to hop 1 but NOT to user_topics
/// - Hop 3: topics connected to hop 2 but NOT to hop 1 or user_topics
///
/// Only topics appearing in 3+ items (non-singletons) are returned.
/// Results are `(topic, hop_count)` pairs.
pub fn find_conceptual_neighbors(
    graph: &[ConceptEdge],
    user_topics: &[String],
    max_hops: u8,
) -> Vec<(String, u8)> {
    if graph.is_empty() || user_topics.is_empty() || max_hops == 0 {
        return Vec::new();
    }

    // Compute median weight for the "strong edge" threshold
    let median_weight = {
        let mut weights: Vec<f32> = graph.iter().map(|e| e.weight).collect();
        weights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if weights.is_empty() {
            0.0
        } else {
            weights[weights.len() / 2]
        }
    };

    // Build adjacency map (only strong edges)
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in graph {
        if edge.weight >= median_weight {
            adjacency
                .entry(&edge.topic_a)
                .or_default()
                .push(&edge.topic_b);
            adjacency
                .entry(&edge.topic_b)
                .or_default()
                .push(&edge.topic_a);
        }
    }

    // BFS from user_topics using owned Strings for frontiers
    let mut visited: HashSet<String> = user_topics.iter().cloned().collect();
    let mut results: Vec<(String, u8)> = Vec::new();

    // current_frontier starts as user_topics (hop 0) — owned for borrow safety
    let mut current_frontier: HashSet<String> = user_topics.iter().cloned().collect();

    for hop in 1..=max_hops {
        let mut next_frontier: HashSet<String> = HashSet::new();

        for topic in &current_frontier {
            if let Some(neighbors) = adjacency.get(topic.as_str()) {
                for &neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor.to_string());
                        next_frontier.insert(neighbor.to_string());
                    }
                }
            }
        }

        if next_frontier.is_empty() {
            break;
        }

        // Record discovered topics with their hop distance
        for topic in &next_frontier {
            results.push((topic.clone(), hop));
        }

        // Advance: next_frontier becomes current_frontier for the next hop
        current_frontier = next_frontier;
    }

    results
}

// ============================================================================
// Serendipity Item Selection
// ============================================================================

/// Score threshold for serendipity candidates — items must be reasonably high quality.
const SERENDIPITY_SCORE_THRESHOLD: f64 = 0.30;

/// Select a serendipitous source item based on conceptual neighbors.
///
/// Finds source items matching topics at 2-3 hops distance (the "interesting
/// but not obvious" zone). Filters for content quality and returns the best
/// candidate's item_id.
///
/// Returns `None` if no suitable candidates exist.
pub fn select_serendipity_item(
    conn: &Connection,
    neighbors: &[(String, u8)],
) -> Result<Option<i64>> {
    // Only consider topics at hop distance 2-3 (the serendipity zone)
    let distant_topics: Vec<&str> = neighbors
        .iter()
        .filter(|(_, hop)| *hop >= 2)
        .map(|(topic, _)| topic.as_str())
        .collect();

    if distant_topics.is_empty() {
        return Ok(None);
    }

    // Query recent items and find those matching distant topics.
    // We use a title-based approach: extract topics from each item's title
    // and check for overlap with our distant topics.
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, COALESCE(si.content, '') as content,
                    CAST(SUM(f.relevant) AS REAL) / COUNT(f.id) AS quality
             FROM source_items si
             LEFT JOIN feedback f ON f.source_item_id = si.id
             WHERE si.created_at >= datetime('now', '-30 days')
             GROUP BY si.id
             HAVING quality IS NULL OR quality >= ?1
             ORDER BY si.created_at DESC
             LIMIT 2000",
        )
        .context("Failed to prepare serendipity item query")?;

    let distant_set: HashSet<&str> = distant_topics.into_iter().collect();

    // Find the best matching item
    let mut best_item: Option<(i64, f64)> = None;

    let rows = stmt
        .query_map([SERENDIPITY_SCORE_THRESHOLD], |row| {
            let id: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            let content: String = row.get(2)?;
            let quality: Option<f64> = row.get(3)?;
            Ok((id, title, content, quality.unwrap_or(0.5)))
        })
        .context("Failed to query serendipity items")?;

    for row in rows.flatten() {
        let (id, title, content, quality) = row;
        let item_topics = extract_topics(&title, &content, &[]);

        // Check if any of this item's topics are in the distant neighbor set
        let matches = item_topics.iter().any(|t| distant_set.contains(t.as_str()));

        if matches && quality >= SERENDIPITY_SCORE_THRESHOLD {
            if best_item.is_none() || quality > best_item.as_ref().map(|b| b.1).unwrap_or(0.0) {
                best_item = Some((id, quality));
            }
        }
    }

    Ok(best_item.map(|(id, _)| id))
}

#[cfg(test)]
#[path = "concept_graph_tests.rs"]
mod tests;
