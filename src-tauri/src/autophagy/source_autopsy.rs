// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source autopsy — per-source engagement quality analysis.
//!
//! Computes engagement rates for each source_type, revealing which sources
//! consistently deliver content users actually interact with.

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

/// Analyze sources: compute engagement rates per source_type within the analysis window.
///
/// For each source_type, counts total items surfaced vs items that received positive
/// feedback. This reveals source quality and helps the scoring pipeline weight sources.
pub(crate) fn analyze_sources(conn: &Connection, max_age_days: i64) -> Vec<super::SourceAutopsy> {
    let age_param = format!("-{max_age_days} days");

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
/// Uses a transaction to batch all writes for performance.
pub(crate) fn store_source_autopsies(
    conn: &Connection,
    autopsies: &[super::SourceAutopsy],
) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .context("Failed to begin transaction for source autopsies")?;

    for autopsy in autopsies {
        let data = serde_json::to_string(&serde_json::json!({
            "source_type": autopsy.source_type,
            "items_surfaced": autopsy.items_surfaced,
            "items_engaged": autopsy.items_engaged,
            "engagement_rate": autopsy.engagement_rate,
        }))?;

        let subject = format!("{}:{}", autopsy.source_type, autopsy.topic);

        // Insert new autopsy first, then point old rows at it.
        // This order satisfies the FK constraint on superseded_by -> digested_intelligence(id).
        tx.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('source_autopsy', ?1, ?2, ?3, ?4)",
            params![
                subject,
                data,
                (autopsy.items_surfaced as f32 / 50.0).min(1.0),
                autopsy.items_surfaced,
            ],
        )
        .with_context(|| format!("Failed to insert source autopsy for {subject}"))?;

        let new_id = tx.last_insert_rowid();

        // Supersede previous autopsies for this source+topic (excluding the one just inserted)
        tx.execute(
            "UPDATE digested_intelligence
             SET superseded_by = ?1
             WHERE digest_type = 'source_autopsy' AND subject = ?2 AND superseded_by IS NULL AND id != ?1",
            params![new_id, subject],
        )
        .with_context(|| format!("Failed to supersede source autopsy for {subject}"))?;
    }

    tx.commit().context("Failed to commit source autopsies")?;

    debug!(target: "4da::autophagy", count = autopsies.len(), "Stored source autopsies");
    Ok(())
}

/// Load the latest source autopsy data (per-source engagement rates).
/// Returns map of source_type -> engagement_rate (0.0-1.0).
///
/// Reads only non-superseded rows from `digested_intelligence` where
/// `digest_type = 'source_autopsy'`. Returns an empty map if no data exists
/// yet (autophagy needs 7+ days of user interaction to produce results).
pub fn load_source_autopsies(conn: &Connection) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    let Ok(mut stmt) = conn.prepare(
        "SELECT subject, data FROM digested_intelligence
         WHERE digest_type = 'source_autopsy' AND superseded_by IS NULL",
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
        // Parse JSON data to extract engagement_rate
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(rate) = parsed.get("engagement_rate").and_then(|v| v.as_f64()) {
                // Subject format: "source_type:topic" -- extract source_type
                let source_type = subject.split(':').next().unwrap_or(&subject).to_string();
                result.insert(source_type, rate as f32);
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

    #[test]
    fn test_load_source_autopsies_roundtrip() {
        let conn = setup_test_db();

        // Store two autopsies
        let autopsies = vec![
            super::super::SourceAutopsy {
                source_type: "hackernews".to_string(),
                topic: "hackernews".to_string(),
                items_surfaced: 100,
                items_engaged: 15,
                engagement_rate: 0.15,
            },
            super::super::SourceAutopsy {
                source_type: "arxiv".to_string(),
                topic: "arxiv".to_string(),
                items_surfaced: 50,
                items_engaged: 30,
                engagement_rate: 0.60,
            },
        ];
        store_source_autopsies(&conn, &autopsies).expect("store");

        // Load and verify
        let loaded = load_source_autopsies(&conn);
        assert_eq!(loaded.len(), 2);
        assert!((loaded["hackernews"] - 0.15).abs() < 0.01);
        assert!((loaded["arxiv"] - 0.60).abs() < 0.01);
    }

    #[test]
    fn test_load_source_autopsies_empty() {
        let conn = setup_test_db();
        let loaded = load_source_autopsies(&conn);
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_source_autopsies_superseded_excluded() {
        let conn = setup_test_db();

        // Store first batch
        let autopsies_v1 = vec![super::super::SourceAutopsy {
            source_type: "hackernews".to_string(),
            topic: "hackernews".to_string(),
            items_surfaced: 100,
            items_engaged: 5,
            engagement_rate: 0.05,
        }];
        store_source_autopsies(&conn, &autopsies_v1).expect("store v1");

        // Store second batch (supersedes v1)
        let autopsies_v2 = vec![super::super::SourceAutopsy {
            source_type: "hackernews".to_string(),
            topic: "hackernews".to_string(),
            items_surfaced: 200,
            items_engaged: 40,
            engagement_rate: 0.20,
        }];
        store_source_autopsies(&conn, &autopsies_v2).expect("store v2");

        // Should load only the latest (v2)
        let loaded = load_source_autopsies(&conn);
        assert_eq!(loaded.len(), 1);
        assert!((loaded["hackernews"] - 0.20).abs() < 0.01);
    }
}
