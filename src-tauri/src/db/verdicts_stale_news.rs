// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `stale_news` — the feed verdict for a news item that was ALREADY stale when
//! it was ingested (2026-10-02 adversarial audit).
//!
//! Measured live that day: 40 of the 193 dated items shown in the Signal feed
//! were more than 90 days old by publication date and 29 more than a year
//! (2012–2025). Two leaks put them there: Lobste.rs serving cached archive
//! pages under the plain feed URL, and RSS feeds delivering their back
//! catalogue on first ingest. The ingest gate
//! (`sources::drop_items_past_max_age`) stops new ones; this rule takes the
//! rows already held out of the display set, without touching a score.
//!
//! ## The predicate
//!
//! A row is stale news when its source has a max age
//! (`sources::freshness::news_max_item_age_days`) and the row's publication
//! date was more than that many days BEFORE its first ingest
//! (`created_at - published_at`). That is exactly "the ingest gate would have
//! dropped this row", so the two rules agree by construction. Measured at
//! ingest rather than against today, deliberately:
//!
//! - it is a fixed fact about the row, so the verdict never flips back and
//!   forth as the clock moves (convergent in one pass, nothing to damp);
//! - an item that was news when it arrived and has simply aged keeps its
//!   verdict — every display surface is already recency-windowed, and the
//!   curated history (content graph, MCP curation guard, first-curated recall
//!   metrics) should not be rewritten because time passed.
//!
//! Rows without a publication date are never stale news: the rule removes
//! only what is provably old.
//!
//! ## Where it binds (AD-038 / AD-039 shape, demote-only)
//!
//! 1. At THE persist boundary (`persist_feed_verdicts_with_reasons`): any
//!    promotion of a stale-news row — cycle verdict, risen sweep, judge drain,
//!    serendipity — is written `feed_relevant = 0`, reason `stale_news`. A
//!    reasoned write is immediate, so no deferred flip can later re-promote it.
//! 2. On the reconciliation cadence ([`Database::demote_stale_news_verdicts`]):
//!    curated rows that are already stale news leave the feed. The display set
//!    then follows the durable verdict through
//!    `analysis_verdicts::converge_display_on_durable_verdicts`
//!    (`excluded_by = "verdict:stale_news"`).
//!
//! `stale_news` is a reasoned rejection outside the risen sweep's
//! promotable set (`get_risen_verdict_candidates` admits only unreasoned,
//! `stale_version` and `score_sunk_in_version` rejections), so nothing lifts
//! it. Rows are never deleted.

use rusqlite::{params, Result as SqliteResult};

use super::{Database, VerdictReason};

/// The row's age at first ingest, in days (`NULL` when undated). Spliced into
/// the persist boundary's read so the predicate costs no extra query.
pub(super) const INGEST_AGE_DAYS_SQL: &str = "(julianday(created_at) - julianday(published_at))";

/// Whether a row of `source_type` that was `ingest_age_days` old at first
/// ingest is stale news. `None` age (undated) or an unbounded source → false.
pub(super) fn stale_at_ingest(source_type: Option<&str>, ingest_age_days: Option<f64>) -> bool {
    let Some(max_days) = source_type.and_then(crate::sources::freshness::news_max_item_age_days)
    else {
        return false;
    };
    ingest_age_days.is_some_and(|age| age > f64::from(max_days))
}

