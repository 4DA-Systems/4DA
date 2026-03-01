#![allow(dead_code)]
//! Calibration analysis — compares what the system scored vs what users engaged with.
//!
//! Items in the pruning window (approaching max_age_days) are cross-referenced with
//! the feedback table. Per-topic deltas reveal systematic scoring biases.

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Analyze calibration: compare scored items vs actual user engagement.
///
/// Examines items in the "pruning window" (between max_age_days-7 and max_age_days old)
/// and cross-references with the feedback table. Groups by source_type and computes
/// engagement rates to identify scoring biases.
pub(crate) fn analyze_calibration(
    conn: &Connection,
    max_age_days: i64,
) -> Vec<super::CalibrationDelta> {
    let window_start_days = max_age_days;
    let window_end_days = max_age_days.saturating_sub(7);

    let window_start = format!("-{} days", window_start_days);
    let window_end = format!("-{} days", window_end_days);

    debug!(
        target: "4da::autophagy",
        window_start_days, window_end_days,
        "Analyzing calibration in pruning window"
    );

    // Query items near pruning with their engagement status.
    // Each row: (source_item_id, source_type, title, engaged_flag)
    let mut stmt = match conn.prepare(
        "SELECT si.id, si.source_type, si.title,
                COALESCE((SELECT MAX(f.relevant) FROM feedback f WHERE f.source_item_id = si.id), 0) AS engaged
         FROM source_items si
         WHERE si.last_seen < datetime('now', ?1)
           AND si.last_seen >= datetime('now', ?2)",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Calibration query failed");
            return vec![];
        }
    };

    // Accumulate per-source_type stats: (total_count, engaged_count)
    let mut topic_stats: HashMap<String, (i64, i64)> = HashMap::new();

    let rows = match stmt.query_map(params![window_end, window_start], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Calibration row iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (_id, source_type, _title, engaged) = row;
        let entry = topic_stats.entry(source_type).or_insert((0, 0));
        entry.0 += 1;
        if engaged > 0 {
            entry.1 += 1;
        }
    }

    if topic_stats.is_empty() {
        debug!(target: "4da::autophagy", "No items in pruning window for calibration");
        return vec![];
    }

    let mut deltas = Vec::new();

    for (topic, (total, engaged)) in &topic_stats {
        if *total == 0 {
            continue;
        }

        // scored_avg = 1.0 (baseline: system surfaced all these items)
        // engaged_avg = proportion with positive feedback
        let scored_avg = 1.0_f32;
        let engaged_avg = *engaged as f32 / *total as f32;
        let delta = engaged_avg - scored_avg;
        let confidence = (*total as f32 / 20.0).min(1.0);

        deltas.push(super::CalibrationDelta {
            topic: topic.clone(),
            scored_avg,
            engaged_avg,
            delta,
            sample_size: *total,
            confidence,
        });
    }

    info!(
        target: "4da::autophagy",
        topics = deltas.len(),
        total_items = topic_stats.values().map(|(t, _)| t).sum::<i64>(),
        "Calibration analysis complete"
    );

    deltas
}

/// Store calibrations to `digested_intelligence`, superseding previous entries
/// for the same topic.
pub(crate) fn store_calibrations(
    conn: &Connection,
    deltas: &[super::CalibrationDelta],
) -> Result<(), String> {
    for delta in deltas {
        let data = serde_json::to_string(&serde_json::json!({
            "scored_avg": delta.scored_avg,
            "engaged_avg": delta.engaged_avg,
            "delta": delta.delta,
            "sample_size": delta.sample_size,
        }))
        .map_err(|e| e.to_string())?;

        // Supersede previous calibration for the same topic
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = (SELECT COALESCE(MAX(id), 0) + 1 FROM digested_intelligence)
             WHERE digest_type = 'calibration' AND subject = ?1 AND superseded_by IS NULL",
            params![delta.topic],
        )
        .map_err(|e| format!("Failed to supersede calibration for {}: {}", delta.topic, e))?;

        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('calibration', ?1, ?2, ?3, ?4)",
            params![delta.topic, data, delta.confidence, delta.sample_size],
        )
        .map_err(|e| format!("Failed to insert calibration for {}: {}", delta.topic, e))?;
    }

    debug!(target: "4da::autophagy", count = deltas.len(), "Stored calibration deltas");
    Ok(())
}

