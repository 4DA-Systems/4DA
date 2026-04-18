// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Anti-pattern detection — finds systematic over-scoring and under-scoring.
//!
//! **Over-scored**: source_types with many surfaced items but very low engagement.
//! The system keeps showing content that users consistently ignore.
//!
//! **Under-scored**: source_types where engaged items exist but total surfacing is low.
//! The system may be filtering out valuable content.

use std::collections::HashMap;

use crate::error::{Result, ResultExt};
use rusqlite::{params, Connection};
use tracing::{debug, info, warn};

/// Minimum items required to declare an anti-pattern. Prevents false positives
/// from small sample sizes.
const MIN_EXPOSURE_COUNT: i64 = 5;

/// Engagement rate below this is considered "over-scored" (system surfacing junk).
const OVER_SCORED_THRESHOLD: f32 = 0.05;

/// Detect anti-patterns: find source_types that are systematically over- or under-scored.
///
/// - **OverScored**: source_type with >= MIN_EXPOSURE_COUNT items but < 5% engagement rate.
///   Suggests the scoring pipeline is too generous for this source.
/// - **UnderScored**: source_type with relatively high engagement rate (>50%) but few
///   total items surfaced, suggesting the pipeline is too strict.
///
/// `_threshold` is reserved for future use with actual score data.
pub(crate) fn detect_anti_patterns(conn: &Connection, _threshold: f32) -> Vec<super::AntiPattern> {
    // Get per-source_type: total items and engaged items
    let mut stmt = match conn.prepare(
        "SELECT si.source_type,
                COUNT(*) AS total,
                COUNT(CASE WHEN f.relevant = 1 THEN 1 END) AS engaged
         FROM source_items si
         LEFT JOIN feedback f ON f.source_item_id = si.id
         GROUP BY si.source_type
         HAVING total >= ?1",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Anti-pattern query failed");
            return vec![];
        }
    };

    let mut patterns = Vec::new();

    let rows = match stmt.query_map(params![MIN_EXPOSURE_COUNT], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Anti-pattern row iteration failed");
            return vec![];
        }
    };

    // Collect all source stats for cross-comparison
    let mut source_stats: Vec<(String, i64, i64)> = Vec::new();
    for row in rows.flatten() {
        source_stats.push(row);
    }

    // Compute global engagement rate for relative comparison
    let global_total: i64 = source_stats.iter().map(|(_, t, _)| t).sum();
    let global_engaged: i64 = source_stats.iter().map(|(_, _, e)| e).sum();
    let global_rate = if global_total > 0 {
        global_engaged as f32 / global_total as f32
    } else {
        0.0
    };

    for (source_type, total, engaged) in &source_stats {
        let engagement_rate = if *total > 0 {
            *engaged as f32 / *total as f32
        } else {
            0.0
        };

        // Over-scored: many items, almost no engagement
        if engagement_rate < OVER_SCORED_THRESHOLD && *total >= MIN_EXPOSURE_COUNT {
            // Penalty proportional to how much junk we're surfacing
            let suggested_penalty = (OVER_SCORED_THRESHOLD - engagement_rate).min(0.2);

            patterns.push(super::AntiPattern {
                pattern_type: "over_scored".to_string(),
                topic: source_type.clone(),
                avg_score: engagement_rate,
                engagement_count: *engaged,
                exposure_count: *total,
                suggested_penalty,
            });
        }

        // Under-scored: high engagement rate but very few items surfaced
        // Only flag if engagement rate is >50% and total items is in the bottom quartile
        if engagement_rate > 0.5
            && *total < 10
            && *engaged > 0
            && global_rate > 0.0
            && engagement_rate > global_rate * 2.0
        {
            // Negative penalty = boost suggestion
            let suggested_penalty = -(engagement_rate - global_rate).min(0.15);

            patterns.push(super::AntiPattern {
                pattern_type: "under_scored".to_string(),
                topic: source_type.clone(),
                avg_score: engagement_rate,
                engagement_count: *engaged,
                exposure_count: *total,
                suggested_penalty,
            });
        }
    }

    info!(
        target: "4da::autophagy",
        over_scored = patterns.iter().filter(|p| p.pattern_type == "over_scored").count(),
        under_scored = patterns.iter().filter(|p| p.pattern_type == "under_scored").count(),
        "Anti-pattern detection complete"
    );

    patterns
}

/// Store anti-patterns to `digested_intelligence`, superseding previous entries.
pub(crate) fn store_anti_patterns(
    conn: &Connection,
    patterns: &[super::AntiPattern],
) -> Result<()> {
    for pattern in patterns {
        let data = serde_json::to_string(&serde_json::json!({
            "pattern_type": pattern.pattern_type,
            "avg_score": pattern.avg_score,
            "engagement_count": pattern.engagement_count,
            "exposure_count": pattern.exposure_count,
            "suggested_penalty": pattern.suggested_penalty,
        }))?;

        let subject = format!("{}:{}", pattern.pattern_type, pattern.topic);

        // Insert new anti-pattern first, then point old rows at it.
        // This order satisfies the FK constraint on superseded_by -> digested_intelligence(id).
        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('anti_pattern', ?1, ?2, ?3, ?4)",
            params![
                subject,
                data,
                (pattern.exposure_count as f32 / 20.0).min(1.0),
                pattern.exposure_count,
            ],
        )
        .with_context(|| format!("Failed to insert anti-pattern for {subject}"))?;

        let new_id = conn.last_insert_rowid();

        // Supersede previous anti-patterns for this type+topic (excluding the one just inserted)
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = ?1
             WHERE digest_type = 'anti_pattern' AND subject = ?2 AND superseded_by IS NULL AND id != ?1",
            params![new_id, subject],
        )
        .with_context(|| format!("Failed to supersede anti-pattern for {subject}"))?;
    }

    debug!(target: "4da::autophagy", count = patterns.len(), "Stored anti-patterns");
    Ok(())
}

