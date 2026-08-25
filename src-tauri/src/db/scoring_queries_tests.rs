// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stability + evidence/rank separation tests for the score-persist boundary
//! (scoring_queries.rs). Split into a sibling test file when the inline
//! module pushed the production file against the 1000-line error gate.

use super::*;
use crate::test_utils::{insert_test_item, test_db};

fn score_and_version(db: &Database, id: i64) -> (Option<f64>, i64) {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT relevance_score, scored_pipeline_version FROM source_items WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// Item 10 (score side): a sub-hysteresis re-score keeps the durable score
/// but STILL advances the version stamp — skipping the stamp would trap the
/// item in the version-drain set forever. A move at/above the hysteresis
/// writes through, and a first-ever score (old NULL) always writes.
#[test]
fn hysteresis_keeps_score_but_stamps_version() {
    let db = test_db();
    let wobble = insert_test_item(&db, "hackernews", "hy1", "Wobbler", "x");
    let mover = insert_test_item(&db, "hackernews", "hy2", "Mover", "x");
    let fresh = insert_test_item(&db, "hackernews", "hy3", "First score", "x");

    // Seed wobble+mover with a prior score at an OLD pipeline version, so
    // the version stamp is observable.
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.50, scored_pipeline_version = 1
             WHERE id IN (?1, ?2)",
            params![wobble, mover],
        )
        .unwrap();
    }

    db.persist_analysis_scores(
        &[
            (wobble, 0.52, None, None), // |Δ| = 0.02 < 0.05 → damped
            (mover, 0.60, None, None),  // |Δ| = 0.10 ≥ 0.05 → written
            (fresh, 0.03, None, None),  // old NULL → always written
        ],
        "analysis",
    )
    .unwrap();

    let (s_wobble, v_wobble) = score_and_version(&db, wobble);
    assert_eq!(
        s_wobble,
        Some(0.50),
        "sub-hysteresis move keeps the old score"
    );
    assert_eq!(
        v_wobble,
        i64::from(crate::scoring::PIPELINE_VERSION),
        "damped write must still stamp the pipeline version (drain safety)"
    );
    let (s_mover, _) = score_and_version(&db, mover);
    assert!(
        (s_mover.unwrap() - 0.60).abs() < 1e-6,
        "real move writes through"
    );
    let (s_fresh, v_fresh) = score_and_version(&db, fresh);
    assert!(s_fresh.is_some(), "first-ever score always writes");
    assert_eq!(v_fresh, i64::from(crate::scoring::PIPELINE_VERSION));
}

/// Tightening T1 (2026-08-25): `scored_at` stamps on EVERY evaluation —
/// hysteresis-suppressed writes included (a suppressed write still means
/// "re-evaluated now"), and zero-evidence items via
/// `mark_items_scored_version` (persist_analysis_scores never sees them).
/// The rolling freshness refresh rotates on this stamp; miss either case and
/// the stalest-first ordering re-picks the same items every cycle.
#[test]
fn scored_at_stamps_on_suppressed_and_zero_evidence_evaluations() {
    let db = test_db();
    let wobble = insert_test_item(&db, "hackernews", "sa1", "Damped wobbler", "x");
    let noise = insert_test_item(&db, "hackernews", "sa2", "Zero evidence", "x");
    let scored_at = |id: i64| -> Option<String> {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT scored_at FROM source_items WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert!(
        scored_at(wobble).is_none(),
        "no stamp before any evaluation"
    );

    // Seed a prior score, then re-score inside the hysteresis band: the
    // durable score is kept, the evaluation stamp is not skipped.
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.50, scored_pipeline_version = 1
             WHERE id = ?1",
            params![wobble],
        )
        .unwrap();
    }
    db.persist_analysis_scores(&[(wobble, 0.52, None, None)], "analysis")
        .unwrap();
    let (score, _) = score_and_version(&db, wobble);
    assert_eq!(score, Some(0.50), "sub-hysteresis move keeps the old score");
    assert!(
        scored_at(wobble).is_some(),
        "a suppressed write still stamps scored_at (re-evaluated now)"
    );

    // Zero-evidence path: the cycle stamps noise via mark_items_scored_version.
    db.mark_items_scored_version(&[noise], crate::scoring::PIPELINE_VERSION)
        .unwrap();
    assert!(
        scored_at(noise).is_some(),
        "zero-evidence evaluations stamp scored_at via the version stamp"
    );
}

