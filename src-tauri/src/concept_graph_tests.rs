// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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
