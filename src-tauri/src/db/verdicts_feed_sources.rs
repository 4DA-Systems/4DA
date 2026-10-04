// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `source_not_in_feed` — the feed verdict for a row whose source is fetched
//! and scored but never admitted to the feed
//! (`sources::feed_admission::FEED_EXCLUDED_SOURCES`; 2026-10-04: reddit,
//! lemmy and youtube, 0 of 20 useful on AI-labelled gold).
//!
//! Same AD-038 / AD-039 shape as `stale_news`, demote-only:
//!
//! 1. At THE persist boundary (`persist_feed_verdicts_with_reasons`) every
//!    promotion of such a row — cycle verdict, risen sweep, judge drain,
//!    serendipity — is written `feed_relevant = 0`, reason
//!    `source_not_in_feed`. A reasoned write is immediate, so no deferred flip
//!    can later re-promote it.
//! 2. On the reconciliation cadence ([`Database::demote_feed_excluded_sources`])
//!    the rows already held leave: curated rows, rows parked `awaiting_judge`
//!    and rows carrying a pending flip (which would otherwise spend a judge
//!    call to reach the same answer). The display set then follows the
//!    durable verdict (`analysis_verdicts::converge_display_on_durable_verdicts`,
//!    `excluded_by = "verdict:source_not_in_feed"`).
//!
//! The reason sits outside the risen sweep's promotable set and the judge
//! gate's rescue set, so nothing lifts it. Rows are never deleted and their
//! scores are untouched: trends, Blind Spots, knowledge gaps and search read
//! the corpus, not the feed verdict.

use rusqlite::{params, Result as SqliteResult};

use super::{Database, VerdictReason};

