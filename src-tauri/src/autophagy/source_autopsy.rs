//! Source autopsy — per-source engagement quality analysis.
//!
//! Computes engagement rates for each source_type, revealing which sources
//! consistently deliver content users actually interact with.

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Analyze sources: compute engagement rates per source_type within the analysis window.
///
/// For each source_type, counts total items surfaced vs items that received positive
/// feedback. This reveals source quality and helps the scoring pipeline weight sources.
pub(crate) fn analyze_sources(conn: &Connection, max_age_days: i64) -> Vec<super::SourceAutopsy> {
    let age_param = format!("-{} days", max_age_days);

    // Count items per source_type within the retention window
    let mut item_stmt = match conn.prepare(
        "SELECT source_type, COUNT(*) AS total
         FROM source_items
         WHERE last_seen >= datetime('now', ?1)
         GROUP BY source_type",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Source autopsy item query failed");
            return vec![];
        }
    };

    let mut source_totals: HashMap<String, i64> = HashMap::new();
    if let Ok(rows) = item_stmt.query_map(params![age_param], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) {
        for row in rows.flatten() {
            source_totals.insert(row.0, row.1);
        }
    }

    if source_totals.is_empty() {
        debug!(target: "4da::autophagy", "No source items for autopsy");
        return vec![];
    }

    // Count engaged items per source_type (have positive feedback)
    let mut engaged_stmt = match conn.prepare(
        "SELECT si.source_type, COUNT(DISTINCT si.id) AS engaged
         FROM source_items si
         JOIN feedback f ON f.source_item_id = si.id
         WHERE f.relevant = 1
           AND si.last_seen >= datetime('now', ?1)
         GROUP BY si.source_type",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Source autopsy engagement query failed");
            return vec![];
        }
    };

    let mut source_engaged: HashMap<String, i64> = HashMap::new();
    if let Ok(rows) = engaged_stmt.query_map(params![age_param], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) {
        for row in rows.flatten() {
            source_engaged.insert(row.0, row.1);
        }
    }

    // Build autopsies
    let mut autopsies = Vec::new();

    for (source_type, total) in &source_totals {
        let engaged = source_engaged.get(source_type).copied().unwrap_or(0);
        let engagement_rate = if *total > 0 {
            engaged as f32 / *total as f32
        } else {
            0.0
        };

        autopsies.push(super::SourceAutopsy {
            source_type: source_type.clone(),
            // Use source_type as the topic since we group by source
            topic: source_type.clone(),
            items_surfaced: *total,
            items_engaged: engaged,
            engagement_rate,
        });
    }

    info!(
        target: "4da::autophagy",
        sources = autopsies.len(),
        "Source autopsy complete"
    );

    autopsies
}

/// Store source autopsies to `digested_intelligence`, superseding previous entries.
pub(crate) fn store_source_autopsies(
    conn: &Connection,
    autopsies: &[super::SourceAutopsy],
) -> Result<(), String> {
    for autopsy in autopsies {
        let data = serde_json::to_string(&serde_json::json!({
            "source_type": autopsy.source_type,
            "items_surfaced": autopsy.items_surfaced,
            "items_engaged": autopsy.items_engaged,
            "engagement_rate": autopsy.engagement_rate,
        }))
        .map_err(|e| e.to_string())?;

        let subject = format!("{}:{}", autopsy.source_type, autopsy.topic);

        // Supersede previous autopsy for this source+topic
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = (SELECT COALESCE(MAX(id), 0) + 1 FROM digested_intelligence)
             WHERE digest_type = 'source_autopsy' AND subject = ?1 AND superseded_by IS NULL",
            params![subject],
        )
        .map_err(|e| format!("Failed to supersede source autopsy for {}: {}", subject, e))?;

        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('source_autopsy', ?1, ?2, ?3, ?4)",
            params![
                subject,
                data,
                (autopsy.items_surfaced as f32 / 50.0).min(1.0),
                autopsy.items_surfaced,
            ],
        )
        .map_err(|e| format!("Failed to insert source autopsy for {}: {}", subject, e))?;
    }

    debug!(target: "4da::autophagy", count = autopsies.len(), "Stored source autopsies");
    Ok(())
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
    fn test_analyze_sources_empty() {
        let conn = setup_test_db();
        let autopsies = analyze_sources(&conn, 30);
        assert!(autopsies.is_empty());
    }

    #[test]
    fn test_analyze_sources_with_data() {
        let conn = setup_test_db();

        // Insert items for two sources
        for i in 0..20 {
            let source = if i < 15 { "hackernews" } else { "arxiv" };
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, last_seen)
                 VALUES (?1, ?2, ?3, datetime('now', '-5 days'))",
                params![source, format!("id_{}", i), format!("Item {}", i)],
            )
            .unwrap();
        }

        // 5 of 15 HN items engaged, 3 of 5 arxiv items engaged
        for i in [1, 3, 5, 7, 9] {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }
        for i in [16, 17, 18] {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }

        let autopsies = analyze_sources(&conn, 30);
        assert_eq!(autopsies.len(), 2);

        let hn = autopsies
            .iter()
            .find(|a| a.source_type == "hackernews")
            .unwrap();
        assert_eq!(hn.items_surfaced, 15);
        assert_eq!(hn.items_engaged, 5);
        assert!((hn.engagement_rate - (5.0 / 15.0)).abs() < 0.01);

        let arxiv = autopsies.iter().find(|a| a.source_type == "arxiv").unwrap();
        assert_eq!(arxiv.items_surfaced, 5);
        assert_eq!(arxiv.items_engaged, 3);
        assert!((arxiv.engagement_rate - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_store_source_autopsies() {
        let conn = setup_test_db();

        let autopsies = vec![super::super::SourceAutopsy {
            source_type: "hackernews".to_string(),
            topic: "hackernews".to_string(),
            items_surfaced: 100,
            items_engaged: 15,
            engagement_rate: 0.15,
        }];

        store_source_autopsies(&conn, &autopsies).expect("store");

        // Verify stored
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM digested_intelligence WHERE digest_type = 'source_autopsy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