/// Load anti-pattern penalties (systematic scoring biases detected by autophagy).
/// Returns map of source_type -> suggested_penalty (-0.15 to +0.20).
///
/// Reads only non-superseded rows from `digested_intelligence` where
/// `digest_type = 'anti_pattern'`. The subject encodes "pattern_type:topic"
/// (e.g. "over_scored:hackernews"). Returns an empty map if no data exists.
pub fn load_anti_patterns(conn: &Connection) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    let Ok(mut stmt) = conn.prepare(
        "SELECT subject, data FROM digested_intelligence
         WHERE digest_type = 'anti_pattern' AND superseded_by IS NULL",
    ) else {
        return result;
    };

    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return result;
    };

    for row in rows.flatten() {
        let (subject, data) = row;
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(penalty) = parsed.get("suggested_penalty").and_then(|v| v.as_f64()) {
                // Subject format: "pattern_type:topic" -- extract the topic (source_type)
                let topic = subject.split(':').nth(1).unwrap_or(&subject).to_string();
                result.insert(topic, penalty as f32);
            }
        }
    }

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
    fn test_detect_anti_patterns_empty() {
        let conn = setup_test_db();
        let patterns = detect_anti_patterns(&conn, 0.35);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_detect_over_scored() {
        let conn = setup_test_db();

        // Insert 50 hackernews items with 0 engagement (over-scored)
        for i in 0..50 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title)
                 VALUES ('hackernews', ?1, ?2)",
                params![format!("hn_{}", i), format!("HN Story {}", i)],
            )
            .unwrap();
        }

        // Also insert 10 arxiv items with 6 engaged (not over-scored)
        for i in 0..10 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title)
                 VALUES ('arxiv', ?1, ?2)",
                params![format!("ax_{}", i), format!("Paper {}", i)],
            )
            .unwrap();
        }
        for i in 51..57 {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }

        let patterns = detect_anti_patterns(&conn, 0.35);

        // hackernews should be flagged as over_scored (0% engagement, 50 items)
        let over = patterns
            .iter()
            .find(|p| p.pattern_type == "over_scored" && p.topic == "hackernews");
        assert!(over.is_some(), "hackernews should be over_scored");
        assert_eq!(over.unwrap().exposure_count, 50);
    }

    #[test]
    fn test_detect_under_scored() {
        let conn = setup_test_db();

        // Source A: 100 items, 5 engaged (5% rate)
        for i in 0..100 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title)
                 VALUES ('hackernews', ?1, ?2)",
                params![format!("hn_{}", i), format!("Story {}", i)],
            )
            .unwrap();
        }
        for i in 1..=5 {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }

        // Source B: 6 items, 4 engaged (66% rate, but very few items = under-scored)
        for i in 0..6 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title)
                 VALUES ('lobsters', ?1, ?2)",
                params![format!("lb_{}", i), format!("Lobster {}", i)],
            )
            .unwrap();
        }
        for i in 101..=104 {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }

        let patterns = detect_anti_patterns(&conn, 0.35);

        let under = patterns
            .iter()
            .find(|p| p.pattern_type == "under_scored" && p.topic == "lobsters");
        assert!(under.is_some(), "lobsters should be under_scored");
        assert!(
            under.unwrap().suggested_penalty < 0.0,
            "under_scored penalty should be negative (boost)"
        );
    }

    #[test]
    fn test_store_anti_patterns() {
        let conn = setup_test_db();

        let patterns = vec![super::super::AntiPattern {
            pattern_type: "over_scored".to_string(),
            topic: "hackernews".to_string(),
            avg_score: 0.02,
            engagement_count: 1,
            exposure_count: 50,
            suggested_penalty: 0.03,
        }];

        store_anti_patterns(&conn, &patterns).expect("store");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM digested_intelligence WHERE digest_type = 'anti_pattern'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_load_anti_patterns_roundtrip() {
        let conn = setup_test_db();

        let patterns = vec![
            super::super::AntiPattern {
                pattern_type: "over_scored".to_string(),
                topic: "hackernews".to_string(),
                avg_score: 0.02,
                engagement_count: 1,
                exposure_count: 50,
                suggested_penalty: 0.03,
            },
            super::super::AntiPattern {
                pattern_type: "under_scored".to_string(),
                topic: "lobsters".to_string(),
                avg_score: 0.66,
                engagement_count: 4,
                exposure_count: 6,
                suggested_penalty: -0.12,
            },
        ];
        store_anti_patterns(&conn, &patterns).expect("store");

        let loaded = load_anti_patterns(&conn);
        assert_eq!(loaded.len(), 2);
        // Subject format is "over_scored:hackernews" -> topic extracted is "hackernews"
        assert!((loaded["hackernews"] - 0.03).abs() < 0.01);
        // Subject format is "under_scored:lobsters" -> topic extracted is "lobsters"
        assert!((loaded["lobsters"] - (-0.12)).abs() < 0.01);
    }

    #[test]
    fn test_load_anti_patterns_empty() {
        let conn = setup_test_db();
        let loaded = load_anti_patterns(&conn);
        assert!(loaded.is_empty());
    }
}
