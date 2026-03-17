//! Calibration analysis — compares what the system scored vs what users engaged with.
//!
//! Items in the pruning window (approaching max_age_days) are cross-referenced with
//! the feedback table. Per-topic deltas reveal systematic scoring biases.

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

/// Analyze calibration: compare scored items vs actual user engagement.
///
/// Examines items in the "pruning window" (between max_age_days-7 and max_age_days old)
/// and cross-references with the feedback table. Groups by source_type and computes
/// engagement rates to identify scoring biases.
pub(crate) fn analyze_calibration(
    conn: &Connection,
    max_age_days: i64,
) -> Vec<super::CalibrationDelta> {
    let cycle_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
        .unwrap_or(0);

    // Early users (< 3 cycles): analyze ALL recent items instead of narrow pruning window
    let (window_start_days, window_end_days) = if cycle_count < 3 {
        debug!(target: "4da::autophagy", cycle_count, "Early user: widening calibration window to all items");
        (max_age_days, 0_i64)
    } else {
        (max_age_days, max_age_days.saturating_sub(7))
    };

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
) -> Result<()> {
    for delta in deltas {
        let data = serde_json::to_string(&serde_json::json!({
            "scored_avg": delta.scored_avg,
            "engaged_avg": delta.engaged_avg,
            "delta": delta.delta,
            "sample_size": delta.sample_size,
        }))?;

        // Insert new calibration first, then point old rows at it.
        // This order satisfies the FK constraint on superseded_by -> digested_intelligence(id).
        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('calibration', ?1, ?2, ?3, ?4)",
            params![delta.topic, data, delta.confidence, delta.sample_size],
        )
        .with_context(|| format!("Failed to insert calibration for {}", delta.topic))?;

        let new_id = conn.last_insert_rowid();

        // Supersede previous calibrations for the same topic (excluding the one just inserted)
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = ?1
             WHERE digest_type = 'calibration' AND subject = ?2 AND superseded_by IS NULL AND id != ?1",
            params![new_id, delta.topic],
        )
        .with_context(|| format!("Failed to supersede calibration for {}", delta.topic))?;
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
}
