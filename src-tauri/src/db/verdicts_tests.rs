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
    assert_eq!(db.persist_feed_verdicts_with_reasons(&[], 18).unwrap(), 0);
    assert_eq!(db.reconcile_feed_verdicts(&[], &[], 18).unwrap(), 0);
}

// ---------------------------------------------------------------------------
// In-version sunk sweep + reason codes (Phase 108, 2026-08-23 audit)
// ---------------------------------------------------------------------------

/// Read the persisted `feed_verdict_reason` for an item.
fn reason_of(db: &Database, id: i64) -> Option<String> {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT feed_verdict_reason FROM source_items WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .unwrap()
}

/// Give a row the live score state the drain would have left it in.
fn set_live_score(db: &Database, id: i64, score: f64, scored_version: i32) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items
         SET relevance_score = ?1, scored_pipeline_version = ?2
         WHERE id = ?3",
        rusqlite::params![score, scored_version, id],
    )
    .unwrap();
}

/// The demote line every sunk-sweep test uses: the default 0.40 threshold
/// minus the anti-thrash epsilon, exactly what the analysis cadence passes.
fn demote_line() -> f32 {
    0.40 - SCORE_SUNK_EPSILON
}

/// The audit's headline case: a verdict granted when the item scored above
/// the line, whose live score then sank to 0.17 WITHIN the same pipeline
/// version. The version-scoped working set never re-judges it; the sunk sweep
/// must — demote-only, with the reason recorded and the grant provenance
/// (version/source) left intact. Running the sweep again applies nothing.
#[test]
fn sunk_in_version_verdict_is_demoted_with_reason() {
    let db = test_db();
    let sunk = insert_test_item(&db, "hackernews", "sv1", "Churned down", "body");
    db.persist_feed_verdicts(&[(sunk, true, VerdictSource::Score)], 18)
        .unwrap();
    set_live_score(&db, sunk, 0.17, 18);

    assert_eq!(db.demote_sunk_verdicts(18, demote_line()).unwrap(), 1);
    assert_eq!(
        verdict_of(&db, sunk),
        (Some(0), Some(18), Some("score".into())),
        "demoted, but the grant provenance (version 18, score) stays intact"
    );
    assert_eq!(
        reason_of(&db, sunk),
        Some("score_sunk_in_version".into()),
        "an in-version demotion must say WHY the verdict flipped"
    );
    assert_eq!(
        db.demote_sunk_verdicts(18, demote_line()).unwrap(),
        0,
        "a demoted row leaves the working set — the sweep converges"
    );
}

/// A score inside the jitter band (0.39 against the 0.37 demote line) is NOT
/// demoted: the audit measured ~300 items jittering across 0.37–0.43 on
/// same-version re-scores, and demoting at the threshold itself would thrash
/// exactly that band. Only clearly-sunk items go.
#[test]
fn sunk_sweep_spares_the_epsilon_jitter_band() {
    let db = test_db();
    let jitter = insert_test_item(&db, "hackernews", "sv2", "Boundary jitter", "body");
    db.persist_feed_verdicts(&[(jitter, true, VerdictSource::Score)], 18)
        .unwrap();
    set_live_score(&db, jitter, 0.39, 18);

    assert_eq!(db.demote_sunk_verdicts(18, demote_line()).unwrap(), 0);
    assert_eq!(verdict_of(&db, jitter).0, Some(1), "still curated");
    assert_eq!(reason_of(&db, jitter), None);
}

/// A fresh serendipity verdict is untouchable by the sunk sweep no matter how
/// low its score — the scorer rejecting an anti-bubble pick is that feature
/// working, not decay. (After the TTL it re-enters the VERSION-scoped working
/// set; the sunk sweep still never touches it.)
#[test]
fn sunk_sweep_never_touches_serendipity_verdicts() {
    let db = test_db();
    let lucky = insert_test_item(&db, "lemmy", "sv3", "Anti-bubble pick", "body");
    db.persist_feed_verdicts(&[(lucky, true, VerdictSource::Serendipity)], 18)
        .unwrap();
    set_live_score(&db, lucky, 0.10, 18);

    assert_eq!(db.demote_sunk_verdicts(18, demote_line()).unwrap(), 0);
    assert_eq!(verdict_of(&db, lucky).0, Some(1), "immune while fresh");
    assert_eq!(reason_of(&db, lucky), None);
}

/// The sweep judges an item ONLY on a score the verdict's own pipeline
/// version (or newer) produced. A verdict re-stamped to the current version
/// while the score drain is still behind must wait for the drain — demoting
/// on a superseded number would be an epoch comparison in disguise.
#[test]
fn sunk_sweep_requires_a_current_score() {
    let db = test_db();
    let behind = insert_test_item(&db, "hackernews", "sv4", "Drain not caught up", "body");
    db.persist_feed_verdicts(&[(behind, true, VerdictSource::Score)], 18)
        .unwrap();
    set_live_score(&db, behind, 0.17, 17); // score is from v17, verdict is v18

    assert_eq!(db.demote_sunk_verdicts(18, demote_line()).unwrap(), 0);
    assert_eq!(verdict_of(&db, behind).0, Some(1));
}