/// Item 13: the churn row names WHICH items moved (top_offenders, raw
/// deltas, largest first) and how many writes the damper suppressed.
#[test]
fn churn_row_carries_offender_list_and_suppressed_count() {
    let db = test_db();
    let a = insert_test_item(&db, "hackernews", "off_a", "big mover", "x");
    let b = insert_test_item(&db, "hackernews", "off_b", "small mover", "x");
    let c = insert_test_item(&db, "hackernews", "off_c", "damped", "x");
    db.persist_analysis_scores(
        &[
            (a, 0.90, None, None),
            (b, 0.40, None, None),
            (c, 0.50, None, None),
        ],
        "analysis",
    )
    .unwrap();
    db.persist_analysis_scores(
        &[
            (a, 0.50, None, None), // Δ = −0.40 → offender #1
            (b, 0.48, None, None), // Δ = +0.08 → offender #2 (written)
            (c, 0.52, None, None), // Δ = +0.02 → damped, below offender floor? 0.02 ≥ 0.01 → listed
        ],
        "analysis",
    )
    .unwrap();

    let conn = db.conn.lock();
    let (suppressed, offenders_json): (Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT suppressed_writes, top_offenders FROM scoring_churn ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(suppressed, Some(1), "exactly c's write was damped");
    let offenders: Vec<serde_json::Value> =
        serde_json::from_str(&offenders_json.expect("offender list present")).unwrap();
    assert_eq!(offenders.len(), 3, "all rescored movers ≥0.01 are listed");
    assert_eq!(
        offenders[0]["id"],
        serde_json::json!(a),
        "largest |Δ| first"
    );
    assert!((offenders[0]["old"].as_f64().unwrap() - 0.90).abs() < 1e-6);
    assert!((offenders[0]["new"].as_f64().unwrap() - 0.50).abs() < 1e-6);

    // First-ever batch (all old NULL) records no offenders and 0 suppressed.
    let (first_suppressed, first_offenders): (Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT suppressed_writes, top_offenders FROM scoring_churn ORDER BY id ASC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(first_suppressed, Some(0));
    assert!(
        first_offenders.is_none(),
        "first-ever scores are not movers"
    );
}

/// The differential gate's pending-drain count mirrors the drain predicate:
/// stale = scored below the current version AND has a persisted score.
#[test]
fn count_stale_scored_items_mirrors_drain_predicate() {
    let db = test_db();
    let stale = insert_test_item(&db, "hackernews", "st1", "stale", "x");
    let current = insert_test_item(&db, "hackernews", "st2", "current", "x");
    let never = insert_test_item(&db, "hackernews", "st3", "never scored", "x");
    let v = crate::scoring::PIPELINE_VERSION;
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.5, scored_pipeline_version = ?1 WHERE id = ?2",
            params![v - 1, stale],
        )
        .unwrap();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.5, scored_pipeline_version = ?1 WHERE id = ?2",
            params![v, current],
        )
        .unwrap();
        // `never` keeps relevance_score NULL / version 0 — that is the
        // NEVER-scored backlog (backfill's job), not the stale drain's.
        let _ = never;
    }
    assert_eq!(db.count_stale_scored_items(v).unwrap(), 1);
    assert_eq!(db.count_stale_scored_items(v - 1).unwrap(), 0);
}

