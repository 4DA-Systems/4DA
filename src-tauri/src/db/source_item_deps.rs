// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source-item dependency links — durable, typed links between ingested content and user deps.
//!
//! Replaces fragile `title LIKE '%dep_name%'` matching in blind spots and other
//! intelligence surfaces with first-class evidence links carrying match type and confidence.

use rusqlite::{params, Result as SqliteResult};

use super::Database;

// ============================================================================
// Types
// ============================================================================

/// A dependency link from a source item to a package.
#[derive(Debug, Clone)]
pub struct SourceItemDep {
    pub id: i64,
    pub source_item_id: i64,
    pub package_name: String,
    pub ecosystem: Option<String>,
    pub match_type: String,
    pub confidence: f64,
    pub evidence_text: Option<String>,
    pub source_url: Option<String>,
}

// ============================================================================
// Database Operations
// ============================================================================

impl Database {
    /// Insert a dependency link for a source item.
    /// Upserts on (source_item_id, package_name, COALESCE(ecosystem, '')):
    /// keeps the higher confidence and its match_type.
    pub fn link_source_item_dep(
        &self,
        source_item_id: i64,
        package_name: &str,
        ecosystem: Option<&str>,
        match_type: &str,
        confidence: f64,
        evidence_text: Option<&str>,
        source_url: Option<&str>,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_item_id, package_name, COALESCE(ecosystem, ''))
             DO UPDATE SET
                match_type = CASE WHEN excluded.confidence > source_item_dependencies.confidence
                             THEN excluded.match_type
                             ELSE source_item_dependencies.match_type END,
                confidence = MAX(source_item_dependencies.confidence, excluded.confidence),
                evidence_text = CASE
                    WHEN excluded.confidence > source_item_dependencies.confidence
                    THEN COALESCE(NULLIF(excluded.evidence_text, ''), source_item_dependencies.evidence_text)
                    ELSE COALESCE(NULLIF(source_item_dependencies.evidence_text, ''), NULLIF(excluded.evidence_text, ''))
                END,
                source_url = COALESCE(NULLIF(source_item_dependencies.source_url, ''), NULLIF(excluded.source_url, ''))
             WHERE excluded.confidence > source_item_dependencies.confidence
                OR ((source_item_dependencies.evidence_text IS NULL OR source_item_dependencies.evidence_text = '')
                    AND excluded.evidence_text IS NOT NULL AND excluded.evidence_text <> '')
                OR ((source_item_dependencies.source_url IS NULL OR source_item_dependencies.source_url = '')
                    AND excluded.source_url IS NOT NULL AND excluded.source_url <> '')",
            params![source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Batch-link multiple deps in a single connection lock.
    /// Each tuple: (source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url).
    pub fn batch_link_source_item_deps(
        &self,
        links: &[(
            i64,
            &str,
            Option<&str>,
            &str,
            f64,
            Option<&str>,
            Option<&str>,
        )],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_item_id, package_name, COALESCE(ecosystem, ''))
             DO UPDATE SET
                match_type = CASE WHEN excluded.confidence > source_item_dependencies.confidence
                             THEN excluded.match_type
                             ELSE source_item_dependencies.match_type END,
                confidence = MAX(source_item_dependencies.confidence, excluded.confidence),
                evidence_text = CASE
                    WHEN excluded.confidence > source_item_dependencies.confidence
                    THEN COALESCE(NULLIF(excluded.evidence_text, ''), source_item_dependencies.evidence_text)
                    ELSE COALESCE(NULLIF(source_item_dependencies.evidence_text, ''), NULLIF(excluded.evidence_text, ''))
                END,
                source_url = COALESCE(NULLIF(source_item_dependencies.source_url, ''), NULLIF(excluded.source_url, ''))
             WHERE excluded.confidence > source_item_dependencies.confidence
                OR ((source_item_dependencies.evidence_text IS NULL OR source_item_dependencies.evidence_text = '')
                    AND excluded.evidence_text IS NOT NULL AND excluded.evidence_text <> '')
                OR ((source_item_dependencies.source_url IS NULL OR source_item_dependencies.source_url = '')
                    AND excluded.source_url IS NOT NULL AND excluded.source_url <> '')",
        )?;
        let mut count = 0;
        for (sid, pkg, eco, mt, conf, ev, url) in links {
            if stmt
                .execute(params![sid, pkg, eco, mt, conf, ev, url])
                .is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get all dependency links for a source item.
    pub fn get_deps_for_source_item(
        &self,
        source_item_id: i64,
    ) -> SqliteResult<Vec<SourceItemDep>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url
             FROM source_item_dependencies WHERE source_item_id = ?1",
        )?;
        let rows = stmt.query_map(params![source_item_id], |row| {
            Ok(SourceItemDep {
                id: row.get(0)?,
                source_item_id: row.get(1)?,
                package_name: row.get(2)?,
                ecosystem: row.get(3)?,
                match_type: row.get(4)?,
                confidence: row.get(5)?,
                evidence_text: row.get(6)?,
                source_url: row.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all source items linked to a specific package, optionally filtered by ecosystem.
    /// Results ordered by confidence descending.
    pub fn get_source_items_for_package(
        &self,
        package_name: &str,
        ecosystem: Option<&str>,
        min_confidence: f64,
    ) -> SqliteResult<Vec<SourceItemDep>> {
        let conn = self.read_conn();
        let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match ecosystem {
            Some(eco) => (
                "SELECT id, source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url
                 FROM source_item_dependencies
                 WHERE LOWER(REPLACE(package_name, '-', '_')) = LOWER(REPLACE(?1, '-', '_'))
                   AND ecosystem = ?2 AND confidence >= ?3
                 ORDER BY confidence DESC",
                vec![
                    Box::new(package_name.to_string()),
                    Box::new(eco.to_string()),
                    Box::new(min_confidence),
                ],
            ),
            None => (
                "SELECT id, source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url
                 FROM source_item_dependencies
                 WHERE LOWER(REPLACE(package_name, '-', '_')) = LOWER(REPLACE(?1, '-', '_'))
                   AND confidence >= ?2
                 ORDER BY confidence DESC",
                vec![
                    Box::new(package_name.to_string()),
                    Box::new(min_confidence),
                ],
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            Ok(SourceItemDep {
                id: row.get(0)?,
                source_item_id: row.get(1)?,
                package_name: row.get(2)?,
                ecosystem: row.get(3)?,
                match_type: row.get(4)?,
                confidence: row.get(5)?,
                evidence_text: row.get(6)?,
                source_url: row.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count dependency links by match type (for diagnostics).
    pub fn count_source_item_deps_by_type(&self) -> SqliteResult<Vec<(String, i64)>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT match_type, COUNT(*) FROM source_item_dependencies GROUP BY match_type ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    /// Helper: insert a minimal source_item row and return its id.
    fn insert_source_item(db: &crate::db::Database, title: &str) -> i64 {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding)
             VALUES ('test', ?1, NULL, ?1, '', ?1, X'')",
            rusqlite::params![title],
        )
        .expect("insert source_item");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_link_and_retrieve() {
        let db = test_db();
        let sid = insert_source_item(&db, "tokio vulnerability found");

        let row_id = db
            .link_source_item_dep(
                sid,
                "tokio",
                Some("crates.io"),
                "title_heuristic",
                0.7,
                Some("title mentions tokio"),
                None,
            )
            .expect("link should succeed");
        assert!(row_id > 0);

        let deps = db
            .get_deps_for_source_item(sid)
            .expect("query should succeed");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].package_name, "tokio");
        assert_eq!(deps[0].ecosystem.as_deref(), Some("crates.io"));
        assert_eq!(deps[0].match_type, "title_heuristic");
        assert!((deps[0].confidence - 0.7).abs() < f64::EPSILON);
        assert_eq!(
            deps[0].evidence_text.as_deref(),
            Some("title mentions tokio")
        );
    }

    #[test]
    fn test_upsert_upgrades_confidence() {
        let db = test_db();
        let sid = insert_source_item(&db, "react update article");

        // First insert at low confidence
        db.link_source_item_dep(
            sid,
            "react",
            Some("npm"),
            "title_heuristic",
            0.3,
            None,
            None,
        )
        .expect("first link");

        // Second insert at higher confidence with different match_type
        db.link_source_item_dep(
            sid,
            "react",
            Some("npm"),
            "llm_analysis",
            0.8,
            Some("LLM confirmed"),
            Some("https://example.com/react"),
        )
        .expect("upsert link");

        let deps = db.get_deps_for_source_item(sid).expect("query");
        assert_eq!(deps.len(), 1, "upsert should not create duplicate");
        assert!(
            (deps[0].confidence - 0.8).abs() < f64::EPSILON,
            "confidence should upgrade to 0.8"
        );
        assert_eq!(
            deps[0].match_type, "llm_analysis",
            "match_type should follow higher confidence"
        );
        assert_eq!(deps[0].evidence_text.as_deref(), Some("LLM confirmed"));
        assert_eq!(
            deps[0].source_url.as_deref(),
            Some("https://example.com/react")
        );
    }

    #[test]
    fn test_upsert_keeps_higher_existing() {
        let db = test_db();
        let sid = insert_source_item(&db, "serde deep dive");

        // First insert at high confidence
        db.link_source_item_dep(
            sid,
            "serde",
            Some("crates.io"),
            "llm_analysis",
            0.9,
            None,
            None,
        )
        .expect("first link");

        // Second insert at lower confidence
        db.link_source_item_dep(
            sid,
            "serde",
            Some("crates.io"),
            "title_heuristic",
            0.4,
            None,
            None,
        )
        .expect("upsert link");

        let deps = db.get_deps_for_source_item(sid).expect("query");
        assert_eq!(deps.len(), 1);
        assert!(
            (deps[0].confidence - 0.9).abs() < f64::EPSILON,
            "higher existing confidence should be kept"
        );
        assert_eq!(
            deps[0].match_type, "llm_analysis",
            "match_type from higher confidence should be kept"
        );
    }

    #[test]
    fn test_get_source_items_for_package() {
        let db = test_db();
        let sid1 = insert_source_item(&db, "article about tokio");
        let sid2 = insert_source_item(&db, "another tokio post");
        let sid3 = insert_source_item(&db, "react hooks guide");

        db.link_source_item_dep(
            sid1,
            "tokio",
            Some("crates.io"),
            "title_heuristic",
            0.7,
            None,
            None,
        )
        .expect("link 1");
        db.link_source_item_dep(
            sid2,
            "tokio",
            Some("crates.io"),
            "llm_analysis",
            0.9,
            None,
            None,
        )
        .expect("link 2");
        db.link_source_item_dep(
            sid3,
            "react",
            Some("npm"),
            "title_heuristic",
            0.6,
            None,
            None,
        )
        .expect("link 3");

        // Query tokio in crates.io ecosystem
        let tokio_deps = db
            .get_source_items_for_package("tokio", Some("crates.io"), 0.0)
            .expect("query tokio");
        assert_eq!(tokio_deps.len(), 2);
        // Should be ordered by confidence DESC
        assert!((tokio_deps[0].confidence - 0.9).abs() < f64::EPSILON);
        assert!((tokio_deps[1].confidence - 0.7).abs() < f64::EPSILON);

        // Query with min_confidence filter
        let high_only = db
            .get_source_items_for_package("tokio", Some("crates.io"), 0.8)
            .expect("query high confidence");
        assert_eq!(high_only.len(), 1);

        // Query across all ecosystems
        let all_tokio = db
            .get_source_items_for_package("tokio", None, 0.0)
            .expect("query all ecosystems");
        assert_eq!(all_tokio.len(), 2);

        // Query react should not return tokio
        let react_deps = db
            .get_source_items_for_package("react", None, 0.0)
            .expect("query react");
        assert_eq!(react_deps.len(), 1);
        assert_eq!(react_deps[0].source_item_id, sid3);
    }

    #[test]
    fn test_count_by_type() {
        let db = test_db();
        let sid1 = insert_source_item(&db, "item 1");
        let sid2 = insert_source_item(&db, "item 2");
        let sid3 = insert_source_item(&db, "item 3");

        db.link_source_item_dep(sid1, "tokio", None, "title_heuristic", 0.5, None, None)
            .expect("link 1");
        db.link_source_item_dep(sid2, "serde", None, "title_heuristic", 0.6, None, None)
            .expect("link 2");
        db.link_source_item_dep(sid3, "react", None, "llm_analysis", 0.8, None, None)
            .expect("link 3");

        let counts = db.count_source_item_deps_by_type().expect("count query");
        assert_eq!(counts.len(), 2);
        // title_heuristic should come first (count 2 > count 1)
        assert_eq!(counts[0].0, "title_heuristic");
        assert_eq!(counts[0].1, 2);
        assert_eq!(counts[1].0, "llm_analysis");
        assert_eq!(counts[1].1, 1);
    }

    #[test]
    fn test_batch_link() {
        let db = test_db();
        let sid = insert_source_item(&db, "multi-dep article");

        let links: Vec<(
            i64,
            &str,
            Option<&str>,
            &str,
            f64,
            Option<&str>,
            Option<&str>,
        )> = vec![
            (
                sid,
                "tokio",
                Some("crates.io"),
                "title_heuristic",
                0.7,
                None,
                None,
            ),
            (
                sid,
                "serde",
                Some("crates.io"),
                "title_heuristic",
                0.6,
                None,
                None,
            ),
            (sid, "react", Some("npm"), "llm_analysis", 0.9, None, None),
        ];

        let count = db.batch_link_source_item_deps(&links).expect("batch link");
        assert_eq!(count, 3);

        let deps = db.get_deps_for_source_item(sid).expect("query");
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn test_cascade_delete() {
        let db = test_db();
        let sid = insert_source_item(&db, "will be deleted");

        db.link_source_item_dep(sid, "tokio", None, "title_heuristic", 0.5, None, None)
            .expect("link");

        // Delete the source item — FK cascade should remove the dep link
        let conn = db.conn.lock();
        conn.execute(
            "DELETE FROM source_items WHERE id = ?1",
            rusqlite::params![sid],
        )
        .expect("delete source item");
        drop(conn);

        let deps = db
            .get_deps_for_source_item(sid)
            .expect("query after delete");
        assert!(deps.is_empty(), "cascade delete should remove dep links");
    }

    #[test]
    fn test_hyphen_underscore_normalization() {
        let db = test_db();
        let sid = insert_source_item(&db, "ts-node article");

        db.link_source_item_dep(
            sid,
            "ts-node",
            Some("npm"),
            "title_heuristic",
            0.7,
            None,
            None,
        )
        .expect("link with hyphen");

        // Query with underscore should find it (LOWER(REPLACE(name, '-', '_')) normalization)
        let deps = db
            .get_source_items_for_package("ts_node", Some("npm"), 0.0)
            .expect("query with underscore");
        assert_eq!(
            deps.len(),
            1,
            "hyphen/underscore normalization should match"
        );
    }
}
