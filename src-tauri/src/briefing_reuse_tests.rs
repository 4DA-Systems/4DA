// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `briefing_reuse` (loaded via #[path] so the production
//! module stays under the Rust file-size ceiling).

use super::*;

// try_reuse_recent_briefing writes the process-global LATEST_BRIEFING on
// success; tests that can reach that write serialize on the lock every
// LATEST_BRIEFING test shares (digest_config's lifecycle test writes it
// too, and a private lock here did not stop that race).
use crate::digest_config::LATEST_BRIEFING_TEST_LOCK as REUSE_TEST_LOCK;

fn backdate_latest_briefing(db: &crate::db::Database, hours: f64) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE briefings SET created_at = datetime('now', ?1 || ' hours')
         WHERE id = (SELECT MAX(id) FROM briefings)",
        rusqlite::params![-hours],
    )
    .unwrap();
}

fn save(db: &crate::db::Database, text: &str, fp: &str) -> i64 {
    let id = db
        .save_briefing(text, Some("claude-sonnet-5"), 7, Some(6290), Some(32000))
        .unwrap();
    remember_fingerprint(db, id, fp, &[], true);
    id
}

/// Today's briefing, same facts: reused in the generation response
/// shape, marked cached, with zero LLM involvement.
#[test]
fn auto_reuse_returns_todays_briefing_while_the_facts_hold() {
    let _guard = REUSE_TEST_LOCK.lock().unwrap();
    let db = crate::test_utils::test_db();
    save(&db, "## Fresh brief", "fp-a");

    let cached = try_reuse_recent_briefing(&db, "fp-a", &[], true).expect("same day, same facts");
    assert_eq!(cached["success"], true);
    assert_eq!(cached["briefing"], "## Fresh brief");
    assert_eq!(cached["item_count"], 7);
    assert_eq!(cached["model"], "claude-sonnet-5");
    assert_eq!(cached["cached"], true);
    assert_eq!(cached["auto_triggered"], true);
    assert_eq!(
        crate::digest_config::get_latest_briefing_text().as_deref(),
        Some("## Fresh brief"),
        "in-memory cache (TTS/handoff) must track the reused content"
    );
}

/// A new confirmed advisory, a fixed install, a new breaking release:
/// the fingerprint moves and the brief is rewritten.
#[test]
fn auto_reuse_regenerates_when_the_facts_change() {
    let db = crate::test_utils::test_db();
    save(&db, "## Brief", "fp-a");
    assert!(try_reuse_recent_briefing(&db, "fp-b", &[], true).is_none());
}

/// Yesterday's briefing is never today's, however fresh its facts.
#[test]
fn auto_reuse_declines_an_earlier_day() {
    let db = crate::test_utils::test_db();
    save(&db, "## Old brief", "fp-a");
    backdate_latest_briefing(&db, 30.0);
    assert!(try_reuse_recent_briefing(&db, "fp-a", &[], true).is_none());
}

/// A briefing persisted without a fingerprint (an older build, or a
/// different latest row) is not trusted to describe today's facts.
#[test]
fn auto_reuse_declines_without_a_matching_fingerprint_record() {
    let db = crate::test_utils::test_db();
    db.save_briefing("## No fingerprint", Some("m"), 3, Some(0), Some(0))
        .unwrap();
    assert!(
        try_reuse_recent_briefing(&db, "fp-a", &[], true).is_none(),
        "nothing recorded"
    );

    let first = save(&db, "## First", "fp-a");
    db.save_briefing("## Newer, unrecorded", Some("m"), 3, Some(0), Some(0))
        .unwrap();
    assert!(first > 0);
    assert!(
        try_reuse_recent_briefing(&db, "fp-a", &[], true).is_none(),
        "the recorded fingerprint belongs to an older briefing"
    );
}

/// No persisted briefing (first run) and clock-skewed future briefings
/// both fall through to regeneration.
#[test]
fn auto_reuse_declines_without_a_sane_briefing() {
    let db = crate::test_utils::test_db();
    assert!(
        try_reuse_recent_briefing(&db, "fp", &[], true).is_none(),
        "empty table"
    );
    save(&db, "## From the future", "fp");
    backdate_latest_briefing(&db, -2.0);
    assert!(
        try_reuse_recent_briefing(&db, "fp", &[], true).is_none(),
        "negative age (clock skew) must regenerate, not reuse"
    );
}

