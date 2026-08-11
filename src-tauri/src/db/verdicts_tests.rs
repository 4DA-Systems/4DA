// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Verdict-epoch persistence tests.
//!
//! The selection predicate is the safety boundary of the whole lane: it decides
//! which curated items a demote-only pass is allowed to touch. Each test below
//! pins one way that boundary can be got wrong.

use super::*;
use crate::test_utils::{insert_test_item, test_db};

/// Read the persisted verdict triple for an item.
fn verdict_of(db: &Database, id: i64) -> (Option<i64>, Option<i64>, Option<String>) {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT feed_relevant, feed_verdict_version, feed_verdict_source
         FROM source_items WHERE id = ?1",
        rusqlite::params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

/// Force a row into the "legacy" shape a pre-Phase-101 verdict has: a flag and
/// a timestamp, no version, no provenance.
fn legacy_verdict(db: &Database, id: i64, relevant: bool) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items
         SET feed_relevant = ?1,
             feed_verdict_at = datetime('now'),
             feed_verdict_version = NULL,
             feed_verdict_source = NULL
         WHERE id = ?2",
        rusqlite::params![i64::from(relevant), id],
    )
    .unwrap();
}

/// A written verdict carries the version and provenance that produced it —
/// without which nothing downstream can tell a current verdict from one a
/// superseded pipeline wrote.
#[test]
fn persist_stamps_version_and_provenance() {
    let db = test_db();
    let scored = insert_test_item(&db, "hackernews", "v1", "Scored", "body");
    let lucky = insert_test_item(&db, "hackernews", "v2", "Serendipity", "body");

    db.persist_feed_verdicts(
        &[
            (scored, true, VerdictSource::Score),
            (lucky, true, VerdictSource::Serendipity),
        ],
        18,
    )
    .unwrap();

    assert_eq!(
        verdict_of(&db, scored),
        (Some(1), Some(18), Some("score".into()))
    );
    assert_eq!(
        verdict_of(&db, lucky),
        (Some(1), Some(18), Some("serendipity".into()))
    );
}

/// The staleness probe and the loader must agree on the same working set, and
/// that set is exactly "curated + stale + score-derived".
#[test]
fn stale_selection_covers_only_curated_stale_score_derived() {
    let db = test_db();
    let stale = insert_test_item(&db, "hackernews", "s1", "Stale positive", "body");
    let current = insert_test_item(&db, "hackernews", "s2", "Current positive", "body");
    let rejected = insert_test_item(&db, "hackernews", "s3", "Rejected", "body");
    let lucky = insert_test_item(&db, "hackernews", "s4", "Serendipity", "body");
    let never = insert_test_item(&db, "hackernews", "s5", "Never judged", "body");

    db.persist_feed_verdicts(&[(stale, true, VerdictSource::Score)], 17)
        .unwrap();
    db.persist_feed_verdicts(&[(current, true, VerdictSource::Score)], 18)
        .unwrap();
    db.persist_feed_verdicts(&[(rejected, false, VerdictSource::Score)], 17)
        .unwrap();
    db.persist_feed_verdicts(&[(lucky, true, VerdictSource::Serendipity)], 17)
        .unwrap();
    // `never` keeps feed_relevant IS NULL — never judged is NOT "rejected".

    assert_eq!(db.count_stale_verdicts(18).unwrap(), 1);
    let ids: Vec<i64> = db
        .get_stale_verdict_items(18, 100)
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(
        ids,
        vec![stale],
        "only the stale, curated, score-derived verdict is reconcilable \
         (current={current}, rejected={rejected}, serendipity={lucky}, unjudged={never})"
    );
}

/// A pre-Phase-101 row (NULL version, NULL provenance) is stale — otherwise the
/// entire pre-existing backlog, which is the whole defect, would never converge.
#[test]
fn legacy_unstamped_verdict_is_stale() {
    let db = test_db();
    let id = insert_test_item(&db, "crates_io", "legacy", "Legacy verdict", "body");
    legacy_verdict(&db, id, true);

    assert_eq!(db.count_stale_verdicts(18).unwrap(), 1);
    assert_eq!(db.get_stale_verdict_items(18, 10).unwrap().len(), 1);
}

/// Demotion clears the curated flag AND stamps, so the item leaves the stale
/// set. Without the stamp the pass would re-pick the same rows forever.
#[test]
fn demote_clears_flag_stamps_and_converges() {
    let db = test_db();
    let doomed = insert_test_item(&db, "crates_io", "d1", "Look-alike release", "body");
    let kept = insert_test_item(&db, "hackernews", "d2", "Still relevant", "body");
    db.persist_feed_verdicts(
        &[
            (doomed, true, VerdictSource::Score),
            (kept, true, VerdictSource::Score),
        ],
        17,
    )
    .unwrap();

    let applied = db.reconcile_feed_verdicts(&[doomed], &[kept], 18).unwrap();
    assert_eq!(applied, 2);

    assert_eq!(
        verdict_of(&db, doomed),
        (Some(0), Some(18), Some("score".into()))
    );
    assert_eq!(
        verdict_of(&db, kept),
        (Some(1), Some(18), Some("score".into()))
    );
    assert_eq!(
        db.count_stale_verdicts(18).unwrap(),
        0,
        "both outcomes must leave the stale set or the pass never converges"
    );
}

