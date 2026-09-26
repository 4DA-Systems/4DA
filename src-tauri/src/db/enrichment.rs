// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Storage for intake enrichment: which recently ingested items are still
//! thin, and what happened each time we tried to fetch their text.
//!
//! Attempts live in a side table created on first use rather than in a
//! schema migration. It holds only retry bookkeeping for items inside the
//! enrichment window, so an older binary that ignores it loses nothing, and
//! adding it needs neither a pre-migration backup nor a coordinated rebuild.

use rusqlite::{params, Connection, Result as SqliteResult};

use super::Database;

/// One item the enrichment pass may fetch text for.
#[derive(Debug, Clone)]
pub(crate) struct EnrichmentCandidate {
    pub id: i64,
    pub source_type: String,
    pub url: String,
    pub title: String,
    /// Raw stored content, HTML included: link posts carry their real
    /// target only inside it (Reddit's `[link]`, a Mastodon `<a href>`).
    pub content: String,
}

/// How one enrichment attempt ended. Only `Failed` is retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnrichmentOutcome {
    /// Text was fetched and stored.
    Enriched,
    /// The item already carries enough text, or has nothing fetchable.
    Skipped,
    /// The fetch or extraction failed; eligible for one retry later.
    Failed,
}

impl EnrichmentOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Enriched => "enriched",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// Attempts per item before it is given up on.
pub(crate) const MAX_ENRICHMENT_ATTEMPTS: i64 = 2;

fn ensure_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS enrichment_attempts (
             item_id INTEGER PRIMARY KEY,
             attempts INTEGER NOT NULL DEFAULT 0,
             last_attempt_at TEXT NOT NULL,
             outcome TEXT NOT NULL
         );",
    )
}

impl Database {
    /// Recently ingested items not yet settled by enrichment, newest first.
    ///
    /// Settled means enriched, skipped, or failed `MAX_ENRICHMENT_ATTEMPTS`
    /// times; a failure is retried once after `retry_after_hours`. Whether an
    /// item is actually thin is decided by the caller on its visible text,
    /// which SQL cannot see through HTML, so `limit` should leave headroom.
    pub(crate) fn enrichment_candidates(
        &self,
        window_hours: i64,
        retry_after_hours: i64,
        limit: usize,
    ) -> SqliteResult<Vec<EnrichmentCandidate>> {
        let conn = self.conn.lock();
        ensure_table(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT s.id, s.source_type, s.url, s.title, COALESCE(s.content, '')
             FROM source_items s
             LEFT JOIN enrichment_attempts a ON a.item_id = s.id
             WHERE s.created_at > datetime('now', ?1)
               AND s.url LIKE 'http%'
               AND (a.item_id IS NULL
                    OR (a.outcome = 'failed'
                        AND a.attempts < ?2
                        AND a.last_attempt_at < datetime('now', ?3)))
             ORDER BY s.created_at DESC
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![
                format!("-{window_hours} hours"),
                MAX_ENRICHMENT_ATTEMPTS,
                format!("-{retry_after_hours} hours"),
                limit as i64
            ],
            |row| {
                Ok(EnrichmentCandidate {
                    id: row.get(0)?,
                    source_type: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                })
            },
        )?;
        rows.collect()
    }

    /// Record one attempt, and drop bookkeeping for items long past the window.
    pub(crate) fn record_enrichment_attempt(
        &self,
        item_id: i64,
        outcome: EnrichmentOutcome,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        ensure_table(&conn)?;
        conn.execute(
            "INSERT INTO enrichment_attempts (item_id, attempts, last_attempt_at, outcome)
             VALUES (?1, 1, datetime('now'), ?2)
             ON CONFLICT(item_id) DO UPDATE SET
                 attempts = attempts + 1,
                 last_attempt_at = datetime('now'),
                 outcome = excluded.outcome",
            params![item_id, outcome.as_str()],
        )?;
        Ok(())
    }

    /// Delete attempt rows older than `days`; their items left the window.
    pub(crate) fn prune_enrichment_attempts(&self, days: i64) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        ensure_table(&conn)?;
        conn.execute(
            "DELETE FROM enrichment_attempts WHERE last_attempt_at < datetime('now', ?1)",
            params![format!("-{days} days")],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item_with_url, test_db};

    #[test]
    fn a_new_item_is_a_candidate_until_it_is_settled() {
        let db = test_db();
        let id =
            insert_test_item_with_url(&db, "hackernews", "1", "https://example.com/a", "T", "");
        let ids = |db: &Database| -> Vec<i64> {
            db.enrichment_candidates(48, 6, 50)
                .unwrap()
                .iter()
                .map(|c| c.id)
                .collect()
        };
        assert_eq!(ids(&db), vec![id]);

        db.record_enrichment_attempt(id, EnrichmentOutcome::Enriched)
            .unwrap();
        assert!(
            ids(&db).is_empty(),
            "an enriched item is never fetched again"
        );
    }

    #[test]
    fn a_skipped_item_is_never_reconsidered() {
        let db = test_db();
        let id =
            insert_test_item_with_url(&db, "mastodon", "1", "https://m.example/@a/1", "T", "long");
        db.record_enrichment_attempt(id, EnrichmentOutcome::Skipped)
            .unwrap();
        assert!(db.enrichment_candidates(48, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn a_failure_waits_for_the_retry_delay_then_stops_at_the_attempt_cap() {
        let db = test_db();
        let id = insert_test_item_with_url(&db, "lobsters", "1", "https://example.com/b", "T", "");
        db.record_enrichment_attempt(id, EnrichmentOutcome::Failed)
            .unwrap();

        // Within the retry delay: not offered.
        assert!(db.enrichment_candidates(48, 6, 50).unwrap().is_empty());

        // Pretend the delay passed: offered again.
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE enrichment_attempts SET last_attempt_at = datetime('now', '-7 hours')",
                [],
            )
            .unwrap();
        }
        assert_eq!(db.enrichment_candidates(48, 6, 50).unwrap().len(), 1);

        // A second failure reaches the cap: never offered again.
        db.record_enrichment_attempt(id, EnrichmentOutcome::Failed)
            .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE enrichment_attempts SET last_attempt_at = datetime('now', '-7 hours')",
                [],
            )
            .unwrap();
        }
        assert!(db.enrichment_candidates(48, 6, 50).unwrap().is_empty());
    }

    #[test]
    fn items_outside_the_window_or_without_a_web_url_are_not_candidates() {
        let db = test_db();
        let old =
            insert_test_item_with_url(&db, "hackernews", "1", "https://example.com/c", "T", "");
        insert_test_item_with_url(&db, "hackernews", "2", "ftp://example.com/d", "T", "");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET created_at = datetime('now', '-3 days') WHERE id = ?1",
                params![old],
            )
            .unwrap();
        }
        assert!(db.enrichment_candidates(48, 6, 50).unwrap().is_empty());
    }

    #[test]
    fn pruning_removes_only_old_attempt_rows() {
        let db = test_db();
        let a = insert_test_item_with_url(&db, "hackernews", "1", "https://example.com/e", "T", "");
        let b = insert_test_item_with_url(&db, "hackernews", "2", "https://example.com/f", "T", "");
        db.record_enrichment_attempt(a, EnrichmentOutcome::Enriched)
            .unwrap();
        db.record_enrichment_attempt(b, EnrichmentOutcome::Enriched)
            .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE enrichment_attempts SET last_attempt_at = datetime('now', '-20 days') WHERE item_id = ?1",
                params![a],
            )
            .unwrap();
        }
        assert_eq!(db.prune_enrichment_attempts(14).unwrap(), 1);
    }
}
