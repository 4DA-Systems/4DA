// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Auto-trigger briefing reuse: one brief a day, plus one whenever the facts
//! it was written from change.
//!
//! Split from `digest_commands.rs` for size hygiene (declared there via
//! `#[path]`).
//!
//! History. 2026-08-31: opening the window regenerated a perfectly fresh
//! briefing (32 s, 6,290 tokens), so AUTO triggers got a 4-hour reuse
//! window. 2026-10-01: 168 briefs in 6 days (~28/day), 73% of topics
//! repeating the previous one — because no frontend caller ever passed
//! `auto: true`, and because any "immediate" item (including a false hono
//! "Critical") busted the window. Decision 2 replaces the window with the
//! rule a reader expects: the brief is today's brief until something it
//! reports changes. "Changes" is the `brief_facts` fingerprint — confirmed
//! security (urgency, installed versions, fix) and breaking upgrades —
//! not "another article arrived".

use tracing::info;

use crate::brief_cadence::{has_new_act_now, DAILY_AUTO_BRIEF_CAP};

/// How fresh the LATEST briefing's filter verdicts must be to bind the
/// display surfaces (AD-035, `brief_verdict_display`). Independent of reuse.
pub(super) const BRIEFING_REUSE_WINDOW_HOURS: f64 = 4.0;

/// One brief generation at a time. The reuse/cap decision reads the latest
/// brief and today's count, and the write lands up to ~30 s later (facts
/// build + model call); two triggers in that window both saw "regenerate"
/// and both wrote. Live 2026-10-03T19:50:10Z/19:50:13Z two auto triggers
/// overlapped (both reused that time, because the facts held). Held from
/// before the decision until the brief is persisted, the second trigger sees
/// the first one's brief and reuses it, so the daily cap holds under
/// concurrency too.
pub(super) static BRIEF_GENERATION_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `kv_store` key: the fingerprint the latest briefing was written from.
const FINGERPRINT_KV_KEY: &str = "brief_fingerprint_v1";

/// Record the facts a persisted briefing was written from: the fingerprint,
/// and the act-now identities the daily cap compares against.
pub(super) fn remember_fingerprint(
    db: &crate::db::Database,
    briefing_id: i64,
    fingerprint: &str,
    act_now: &[String],
) {
    let value =
        serde_json::json!({ "id": briefing_id, "fp": fingerprint, "act": act_now }).to_string();
    if let Err(e) = db.set_kv(FINGERPRINT_KV_KEY, &value) {
        tracing::warn!(target: "4da::briefing", error = %e, "briefing fingerprint not persisted");
    }
}

/// The latest briefing, in the same response shape as a fresh generation
/// (plus `"cached": true`), when it was written TODAY (local time) and either
/// the facts are unchanged, or today already holds [`DAILY_AUTO_BRIEF_CAP`]
/// briefs and no NEW act-now fact appeared. `None` means "regenerate",
/// including on any read error, a briefing with no recorded fingerprint, or
/// clock skew: reuse must fail toward regeneration, never toward stale
/// intelligence. A manual Regenerate never calls this.
pub(super) fn try_reuse_recent_briefing(
    db: &crate::db::Database,
    fingerprint: &str,
    act_now: &[String],
) -> Option<serde_json::Value> {
    let offset = *chrono::Local::now().offset();
    try_reuse_at(db, fingerprint, act_now, chrono::Utc::now(), offset)
}

/// The latest briefing row: id, content, model, item_count, created_at.
type LatestBriefing = (i64, String, Option<String>, i64, String);

fn latest_briefing(db: &crate::db::Database) -> Option<LatestBriefing> {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT id, content, model, item_count, created_at
         FROM briefings ORDER BY created_at DESC, id DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .ok()
}

/// Midnight of `now`'s local day (at `offset`), in UTC: the boundary the
/// `briefings.created_at` column (UTC, SQLite `datetime('now')`) compares to.
fn local_day_start_utc(
    now: chrono::DateTime<chrono::Utc>,
    offset: chrono::FixedOffset,
) -> chrono::NaiveDateTime {
    let local_midnight = now
        .with_timezone(&offset)
        .date_naive()
        .and_time(chrono::NaiveTime::MIN);
    local_midnight - chrono::Duration::seconds(i64::from(offset.local_minus_utc()))
}