/// An OLD-version verdict is the version-scoped pass's territory, not the
/// sunk sweep's — it gets a full re-score there and, if demoted, the
/// 'stale_version' reason. Double-handling would write the wrong provenance.
#[test]
fn sunk_sweep_leaves_stale_version_verdicts_to_the_stale_pass() {
    let db = test_db();
    let stale = insert_test_item(&db, "hackernews", "sv5", "Old-brain verdict", "body");
    db.persist_feed_verdicts(&[(stale, true, VerdictSource::Score)], 17)
        .unwrap();
    set_live_score(&db, stale, 0.17, 18);

    assert_eq!(db.demote_sunk_verdicts(18, demote_line()).unwrap(), 0);
    assert_eq!(verdict_of(&db, stale).0, Some(1));
    assert_eq!(
        db.count_stale_verdicts(18).unwrap(),
        1,
        "the version-scoped working set owns this row"
    );
}

/// Version-scoped reconciliation stamps its own reason: demotions record
/// 'stale_version'; confirmations CLEAR any leftover reason, because a
/// confirmed verdict is a normal current verdict and a stale explanation
/// would describe a flip that no longer holds.
#[test]
fn reconcile_stamps_stale_version_reason_and_clears_on_confirm() {
    let db = test_db();
    let doomed = insert_test_item(&db, "crates_io", "rr1", "Rejected by v18", "body");
    let kept = insert_test_item(&db, "hackernews", "rr2", "Still relevant", "body");
    db.persist_feed_verdicts_with_reasons(
        &[
            (doomed, true, VerdictSource::Score, None),
            (
                kept,
                true,
                VerdictSource::Score,
                Some(VerdictReason::ScoreSunkInVersion),
            ),
        ],
        17,
    )
    .unwrap();

    db.reconcile_feed_verdicts(&[doomed], &[kept], 18).unwrap();
    assert_eq!(reason_of(&db, doomed), Some("stale_version".into()));
    assert_eq!(
        reason_of(&db, kept),
        None,
        "a confirmed verdict must not keep a superseded explanation"
    );
}

/// A full-cycle verdict write clears a repair-pass reason: the fresh judgment
/// supersedes the old explanation. This is the composition contract the
/// verdict-flip hysteresis wave builds on at the same persist boundary.
#[test]
fn persist_clears_a_prior_repair_reason() {
    let db = test_db();
    let item = insert_test_item(&db, "hackernews", "pr1", "Recovered", "body");
    db.persist_feed_verdicts(&[(item, true, VerdictSource::Score)], 18)
        .unwrap();
    set_live_score(&db, item, 0.17, 18);
    db.demote_sunk_verdicts(18, demote_line()).unwrap();
    assert_eq!(reason_of(&db, item), Some("score_sunk_in_version".into()));

    // The next full analysis cycle re-selects the item (its score recovered).
    db.persist_feed_verdicts(&[(item, true, VerdictSource::Score)], 18)
        .unwrap();
    assert_eq!(verdict_of(&db, item).0, Some(1));
    assert_eq!(
        reason_of(&db, item),
        None,
        "a fresh cycle verdict needs no explanation"
    );
}

/// The reason strings are a persisted schema contract, same as
/// `VerdictSource::as_str`.
#[test]
fn reason_codes_are_stable_strings() {
    assert_eq!(
        VerdictReason::ScoreSunkInVersion.as_str(),
        "score_sunk_in_version"
    );
    assert_eq!(VerdictReason::StaleVersion.as_str(), "stale_version");
}

/// Phase 108 adds `feed_verdict_reason` — nullable, no default (NULL means
/// "no explanation needed"; a default would stamp a claim the DB cannot
/// support) — and re-running the migration from a rewound version must not
/// duplicate the column (the ALTER is guarded).
#[test]
fn phase_108_reason_column_added_idempotently() {
    let db = test_db();
    let count_col = |db: &Database| -> i64 {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('source_items')
             WHERE name = 'feed_verdict_reason'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        count_col(&db),
        1,
        "feed_verdict_reason missing after migrate"
    );

    {
        let conn = db.conn.lock();
        let (notnull, dflt): (i64, Option<String>) = conn
            .query_row(
                "SELECT \"notnull\", dflt_value FROM pragma_table_info('source_items')
                 WHERE name = 'feed_verdict_reason'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(notnull, 0, "feed_verdict_reason must be nullable");
        assert!(dflt.is_none(), "feed_verdict_reason must have no default");
    }

    // Rewind to pre-108 and migrate again (the migration-test harness
    // convention, see test_phase_106): the guarded ALTER must be a no-op.
    {
        let conn = db.conn.lock();
        conn.execute_batch("UPDATE schema_version SET version = 107;")
            .unwrap();
    }
    db.migrate().expect("re-running migrations from v107");
    assert_eq!(
        count_col(&db),
        1,
        "re-migration must not duplicate feed_verdict_reason"
    );
}