// ---- Daily cap (audit 2026-10-07: four auto briefs on 2026-10-06) ----

fn utc(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse().expect("rfc3339")
}

fn plus_ten() -> chrono::FixedOffset {
    chrono::FixedOffset::east_opt(10 * 3600).expect("UTC+10")
}

/// A briefing persisted at an exact UTC `created_at`.
fn save_at(db: &crate::db::Database, created: &str, fp: &str, act: &[String]) -> i64 {
    save_in(db, created, fp, act, Readiness::Checked)
}

/// A briefing persisted at an exact UTC `created_at`, recorded on its
/// local (UTC+10) day with the given readiness.
fn save_in(
    db: &crate::db::Database,
    created: &str,
    fp: &str,
    act: &[String],
    readiness: Readiness,
) -> i64 {
    let id = db
        .save_briefing("## Brief", Some("claude-sonnet-5"), 5, Some(0), Some(0))
        .unwrap();
    db.conn
        .lock()
        .execute(
            "UPDATE briefings SET created_at = ?1 WHERE id = ?2",
            rusqlite::params![created, id],
        )
        .unwrap();
    let local = chrono::NaiveDateTime::parse_from_str(created, "%Y-%m-%d %H:%M:%S").unwrap()
        + chrono::Duration::hours(10);
    let day = local.format("%Y-%m-%d").to_string();
    remember_fingerprint_on(db, &day, id, fp, act, readiness);
    id
}

fn act(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| (*s).to_string()).collect()
}

/// Three briefs on local 2026-10-06 (UTC+10): the 4th auto call reuses
/// the latest even though the fingerprint moved; the 3rd did not.
#[test]
fn the_fourth_auto_call_of_a_local_day_reuses() {
    let _guard = REUSE_TEST_LOCK.lock().unwrap();
    let db = crate::test_utils::test_db();
    let a = act(&["npm:x:navcal#GHSA-a"]);
    let now = utc("2026-10-06T12:00:00Z"); // 22:00 local
    save_at(&db, "2026-10-05 21:00:00", "fp-1", &a); // 07:00 local
    save_at(&db, "2026-10-06 01:00:00", "fp-2", &a); // 11:00 local
    assert!(
        try_reuse_at(&db, "fp-3", &a, true, now, plus_ten()).is_none(),
        "two briefs today: the facts changed, regenerate"
    );
    save_at(&db, "2026-10-06 05:00:00", "fp-3", &a); // 15:00 local
    let cached = try_reuse_at(&db, "fp-4", &a, true, now, plus_ten()).expect("capped");
    assert_eq!(cached["cached"], true);
}

/// Past the cap, a NEW high/critical advisory still regenerates.
#[test]
fn a_new_act_now_advisory_breaks_through_the_cap() {
    let db = crate::test_utils::test_db();
    let a = act(&["npm:x:navcal#GHSA-a"]);
    let now = utc("2026-10-06T12:00:00Z");
    for t in [
        "2026-10-05 21:00:00",
        "2026-10-06 01:00:00",
        "2026-10-06 05:00:00",
    ] {
        save_at(&db, t, "fp-1", &a);
    }
    let grown = act(&["npm:x:navcal#GHSA-a", "npm:x:navcal#GHSA-b"]);
    assert!(try_reuse_at(&db, "fp-2", &grown, true, now, plus_ten()).is_none());
    let other = act(&["npm:y:navcal#GHSA-c"]);
    assert!(try_reuse_at(&db, "fp-2", &other, true, now, plus_ten()).is_none());
}

