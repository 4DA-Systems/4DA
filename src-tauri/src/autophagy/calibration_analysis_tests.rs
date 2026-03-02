use super::*;
use rusqlite::{params, Connection};

use crate::autophagy::calibration::load_calibration_deltas;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL DEFAULT '',
            url TEXT,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            content_hash TEXT NOT NULL DEFAULT '',
            embedding BLOB,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen TEXT NOT NULL DEFAULT (datetime('now')),
            summary TEXT,
            embedding_status TEXT DEFAULT 'pending',
            embed_text TEXT
        );
        CREATE TABLE feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER NOT NULL,
            relevant INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (source_item_id) REFERENCES source_items(id)
        );
        CREATE TABLE digested_intelligence (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            digest_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            sample_size INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT,
            superseded_by INTEGER
        );",
    )
    .expect("create tables");
    conn
}

fn setup_ace_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            action_type TEXT NOT NULL,
            action_data TEXT,
            item_topics TEXT,
            item_source TEXT,
            signal_strength REAL NOT NULL,
            timestamp TEXT DEFAULT (datetime('now'))
        );",
    )
    .expect("create ACE tables");
    conn
}

// ====================================================================
// Accuracy Feedback Tests
// ====================================================================

#[test]
fn test_accuracy_feedback_empty_db() {
    let ace_conn = setup_ace_db();
    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert!(deltas.is_empty());
}

#[test]
fn test_accuracy_feedback_with_positive_topic() {
    let ace_conn = setup_ace_db();

    // 10 positive interactions with "rust" topic
    for i in 0..10 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, 'save', '[\"rust\"]', 'hackernews', 1.0)",
                params![i],
            )
            .unwrap();
    }

    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].topic, "rust");
    assert_eq!(deltas[0].sample_size, 10);
    // engaged_avg = 10/10 = 1.0, scored_avg = 0.5
    assert!((deltas[0].engaged_avg - 1.0).abs() < f32::EPSILON);
    // delta = 0.6 * (1.0 - 0.5) + 0.4 * 1.0 = 0.3 + 0.4 = 0.7
    assert!(
        deltas[0].delta > 0.0,
        "Positive topic should have positive delta"
    );
}

#[test]
fn test_accuracy_feedback_with_negative_topic() {
    let ace_conn = setup_ace_db();

    // 8 dismissals of "career" topic
    for i in 0..8 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, 'dismiss', '[\"career\"]', 'hackernews', -0.8)",
                params![i],
            )
            .unwrap();
    }

    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].topic, "career");
    // engaged_avg = 0/8 = 0.0, scored_avg = 0.5
    assert!((deltas[0].engaged_avg - 0.0).abs() < f32::EPSILON);
    // delta = 0.6 * (0.0 - 0.5) + 0.4 * (-0.8) = -0.3 + -0.32 = -0.62
    assert!(
        deltas[0].delta < 0.0,
        "Negative topic should have negative delta"
    );
}

#[test]
fn test_accuracy_feedback_min_samples_filter() {
    let ace_conn = setup_ace_db();

    // Only 3 interactions (below min_samples=5 threshold)
    for i in 0..3 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, 'save', '[\"niche\"]', 'hackernews', 1.0)",
                params![i],
            )
            .unwrap();
    }

    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert!(
        deltas.is_empty(),
        "Topics with fewer than 5 interactions should be filtered"
    );
}

#[test]
fn test_accuracy_feedback_multiple_topics_per_interaction() {
    let ace_conn = setup_ace_db();

    // Each interaction has multiple topics
    for i in 0..6 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, 'save', '[\"rust\", \"systems\"]', 'hackernews', 1.0)",
                params![i],
            )
            .unwrap();
    }

    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert_eq!(deltas.len(), 2, "Both topics should get deltas");
    for d in &deltas {
        assert!(d.delta > 0.0);
        assert_eq!(d.sample_size, 6);
    }
}

#[test]
fn test_accuracy_feedback_lookback_window() {
    let ace_conn = setup_ace_db();

    // 5 recent interactions
    for i in 0..5 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, 'save', '[\"rust\"]', 'hackernews', 1.0)",
                params![i],
            )
            .unwrap();
    }
    // 5 old interactions (60 days ago)
    for i in 0..5 {
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength, timestamp)
                 VALUES (?1, 'dismiss', '[\"rust\"]', 'hackernews', -0.8, datetime('now', '-60 days'))",
                params![i + 100],
            )
            .unwrap();
    }

    // 30-day lookback should only see the 5 recent positive ones
    let deltas = analyze_accuracy_feedback(&ace_conn, 30);
    assert_eq!(deltas.len(), 1);
    assert!(
        deltas[0].delta > 0.0,
        "Only recent positive signals counted"
    );
    assert_eq!(deltas[0].sample_size, 5);
}

