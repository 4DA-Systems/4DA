use super::*;
use crate::test_utils::{insert_test_item_with_url, test_db};

fn item(db: &Database, source: &str, sid: &str) -> i64 {
    insert_test_item_with_url(
        db,
        source,
        sid,
        &format!("https://example.test/{sid}"),
        &format!("Story {sid}"),
        "body",
    )
}

fn verdict(db: &Database, id: i64) -> (Option<i64>, Option<String>) {
    db.conn
        .lock()
        .query_row(
            "SELECT feed_relevant, feed_verdict_reason FROM source_items WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

fn judge(db: &Database, id: i64, relevance: f64, prompt_version: &str) {
    db.upsert_llm_judgment(
        id,
        relevance,
        "why",
        None,
        0.9,
        "gemma4:26b",
        prompt_version,
    )
    .unwrap();
}

const V: i32 = crate::scoring::PIPELINE_VERSION;

/// The measured rule at the persist boundary: an unjudged dev.to promotion
/// waits, a card-aware judgment at the bar admits it, one below rejects it,
/// and a thin-context (v6) judgment does not count.
#[test]
fn a_gated_promotion_needs_a_card_aware_judgment() {
    crate::judge_gate::set_active_for_test(true);
    let db = test_db();
    let [card, _] = crate::judge_gate::CARD_PROMPT_VERSIONS;
    let (waits, passes, fails, thin, hn) = (
        item(&db, "devto", "d1"),
        item(&db, "devto", "d2"),
        item(&db, "mastodon", "m1"),
        item(&db, "reddit", "r1"),
        item(&db, "hackernews", "h1"),
    );
    judge(&db, passes, 0.7, card);
    judge(&db, fails, 0.2, card);
    judge(&db, thin, 0.9, "v6");
    let promote: Vec<_> = [waits, passes, fails, thin, hn]
        .iter()
        .map(|&id| (id, true, VerdictSource::Score, None))
        .collect();
    db.persist_feed_verdicts_with_reasons(&promote, V).unwrap();

    assert_eq!(
        verdict(&db, waits),
        (Some(0), Some("awaiting_judge".into()))
    );
    assert_eq!(verdict(&db, passes), (Some(1), None));
    assert_eq!(verdict(&db, fails), (Some(0), Some("llm_reject".into())));
    assert_eq!(
        verdict(&db, thin),
        (Some(0), Some("awaiting_judge".into())),
        "a thin-context judgment is not the gate's evidence"
    );
    assert_eq!(
        verdict(&db, hn),
        (Some(1), None),
        "ungated sources decide by score"
    );
    crate::judge_gate::set_active_for_test(false);
}

/// Grandfathering and the sweep: a curated dev.to item keeps its place until
/// judged, then the sweep applies the bar; a waiting item the judge clears is
/// released for the risen sweep (verdict withdrawn).
#[test]
fn the_sweep_rejects_below_the_bar_and_releases_what_the_judge_cleared() {
    let db = test_db();
    let [card, _] = crate::judge_gate::CARD_PROMPT_VERSIONS;
    let (curated_bad, curated_unjudged, waiting_good, waiting_bad) = (
        item(&db, "devto", "c1"),
        item(&db, "lobsters", "c2"),
        item(&db, "mastodon", "w1"),
        item(&db, "reddit", "w2"),
    );
    db.persist_feed_verdicts(
        &[
            (curated_bad, true, VerdictSource::Score),
            (curated_unjudged, true, VerdictSource::Score),
        ],
        V,
    )
    .unwrap();
    crate::judge_gate::set_active_for_test(true);
    db.persist_feed_verdicts_with_reasons(
        &[
            (curated_unjudged, true, VerdictSource::Score, None),
            (waiting_good, true, VerdictSource::Score, None),
            (waiting_bad, true, VerdictSource::Score, None),
        ],
        V,
    )
    .unwrap();
    assert_eq!(
        verdict(&db, curated_unjudged),
        (Some(1), None),
        "grandfathered until judged"
    );
    assert_eq!(
        verdict(&db, waiting_good).1.as_deref(),
        Some("awaiting_judge")
    );

    judge(&db, curated_bad, 0.3, card);
    judge(&db, waiting_good, 0.8, card);
    judge(&db, waiting_bad, 0.1, card);
    let sweep = db.reconcile_judge_gate(true, V).unwrap();
    assert_eq!(
        sweep,
        JudgeGateSweep {
            rejected: 2,
            released: 1
        }
    );
    assert_eq!(
        verdict(&db, curated_bad),
        (Some(0), Some("llm_reject".into()))
    );
    assert_eq!(verdict(&db, curated_unjudged), (Some(1), None));
    assert_eq!(
        verdict(&db, waiting_good),
        (None, None),
        "released for the risen sweep"
    );
    assert_eq!(
        verdict(&db, waiting_bad),
        (Some(0), Some("llm_reject".into()))
    );

    // Released rows pass the boundary now that the judge cleared them.
    db.persist_feed_verdicts_with_reasons(&[(waiting_good, true, VerdictSource::Score, None)], V)
        .unwrap();
    assert_eq!(verdict(&db, waiting_good), (Some(1), None));
    crate::judge_gate::set_active_for_test(false);
}

/// No judge can run: every waiting item is released, so a missing model
/// never strands a source.
#[test]
fn without_a_judge_nothing_is_left_waiting() {
    crate::judge_gate::set_active_for_test(true);
    let db = test_db();
    let w = item(&db, "devto", "x1");
    db.persist_feed_verdicts_with_reasons(&[(w, true, VerdictSource::Score, None)], V)
        .unwrap();
    assert_eq!(verdict(&db, w).1.as_deref(), Some("awaiting_judge"));
    crate::judge_gate::set_active_for_test(false);
    let sweep = db.reconcile_judge_gate(false, V).unwrap();
    assert_eq!(sweep.released, 1);
    assert_eq!(verdict(&db, w), (None, None));
    db.persist_feed_verdicts_with_reasons(&[(w, true, VerdictSource::Score, None)], V)
        .unwrap();
    assert_eq!(verdict(&db, w), (Some(1), None), "gate off: score decides");
}

/// The judge's work queue covers the gate: a curated or waiting gated item
/// with only a thin judgment is selected (14-day window, gate floor); an
/// ungated item with any judgment is not; a gated item already rejected for
/// another reason is not.
#[test]
fn the_judge_queue_includes_gated_items_without_a_card_aware_judgment() {
    let db = test_db();
    let (gated_curated, gated_old_thin, ungated_judged, gated_twin) = (
        item(&db, "devto", "q1"),
        item(&db, "mastodon", "q2"),
        item(&db, "hackernews", "q3"),
        item(&db, "reddit", "q4"),
    );
    {
        let conn = db.conn.lock();
        conn.execute("UPDATE source_items SET relevance_score = 0.6", [])
            .unwrap();
        conn.execute(
            "UPDATE source_items SET created_at = datetime('now', '-10 days') WHERE id = ?1",
            params![gated_old_thin],
        )
        .unwrap();
        conn.execute(
            "UPDATE source_items SET feed_relevant = 0, feed_verdict_reason = 'duplicate_curated' WHERE id = ?1",
            params![gated_twin],
        )
        .unwrap();
        conn.execute(
            "UPDATE source_items SET feed_relevant = 1 WHERE id = ?1",
            params![gated_curated],
        )
        .unwrap();
    }
    for id in [gated_curated, gated_old_thin, ungated_judged, gated_twin] {
        judge(&db, id, 0.9, "v6");
    }
    let mut got = db.get_unjudged_item_ids(0.25, 0.37, 40).unwrap();
    got.sort_unstable();
    assert_eq!(got, vec![gated_curated, gated_old_thin]);
}