/// The day is the user's LOCAL day. At 01:30 on 2026-10-07 (UTC+10) the
/// three briefs written the evening before do not count; in UTC they
/// would all be "2026-10-06" and the 4th call would be capped.
#[test]
fn the_cap_counts_the_local_day_not_the_utc_day() {
    let _guard = REUSE_TEST_LOCK.lock().unwrap();
    let db = crate::test_utils::test_db();
    let a = act(&[]);
    for t in [
        "2026-10-06 10:00:00",
        "2026-10-06 12:00:00",
        "2026-10-06 13:00:00",
    ] {
        save_at(&db, t, "fp-1", &a); // 20:00-23:00 local on the 6th
    }
    save_at(&db, "2026-10-06 14:30:00", "fp-1", &a); // 00:30 local on the 7th
    let now = utc("2026-10-06T15:30:00Z"); // 01:30 local on the 7th
    assert!(
        try_reuse_at(&db, "fp-2", &a, true, now, plus_ten()).is_none(),
        "one brief so far on local 2026-10-07"
    );
    let utc_offset = chrono::FixedOffset::east_opt(0).expect("UTC");
    assert!(
        try_reuse_at(&db, "fp-2", &a, true, now, utc_offset).is_some(),
        "four briefs on the UTC day"
    );
    assert!(
        try_reuse_at(&db, "fp-1", &a, true, now, plus_ten()).is_some(),
        "today's brief, same facts"
    );
}

/// Two auto triggers in flight at once (live 2026-10-03T19:50:10Z and
/// :13Z) with changed facts: under the generation gate the second sees
/// the first one's brief and reuses it. Without the gate both pass the
/// decision before either writes, and the day gets two briefs for one
/// change (and, at two briefs, a fourth past the cap).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::await_holding_lock)] // the guard serializes LATEST_BRIEFING writers
async fn concurrent_triggers_write_one_brief_not_two() {
    let _guard = REUSE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let db = std::sync::Arc::new(crate::test_utils::test_db());
    save(&db, "## Fresh brief", "fp-a");
    save(&db, "## Fresh brief", "fp-a");
    let run = |db: std::sync::Arc<crate::db::Database>| async move {
        let _gate = BRIEF_GENERATION_GATE.lock().await;
        if try_reuse_recent_briefing(&db, "fp-b", &[], true).is_none() {
            // The facts build + model call the gate spans.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            save(&db, "## Fresh brief", "fp-b");
        }
    };
    let (a, b) = tokio::join!(tokio::spawn(run(db.clone())), tokio::spawn(run(db.clone())));
    a.expect("first trigger");
    b.expect("second trigger");
    let count: i64 = db
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM briefings", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 3, "one regeneration for one change of facts");
}

/// A record written before the cap existed has no act-now list: it cannot
/// prove nothing new arrived, so the cap never applies to it.
#[test]
fn the_cap_needs_a_recorded_act_now_list() {
    let db = crate::test_utils::test_db();
    let now = utc("2026-10-06T12:00:00Z");
    let mut last = 0;
    for t in [
        "2026-10-05 21:00:00",
        "2026-10-06 01:00:00",
        "2026-10-06 05:00:00",
    ] {
        last = save_at(&db, t, "fp-1", &[]);
    }
    let legacy = serde_json::json!({ "id": last, "fp": "fp-1" }).to_string();
    db.set_kv(FINGERPRINT_KV_KEY, &legacy).unwrap();
    assert!(try_reuse_at(&db, "fp-2", &[], true, now, plus_ten()).is_none());
}

const TODAY_AT: [&str; 3] = [
    "2026-10-05 21:00:00",
    "2026-10-06 01:00:00",
    "2026-10-06 05:00:00",
];

/// PR #961 review: the advisory check's readiness is part of what the
/// brief says (an all-clear only when ready), so a day that has not yet
/// been briefed in the new readiness gets one, in either direction, even
/// past the daily cap.
#[test]
fn a_readiness_the_day_has_not_seen_regenerates_even_past_the_cap() {
    use crate::brief_facts::with_advisory_state;
    use crate::osv::readiness::AdvisorySyncState as S;
    let now = utc("2026-10-06T12:00:00Z");
    let a = act(&[]);
    let fp = |s| with_advisory_state("facts", Some(s));
    for (before, after) in [(S::Ready, S::Failed), (S::NotStarted, S::Ready)] {
        let db = crate::test_utils::test_db();
        for t in TODAY_AT {
            save_in(&db, t, &fp(before), &a, Readiness::of(Some(before)));
        }
        assert_ne!(fp(before), fp(after), "the identity carries readiness");
        assert!(
            try_reuse_at(
                &db,
                &fp(after),
                &a,
                Readiness::of(Some(after)),
                now,
                plus_ten()
            )
            .is_none(),
            "{before:?} -> {after:?} must regenerate"
        );
        // Same readiness past the cap still reuses (the cap holds).
        assert!(
            try_reuse_at(
                &db,
                "other",
                &a,
                Readiness::of(Some(before)),
                now,
                plus_ten()
            )
            .is_some(),
            "{before:?}: the cap still applies without a readiness change"
        );
    }
}

