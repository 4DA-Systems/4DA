// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Topic Hotness — cross-source signal consolidation.
//!
//! Tracks when the same topic appears across multiple sources within a time window.
//! When hotness crosses a threshold, the topic is "materialized" — it gets a
//! consolidated section in the briefing instead of N duplicate items.

use rusqlite::{params, Connection};
use tracing::{debug, info};

// ============================================================================
// Constants
// ============================================================================

const MATERIALIZE_THRESHOLD: f64 = 3.0;
const ARCHIVE_THRESHOLD: f64 = 0.5;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct HotTopic {
    pub topic_key: String,
    pub mention_count: i64,
    pub distinct_sources: i64,
    pub last_seen_at: i64,
    pub query_hits: i64,
    pub hotness_score: f64,
    pub materialized: bool,
    pub first_seen_at: i64,
}

// ============================================================================
// Hotness Computation
// ============================================================================

fn compute_hotness(
    mention_count: i64,
    distinct_sources: i64,
    last_seen_at: i64,
    query_hits: i64,
    now: i64,
) -> f64 {
    let mentions = (mention_count as f64 + 1.0).ln();
    let sources = 0.5 * distinct_sources as f64;
    let age_hours = (now - last_seen_at).max(0) as f64 / 3600.0;
    let recency = if age_hours <= 24.0 {
        1.0
    } else if age_hours <= 168.0 {
        0.5
    } else {
        // Decay penalty: score drops the longer the topic stays unseen
        -(age_hours / 168.0).min(2.0)
    };
    let queries = 2.0 * query_hits as f64;

    (mentions + sources + recency + queries).max(0.0)
}

// ============================================================================
// Recording
// ============================================================================

pub fn record_topic_mention(conn: &Connection, topic_key: &str, source_type: &str) {
    let now = now_unix();
    let normalized = normalize_topic_key(topic_key);
    if normalized.is_empty() || normalized.len() < 3 {
        return;
    }

    // Upsert topic row
    if let Err(e) = conn.execute(
        "INSERT INTO topic_hotness (topic_key, mention_count, distinct_sources, last_seen_at, query_hits, hotness_score, materialized, first_seen_at)
         VALUES (?1, 1, 1, ?2, 0, 0.0, 0, ?2)
         ON CONFLICT(topic_key) DO UPDATE SET
             mention_count = mention_count + 1,
             last_seen_at = ?2",
        params![normalized, now],
    ) {
        debug!(target: "4da::hotness", error = %e, topic = normalized, "Failed to upsert topic");
        return;
    }

    // Track distinct sources: only increment if this source hasn't seen this topic today
    let today_key = format!("{}:{}", normalized, source_type);
    let already_counted: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM topic_hotness_sources WHERE day_source_key = ?1",
            params![today_key],
            |row| row.get(0),
        )
        .unwrap_or(true);

    if !already_counted {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO topic_hotness_sources (day_source_key, topic_key, source_type, seen_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![today_key, normalized, source_type, now],
        );
        let _ = conn.execute(
            "UPDATE topic_hotness SET distinct_sources = (
                 SELECT COUNT(DISTINCT source_type) FROM topic_hotness_sources WHERE topic_key = ?1
             ) WHERE topic_key = ?1",
            params![normalized],
        );
    }
}

pub fn record_query_hit(conn: &Connection, topic_key: &str) {
    let normalized = normalize_topic_key(topic_key);
    let _ = conn.execute(
        "UPDATE topic_hotness SET query_hits = query_hits + 1 WHERE topic_key = ?1",
        params![normalized],
    );
}

// ============================================================================
// Rebuild & Materialization
// ============================================================================

pub fn rebuild_hotness(conn: &Connection) -> usize {
    let now = now_unix();

    let topics: Vec<(String, i64, i64, i64, i64)> = {
        let mut stmt = match conn.prepare(
            "SELECT topic_key, mention_count, distinct_sources, last_seen_at, query_hits FROM topic_hotness",
        ) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    };

    let mut updated = 0;
    for (key, mentions, sources, last_seen, queries) in &topics {
        let score = compute_hotness(*mentions, *sources, *last_seen, *queries, now);
        let materialized = if score >= MATERIALIZE_THRESHOLD { 1 } else { 0 };

        if let Ok(changed) = conn.execute(
            "UPDATE topic_hotness SET hotness_score = ?1, materialized = ?2 WHERE topic_key = ?3",
            params![score, materialized, key],
        ) {
            if changed > 0 {
                updated += 1;
            }
        }
    }

    // Archive cold topics (hotness below threshold, not seen in 7+ days)
    let archive_cutoff = now - 7 * 86400;
    let archived = conn
        .execute(
            "DELETE FROM topic_hotness WHERE hotness_score < ?1 AND last_seen_at < ?2 AND materialized = 0",
            params![ARCHIVE_THRESHOLD, archive_cutoff],
        )
        .unwrap_or(0);

    // Clean up old source tracking entries (>7 days)
    let _ = conn.execute(
        "DELETE FROM topic_hotness_sources WHERE seen_at < ?1",
        params![archive_cutoff],
    );

    if updated > 0 || archived > 0 {
        debug!(
            target: "4da::hotness",
            updated,
            archived,
            total = topics.len(),
            "Hotness rebuild complete"
        );
    }

    updated
}

// ============================================================================
// Queries
// ============================================================================