/// Load the latest (non-superseded) calibration deltas for the scoring pipeline.
///
/// Returns a map of topic -> delta value. Positive delta means the system
/// under-scored items users liked; negative means over-scored.
pub(crate) fn load_calibration_deltas(conn: &Connection) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    let mut stmt = match conn.prepare(
        "SELECT subject, data FROM digested_intelligence
         WHERE digest_type = 'calibration' AND superseded_by IS NULL
         ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Failed to load calibration deltas");
            return result;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Failed to iterate calibration rows");
            return result;
        }
    };

    for row in rows.flatten() {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&row.1) {
            if let Some(delta) = data.get("delta").and_then(|v| v.as_f64()) {
                result.insert(row.0, delta as f32);
            }
        }
    }

    debug!(target: "4da::autophagy", count = result.len(), "Loaded calibration deltas");
    result
}

/// Analyze accuracy feedback from ACE behavior data to produce topic-level calibration.
///
/// Reads implicit interaction signals (save, click, dismiss, ignore, scroll) from the
/// ACE database, groups by content topic, and computes per-topic engagement deltas.
/// These deltas feed directly into the scoring pipeline's calibration correction.
///
/// Unlike `analyze_calibration()` which groups by source_type, this groups by content
/// topic — catching biases the source-level analysis misses (e.g., "we over-score
/// security articles regardless of source").
pub(crate) fn analyze_accuracy_feedback(
    ace_conn: &Connection,
    lookback_days: i64,
) -> Vec<super::CalibrationDelta> {
    let window = format!("-{} days", lookback_days);

    // Query all interactions within the lookback window that have topics
    let mut stmt = match ace_conn.prepare(
        "SELECT item_topics, signal_strength
         FROM interactions
         WHERE timestamp >= datetime('now', ?1)
           AND item_topics IS NOT NULL
           AND item_topics != '[]'",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Accuracy feedback query failed");
            return vec![];
        }
    };

    // Accumulate per-topic stats: (total, positive_count, total_signal)
    let mut topic_stats: HashMap<String, (i64, i64, f64)> = HashMap::new();

    let rows = match stmt.query_map(params![window], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Accuracy feedback row iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (topics_json, signal_strength) = row;
        let topics: Vec<String> = match serde_json::from_str(&topics_json) {
            Ok(t) => t,
            Err(_) => continue,
        };

        for topic in topics {
            let entry = topic_stats.entry(topic).or_insert((0, 0, 0.0));
            entry.0 += 1; // total
            if signal_strength > 0.0 {
                entry.1 += 1; // positive count
            }
            entry.2 += signal_strength; // sum of signals
        }
    }

    if topic_stats.is_empty() {
        debug!(target: "4da::autophagy", "No accuracy feedback data available");
        return vec![];
    }

    let mut deltas = Vec::new();
    let min_samples = 5;

    for (topic, (total, positive, signal_sum)) in &topic_stats {
        if *total < min_samples {
            continue;
        }

        // Engagement rate: proportion of positive interactions
        let engaged_avg = *positive as f32 / *total as f32;
        // System baseline: we expect roughly 50% positive engagement
        // (if scoring is perfectly calibrated, users like half of what we show)
        let scored_avg = 0.5_f32;
        // Delta: positive = users like this topic more than expected (boost it)
        //        negative = users reject this topic (penalize it)
        let delta = engaged_avg - scored_avg;
        // Also factor in signal magnitude (save=1.0 is stronger than click=0.5)
        let avg_signal = *signal_sum as f32 / *total as f32;
        // Blend engagement rate delta with signal magnitude for richer correction
        let blended_delta = delta * 0.6 + avg_signal * 0.4;
        let confidence = (*total as f32 / 15.0).min(1.0);

        deltas.push(super::CalibrationDelta {
            topic: topic.clone(),
            scored_avg,
            engaged_avg,
            delta: blended_delta,
            sample_size: *total,
            confidence,
        });
    }

    info!(
        target: "4da::autophagy",
        topics = deltas.len(),
        total_interactions = topic_stats.values().map(|(t, _, _)| t).sum::<i64>(),
        "Accuracy feedback analysis complete"
    );

    deltas
}

