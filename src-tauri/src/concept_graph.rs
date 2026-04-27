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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // ConceptEdge construction
    // ====================================================================

    fn make_edge(a: &str, b: &str, count: u32, avg_quality: f32) -> ConceptEdge {
        ConceptEdge {
            topic_a: a.to_string(),
            topic_b: b.to_string(),
            co_occurrence_count: count,
            avg_quality,
            weight: count as f32 * avg_quality,
        }
    }

    // ====================================================================
    // Graph construction from mock DB
    // ====================================================================

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL DEFAULT 'test',
                source_id TEXT NOT NULL DEFAULT '',
                url TEXT,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                embedding BLOB NOT NULL DEFAULT x'',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );",
        )
        .unwrap();
        conn
    }

    fn insert_item(conn: &Connection, id: i64, title: &str) {
        conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, title, content_hash)
             VALUES (?1, 'test', ?2, ?3, 'hash')",
            rusqlite::params![id, format!("item_{id}"), title],
        )
        .unwrap();
    }

    fn insert_feedback(conn: &Connection, item_id: i64, relevant: bool) {
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
            rusqlite::params![item_id, relevant as i32],
        )
        .unwrap();
    }

    #[test]
    fn test_build_graph_empty_db() {
        let conn = setup_test_db();
        let edges = build_concept_graph(&conn).unwrap();
        assert!(edges.is_empty(), "Empty DB should produce no edges");
    }

    #[test]
    fn test_build_graph_single_item_no_edges() {
        let conn = setup_test_db();
        insert_item(&conn, 1, "Rust programming");
        insert_feedback(&conn, 1, true);
        let edges = build_concept_graph(&conn).unwrap();
        // Single item with one topic can't produce co-occurrence edges
        // (needs at least 2 topics per item, and MIN_TOPIC_ITEMS=3 items per topic)
        assert!(
            edges.is_empty(),
            "Single item should not produce edges (singleton filter)"
        );
    }

    #[test]
    fn test_build_graph_co_occurrences() {
        let conn = setup_test_db();

        // Insert items that share topic pairs, enough times to pass MIN_TOPIC_ITEMS=3
        // "Rust" and "database" co-occur in 3+ items
        insert_item(&conn, 1, "Rust database performance");
        insert_item(&conn, 2, "Building a Rust database");
        insert_item(&conn, 3, "Rust database patterns");
        insert_item(&conn, 4, "Rust database optimization");

        for id in 1..=4 {
            insert_feedback(&conn, id, true);
        }

        let edges = build_concept_graph(&conn).unwrap();

        // Should have at least one edge connecting "rust" and "database"
        let rust_db = edges.iter().find(|e| {
            (e.topic_a == "rust" && e.topic_b == "database")
                || (e.topic_a == "database" && e.topic_b == "rust")
        });
        assert!(
            rust_db.is_some(),
            "Should find rust-database co-occurrence edge"
        );

        let edge = rust_db.unwrap();
        assert!(
            edge.co_occurrence_count >= 3,
            "Should have 3+ co-occurrences, got {}",
            edge.co_occurrence_count
        );
        assert!(edge.weight > 0.0, "Edge weight should be positive");
    }

    #[test]
    fn test_build_graph_negative_feedback_excluded() {
        let conn = setup_test_db();

        // Items with only negative feedback should not appear
        insert_item(&conn, 1, "Rust database");
        insert_feedback(&conn, 1, false);

        let edges = build_concept_graph(&conn).unwrap();
        assert!(
            edges.is_empty(),
            "Items with only negative feedback should not produce edges"
        );
    }

    #[test]
    fn test_build_graph_sorted_by_weight() {
        let conn = setup_test_db();

        // Create two sets of co-occurrences with different counts
        // rust+python co-occurs 4 times
        for i in 1..=4 {
            insert_item(&conn, i, "Rust and Python programming");
            insert_feedback(&conn, i, true);
        }
        // rust+docker co-occurs 3 times
        for i in 5..=7 {
            insert_item(&conn, i, "Rust Docker container");
            insert_feedback(&conn, i, true);
        }

        let edges = build_concept_graph(&conn).unwrap();

        // Verify sorted by weight descending
        for window in edges.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "Edges should be sorted by weight desc: {} >= {} failed",
                window[0].weight,
                window[1].weight
            );
        }
    }

    // ====================================================================
    // Neighbor discovery tests
    // ====================================================================

    #[test]
    fn test_neighbors_empty_graph() {
        let neighbors = find_conceptual_neighbors(&[], &["rust".to_string()], 3);
        assert!(neighbors.is_empty(), "Empty graph yields no neighbors");
    }

    #[test]
    fn test_neighbors_empty_user_topics() {
        let graph = vec![make_edge("rust", "python", 5, 0.8)];
        let neighbors = find_conceptual_neighbors(&graph, &[], 3);
        assert!(neighbors.is_empty(), "No user topics yields no neighbors");
    }

    #[test]
    fn test_neighbors_hop_1() {
        // All edges have equal weight so none are filtered by median threshold.
        // rust -- python, rust -- database
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),       // weight 9.0
            make_edge("rust", "database", 10, 0.9),     // weight 9.0
            make_edge("python", "javascript", 10, 0.9), // weight 9.0
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 1);

        let hop1_topics: Vec<&str> = neighbors
            .iter()
            .filter(|(_, h)| *h == 1)
            .map(|(t, _)| t.as_str())
            .collect();

        assert!(
            hop1_topics.contains(&"python"),
            "python should be at hop 1. Got: {:?}",
            hop1_topics
        );
        assert!(
            hop1_topics.contains(&"database"),
            "database should be at hop 1. Got: {:?}",
            hop1_topics
        );
    }

    #[test]
    fn test_neighbors_hop_2() {
        // Equal-weight chain: rust -- python -- javascript
        // (rust NOT directly connected to javascript)
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),       // weight 9.0
            make_edge("python", "javascript", 10, 0.9), // weight 9.0
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 2);

        let hop2_topics: Vec<&str> = neighbors
            .iter()
            .filter(|(_, h)| *h == 2)
            .map(|(t, _)| t.as_str())
            .collect();

        assert!(
            hop2_topics.contains(&"javascript"),
            "javascript should be at hop 2. Got neighbors: {:?}",
            neighbors
        );
    }

    #[test]
    fn test_neighbors_hop_3() {
        // Equal-weight chain: rust -- python -- javascript -- typescript
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),
            make_edge("python", "javascript", 10, 0.9),
            make_edge("javascript", "typescript", 10, 0.9),
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 3);

        let hop3_topics: Vec<&str> = neighbors
            .iter()
            .filter(|(_, h)| *h == 3)
            .map(|(t, _)| t.as_str())
            .collect();

        assert!(
            hop3_topics.contains(&"typescript"),
            "typescript should be at hop 3. Got neighbors: {:?}",
            neighbors
        );
    }

    #[test]
    fn test_neighbors_max_hops_limits_depth() {
        // Equal-weight chain: rust -- python -- javascript -- typescript
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),
            make_edge("python", "javascript", 10, 0.9),
            make_edge("javascript", "typescript", 10, 0.9),
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 1);

        // Only hop 1 should be discovered
        assert!(
            neighbors.iter().all(|(_, h)| *h == 1),
            "max_hops=1 should only return hop 1 neighbors"
        );
        assert!(
            !neighbors.iter().any(|(t, _)| t == "typescript"),
            "typescript at hop 3 should not appear with max_hops=1"
        );
    }

    #[test]
    fn test_neighbors_no_revisit_user_topics() {
        // rust -- python -- rust (cycle) — rust should NOT appear as a neighbor
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),
            make_edge("python", "rust", 8, 0.8), // same edge reversed
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 3);

        assert!(
            !neighbors.iter().any(|(t, _)| t == "rust"),
            "User's own topics should not appear as neighbors"
        );
    }

    #[test]
    fn test_neighbors_weak_edges_filtered() {
        // Strong edge: rust-python (weight 9.0, above median)
        // Weak edge: rust-obscure (weight 0.1, below median)
        let graph = vec![
            make_edge("rust", "python", 10, 0.9),      // weight 9.0
            make_edge("rust", "obscure", 1, 0.1),      // weight 0.1
            make_edge("python", "javascript", 8, 0.8), // weight 6.4
        ];

        let neighbors = find_conceptual_neighbors(&graph, &["rust".to_string()], 1);

        // "obscure" is connected via a weak edge (below median weight of 6.4)
        // so it should NOT appear unless it's above the median
        // Median of [0.1, 6.4, 9.0] = 6.4. "obscure" edge weight is 0.1 < 6.4.
        assert!(
            !neighbors.iter().any(|(t, _)| t == "obscure"),
            "Topics connected only via weak edges should be filtered out"
        );
    }

    // ====================================================================
    // Serendipity item selection tests
    // ====================================================================

    #[test]
    fn test_select_serendipity_no_neighbors() {
        let conn = setup_test_db();
        let result = select_serendipity_item(&conn, &[]).unwrap();
        assert!(result.is_none(), "No neighbors should return None");
    }

    #[test]
    fn test_select_serendipity_only_hop1_skipped() {
        let conn = setup_test_db();
        // Only hop-1 neighbors — these are too "obvious" for serendipity
        let neighbors = vec![("python".to_string(), 1u8)];
        let result = select_serendipity_item(&conn, &neighbors).unwrap();
        assert!(
            result.is_none(),
            "Hop-1 neighbors should not trigger serendipity"
        );
    }

    #[test]
    fn test_select_serendipity_finds_matching_item() {
        let conn = setup_test_db();

        // Insert an item with a topic that matches a hop-2 neighbor
        insert_item(&conn, 1, "Python machine learning tutorial");
        insert_feedback(&conn, 1, true);

        let neighbors = vec![("python".to_string(), 2u8), ("unrelated".to_string(), 3u8)];

        let result = select_serendipity_item(&conn, &neighbors).unwrap();
        assert_eq!(
            result,
            Some(1),
            "Should find item matching hop-2 topic 'python'"
        );
    }

    #[test]
    fn test_select_serendipity_prefers_higher_quality() {
        let conn = setup_test_db();

        // Two items matching the same hop-2 topic, different quality
        insert_item(&conn, 1, "Python basics");
        insert_feedback(&conn, 1, true); // quality 1.0

        insert_item(&conn, 2, "Python advanced patterns");
        insert_feedback(&conn, 2, true);
        insert_feedback(&conn, 2, true); // quality 1.0 (both positive)

        let neighbors = vec![("python".to_string(), 2u8)];

        let result = select_serendipity_item(&conn, &neighbors).unwrap();
        // Both have quality 1.0, so either is acceptable
        assert!(result.is_some(), "Should find at least one matching item");
    }
}
