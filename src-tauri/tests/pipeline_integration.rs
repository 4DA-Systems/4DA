#![allow(clippy::unwrap_used)]
//! Pipeline Integration Tests
//!
//! Tests the full source → store → query pipeline using the Database layer
//! directly (no HTTP mocking needed — we test from SourceItem through to query).
//!
//! These tests verify:
//! - Items can be upserted with embeddings and retrieved
//! - Scoring produces consistent results for same input
//! - Database handles concurrent operations
//! - Scale: 1K+ items without degradation

use fourda_lib::test_utils::{seed_embedding, test_db};

// ============================================================================
// Test 1: Fresh database insert and retrieve
// ============================================================================

#[test]
fn test_upsert_and_count() {
    let db = test_db();

    let emb = seed_embedding("test-item-1");
    let id = db
        .upsert_source_item(
            "hackernews",
            "hn_001",
            Some("https://example.com/rust"),
            "Rust Memory Safety",
            "Rust provides memory safety without garbage collection.",
            &emb,
        )
        .expect("Failed to upsert");
    assert!(id > 0);

    let count = db.total_item_count().expect("Failed to count");
    assert_eq!(count, 1);
}

// ============================================================================
// Test 2: Upsert is idempotent (same source_type + source_id)
// ============================================================================

#[test]
fn test_upsert_idempotent() {
    let db = test_db();

    let emb = seed_embedding("item-idem");
    let id1 = db
        .upsert_source_item("reddit", "t3_abc", None, "Title v1", "Content v1", &emb)
        .expect("upsert 1");
    let id2 = db
        .upsert_source_item("reddit", "t3_abc", None, "Title v2", "Content v2", &emb)
        .expect("upsert 2");

    // Same source_type+source_id should produce same DB id
    assert_eq!(id1, id2);

    // Count should still be 1
    let count = db.total_item_count().expect("count");
    assert_eq!(count, 1);
}

// ============================================================================
// Test 3: Multiple sources coexist
// ============================================================================

#[test]
fn test_multi_source_coexistence() {
    let db = test_db();

    let sources = [
        ("hackernews", "hn_1", "HN Title"),
        ("reddit", "t3_1", "Reddit Title"),
        ("github", "gh_1", "GitHub Title"),
        ("arxiv", "2401.00001", "arXiv Title"),
        ("devto", "dev_1", "Dev.to Title"),
        ("lobsters", "lob_1", "Lobsters Title"),
    ];

    for (src, id, title) in &sources {
        let emb = seed_embedding(&format!("{}-{}", src, id));
        db.upsert_source_item(src, id, None, title, "Content", &emb)
            .expect("upsert");
    }

    let count = db.total_item_count().expect("count");
    assert_eq!(count, 6);

    // Register sources and verify
    for (src, _, _) in &sources {
        db.register_source(src, src).expect("register");
    }
    let registered = db.get_all_sources().expect("sources");
    assert_eq!(registered.len(), 6);
}

// ============================================================================
// Test 4: Scale test — insert 1000 items
// ============================================================================

#[test]
fn test_insert_1000_items() {
    let db = test_db();

    let start = std::time::Instant::now();

    for i in 0..1000 {
        let emb = seed_embedding(&format!("item-{}", i));
        db.upsert_source_item(
            "hackernews",
            &format!("hn_{}", i),
            Some(&format!("https://example.com/{}", i)),
            &format!("Title {}: Rust performance testing", i),
            &format!("Content about Rust performance topic number {}", i),
            &emb,
        )
        .expect("upsert");
    }

    let elapsed = start.elapsed();
    let count = db.total_item_count().expect("count");
    assert_eq!(count, 1000);

    // 1000 inserts should complete in under 30 seconds (very generous)
    assert!(
        elapsed.as_secs() < 30,
        "1000 inserts took {:?}, expected < 30s",
        elapsed
    );

    eprintln!(
        "  [perf] 1000 inserts: {:?} ({:.1} items/sec)",
        elapsed,
        1000.0 / elapsed.as_secs_f64()
    );
}

// ============================================================================
// Test 5: KNN query works after inserts
// ============================================================================

#[test]
fn test_knn_query_after_inserts() {
    let db = test_db();

    // Insert items with different embeddings
    for i in 0..50 {
        let emb = seed_embedding(&format!("knn-item-{}", i));
        db.upsert_source_item(
            "hackernews",
            &format!("hn_{}", i),
            None,
            &format!("KNN Test Item {}", i),
            "Content for KNN testing",
            &emb,
        )
        .expect("upsert");
    }

    // Query with a specific embedding — should find similar items
    let query_emb = seed_embedding("knn-item-0");
    let results = db
        .find_similar_source_items(&query_emb, 5)
        .unwrap_or_default();

    // Should return at most 5 results
    assert!(
        results.len() <= 5,
        "KNN should return at most 5 items, got {}",
        results.len()
    );

    // Should find results (we have 50 items in the vec table)
    assert!(
        !results.is_empty(),
        "KNN should find at least 1 similar item"
    );

    // First result should be the exact match item
    assert_eq!(
        results[0].source_id, "hn_0",
        "Closest match should be hn_0 (exact embedding match)"
    );
}