/// Local 2026-10-25: four auto briefs against a cap of three. Every
/// six-hourly advisory sync (and every start) passes through `Syncing`
/// and `Matching`; each pass flipped "ready" and bought a brief past the
/// cap, and the return to `Ready` bought another. A sync in progress
/// says nothing new, and a readiness today was already briefed in is not
/// news either.
#[test]
fn a_sync_passing_through_never_buys_a_brief_past_the_cap() {
    use crate::osv::readiness::AdvisorySyncState as S;
    let now = utc("2026-10-06T12:00:00Z");
    let a = act(&["crates.io:zip:app#GHSA-a"]);
    let db = crate::test_utils::test_db();
    for t in TODAY_AT {
        save_in(&db, t, "fp-ready", &a, Readiness::Checked);
    }
    for state in [S::Syncing, S::Matching, S::Ready] {
        assert!(
            try_reuse_at(
                &db,
                "fp-moved",
                &a,
                Readiness::of(Some(state)),
                now,
                plus_ten()
            )
            .is_some(),
            "{state:?} past the cap holds"
        );
    }
    // A day briefed both checked and unchecked has nothing left to gain.
    save_in(
        &db,
        "2026-10-06 06:00:00",
        "fp-failed",
        &a,
        Readiness::Unchecked,
    );
    for state in [S::Ready, S::Failed] {
        assert!(
            try_reuse_at(
                &db,
                "fp-moved",
                &a,
                Readiness::of(Some(state)),
                now,
                plus_ten()
            )
            .is_some(),
            "{state:?} was already briefed today"
        );
    }
}

/// The other 2026-10-25 hole: an act-now advisory that dropped out of one
/// brief and came back counted as "new" against the latest brief alone.
/// It was in today's brief already; only an advisory no brief today
/// carried breaks through.
#[test]
fn an_act_now_advisory_that_comes_back_is_not_new() {
    let now = utc("2026-10-06T12:00:00Z");
    let zod = act(&["npm:zod:app#GHSA-z"]);
    let db = crate::test_utils::test_db();
    save_at(&db, TODAY_AT[0], "fp-1", &zod);
    save_at(&db, TODAY_AT[1], "fp-2", &zod);
    save_at(&db, TODAY_AT[2], "fp-3", &act(&[])); // zod briefly absent
    assert!(
        try_reuse_at(&db, "fp-4", &zod, true, now, plus_ten()).is_some(),
        "zod was in today's first two briefs"
    );
    let new = act(&["npm:zod:app#GHSA-z", "npm:next:app#GHSA-n"]);
    assert!(
        try_reuse_at(&db, "fp-4", &new, true, now, plus_ten()).is_none(),
        "an advisory no brief today carried still breaks through"
    );
}

/// The record of today's briefs starts over on a new local day.
#[test]
fn todays_record_starts_over_on_a_new_day() {
    let db = crate::test_utils::test_db();
    let x = act(&["npm:x:app#GHSA-x"]);
    save_at(&db, "2026-10-05 05:00:00", "fp-1", &x); // local 10-05
    save_at(&db, "2026-10-05 21:00:00", "fp-2", &act(&[])); // local 10-06
    let stored: serde_json::Value =
        serde_json::from_str(&db.get_kv(FINGERPRINT_KV_KEY).unwrap().unwrap()).unwrap();
    assert_eq!(stored["day"], "2026-10-06");
    assert_eq!(stored["act_today"], serde_json::json!([]));
    assert_eq!(stored["readiness_today"], serde_json::json!(["checked"]));
}
