// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Pending-verdict drain tests.
//!
//! The LLM call itself is exercised nowhere here (hermetic suite); what these
//! tests pin is the drain's SAFETY BOUNDARY: which items each phase may
//! touch, what each judgment outcome is allowed to do to a verdict, and that
//! a pass with no provider or no budget still performs the pure-DB terminal
//! work. `run_drain_with` is gate-injectable for exactly this reason,
//! mirroring `llm_judgments::run_post_cycle_with`.

use super::*;
use crate::db::{PendingMarker, VerdictSource};
use crate::test_utils::{insert_test_item, test_db};

fn set_marker(db: &Database, id: i64, direction: bool, days_ago: i64, attempts: u32) {
    let marker = PendingMarker {
        direction,
        first_seen: chrono::Utc::now() - chrono::Duration::days(days_ago),
        attempts,
    };
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items SET feed_verdict_pending = ?1 WHERE id = ?2",
        rusqlite::params![marker.to_marker_string(), id],
    )
    .unwrap();
}

fn set_raw_marker(db: &Database, id: i64, marker: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items SET feed_verdict_pending = ?1 WHERE id = ?2",
        rusqlite::params![marker, id],
    )
    .unwrap();
}

fn verdict_of(db: &Database, id: i64) -> (Option<i64>, Option<String>, Option<String>) {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT feed_relevant, feed_verdict_source, feed_verdict_reason
         FROM source_items WHERE id = ?1",
        rusqlite::params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn pending_of(db: &Database, id: i64) -> Option<String> {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT feed_verdict_pending FROM source_items WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// resolve_action — the per-judgment decision table
// ---------------------------------------------------------------------------

/// A clear reject is a real verdict in either pending direction.
#[test]
fn clear_reject_demotes_regardless_of_direction() {
    assert_eq!(resolve_action(Some(false), 0.1), DrainAction::Demote);
    assert_eq!(resolve_action(Some(true), 0.1), DrainAction::Demote);
    assert_eq!(resolve_action(None, 0.1), DrainAction::Demote);
}

/// A clearly RELEVANT read kills a pending DEMOTE (standing curated verdict
/// re-affirmed) but must never resolve a pending PROMOTE — promotion needs a
/// full run's context, so that read only escalates.
#[test]
fn relevant_read_clears_only_pending_demotes() {
    assert_eq!(resolve_action(Some(false), 0.8), DrainAction::ClearPending);
    assert_eq!(resolve_action(Some(true), 0.8), DrainAction::Escalate);
    assert_eq!(resolve_action(None, 0.8), DrainAction::Escalate);
}

/// The mid-band shrug (>= reject line, < confirm line) is not evidence — it
/// escalates and the attempt/age budget eventually resolves it.
#[test]
fn mid_band_escalates() {
    assert_eq!(resolve_action(Some(false), 0.4), DrainAction::Escalate);
    assert_eq!(resolve_action(Some(true), 0.35), DrainAction::Escalate);
}

/// The reject line IS the main lane's measured demotion bar — if it moves,
/// the drain moves with it, never past it. And the table takes no confidence
/// input at all: measured live 2026-08-31, the judge rejected 77/90 items at
/// avg confidence 0.527 with ZERO rows >= 0.6 (it expresses rejection AS low
/// confidence), so any confidence gate on rejects would match nothing — the
/// signature not having the parameter is the regression-proof form.
#[test]
fn drain_reuses_the_main_lane_relevance_bar() {
    let just_below = crate::llm_judgments::DEMOTION_RELEVANCE_BELOW - 0.01;
    let at_line = crate::llm_judgments::DEMOTION_RELEVANCE_BELOW;
    assert_eq!(resolve_action(Some(true), just_below), DrainAction::Demote);
    assert_eq!(resolve_action(Some(true), at_line), DrainAction::Escalate);
}

// ---------------------------------------------------------------------------
// run_drain_with — phase A runs without provider or budget
// ---------------------------------------------------------------------------

/// Terminal resolution and corrupt-marker hygiene are pure DB work and must
/// run even when the LLM lane is unavailable — the budget being gone is not a
/// reason to keep a 19-day-old marker starving.
#[tokio::test]
async fn terminal_resolution_runs_without_provider_or_budget() {
    let db = test_db();

    // Exhausted: at the attempt cap AND pending longer than the age gate.
    let exhausted = insert_test_item(&db, "hackernews", "dr1", "Exhausted", "body");
    db.persist_feed_verdicts(&[(exhausted, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(
        &db,
        exhausted,
        false,
        MIN_EXHAUST_AGE_DAYS + 5,
        MAX_DRAIN_ATTEMPTS,
    );

    // At the attempt cap but too YOUNG: must wait out the age gate.
    let young = insert_test_item(&db, "hackernews", "dr2", "Young", "body");
    db.persist_feed_verdicts(&[(young, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(&db, young, false, 1, MAX_DRAIN_ATTEMPTS);

    // Old but with attempts left: the LLM lane's job, not phase A's.
    let retryable = insert_test_item(&db, "hackernews", "dr3", "Retryable", "body");
    db.persist_feed_verdicts(&[(retryable, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(&db, retryable, false, MIN_EXHAUST_AGE_DAYS + 5, 1);

    // Corrupt marker: cleared, never trusted.
    let corrupt = insert_test_item(&db, "hackernews", "dr4", "Corrupt", "body");
    set_raw_marker(&db, corrupt, "garbled-junk");

    let summary = run_drain_with(&db, false, None).await;

    assert_eq!(summary.exhausted, 1);
    assert_eq!(summary.corrupt_cleared, 1);
    assert_eq!(summary.judged, 0, "no provider — no LLM work");
    assert_eq!(summary.skipped, Some("no_llm_provider"));

    let (relevant, source, reason) = verdict_of(&db, exhausted);
    assert_eq!(relevant, Some(0), "terminal resolution is demote-only");
    assert_eq!(source, Some("fallback_exhausted".into()));
    assert_eq!(reason, Some("pending_retries_exhausted".into()));
    assert_eq!(pending_of(&db, exhausted), None);

    assert!(pending_of(&db, young).is_some(), "age gate holds");
    assert!(
        pending_of(&db, retryable).is_some(),
        "attempts left — not terminal"
    );
    assert_eq!(pending_of(&db, corrupt), None, "corrupt marker cleared");

    // Convergent: a second pass finds nothing terminal.
    let summary2 = run_drain_with(&db, false, None).await;
    assert_eq!(summary2.exhausted, 0);
    assert_eq!(summary2.corrupt_cleared, 0);
}

/// A pending PROMOTE (direction 1 — the live audit's dominant shape) exhausts
/// to the STANDING 0, never to 1: no promotion without a real verdict.
#[tokio::test]
async fn exhausted_pending_promote_never_promotes() {
    let db = test_db();
    let item = insert_test_item(&db, "hackernews", "dr5", "Starved promote", "body");
    db.persist_feed_verdicts(&[(item, false, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(
        &db,
        item,
        true,
        MIN_EXHAUST_AGE_DAYS + 12,
        MAX_DRAIN_ATTEMPTS,
    );

    let summary = run_drain_with(&db, false, None).await;
    assert_eq!(summary.exhausted, 1);
    let (relevant, source, _) = verdict_of(&db, item);
    assert_eq!(relevant, Some(0), "NEVER promoted without a real verdict");
    assert_eq!(source, Some("fallback_exhausted".into()));
}

/// Budget exhausted: phase A still runs, phase B is skipped with the same
/// skip label the main lane uses.
///
/// The backlog carries BOTH an exhausted marker (phase A work) and a
/// retryable one (phase B work). The second is load-bearing: without it phase
/// B has nothing to judge, and a pass that reports `llm_budget_reached` while
/// having no work to do is claiming the budget blocked something it did not.
/// The companion test below pins that distinction.
#[tokio::test]
async fn budget_gate_skips_only_the_llm_phase() {
    let db = test_db();
    let exhausted = insert_test_item(&db, "hackernews", "dr6", "Exhausted", "body");
    db.persist_feed_verdicts(&[(exhausted, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(
        &db,
        exhausted,
        false,
        MIN_EXHAUST_AGE_DAYS + 1,
        MAX_DRAIN_ATTEMPTS,
    );

    // Real phase-B work: retryable, unjudged, so only an LLM read could
    // resolve it — and the exhausted budget is what stops that.
    let retryable = insert_test_item(&db, "hackernews", "dr6b", "Retryable", "body");
    db.persist_feed_verdicts(&[(retryable, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(&db, retryable, false, 2, 1);

    let summary = run_drain_with(&db, true, None).await;
    assert_eq!(summary.exhausted, 1, "terminal work is not budget-gated");
    assert_eq!(summary.skipped, Some("llm_budget_reached"));
    assert_eq!(summary.judged, 0);
    assert_eq!(summary.reused, 0, "nothing was reusable here");
    assert!(
        pending_of(&db, retryable).is_some(),
        "an unresolvable marker must survive an exhausted budget"
    );
}

/// …and a pass with no phase-B work carries NO skip label, even with the
/// budget gone. "The budget blocked me" and "there was nothing to do" are
/// different states, and a label that cannot tell them apart is the same
/// silent-success class the demotion probe exists to prevent.
#[tokio::test]
async fn an_exhausted_budget_with_no_llm_work_reports_no_skip() {
    let db = test_db();
    let exhausted = insert_test_item(&db, "hackernews", "dr6c", "Exhausted only", "body");
    db.persist_feed_verdicts(&[(exhausted, true, VerdictSource::Score)], 18)
        .unwrap();
    set_marker(
        &db,
        exhausted,
        false,
        MIN_EXHAUST_AGE_DAYS + 1,
        MAX_DRAIN_ATTEMPTS,
    );

    let summary = run_drain_with(&db, true, None).await;
    assert_eq!(summary.exhausted, 1, "phase A still runs");
    assert_eq!(
        summary.skipped, None,
        "no LLM work existed, so the budget blocked nothing"
    );
}

/// An empty backlog is a silent no-op — no skip label, no log noise, no
/// counters.
#[tokio::test]
async fn empty_backlog_is_a_noop() {
    let db = test_db();
    let summary = run_drain_with(&db, false, None).await;
    assert_eq!(
        summary.judged + summary.exhausted + summary.corrupt_cleared,
        0
    );
    assert_eq!(summary.skipped, None);
}

// ---------------------------------------------------------------------------
// parse_drain_response — reply tolerance
// ---------------------------------------------------------------------------

#[test]
fn parse_tolerates_fences_and_drops_idless_elements() {
    let reply = "```json\n[\n {\"id\": 7, \"relevance\": 0.2, \"confidence\": 0.8, \"reason\": \"off-stack\"},\n {\"relevance\": 0.9, \"confidence\": 0.9, \"reason\": \"lost\"}\n]\n```";
    let parsed = parse_drain_response(reply).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].id, Some(7));
    assert_eq!(
        parsed[1].id, None,
        "id-less element survives parsing and is dropped at attach time"
    );
}

#[test]
fn parse_rejects_non_array_replies() {
    assert!(parse_drain_response("I cannot evaluate these items.").is_err());
    assert!(parse_drain_response("{\"id\": 1}").is_err());
}

/// The #559 live regression, applied to the drain: the cheap judge model
/// quotes numbers. Quoted ids and scores must parse, not fail the batch.
#[test]
fn parse_tolerates_quoted_numbers() {
    let reply =
        r#"[{"id": "60146", "relevance": "0.25", "confidence": "0.8", "reason": "off-stack"}]"#;
    let parsed = parse_drain_response(reply).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, Some(60146));
    assert_eq!(parsed[0].relevance, Some(0.25));
    assert_eq!(parsed[0].confidence, Some(0.8));

    // Non-numeric junk in a numeric field degrades to None, never an error.
    let junk = r#"[{"id": 7, "relevance": "high", "confidence": null, "reason": "x"}]"#;
    let parsed = parse_drain_response(junk).unwrap();
    assert_eq!(parsed[0].relevance, None);
    assert_eq!(parsed[0].confidence, None);
}

// ============================================================================
// Reuse before re-buying (B1)
//
// The drain used to buy an LLM read for every pending marker, including ones
// the ingest lane had already judged. Measured 2026-09-01: across 109 items
// judged by both lanes the drain changed the call 7 times (6.4%) for 38% of
// all judge spend. These pin the two ways reuse could be WRONG.
// ============================================================================

/// Writes a judgment row directly, at a chosen prompt version and timestamp.
fn store_judgment_at(db: &Database, id: i64, relevance: f64, version: &str, judged_at: &str) {
    db.upsert_llm_judgment(id, relevance, "reason", None, 0.9, "m", version)
        .unwrap();
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE llm_judgments SET judged_at = ?1 WHERE source_item_id = ?2 AND prompt_version = ?3",
        rusqlite::params![judged_at, id, version],
    )
    .unwrap();
}

fn sqlite_stamp(offset_days: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::days(offset_days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// The whole point: a fresh ingest-lane judgment resolves the marker with no
/// LLM call, and the verdict it produces is the same one the paid read would
/// have produced. Runs with the budget EXHAUSTED and no provider — reuse is
/// free, so it must work on precisely the days the paid lane cannot.
#[tokio::test]
async fn fresh_ingest_judgment_resolves_without_an_llm_call() {
    let db = test_db();
    let id = insert_test_item(
        &db,
        "hackernews",
        "reuse1",
        "Rejected by the ingest lane",
        "body",
    );
    set_marker(&db, id, false, 2, 1);
    // Judged AFTER the flip was deferred, at the ingest lane's current version,
    // below the reject line.
    store_judgment_at(
        &db,
        id,
        0.05,
        crate::llm_judgments::PROMPT_VERSION,
        &sqlite_stamp(0),
    );

    let summary = run_drain_with(&db, true, None).await;

    assert_eq!(summary.reused, 1, "the stored judgment must be reused");
    assert_eq!(summary.judged, 0, "no LLM judgment may be bought");
    assert_eq!(summary.demoted, 1, "a clear reject still demotes");
    let (relevant, _, reason) = verdict_of(&db, id);
    assert_eq!(relevant, Some(0));
    assert_eq!(reason.as_deref(), Some("llm_reject"));
}

/// Circularity guard. A `drain_v1` row is this lane's OWN earlier output.
/// Resolving a marker from it would let the drain confirm its own previous
/// guess and record that as a second opinion.
#[tokio::test]
async fn the_drains_own_prior_judgment_is_never_reused() {
    let db = test_db();
    let id = insert_test_item(
        &db,
        "hackernews",
        "reuse2",
        "Previously shrugged at by the drain",
        "body",
    );
    set_marker(&db, id, false, 2, 1);
    store_judgment_at(&db, id, 0.05, DRAIN_PROMPT_VERSION, &sqlite_stamp(0));

    let summary = run_drain_with(&db, true, None).await;

    assert_eq!(
        summary.reused, 0,
        "a drain_v1 judgment is this lane's own output, not evidence"
    );
    assert_eq!(summary.demoted, 0, "nothing may resolve from it");
    assert!(
        pending_of(&db, id).is_some(),
        "the marker must survive for a real second opinion"
    );
}

/// Staleness guard. A judgment made BEFORE the flip was deferred is part of
/// what the pipeline had already weighed when it deferred — reusing it would
/// resolve the marker with the very data that failed to resolve it.
#[tokio::test]
async fn a_judgment_older_than_the_marker_is_never_reused() {
    let db = test_db();
    let id = insert_test_item(
        &db,
        "hackernews",
        "reuse3",
        "Judged long before the flip",
        "body",
    );
    set_marker(&db, id, false, 2, 1);
    store_judgment_at(
        &db,
        id,
        0.05,
        crate::llm_judgments::PROMPT_VERSION,
        &sqlite_stamp(-10),
    );

    let summary = run_drain_with(&db, true, None).await;

    assert_eq!(
        summary.reused, 0,
        "a pre-marker judgment is not evidence about the flip"
    );
    assert_eq!(summary.demoted, 0);
    assert!(pending_of(&db, id).is_some(), "the marker must survive");
}