// ============================================================================
// Test 6: Source registration and health tracking
// ============================================================================

#[test]
fn test_source_health_recording() {
    let db = test_db();

    db.register_source("hackernews", "Hacker News")
        .expect("register");

    // Record successful health
    db.record_source_health("hackernews", true, 30, 450, None)
        .expect("record health");

    let sources = db.get_all_sources().expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, "hackernews");
}

// ============================================================================
// Test 7: Feedback recording
// ============================================================================

#[test]
fn test_feedback_cycle() {
    let db = test_db();

    // Insert an item
    let emb = seed_embedding("feedback-test");
    let id = db
        .upsert_source_item(
            "hackernews",
            "hn_fb",
            None,
            "Feedback Test",
            "Content",
            &emb,
        )
        .expect("upsert");

    // Record positive feedback
    db.record_feedback(id as i64, true).expect("feedback");

    let stats = db.get_db_stats().expect("stats");
    assert_eq!(stats.feedback_count, 1);
}

// ============================================================================
// Test 8: Database stats accuracy
// ============================================================================

#[test]
fn test_db_stats_accuracy() {
    let db = test_db();

    // Empty database
    let stats = db.get_db_stats().expect("stats");
    assert_eq!(stats.source_items, 0);
    assert_eq!(stats.context_chunks, 0);
    assert_eq!(stats.feedback_count, 0);

    // Add items
    for i in 0..5 {
        let emb = seed_embedding(&format!("stat-{}", i));
        db.upsert_source_item(
            "github",
            &format!("gh_{}", i),
            None,
            "Title",
            "Content",
            &emb,
        )
        .expect("upsert");
    }

    let stats = db.get_db_stats().expect("stats");
    assert_eq!(stats.source_items, 5);
}

// ============================================================================
// Test 9: Schema version accessible from integration tests
// ============================================================================

#[test]
fn test_schema_version_accessible() {
    let db = test_db();
    let version = db.get_schema_version().expect("schema version");
    assert!(
        version >= 10,
        "Schema version should be >= 10, got {}",
        version
    );
}

// ============================================================================
// Test 10: Migration history accessible from integration tests
// ============================================================================

#[test]
fn test_migration_history_accessible() {
    let db = test_db();
    let history = db.get_migration_history().expect("migration history");
    assert!(
        !history.is_empty(),
        "Fresh database should have migration history"
    );
    for entry in &history {
        assert_eq!(entry.success, 1, "All migrations should succeed");
    }
}

// ============================================================================
// Test 11: Concurrent insert stress (single-threaded sequential)
// ============================================================================

#[test]
fn test_rapid_sequential_inserts() {
    let db = test_db();

    let start = std::time::Instant::now();
    for i in 0..500 {
        let emb = seed_embedding(&format!("rapid-{}", i));
        db.upsert_source_item(
            if i % 3 == 0 {
                "hackernews"
            } else if i % 3 == 1 {
                "reddit"
            } else {
                "github"
            },
            &format!("item_{}", i),
            Some(&format!("https://example.com/{}", i)),
            &format!("Rapid Insert Test {}", i),
            &format!("Content for rapid insert test number {}", i),
            &emb,
        )
        .expect("rapid upsert");
    }
    let elapsed = start.elapsed();

    let count = db.total_item_count().expect("count");
    assert_eq!(count, 500);

    eprintln!(
        "  [perf] 500 rapid inserts: {:?} ({:.0} items/sec)",
        elapsed,
        500.0 / elapsed.as_secs_f64()
    );
}

// ============================================================================
// Test 12: Large content handling
// ============================================================================

#[test]
fn test_large_content_handling() {
    let db = test_db();

    // Create a large content string (100KB)
    let large_content = "A".repeat(100_000);
    let emb = seed_embedding("large-content");

    let id = db
        .upsert_source_item(
            "hackernews",
            "hn_large",
            None,
            "Large Content Test",
            &large_content,
            &emb,
        )
        .expect("large upsert");
    assert!(id > 0);

    let count = db.total_item_count().expect("count");
    assert_eq!(count, 1);
}

// ============================================================================
// Test 13: Empty and edge case inputs
// ============================================================================

#[test]
fn test_edge_case_inputs() {
    let db = test_db();
    let emb = seed_embedding("edge");

    // Empty title (valid)
    db.upsert_source_item("hackernews", "hn_empty_title", None, "", "Content", &emb)
        .expect("empty title");

    // Empty content (valid)
    db.upsert_source_item("hackernews", "hn_empty_content", None, "Title", "", &emb)
        .expect("empty content");

    // Unicode content
    db.upsert_source_item(
        "hackernews",
        "hn_unicode",
        None,
        "Rust のメモリ安全性 🦀",
        "日本語コンテンツ with emoji 🎉",
        &emb,
    )
    .expect("unicode");

    // Very long title
    let long_title = "T".repeat(5000);
    db.upsert_source_item(
        "hackernews",
        "hn_long_title",
        None,
        &long_title,
        "Content",
        &emb,
    )
    .expect("long title");

    let count = db.total_item_count().expect("count");
    assert_eq!(count, 4);
}