/// Analyze calibration at the content-topic level (not source_type level).
///
/// Extracts topics from item titles in the pruning window, cross-references with
/// feedback, and computes per-topic engagement deltas. This catches biases that
/// source-level analysis misses — e.g., "we over-score articles about kubernetes
/// regardless of whether they come from HN or Reddit."
pub(crate) fn analyze_topic_calibration(
    conn: &Connection,
    max_age_days: i64,
) -> Vec<super::CalibrationDelta> {
    let window_start_days = max_age_days;
    let window_end_days = max_age_days.saturating_sub(7);
    let window_start = format!("-{} days", window_start_days);
    let window_end = format!("-{} days", window_end_days);

    // Query items in pruning window with their engagement status
    let mut stmt = match conn.prepare(
        "SELECT si.title,
                COALESCE((SELECT MAX(f.relevant) FROM feedback f WHERE f.source_item_id = si.id), 0) AS engaged
         FROM source_items si
         WHERE si.last_seen < datetime('now', ?1)
           AND si.last_seen >= datetime('now', ?2)",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic calibration query failed");
            return vec![];
        }
    };

    // Accumulate per-topic stats: (total, engaged_count)
    let mut topic_stats: HashMap<String, (i64, i64)> = HashMap::new();

    let rows = match stmt.query_map(params![window_end, window_start], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic calibration iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (title, engaged) = row;
        let topics = extract_title_topics(&title);
        for topic in topics {
            let entry = topic_stats.entry(topic).or_insert((0, 0));
            entry.0 += 1;
            if engaged > 0 {
                entry.1 += 1;
            }
        }
    }

    if topic_stats.is_empty() {
        debug!(target: "4da::autophagy", "No items for topic-level calibration");
        return vec![];
    }

    let min_samples = 3;
    let mut deltas = Vec::new();

    for (topic, (total, engaged)) in &topic_stats {
        if *total < min_samples {
            continue;
        }

        let scored_avg = 1.0_f32;
        let engaged_avg = *engaged as f32 / *total as f32;
        let delta = engaged_avg - scored_avg;
        let confidence = (*total as f32 / 10.0).min(1.0);

        deltas.push(super::CalibrationDelta {
            topic: topic.clone(),
            scored_avg,
            engaged_avg,
            delta,
            sample_size: *total,
            confidence,
        });
    }

    info!(
        target: "4da::autophagy",
        topics = deltas.len(),
        total_items = topic_stats.values().map(|(t, _)| t).sum::<i64>(),
        "Topic-level calibration analysis complete"
    );

    deltas
}

/// Extract meaningful topic keywords from a title.
///
/// Filters to lowercase words > 3 chars, removes common stop words,
/// keeps at most 5 topics per title.
fn extract_title_topics(title: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "the", "this", "that", "with", "from", "your", "about", "what", "when", "where", "which",
        "their", "there", "they", "have", "been", "will", "would", "could", "should", "more",
        "most", "some", "than", "then", "into", "also", "just", "very", "much", "does", "like",
        "each", "every", "only", "over", "under", "after", "before", "show", "need", "make",
        "here", "best", "good", "know",
    ];

    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .filter(|w| !STOP_WORDS.contains(w))
        .take(5)
        .map(|s| s.to_string())
        .collect()
}

