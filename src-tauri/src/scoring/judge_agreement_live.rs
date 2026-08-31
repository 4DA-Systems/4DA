// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Live accuracy measurement: the curated feed against the LLM judge's verdicts.
//!
//! `benchmark_scenarios::benchmark_scoring_accuracy` is a structural FLOOR test
//! — it scores synthetic scenarios with a ZERO embedding, so the context and
//! semantic axes never run. It cannot see an accuracy problem; it did not see
//! that the dependency axis was 75% phantom, for months.
//!
//! This can. 4DA already runs an LLM judge over its own feed and stores every
//! verdict in `llm_judgments` with a score, a confidence and an explanation.
//! That is an independent second opinion on exactly the question the pipeline
//! answers, and until 2026-08-27 nothing measured the two against each other.
//! The measurement that produced this file:
//!
//! ```text
//!   feed items                      520
//!   judged                          324   (62%)
//!   judged below 0.5                162   (50% of judged)
//!   demoted by the demotion gate      0
//! ```
//!
//! `#[ignore]`d because it needs a real database. Point it at a SNAPSHOT:
//!
//! ```text
//! FOURDA_DB_PATH=/path/to/snapshot.db cargo test --lib \
//!     judge_agreement_live -- --ignored --nocapture
//! ```
//!
//! The judge is a TRIPWIRE, not a label — its apparent precision swings from
//! 50% to 12% depending where its score is thresholded. So this asserts on the
//! shape that holds across thresholds, never on a single number.

use rusqlite::Connection;

/// Feed items whose judged relevance sits below this are "disputed".
const DISPUTED_BELOW: f64 = 0.5;

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn judge_agreement_live() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        eprintln!("FOURDA_DB_PATH not set — nothing to verify");
        return;
    };
    let conn = Connection::open(&path).expect("open snapshot");
    let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };

    let feed_total = one("SELECT COUNT(*) FROM source_items WHERE feed_relevant = 1");
    let judged = one("SELECT COUNT(*) FROM source_items si
         JOIN llm_judgments lj ON lj.source_item_id = si.id
         WHERE si.feed_relevant = 1");
    let disputed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items si
             JOIN llm_judgments lj ON lj.source_item_id = si.id
             WHERE si.feed_relevant = 1 AND lj.relevance_score < ?1",
            [DISPUTED_BELOW],
            |r| r.get(0),
        )
        .unwrap_or(0);
    // Demotions the gate has actually PERFORMED. This is the number that says
    // whether the safety net works; see the correction note on the assertion
    // below for why the pending-candidate count is not.
    let demotions_performed =
        one("SELECT COUNT(*) FROM source_items WHERE feed_verdict_reason = 'llm_reject'");

    // Candidates still PENDING at the shipped thresholds. Reads the shipped
    // constants, so it can never measure a threshold pair the product no
    // longer uses. Expected to be ~0 on a healthy system: the gate is
    // convergent, and a demoted item stops being feed_relevant.
    let pending_at_gate: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items si
             JOIN llm_judgments lj ON lj.source_item_id = si.id AND lj.prompt_version = 'v2'
             WHERE si.feed_relevant = 1
               AND COALESCE(si.feed_verdict_source,'score') = 'score'
               AND lj.relevance_score < ?1 AND lj.confidence >= ?2",
            rusqlite::params![
                crate::llm_judgments::DEMOTION_RELEVANCE_BELOW,
                crate::llm_judgments::DEMOTION_CONFIDENCE_MIN
            ],
            |r| r.get(0),
        )
        .unwrap_or(0);

    println!("snapshot: {path}");
    println!("  feed items                : {feed_total}");
    println!("  ...judged                 : {judged}");
    println!("  ...judged below {DISPUTED_BELOW}      : {disputed}");
    println!("  demotions performed       : {demotions_performed}");
    println!("  candidates still pending  : {pending_at_gate}");

    println!("\n  source          judged   agree  dispute");
    let mut stmt = conn
        .prepare(
            "SELECT si.source_type, COUNT(*) AS judged,
                    SUM(CASE WHEN lj.relevance_score >= ?1 THEN 1 ELSE 0 END) AS agree
             FROM source_items si
             JOIN llm_judgments lj ON lj.source_item_id = si.id
             WHERE si.feed_relevant = 1
             GROUP BY si.source_type ORDER BY judged DESC",
        )
        .expect("prepare");
    let rows: Vec<(String, i64, i64)> = stmt
        .query_map([DISPUTED_BELOW], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("query")
        .filter_map(|r| r.ok())
        .collect();
    for (src, j, agree) in &rows {
        println!("  {src:<14} {j:>6} {agree:>7} {:>8}", j - agree);
    }

    if judged == 0 {
        eprintln!("no judged feed items in this snapshot — nothing to assert");
        return;
    }

    // The invariant: the system must be ABLE to act on the judge disagreeing
    // with it. An inert safety net is indistinguishable from no safety net
    // until somebody runs an audit.
    //
    // CORRECTION, 2026-08-27. The first version of this test asserted on
    // PENDING CANDIDATES and read their absence as "the gate has never fired".
    // That was wrong, and wrong in exactly the way the audit this file comes
    // from was about: the gate is CONVERGENT — demoting an item clears its
    // `feed_relevant`, which removes it from the candidate query — so zero
    // candidates means "nothing left to do", not "never did anything". The
    // 2026-08-26 snapshot that produced the "zero" already carried 120 items
    // stamped `llm_reject`. A zero is not an absence until you have checked
    // what it is a zero OF.
    //
    // So the assertion is on demotions PERFORMED. A system where the judge
    // disputes a meaningful share of the feed and NOTHING has ever been
    // demoted is the broken state worth failing on.
    let disputed_share = disputed as f64 / judged as f64;
    assert!(
        disputed_share <= 0.25 || demotions_performed > 0,
        "{:.0}% of judged feed items are disputed ({disputed} of {judged}) and the demotion \
         gate has NEVER demoted anything — it cannot act on the judge at all",
        disputed_share * 100.0
    );
}
