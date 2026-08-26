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
    // Reads the SHIPPED constants, so this can never measure a threshold pair
    // the product no longer uses.
    let demoted_at_gate: i64 = conn
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
    println!("  demotion gate would demote: {demoted_at_gate}");

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

    // The invariant this arc exists to hold: the system must be ABLE to act on
    // the judge disagreeing with it. A gate that demotes nothing while half the
    // judged feed is disputed is inert, and an inert safety net is
    // indistinguishable from no safety net until somebody runs an audit.
    let disputed_share = disputed as f64 / judged as f64;
    assert!(
        disputed_share <= 0.25 || demoted_at_gate > 0,
        "{:.0}% of judged feed items are disputed ({disputed} of {judged}) yet the demotion \
         gate would demote NONE — it is calibrated past the judge's output distribution",
        disputed_share * 100.0
    );
}
