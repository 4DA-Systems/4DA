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

/// How fresh the LATEST briefing's filter verdicts must be to bind the
/// display surfaces (AD-035, `brief_verdict_display`). Independent of reuse.
pub(super) const BRIEFING_REUSE_WINDOW_HOURS: f64 = 4.0;

/// `kv_store` key: the fingerprint the latest briefing was written from.
const FINGERPRINT_KV_KEY: &str = "brief_fingerprint_v1";

/// Record the facts fingerprint a persisted briefing was written from.
pub(super) fn remember_fingerprint(db: &crate::db::Database, briefing_id: i64, fingerprint: &str) {
    let value = serde_json::json!({ "id": briefing_id, "fp": fingerprint }).to_string();
    if let Err(e) = db.set_kv(FINGERPRINT_KV_KEY, &value) {
        tracing::warn!(target: "4da::briefing", error = %e, "briefing fingerprint not persisted");
    }
}

/// The latest briefing, in the same response shape as a fresh generation
/// (plus `"cached": true`), when it was written TODAY (local time) from the
/// facts the caller holds now. `None` means "regenerate", including on any
/// read error, a briefing with no recorded fingerprint, or clock skew: reuse
/// must fail toward regeneration, never toward stale intelligence.
pub(super) fn try_reuse_recent_briefing(
    db: &crate::db::Database,
    fingerprint: &str,
) -> Option<serde_json::Value> {
    let (id, content, model, item_count, created_at, age_hours, today): (
        i64,
        String,
        Option<String>,
        i64,
        String,
        f64,
        bool,
    ) = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT id, content, model, item_count, created_at,
                    (julianday('now') - julianday(created_at)) * 24.0,
                    date(created_at, 'localtime') = date('now', 'localtime')
             FROM briefings ORDER BY created_at DESC, id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get::<_, i64>(6)? != 0,
                ))
            },
        )
        .ok()?
    };
    if age_hours < 0.0 || !today {
        return None;
    }
    let stored: serde_json::Value = db
        .get_kv(FINGERPRINT_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    let same_brief = stored.get("id").and_then(serde_json::Value::as_i64) == Some(id);
    let same_facts = stored.get("fp").and_then(serde_json::Value::as_str) == Some(fingerprint);
    if !(same_brief && same_facts) {
        info!(
            target: "4da::briefing",
            same_brief,
            same_facts,
            "Auto-trigger regenerating — the facts changed since today's briefing"
        );
        return None;
    }
    info!(
        target: "4da::briefing",
        age_hours = format!("{age_hours:.1}"),
        item_count,
        "Auto-trigger reusing today's briefing — facts unchanged"
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
        remember_fingerprint(db, id, fp);
        id
    }

    /// Today's briefing, same facts: reused in the generation response
    /// shape, marked cached, with zero LLM involvement.
    #[test]
    fn auto_reuse_returns_todays_briefing_while_the_facts_hold() {
        let _guard = REUSE_TEST_LOCK.lock().unwrap();
        let db = crate::test_utils::test_db();
        save(&db, "## Fresh brief", "fp-a");

        let cached = try_reuse_recent_briefing(&db, "fp-a").expect("same day, same facts");
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
        assert!(try_reuse_recent_briefing(&db, "fp-b").is_none());
    }

    /// Yesterday's briefing is never today's, however fresh its facts.
    #[test]
    fn auto_reuse_declines_an_earlier_day() {
        let db = crate::test_utils::test_db();
        save(&db, "## Old brief", "fp-a");
        backdate_latest_briefing(&db, 30.0);
        assert!(try_reuse_recent_briefing(&db, "fp-a").is_none());
    }

    /// A briefing persisted without a fingerprint (an older build, or a
    /// different latest row) is not trusted to describe today's facts.
    #[test]
    fn auto_reuse_declines_without_a_matching_fingerprint_record() {
        let db = crate::test_utils::test_db();
        db.save_briefing("## No fingerprint", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        assert!(
            try_reuse_recent_briefing(&db, "fp-a").is_none(),
            "nothing recorded"
        );

        let first = save(&db, "## First", "fp-a");
        db.save_briefing("## Newer, unrecorded", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        assert!(first > 0);
        assert!(
            try_reuse_recent_briefing(&db, "fp-a").is_none(),
            "the recorded fingerprint belongs to an older briefing"
        );
    }

    /// No persisted briefing (first run) and clock-skewed future briefings
    /// both fall through to regeneration.
    #[test]
    fn auto_reuse_declines_without_a_sane_briefing() {
        let db = crate::test_utils::test_db();
        assert!(
            try_reuse_recent_briefing(&db, "fp").is_none(),
            "empty table"
        );
        save(&db, "## From the future", "fp");
        backdate_latest_briefing(&db, -2.0);
        assert!(
            try_reuse_recent_briefing(&db, "fp").is_none(),
            "negative age (clock skew) must regenerate, not reuse"
        );
    }
}