pub fn get_hot_topics(conn: &Connection, limit: usize) -> Vec<HotTopic> {
    let mut stmt = match conn.prepare(
        "SELECT topic_key, mention_count, distinct_sources, last_seen_at, query_hits, hotness_score, materialized, first_seen_at
         FROM topic_hotness
         WHERE materialized = 1
         ORDER BY hotness_score DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![limit as i64], |row| {
        Ok(HotTopic {
            topic_key: row.get(0)?,
            mention_count: row.get(1)?,
            distinct_sources: row.get(2)?,
            last_seen_at: row.get(3)?,
            query_hits: row.get(4)?,
            hotness_score: row.get(5)?,
            materialized: row.get::<_, i64>(6)? == 1,
            first_seen_at: row.get(7)?,
        })
    })
    .ok()
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

pub fn get_all_tracked(conn: &Connection) -> Vec<HotTopic> {
    let mut stmt = match conn.prepare(
        "SELECT topic_key, mention_count, distinct_sources, last_seen_at, query_hits, hotness_score, materialized, first_seen_at
         FROM topic_hotness
         ORDER BY hotness_score DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| {
        Ok(HotTopic {
            topic_key: row.get(0)?,
            mention_count: row.get(1)?,
            distinct_sources: row.get(2)?,
            last_seen_at: row.get(3)?,
            query_hits: row.get(4)?,
            hotness_score: row.get(5)?,
            materialized: row.get::<_, i64>(6)? == 1,
            first_seen_at: row.get(7)?,
        })
    })
    .ok()
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

// ============================================================================
// Helpers
// ============================================================================

fn normalize_topic_key(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("-")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE topic_hotness (
                topic_key TEXT PRIMARY KEY,
                mention_count INTEGER NOT NULL DEFAULT 0,
                distinct_sources INTEGER NOT NULL DEFAULT 0,
                last_seen_at INTEGER NOT NULL,
                query_hits INTEGER NOT NULL DEFAULT 0,
                hotness_score REAL NOT NULL DEFAULT 0.0,
                materialized INTEGER NOT NULL DEFAULT 0,
                first_seen_at INTEGER NOT NULL
            );
            CREATE TABLE topic_hotness_sources (
                day_source_key TEXT PRIMARY KEY,
                topic_key TEXT NOT NULL,
                source_type TEXT NOT NULL,
                seen_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_single_mention_below_threshold() {
        let conn = setup_db();
        record_topic_mention(&conn, "Rust 1.80", "hn");
        rebuild_hotness(&conn);

        let hot = get_hot_topics(&conn, 10);
        assert!(hot.is_empty(), "Single mention should not materialize");
    }

    #[test]
    fn test_multi_source_materializes() {
        let conn = setup_db();
        // Same topic from 4 different sources
        record_topic_mention(&conn, "Rust 1.80", "hn");
        record_topic_mention(&conn, "Rust 1.80", "reddit");
        record_topic_mention(&conn, "Rust 1.80", "rss");
        record_topic_mention(&conn, "Rust 1.80", "lobsters");
        // Extra mentions
        record_topic_mention(&conn, "Rust 1.80", "hn");
        record_topic_mention(&conn, "Rust 1.80", "reddit");

        rebuild_hotness(&conn);

        let hot = get_hot_topics(&conn, 10);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].topic_key, "rust-180");
        assert!(hot[0].materialized);
        assert_eq!(hot[0].distinct_sources, 4);
    }

    #[test]
    fn test_hotness_formula() {
        let now = now_unix();
        // 6 mentions, 3 sources, seen now, 1 query hit
        let score = compute_hotness(6, 3, now, 1, now);
        // ln(7) + 0.5*3 + 1.0 + 2.0*1 = 1.94 + 1.5 + 1.0 + 2.0 = 6.44
        assert!(score > 6.0 && score < 7.0, "score={score}");
    }

    #[test]
    fn test_recency_decay() {
        let now = now_unix();
        let recent = compute_hotness(3, 2, now, 0, now);
        let stale = compute_hotness(3, 2, now - 200 * 3600, 0, now); // 200 hours ago

        assert!(
            recent > stale,
            "Recent should score higher: {recent} vs {stale}"
        );
        // Recent gets recency=1.0, stale gets decay penalty — significant gap
        assert!(
            recent - stale > 1.5,
            "Decay gap should be >1.5: {}",
            recent - stale
        );
    }

    #[test]
    fn test_normalize_topic_key() {
        assert_eq!(normalize_topic_key("Rust 1.80"), "rust-180");
        assert_eq!(
            normalize_topic_key("React Server Components"),
            "react-server-components"
        );
        assert_eq!(normalize_topic_key("CVE-2024-1234"), "cve-2024-1234");
    }

    #[test]
    fn test_query_hits_boost() {
        let now = now_unix();
        let no_queries = compute_hotness(3, 2, now, 0, now);
        let with_queries = compute_hotness(3, 2, now, 3, now);
        assert!((with_queries - no_queries - 6.0).abs() < 0.01); // 2.0 * 3 = 6.0
    }

    #[test]
    fn test_archive_old_cold_topics() {
        let conn = setup_db();
        let old = now_unix() - 10 * 86400; // 10 days ago

        conn.execute(
            "INSERT INTO topic_hotness (topic_key, mention_count, distinct_sources, last_seen_at, query_hits, hotness_score, materialized, first_seen_at)
             VALUES ('old-topic', 1, 1, ?1, 0, 0.3, 0, ?1)",
            params![old],
        )
        .unwrap();

        rebuild_hotness(&conn);

        let all = get_all_tracked(&conn);
        assert!(all.is_empty(), "Cold old topic should be archived");
    }
}