/// Reconciliation is demote-only: it can never turn a rejection into a
/// curation. Promotion needs the full run's dedup/diversity/rerank context that
/// a per-item pass does not have.
#[test]
fn reconcile_never_promotes_a_rejected_verdict() {
    let db = test_db();
    let rejected = insert_test_item(&db, "hackernews", "p1", "Rejected", "body");
    db.persist_feed_verdicts(&[(rejected, false, VerdictSource::Score)], 17)
        .unwrap();

    // Even named on the CONFIRM list (the branch that keeps a flag), a rejected
    // item must not become curated — the guard is in the SQL, not the caller.
    db.reconcile_feed_verdicts(&[], &[rejected], 18).unwrap();
    assert_eq!(
        verdict_of(&db, rejected).0,
        Some(0),
        "a 0 verdict must never be promoted to 1 by reconciliation"
    );
}

/// A never-judged item (`feed_relevant IS NULL`) is not "rejected" and must not
/// be silently converted into one.
#[test]
fn reconcile_leaves_never_judged_items_untouched() {
    let db = test_db();
    let never = insert_test_item(&db, "hackernews", "n1", "Never judged", "body");

    db.reconcile_feed_verdicts(&[never], &[never], 18).unwrap();
    assert_eq!(
        verdict_of(&db, never),
        (None, None, None),
        "NULL means never judged — reconciliation must not manufacture a verdict"
    );
}

/// Serendipity verdicts survive reconciliation WHILE FRESH. The current
/// pipeline rejecting an anti-bubble pick is that feature working as designed
/// — but only for `SERENDIPITY_VERDICT_TTL_DAYS`. v19: an expired pick
/// re-enters the working set so anti-bubble slots ROTATE instead of squatting
/// (measured live 2026-08-11: immune-forever picks had accumulated to 17.6%
/// of the curated feed against an 8% budget).
#[test]
fn serendipity_verdicts_immune_while_fresh_reconcilable_after_ttl() {
    let db = test_db();
    let lucky = insert_test_item(&db, "lemmy", "sr1", "Anti-bubble pick", "body");
    db.persist_feed_verdicts(&[(lucky, true, VerdictSource::Serendipity)], 1)
        .unwrap();

    assert_eq!(
        db.count_stale_verdicts(18).unwrap(),
        0,
        "a FRESH serendipity verdict is never stale for reconciliation purposes"
    );
    assert!(db.get_stale_verdict_items(18, 10).unwrap().is_empty());
    assert_eq!(verdict_of(&db, lucky).0, Some(1));

    // Age the verdict past the TTL — it becomes reconcilable like any other.
    // (Derived from the const so the SQL literal and the documented TTL can
    // never drift apart silently.)
    {
        let conn = db.conn.lock();
        conn.execute(
            &format!(
                "UPDATE source_items SET feed_verdict_at = datetime('now', '-{} days') WHERE id = ?1",
                super::SERENDIPITY_VERDICT_TTL_DAYS + 1
            ),
            rusqlite::params![lucky],
        )
        .unwrap();
    }
    assert_eq!(
        db.count_stale_verdicts(18).unwrap(),
        1,
        "an EXPIRED serendipity verdict re-enters the reconciliation working set"
    );
    let ids: Vec<i64> = db
        .get_stale_verdict_items(18, 10)
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec![lucky]);
}

/// Running the pass twice must be a no-op the second time.
#[test]
fn reconciliation_is_idempotent() {
    let db = test_db();
    let doomed = insert_test_item(&db, "crates_io", "i1", "Look-alike", "body");
    db.persist_feed_verdicts(&[(doomed, true, VerdictSource::Score)], 17)
        .unwrap();

    db.reconcile_feed_verdicts(&[doomed], &[], 18).unwrap();
    let after_first = verdict_of(&db, doomed);

    assert_eq!(db.count_stale_verdicts(18).unwrap(), 0);
    assert!(db.get_stale_verdict_items(18, 10).unwrap().is_empty());
    // A second application changes nothing (the WHERE guard rejects it).
    let applied = db.reconcile_feed_verdicts(&[doomed], &[], 18).unwrap();
    assert_eq!(applied, 0, "second run must apply nothing");
    assert_eq!(verdict_of(&db, doomed), after_first);
}

/// Provenance is derived from the flag both anti-bubble paths already set —
/// pinned here because an inferred signature (`top_score == 0.45`) silently
/// misses `compute_serendipity_candidates`, which keeps the item's own score.
#[test]
fn provenance_maps_from_the_serendipity_flag() {
    assert_eq!(
        VerdictSource::from_serendipity(true),
        VerdictSource::Serendipity
    );
    assert_eq!(VerdictSource::from_serendipity(false), VerdictSource::Score);
    assert_eq!(VerdictSource::Score.as_str(), "score");
    assert_eq!(VerdictSource::Serendipity.as_str(), "serendipity");
}

/// Empty input must not open a transaction or write anything.
#[test]
fn empty_batches_are_no_ops() {
    let db = test_db();
    assert_eq!(db.persist_feed_verdicts(&[], 18).unwrap(), 0);
    assert_eq!(db.reconcile_feed_verdicts(&[], &[], 18).unwrap(), 0);
}
