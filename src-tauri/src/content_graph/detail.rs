// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Per-node detail lookup for the content graph side panel.
//!
//! The graph payload stays lean (story members cross the IPC boundary as ids +
//! capped titles); when the user selects a node, this keyed lookup hydrates the
//! representative and every collapsed member with the fields the detail panel
//! needs — url, source, time, and the dependency that grounds the gold ring.
//! This is a detail fetch for items already surfaced by `build_content_graph`,
//! not a ranked feed, so it does not route through the Evidence Materializer
//! (doctrine rule 4 covers commands that *rank* intelligence).

use serde::Serialize;

use crate::error::Result;

/// Hard cap on ids per lookup — the largest observed story is ~30 members;
/// 64 leaves headroom without letting a caller turn this into a table scan.
const MAX_IDS: usize = 64;

#[derive(Debug, Serialize)]
pub struct GraphNodeDetail {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub created_at: String,
    /// Highest-confidence dependency link — why this touches the user's stack.
    pub matched_package: Option<String>,
    /// Cached AI summary, if one was already generated for this item.
    pub summary: Option<String>,
}

pub(super) fn fetch_node_details(
    conn: &rusqlite::Connection,
    item_ids: &[i64],
) -> Result<Vec<GraphNodeDetail>> {
    if item_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = item_ids.iter().copied().take(MAX_IDS).collect();

    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.summary,
                (SELECT sid.package_name FROM source_item_dependencies sid
                 WHERE sid.source_item_id = si.id
                 ORDER BY sid.confidence DESC, sid.id LIMIT 1) AS matched_package
         FROM source_items si
         WHERE si.id IN ({placeholders})
         ORDER BY si.relevance_score DESC"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
        Ok(GraphNodeDetail {
            id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            source_type: row.get(3)?,
            relevance_score: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0) as f32,
            created_at: row.get(5)?,
            summary: row.get(6)?,
            matched_package: row.get(7)?,
        })
    })?;

    let mut details = Vec::new();
    for row in rows {
        details.push(row?);
    }
    Ok(details)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_items (
                 id INTEGER PRIMARY KEY,
                 title TEXT NOT NULL,
                 url TEXT,
                 source_type TEXT NOT NULL,
                 relevance_score REAL,
                 created_at TEXT NOT NULL,
                 summary TEXT
             );
             CREATE TABLE source_item_dependencies (
                 id INTEGER PRIMARY KEY,
                 source_item_id INTEGER NOT NULL,
                 package_name TEXT NOT NULL,
                 confidence REAL NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    fn insert_item(conn: &rusqlite::Connection, id: i64, title: &str, score: f64) {
        conn.execute(
            "INSERT INTO source_items (id, title, url, source_type, relevance_score, created_at)
             VALUES (?1, ?2, ?3, 'osv', ?4, '2026-07-16T00:00:00Z')",
            rusqlite::params![id, title, format!("https://example.com/{id}"), score],
        )
        .unwrap();
    }

    #[test]
    fn empty_ids_returns_empty() {
        let conn = test_conn();
        assert!(fetch_node_details(&conn, &[]).unwrap().is_empty());
    }

    #[test]
    fn fetches_members_sorted_by_relevance() {
        let conn = test_conn();
        insert_item(&conn, 1, "low", 0.3);
        insert_item(&conn, 2, "high", 0.9);
        insert_item(&conn, 3, "mid", 0.6);

        let details = fetch_node_details(&conn, &[1, 2, 3]).unwrap();
        let titles: Vec<&str> = details.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["high", "mid", "low"]);
        assert_eq!(details[0].url.as_deref(), Some("https://example.com/2"));
    }

    #[test]
    fn picks_highest_confidence_package() {
        let conn = test_conn();
        insert_item(&conn, 1, "advisory", 0.8);
        conn.execute_batch(
            "INSERT INTO source_item_dependencies (source_item_id, package_name, confidence)
             VALUES (1, 'lodash', 0.4), (1, 'tokio', 0.9);",
        )
        .unwrap();

        let details = fetch_node_details(&conn, &[1]).unwrap();
        assert_eq!(details[0].matched_package.as_deref(), Some("tokio"));
    }

    #[test]
    fn missing_ids_are_skipped_and_cap_holds() {
        let conn = test_conn();
        insert_item(&conn, 100, "only", 0.5);
        let mut ids: Vec<i64> = (1..=99).collect();
        ids.push(100);
        // 100 sits beyond the MAX_IDS window (only ids 1..=64 are queried),
        // and none of those exist — so nothing comes back.
        assert!(fetch_node_details(&conn, &ids).unwrap().is_empty());
        // Unknown ids are skipped silently, known ones still resolve.
        assert_eq!(fetch_node_details(&conn, &[100, 999]).unwrap().len(), 1);
    }
}