impl Database {
    /// Take held rows of feed-excluded sources out of the feed — reason
    /// `source_not_in_feed`, rows kept. Verdict provenance (`_version` /
    /// `_source` / `_at`) is left intact, as the other demote sweeps do.
    /// Convergent: a second pass matches nothing.
    pub fn demote_feed_excluded_sources(&self) -> SqliteResult<usize> {
        let excluded = crate::sources::feed_admission::feed_excluded_sources_sql();
        let conn = self.conn.lock();
        conn.execute(
            &format!(
                "UPDATE source_items
                 SET feed_relevant = 0,
                     feed_verdict_reason = ?1,
                     feed_verdict_pending = NULL
                 WHERE source_type IN ({excluded})
                   AND (feed_relevant = 1
                        OR feed_verdict_reason = 'awaiting_judge'
                        OR feed_verdict_pending IS NOT NULL)"
            ),
            params![VerdictReason::SourceNotInFeed.as_str()],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::VerdictSource;
    use crate::test_utils::{insert_test_item, test_db};

    fn set(
        db: &Database,
        id: i64,
        relevant: Option<i64>,
        reason: Option<&str>,
        pending: Option<&str>,
    ) {
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET feed_relevant = ?1, feed_verdict_reason = ?2,
                     feed_verdict_pending = ?3, feed_verdict_source = 'score'
                 WHERE id = ?4",
                params![relevant, reason, pending, id],
            )
            .expect("set verdict");
    }

    fn verdict(db: &Database, id: i64) -> (Option<i64>, Option<String>, Option<String>) {
        db.conn
            .lock()
            .query_row(
                "SELECT feed_relevant, feed_verdict_reason, feed_verdict_pending
                 FROM source_items WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("row")
    }

    fn cut() -> (Option<i64>, Option<String>, Option<String>) {
        (Some(0), Some("source_not_in_feed".to_string()), None)
    }

    #[test]
    fn sweep_takes_held_rows_of_excluded_sources_out_and_nothing_else() {
        let db = test_db();
        let reddit = insert_test_item(&db, "reddit", "fs1", "rust-analyzer changelog #347", "");
        let lemmy = insert_test_item(&db, "lemmy", "fs2", "File-notification exploit", "");
        let youtube = insert_test_item(&db, "youtube", "fs3", "OpenAI Dev Day being rough", "");
        let waiting = insert_test_item(&db, "reddit", "fs4", "waiting for the judge", "");
        let pending = insert_test_item(&db, "reddit", "fs5", "pending promotion", "");
        let rejected = insert_test_item(&db, "reddit", "fs6", "already rejected", "");
        let unjudged = insert_test_item(&db, "reddit", "fs7", "never judged", "");
        let hn = insert_test_item(&db, "hackernews", "fs8", "Show HN: kept", "");
        let devto = insert_test_item(&db, "devto", "fs9", "dev.to awaiting judge", "");
        for id in [reddit, lemmy, youtube, hn] {
            set(&db, id, Some(1), None, None);
        }
        set(&db, waiting, Some(0), Some("awaiting_judge"), None);
        set(&db, pending, Some(0), None, Some("1@2026-10-04T00:00:00Z"));
        set(&db, rejected, Some(0), Some("llm_reject"), None);
        set(&db, devto, Some(0), Some("awaiting_judge"), None);

        assert_eq!(db.demote_feed_excluded_sources().expect("sweep"), 5);
        for id in [reddit, lemmy, youtube, waiting, pending] {
            assert_eq!(verdict(&db, id), cut(), "row {id}");
        }
        assert_eq!(
            verdict(&db, rejected),
            (Some(0), Some("llm_reject".to_string()), None),
            "a standing reasoned rejection keeps its own reason"
        );
        assert_eq!(verdict(&db, unjudged), (None, None, None));
        assert_eq!(verdict(&db, hn), (Some(1), None, None));
        assert_eq!(
            verdict(&db, devto),
            (Some(0), Some("awaiting_judge".to_string()), None),
            "admitted sources are untouched"
        );
        // Convergent.
        assert_eq!(db.demote_feed_excluded_sources().expect("sweep"), 0);
    }

    #[test]
    fn persist_boundary_never_admits_an_excluded_source() {
        let db = test_db();
        let reddit = insert_test_item(&db, "reddit", "fs10", "Generic Const Args and You", "");
        let youtube = insert_test_item(&db, "youtube", "fs11", "OpenAI Hacked", "");
        let lemmy = insert_test_item(&db, "lemmy", "fs12", "GPT-6 Astra", "");
        let hn = insert_test_item(&db, "hackernews", "fs13", "Show HN: kept", "");
        let version = crate::scoring::PIPELINE_VERSION;
        crate::judge_gate::set_active_for_test(true);
        db.persist_feed_verdicts_with_reasons(
            &[
                (reddit, true, VerdictSource::Score, None),
                // Serendipity is not exempt: the source never enters the feed.
                (youtube, true, VerdictSource::Serendipity, None),
                (lemmy, true, VerdictSource::Score, None),
                (hn, true, VerdictSource::Score, None),
            ],
            version,
        )
        .expect("persist");
        crate::judge_gate::set_active_for_test(false);
        for id in [reddit, youtube, lemmy] {
            assert_eq!(verdict(&db, id), cut(), "row {id}");
        }
        assert_eq!(verdict(&db, hn), (Some(1), None, None));

        // A later promotion is re-written immediately, never parked as a
        // deferred flip that a second run could confirm.
        db.persist_feed_verdicts_with_reasons(
            &[(reddit, true, VerdictSource::Score, None)],
            version,
        )
        .expect("persist again");
        assert_eq!(verdict(&db, reddit), cut());
        // A non-relevant verdict is written as usual (unreasoned).
        let other = insert_test_item(&db, "reddit", "fs14", "not relevant", "");
        db.persist_feed_verdicts_with_reasons(
            &[(other, false, VerdictSource::Score, None)],
            version,
        )
        .expect("persist reject");
        assert_eq!(verdict(&db, other), (Some(0), None, None));
    }

    #[test]
    fn source_cut_is_never_a_risen_promotion_candidate() {
        let db = test_db();
        let id = insert_test_item(&db, "reddit", "fs20", "risen reddit row", "");
        set(&db, id, Some(1), None, None);
        db.demote_feed_excluded_sources().expect("sweep");
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
    fn display_set_converges_on_the_source_cut() {
        let db = test_db();
        let id = insert_test_item(&db, "reddit", "fs30", "nine npm packages shipping worm", "");
        set(&db, id, Some(1), None, None);
        let mut results: Vec<crate::types::SourceRelevance> =
            vec![serde_json::from_value(serde_json::json!({
                "id": id,
                "title": "nine npm packages shipping worm",
                "url": null,
                "top_score": 0.9,
                "matches": [],
                "relevant": true,
                "source_type": "reddit",
            }))
            .expect("minimal SourceRelevance deserializes")];

        db.demote_feed_excluded_sources().expect("sweep");
        let outcome =
            crate::analysis_verdicts::converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(outcome.demoted, 1);
        assert!(!results[0].relevant && results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("verdict:source_not_in_feed")
        );
    }
}