/// Bridge accuracy feedback from ACE behavior data into the main calibration system.
///
/// Reads from the ACE database, analyzes per-topic engagement, and stores the resulting
/// calibration deltas in the main database's `digested_intelligence` table.
/// Returns the number of calibration entries produced.
pub(crate) fn bridge_accuracy_feedback(
    ace_conn: &Connection,
    main_conn: &Connection,
    lookback_days: i64,
) -> Result<usize, String> {
    let deltas = analyze_accuracy_feedback(ace_conn, lookback_days);
    if deltas.is_empty() {
        return Ok(0);
    }

    let count = deltas.len();
    store_calibrations(main_conn, &deltas)?;

    info!(
        target: "4da::autophagy",
        count,
        "Bridged accuracy feedback to calibration system"
    );

    Ok(count)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_analyze_calibration_empty_db() {
        let conn = setup_test_db();
        let deltas = analyze_calibration(&conn, 30);
        assert!(deltas.is_empty());
    }

    #[test]
    fn test_analyze_calibration_with_data() {
        let conn = setup_test_db();

        // Insert items in the pruning window (25 days old)
        for i in 0..10 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, last_seen)
                 VALUES ('hackernews', ?1, ?2, datetime('now', '-25 days'))",
                params![format!("hn_{}", i), format!("HN Story {}", i)],
            )
            .unwrap();
        }

        // 3 out of 10 have positive feedback
        for i in 0..3 {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i + 1],
            )
            .unwrap();
        }

        let deltas = analyze_calibration(&conn, 30);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].topic, "hackernews");
        assert_eq!(deltas[0].sample_size, 10);
        assert!((deltas[0].engaged_avg - 0.3).abs() < f32::EPSILON);
        // delta = engaged_avg - scored_avg = 0.3 - 1.0 = -0.7
        assert!((deltas[0].delta - (-0.7)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_store_and_load_calibrations() {
        let conn = setup_test_db();

        let deltas = vec![
            super::super::CalibrationDelta {
                topic: "hackernews".to_string(),
                scored_avg: 1.0,
                engaged_avg: 0.3,
                delta: -0.7,
                sample_size: 10,
                confidence: 0.5,
            },
            super::super::CalibrationDelta {
                topic: "arxiv".to_string(),
                scored_avg: 1.0,
                engaged_avg: 0.1,
                delta: -0.9,
                sample_size: 20,
                confidence: 1.0,
            },
        ];

        store_calibrations(&conn, &deltas).expect("store");

        let loaded = load_calibration_deltas(&conn);
        assert_eq!(loaded.len(), 2);
        assert!((loaded["hackernews"] - (-0.7)).abs() < 0.01);
        assert!((loaded["arxiv"] - (-0.9)).abs() < 0.01);
    }

    #[test]
    fn test_superseding_calibrations() {
        let conn = setup_test_db();

        let deltas_v1 = vec![super::super::CalibrationDelta {
            topic: "hackernews".to_string(),
            scored_avg: 1.0,
            engaged_avg: 0.3,
            delta: -0.7,
            sample_size: 10,
            confidence: 0.5,
        }];
        store_calibrations(&conn, &deltas_v1).expect("store v1");

        // Store a new version
        let deltas_v2 = vec![super::super::CalibrationDelta {
            topic: "hackernews".to_string(),
            scored_avg: 1.0,
            engaged_avg: 0.5,
            delta: -0.5,
            sample_size: 20,
            confidence: 1.0,
        }];
        store_calibrations(&conn, &deltas_v2).expect("store v2");

        // Only the latest should be returned
        let loaded = load_calibration_deltas(&conn);
        assert_eq!(loaded.len(), 1);
        assert!((loaded["hackernews"] - (-0.5)).abs() < 0.01);
    }

    // ====================================================================
    // Accuracy Feedback Bridge Tests
    // ====================================================================

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
}