// ====================================================================
// Topic-Level Calibration Tests
// ====================================================================

#[test]
fn test_extract_title_topics() {
    let topics = extract_title_topics("Rust 2024 Edition Systems Programming");
    assert!(topics.contains(&"rust".to_string()));
    assert!(topics.contains(&"edition".to_string()));
    assert!(topics.contains(&"systems".to_string()));
    assert!(topics.contains(&"programming".to_string()));
    // "2024" is 4 chars so it passes length filter
    assert!(topics.contains(&"2024".to_string()));

    // Stop words are filtered
    let topics2 = extract_title_topics("The Best Guide About What You Should Know");
    // "the", "best", "about", "what", "should", "know" are stop words or <= 3 chars
    assert!(topics2.contains(&"guide".to_string()));
}

#[test]
fn test_extract_title_topics_max_five() {
    let topics = extract_title_topics(
        "Building Scalable Distributed Systems Architecture Patterns Kubernetes Docker Terraform",
    );
    assert!(topics.len() <= 5, "Should extract at most 5 topics");
}

#[test]
fn test_topic_calibration_empty_db() {
    let conn = setup_test_db();
    let deltas = analyze_topic_calibration(&conn, 30);
    assert!(deltas.is_empty());
}

#[test]
fn test_topic_calibration_with_data() {
    let conn = setup_test_db();

    // Insert items with "rust" in title in the pruning window
    for i in 0..6 {
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, title, last_seen)
             VALUES ('hackernews', ?1, 'Rust Programming Update', datetime('now', '-25 days'))",
            params![format!("hn_{}", i)],
        )
        .unwrap();
    }

    // 2 out of 6 have positive feedback
    for i in 1..=2 {
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
            params![i],
        )
        .unwrap();
    }

    let deltas = analyze_topic_calibration(&conn, 30);
    assert!(!deltas.is_empty(), "Should produce topic deltas");
    let rust_delta = deltas.iter().find(|d| d.topic == "rust");
    assert!(rust_delta.is_some(), "Should have a delta for 'rust'");
    let rd = rust_delta.unwrap();
    assert_eq!(rd.sample_size, 6);
    // engaged_avg = 2/6 ≈ 0.333
    assert!((rd.engaged_avg - 2.0 / 6.0).abs() < 0.01);
}

#[test]
fn test_topic_calibration_min_samples() {
    let conn = setup_test_db();

    // Only 2 items (below min_samples=3)
    for i in 0..2 {
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, title, last_seen)
             VALUES ('hackernews', ?1, 'Niche Topic Article', datetime('now', '-25 days'))",
            params![format!("hn_{}", i)],
        )
        .unwrap();
    }

    let deltas = analyze_topic_calibration(&conn, 30);
    // "niche", "topic", "article" each have only 2 occurrences → filtered out
    assert!(
        deltas.is_empty(),
        "Topics below min_samples should be filtered"
    );
}

#[test]
fn test_bridge_accuracy_feedback_end_to_end() {
    let ace_conn = setup_ace_db();
    let main_conn = setup_test_db();

    // Add ACE interactions
    for i in 0..10 {
        let signal = if i < 7 { 1.0 } else { -0.8 };
        let action = if i < 7 { "save" } else { "dismiss" };
        ace_conn
            .execute(
                "INSERT INTO interactions (item_id, action_type, item_topics, item_source, signal_strength)
                 VALUES (?1, ?2, '[\"rust\"]', 'hackernews', ?3)",
                params![i, action, signal],
            )
            .unwrap();
    }

    // Bridge should store calibration deltas
    let count = bridge_accuracy_feedback(&ace_conn, &main_conn, 30).expect("bridge");
    assert_eq!(count, 1);

    // Should be loadable from main DB
    let loaded = load_calibration_deltas(&main_conn);
    assert!(loaded.contains_key("rust"), "Rust delta should be stored");
    // 7 positive out of 10: engaged_avg = 0.7
    // avg_signal = (7*1.0 + 3*(-0.8)) / 10 = 4.6/10 = 0.46
    // delta = 0.6*(0.7-0.5) + 0.4*0.46 = 0.12 + 0.184 = 0.304
    assert!(
        loaded["rust"] > 0.0,
        "Mostly positive topic should produce positive delta"
    );
}