impl Database {
    /// Demote curated rows that were already stale news at ingest — reason
    /// `stale_news`, demote-only, rows kept. Pure SQL over the curated set
    /// (hundreds of rows), filtered in Rust against the ONE max-age table so
    /// the sweep and the persist boundary cannot disagree.
    ///
    /// Verdict provenance (`feed_verdict_version` / `_source` / `_at`) is left
    /// intact, as `demote_sunk_verdicts` does: the row still records which
    /// brain granted the verdict; the reason records why it was pulled.
    pub fn demote_stale_news_verdicts(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let candidates: Vec<i64> = {
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT id, source_type, {INGEST_AGE_DAYS_SQL} FROM source_items
                 WHERE feed_relevant = 1 AND published_at IS NOT NULL"
            ))?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                ))
            })?;
            let mut ids = Vec::new();
            for row in rows {
                let (id, source_type, age) = row?;
                if stale_at_ingest(Some(&source_type), age) {
                    ids.push(id);
                }
            }
            ids
        };
        if candidates.is_empty() {
            return Ok(0);
        }
        let tx = conn.unchecked_transaction()?;
        let mut demoted = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = 0,
                     feed_verdict_reason = ?1,
                     feed_verdict_pending = NULL
                 WHERE id = ?2 AND feed_relevant = 1",
            )?;
            for id in &candidates {
                demoted += stmt.execute(params![VerdictReason::StaleNews.as_str(), id])?;
            }
        }
        tx.commit()?;
        Ok(demoted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::VerdictSource;
    use crate::test_utils::{insert_test_item, test_db};

    /// Insert a row, then pin its ingest and publication dates. Returns its id.
    fn insert(
        db: &Database,
        key: i64,
        source: &str,
        created: &str,
        published: Option<&str>,
    ) -> i64 {
        let id = insert_test_item(
            db,
            source,
            &format!("{source}-{key}"),
            &format!("Item {key}"),
            "",
        );
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET created_at = ?1, published_at = ?2 WHERE id = ?3",
                params![created, published, id],
            )
            .expect("pin dates");
        id
    }

    fn verdict(db: &Database, id: i64) -> (Option<i64>, Option<String>) {
        db.conn
            .lock()
            .query_row(
                "SELECT feed_relevant, feed_verdict_reason FROM source_items WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row")
    }

    fn set_relevant(db: &Database, id: i64) {
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET feed_relevant = 1, feed_verdict_source = 'score'
                 WHERE id = ?1",
                params![id],
            )
            .expect("curate");
    }

    #[test]
    fn predicate_respects_the_source_bound_and_its_boundary() {
        // lobsters = 14 days, rss = 30 days, registries unbounded.
        assert!(!stale_at_ingest(Some("lobsters"), Some(14.0)));
        assert!(stale_at_ingest(Some("lobsters"), Some(14.01)));
        assert!(!stale_at_ingest(Some("rss"), Some(30.0)));
        assert!(stale_at_ingest(Some("rss"), Some(30.5)));
        assert!(!stale_at_ingest(Some("crates_io"), Some(4000.0)));
        assert!(!stale_at_ingest(Some("osv"), Some(4000.0)));
        // Undated rows and unknown sources are never stale news.
        assert!(!stale_at_ingest(Some("rss"), None));
        assert!(!stale_at_ingest(None, Some(4000.0)));
    }

    #[test]
    fn sweep_demotes_only_curated_news_rows_that_arrived_stale() {
        let db = test_db();
        // Archive page: a 2017 Lobsters story ingested last week.
        let archive = insert(
            &db,
            1,
            "lobsters",
            "2026-09-26 07:37:15",
            Some("2017-06-27 11:34:53"),
        );
        // A back-catalogue blog post.
        let backlog = insert(
            &db,
            2,
            "rss",
            "2026-08-12 02:29:04",
            Some("2023-10-15 00:00:00"),
        );
        // Fresh at ingest, merely aged since — keeps its verdict.
        let aged = insert(
            &db,
            3,
            "rss",
            "2026-06-02 10:00:00",
            Some("2026-06-01 09:00:00"),
        );
        // Undated news row.
        let undated = insert(&db, 4, "hackernews", "2026-09-30 10:00:00", None);
        // An old registry release — registries have no age bound.
        let release = insert(
            &db,
            5,
            "crates_io",
            "2026-09-04 03:19:26",
            Some("2026-03-25 15:05:27"),
        );
        // Stale, but not curated — untouched (nothing to demote).
        let uncurated = insert(
            &db,
            6,
            "lobsters",
            "2026-09-26 07:37:15",
            Some("2018-05-18 21:05:10"),
        );
        for id in [archive, backlog, aged, undated, release] {
            set_relevant(&db, id);
        }

        assert_eq!(db.demote_stale_news_verdicts().expect("sweep"), 2);
        let stale = (Some(0), Some("stale_news".to_string()));
        assert_eq!(verdict(&db, archive), stale);
        assert_eq!(verdict(&db, backlog), stale);
        for id in [aged, undated, release] {
            assert_eq!(
                verdict(&db, id),
                (Some(1), None),
                "row {id} must keep its verdict"
            );
        }
        assert_eq!(verdict(&db, uncurated), (None, None));

        // Convergent: a second sweep finds nothing.
        assert_eq!(db.demote_stale_news_verdicts().expect("sweep"), 0);
        // The display set follows: the durable rejection carries the reason.
        let rejected = db.durable_rejections(&[archive, aged]).expect("probe");
        assert_eq!(
            rejected.get(&archive),
            Some(&Some("stale_news".to_string()))
        );
        assert!(!rejected.contains_key(&aged));
    }

    #[test]
    fn persist_boundary_writes_stale_news_on_every_promotion_lane() {
        let db = test_db();
        let tauri = insert(
            &db,
            10,
            "lobsters",
            "2026-09-27 08:17:54",
            Some("2024-10-02 19:19:28"),
        );
        let axum = insert(
            &db,
            11,
            "rss",
            "2026-08-15 16:15:51",
            Some("2025-01-01 00:00:00"),
        );
        let fresh = insert(
            &db,
            12,
            "lobsters",
            "2026-10-01 10:00:00",
            Some("2026-09-30 22:00:00"),
        );
        let release = insert(
            &db,
            13,
            "crates_io",
            "2026-09-04 03:19:26",
            Some("2026-03-25 15:05:27"),
        );
        let version = crate::scoring::PIPELINE_VERSION;

        db.persist_feed_verdicts_with_reasons(
            &[
                (tauri, true, VerdictSource::Score, None),
                // A serendipity pick is not exempt: an archive page is not news.
                (axum, true, VerdictSource::Serendipity, None),
                (fresh, true, VerdictSource::Score, None),
                (release, true, VerdictSource::Score, None),
            ],
            version,
        )
        .expect("persist");

        let stale = (Some(0), Some("stale_news".to_string()));
        assert_eq!(verdict(&db, tauri), stale);
        assert_eq!(verdict(&db, axum), stale);
        assert_eq!(verdict(&db, fresh), (Some(1), None));
        assert_eq!(verdict(&db, release), (Some(1), None));

        // A later unreasoned promotion of the stale row is re-written
        // stale_news immediately — never parked as a deferred flip that a
        // second run could confirm.
        db.persist_feed_verdicts_with_reasons(
            &[(tauri, true, VerdictSource::Score, None)],
            version,
        )
        .expect("persist again");
        assert_eq!(verdict(&db, tauri), stale);
        let pending: Option<String> = db
            .conn
            .lock()
            .query_row(
                "SELECT feed_verdict_pending FROM source_items WHERE id = ?1",
                params![tauri],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(pending, None);
    }

    #[test]
    fn stale_news_is_never_a_risen_promotion_candidate() {
        let db = test_db();
        let id = insert(
            &db,
            20,
            "lobsters",
            "2026-09-27 08:17:54",
            Some("2024-10-02 19:19:28"),
        );
        set_relevant(&db, id);
        assert_eq!(db.demote_stale_news_verdicts().expect("sweep"), 1);
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET relevance_score = 0.95, scored_pipeline_version = ?1
                 WHERE id = ?2",
                params![crate::scoring::PIPELINE_VERSION, id],
            )
            .expect("score");
        let risen = db
            .get_risen_verdict_candidates(crate::scoring::PIPELINE_VERSION, 0.40, 50)
            .expect("risen");
        assert!(risen.iter().all(|c| c.id != id));
    }

    #[test]
    fn display_set_converges_on_a_stale_news_verdict() {
        let db = test_db();
        let id = insert(
            &db,
            30,
            "lobsters",
            "2026-10-01 13:04:35",
            Some("2025-06-26 18:58:39"),
        );
        set_relevant(&db, id);
        let mut results: Vec<crate::types::SourceRelevance> =
            vec![serde_json::from_value(serde_json::json!({
                "id": id,
                "title": "Announcing Rust 1.88.0",
                "url": null,
                "top_score": 0.9,
                "matches": [],
                "relevant": true,
            }))
            .expect("minimal SourceRelevance deserializes")];

        db.demote_stale_news_verdicts().expect("sweep");
        let outcome =
            crate::analysis_verdicts::converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(outcome.demoted, 1);
        assert!(!results[0].relevant && results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("verdict:stale_news")
        );
    }
}