/// Items 12+26: the rank write is fully separate from the evidence write.
/// `persist_rank_scores` touches ONLY the three rank columns — never
/// `relevance_score`, `scored_pipeline_version`, the signal columns, or
/// the `scoring_churn` telemetry. And the evidence write
/// (`persist_analysis_scores`) never touches the rank columns.
#[test]
fn rank_persist_and_evidence_persist_touch_disjoint_columns() {
    let db = test_db();
    let item = insert_test_item(&db, "hackernews", "rk1", "Ranked item", "x");

    // Seed durable evidence at an old version.
    db.persist_analysis_scores(&[(item, 0.60, Some("release".into()), None)], "analysis")
        .unwrap();
    let churn_rows_before: i64 = {
        let conn = db.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM scoring_churn", [], |r| r.get(0))
            .unwrap()
    };

    // Rank write: batch layer ranked it elsewhere, with provenance.
    db.persist_rank_scores(&[(item, 0.85, Some(r#"{"ce":0.25}"#.into()))])
        .unwrap();

    let conn = db.conn.lock();
    let (evidence, version, sig, rank, factors, ranked_at): (
        Option<f64>,
        i64,
        Option<String>,
        Option<f64>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT relevance_score, scored_pipeline_version, signal_type,
                    rank_score, rank_factors, rank_scored_at
             FROM source_items WHERE id = ?1",
            params![item],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap();
    assert!(
        (evidence.unwrap() - 0.6).abs() < 1e-6,
        "rank write must not move evidence"
    );
    assert_eq!(
        version,
        i64::from(crate::scoring::PIPELINE_VERSION),
        "rank write must not re-stamp the pipeline version"
    );
    assert_eq!(sig.as_deref(), Some("release"), "signal columns untouched");
    assert!((rank.unwrap() - 0.85).abs() < 1e-6, "rank written");
    assert_eq!(
        factors.as_deref(),
        Some(r#"{"ce":0.25}"#),
        "provenance recorded"
    );
    assert!(ranked_at.is_some(), "rank_scored_at stamped");
    let churn_rows_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM scoring_churn", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        churn_rows_before, churn_rows_after,
        "rank writes must not add churn telemetry rows (churn tracks evidence)"
    );
    drop(conn);

    // Evidence write (the backfill path calls this directly) leaves the
    // rank columns exactly as the rank write left them.
    db.persist_analysis_scores(&[(item, 0.75, None, None)], "backfill")
        .unwrap();
    let conn = db.conn.lock();
    let (rank2, factors2): (Option<f64>, Option<String>) = conn
        .query_row(
            "SELECT rank_score, rank_factors FROM source_items WHERE id = ?1",
            params![item],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        (rank2.unwrap() - 0.85).abs() < 1e-6,
        "evidence write must not touch rank_score"
    );
    assert_eq!(
        factors2.as_deref(),
        Some(r#"{"ce":0.25}"#),
        "evidence write must not touch rank_factors"
    );
}

/// Items 12+26: the shared ranked-read expression orders by rank when the
/// batch layer ranked the item and falls back to evidence when it never
/// did (NULL rank), with never-scored items sinking last.
#[test]
fn ranked_order_expr_coalesces_rank_over_evidence() {
    let db = test_db();
    let evidence_high = insert_test_item(&db, "hackernews", "ro1", "high evidence", "x");
    let ranked_top = insert_test_item(&db, "hackernews", "ro2", "batch-ranked", "x");
    let low = insert_test_item(&db, "hackernews", "ro3", "low evidence", "x");
    db.persist_analysis_scores(
        &[
            (evidence_high, 0.80, None, None),
            (ranked_top, 0.60, None, None),
            (low, 0.50, None, None),
        ],
        "analysis",
    )
    .unwrap();
    // The batch layer ranked only ro2 (differential run): rank beats
    // evidence for it; the others order by evidence.
    db.persist_rank_scores(&[(ranked_top, 0.92, None)]).unwrap();

    let conn = db.conn.lock();
    let sql = format!(
        "SELECT id FROM source_items WHERE relevance_score IS NOT NULL ORDER BY {RANKED_ORDER_EXPR}"
    );
    let ids: Vec<i64> = conn
        .prepare(&sql)
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        ids,
        vec![ranked_top, evidence_high, low],
        "rank wins where present; COALESCE falls back to evidence elsewhere"
    );
    // The aliased form stays textually consistent with the const.
    assert_eq!(
        ranked_order_expr("si"),
        "COALESCE(si.rank_score, si.relevance_score) DESC"
    );
}

/// Item 11's working-set probe: fresh durable curation = persisted score +
/// recent verdict stamp. Old or never-curated rows fall out of protection
/// (the escape hatch that prevents a permanently-degraded run from
/// freezing the corpus).
#[test]
fn fresh_durable_score_probe_honors_the_age_escape_hatch() {
    let db = test_db();
    let fresh = insert_test_item(&db, "hackernews", "fd1", "fresh", "x");
    let old = insert_test_item(&db, "hackernews", "fd2", "old verdict", "x");
    let unscored = insert_test_item(&db, "hackernews", "fd3", "no score", "x");
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.6, feed_verdict_at = datetime('now') WHERE id = ?1",
            params![fresh],
        )
        .unwrap();
        conn.execute(
            "UPDATE source_items SET relevance_score = 0.6, feed_verdict_at = datetime('now', '-8 days') WHERE id = ?1",
            params![old],
        )
        .unwrap();
        conn.execute(
            "UPDATE source_items SET feed_verdict_at = datetime('now') WHERE id = ?1",
            params![unscored],
        )
        .unwrap();
    }
    let protected = db
        .ids_with_fresh_durable_scores(&[fresh, old, unscored], 7)
        .unwrap();
    assert!(
        protected.contains(&fresh),
        "fresh durable score is protected"
    );
    assert!(
        !protected.contains(&old),
        ">7-day-old curation accepts the write"
    );
    assert!(
        !protected.contains(&unscored),
        "no durable score → nothing to protect"
    );
    assert!(db.ids_with_fresh_durable_scores(&[], 7).unwrap().is_empty());
}