fn briefs_since(db: &crate::db::Database, since: &str) -> i64 {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT COUNT(*) FROM briefings WHERE created_at >= ?1",
        [since],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Past the daily cap, with nothing new to act on? A record from before the
/// cap existed has no act-now list: it cannot prove nothing new arrived.
fn capped(
    db: &crate::db::Database,
    stored: &serde_json::Value,
    act_now: &[String],
    day_start: &str,
) -> bool {
    let recorded: Option<Vec<String>> = stored
        .get("act")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    recorded.is_some_and(|r| !has_new_act_now(&r, act_now))
        && briefs_since(db, day_start) >= DAILY_AUTO_BRIEF_CAP
}

fn try_reuse_at(
    db: &crate::db::Database,
    fingerprint: &str,
    act_now: &[String],
    now: chrono::DateTime<chrono::Utc>,
    offset: chrono::FixedOffset,
) -> Option<serde_json::Value> {
    let (id, content, model, item_count, created_at) = latest_briefing(db)?;
    let created = chrono::NaiveDateTime::parse_from_str(&created_at, "%Y-%m-%d %H:%M:%S").ok()?;
    let day_start = local_day_start_utc(now, offset);
    if created > now.naive_utc() || created < day_start {
        return None;
    }
    let stored: serde_json::Value = db
        .get_kv(FINGERPRINT_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    let same_brief = stored.get("id").and_then(serde_json::Value::as_i64) == Some(id);
    let same_facts = stored.get("fp").and_then(serde_json::Value::as_str) == Some(fingerprint);
    let day_start = day_start.format("%Y-%m-%d %H:%M:%S").to_string();
    let reason = if same_brief && same_facts {
        "facts unchanged"
    } else if same_brief && capped(db, &stored, act_now, &day_start) {
        "daily cap reached, no new act-now fact"
    } else {
        info!(
            target: "4da::briefing",
            same_brief,
            same_facts,
            "Auto-trigger regenerating — the facts changed since today's briefing"
        );
        return None;
    };
    info!(
        target: "4da::briefing",
        item_count,
        reason,
        "Auto-trigger reusing today's briefing"
    );
    // Keep the in-memory cache (TTS / handoff readers) aligned with what the
    // UI is about to show.
    *crate::digest_config::LATEST_BRIEFING.lock() = Some(content.clone());
    Some(serde_json::json!({
        "success": true,
        "briefing": content,
        "item_count": item_count,
        "model": model,
        "latency_ms": 0,
        "cached": true,
        "briefing_created_at": created_at,
        "auto_triggered": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // try_reuse_recent_briefing writes the process-global LATEST_BRIEFING on
    // success; tests that can reach that write serialize on this lock.
    static REUSE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        remember_fingerprint(db, id, fp, &[]);
        id
    }

    /// Today's briefing, same facts: reused in the generation response
    /// shape, marked cached, with zero LLM involvement.
    #[test]
    fn auto_reuse_returns_todays_briefing_while_the_facts_hold() {
        let _guard = REUSE_TEST_LOCK.lock().unwrap();
        let db = crate::test_utils::test_db();
        save(&db, "## Fresh brief", "fp-a");

        let cached = try_reuse_recent_briefing(&db, "fp-a", &[]).expect("same day, same facts");
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
        assert!(try_reuse_recent_briefing(&db, "fp-b", &[]).is_none());
    }

    /// Yesterday's briefing is never today's, however fresh its facts.
    #[test]
    fn auto_reuse_declines_an_earlier_day() {
        let db = crate::test_utils::test_db();
        save(&db, "## Old brief", "fp-a");
        backdate_latest_briefing(&db, 30.0);
        assert!(try_reuse_recent_briefing(&db, "fp-a", &[]).is_none());
    }

    /// A briefing persisted without a fingerprint (an older build, or a
    /// different latest row) is not trusted to describe today's facts.
    #[test]
    fn auto_reuse_declines_without_a_matching_fingerprint_record() {
        let db = crate::test_utils::test_db();
        db.save_briefing("## No fingerprint", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        assert!(
            try_reuse_recent_briefing(&db, "fp-a", &[]).is_none(),
            "nothing recorded"
        );

        let first = save(&db, "## First", "fp-a");
        db.save_briefing("## Newer, unrecorded", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        assert!(first > 0);
        assert!(
            try_reuse_recent_briefing(&db, "fp-a", &[]).is_none(),
            "the recorded fingerprint belongs to an older briefing"
        );
    }

    /// No persisted briefing (first run) and clock-skewed future briefings
    /// both fall through to regeneration.
    #[test]
    fn auto_reuse_declines_without_a_sane_briefing() {
        let db = crate::test_utils::test_db();
        assert!(
            try_reuse_recent_briefing(&db, "fp", &[]).is_none(),
            "empty table"
        );
        save(&db, "## From the future", "fp");
        backdate_latest_briefing(&db, -2.0);
        assert!(
            try_reuse_recent_briefing(&db, "fp", &[]).is_none(),
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
        remember_fingerprint(db, id, fp, act);
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
            try_reuse_at(&db, "fp-3", &a, now, plus_ten()).is_none(),
            "two briefs today: the facts changed, regenerate"
        );
        save_at(&db, "2026-10-06 05:00:00", "fp-3", &a); // 15:00 local
        let cached = try_reuse_at(&db, "fp-4", &a, now, plus_ten()).expect("capped");
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
        assert!(try_reuse_at(&db, "fp-2", &grown, now, plus_ten()).is_none());
        let other = act(&["npm:y:navcal#GHSA-c"]);
        assert!(try_reuse_at(&db, "fp-2", &other, now, plus_ten()).is_none());
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
            try_reuse_at(&db, "fp-2", &a, now, plus_ten()).is_none(),
            "one brief so far on local 2026-10-07"
        );
        let utc_offset = chrono::FixedOffset::east_opt(0).expect("UTC");
        assert!(
            try_reuse_at(&db, "fp-2", &a, now, utc_offset).is_some(),
            "four briefs on the UTC day"
        );
        assert!(
            try_reuse_at(&db, "fp-1", &a, now, plus_ten()).is_some(),
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
            if try_reuse_recent_briefing(&db, "fp-b", &[]).is_none() {
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
        assert!(try_reuse_at(&db, "fp-2", &[], now, plus_ten()).is_none());
    }
}
