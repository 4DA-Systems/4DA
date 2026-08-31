// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Auto-trigger briefing reuse window (2026-08-31 live audit).
//!
//! Split from `digest_commands.rs` for size hygiene (declared there via
//! `#[path]`). Measured live 2026-08-31 on the founder's instance: opening
//! the main window fired `generate_ai_briefing` — 32 seconds and 6,290
//! tokens — with a perfectly fresh briefing sitting in the `briefings`
//! table. The AUTO-triggered path now reuses that briefing when it is young
//! and nothing critical arrived since; explicit user triggers always
//! regenerate.

use tracing::info;

/// How fresh a persisted briefing must be for an AUTO-triggered generation
/// to reuse it instead of regenerating.
pub(super) const BRIEFING_REUSE_WINDOW_HOURS: f64 = 4.0;

/// Auto-trigger reuse gate: the latest persisted briefing, in the same
/// response shape as a fresh generation (plus `"cached": true`), when it is
/// younger than [`BRIEFING_REUSE_WINDOW_HOURS`] AND no critical-urgency item
/// has arrived since it was written. `None` means "regenerate" — including on
/// any read error, negative age (clock skew), or a critical arrival: reuse
/// must fail toward regeneration, never toward stale intelligence.
///
/// "Critical-urgency" is data-true, not vibes: `item_necessity` rows the
/// scoring pipeline stamped `necessity_urgency = 'immediate'` (the CVE /
/// security-critical class) on items created after the briefing.
pub(super) fn try_reuse_recent_briefing(db: &crate::db::Database) -> Option<serde_json::Value> {
    let (content, model, item_count, created_at, age_hours): (
        String,
        Option<String>,
        i64,
        String,
        f64,
    ) = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT content, model, item_count, created_at,
                    (julianday('now') - julianday(created_at)) * 24.0
             FROM briefings ORDER BY created_at DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .ok()?
    };
    if !(0.0..BRIEFING_REUSE_WINDOW_HOURS).contains(&age_hours) {
        return None;
    }
    let new_critical: i64 = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM item_necessity n
             JOIN source_items si ON si.id = n.source_item_id
             WHERE n.necessity_urgency = 'immediate'
               AND si.created_at > ?1",
            rusqlite::params![created_at],
            |r| r.get(0),
        )
        .ok()?
    };
    if new_critical > 0 {
        info!(
            target: "4da::briefing",
            new_critical,
            age_hours = format!("{age_hours:.1}"),
            "Auto-trigger regenerating despite fresh briefing — critical items arrived since"
        );
        return None;
    }
    info!(
        target: "4da::briefing",
        age_hours = format!("{age_hours:.1}"),
        item_count,
        "Auto-trigger reusing persisted briefing — no regeneration"
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
    // success; tests that can reach that write serialize on this lock so the
    // shape test's global assertion cannot race a parallel test's write.
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

    fn insert_item_with_urgency(db: &crate::db::Database, key: &str, urgency: Option<&str>) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, created_at)
             VALUES ('cve', ?1, NULL, ?1, '', ?1, X'', datetime('now'))",
            rusqlite::params![key],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        if let Some(u) = urgency {
            conn.execute(
                "INSERT INTO item_necessity (source_item_id, necessity_score, necessity_reason, necessity_category, necessity_urgency, scored_at)
                 VALUES (?1, 0.95, 'test', 'security', ?2, datetime('now'))",
                rusqlite::params![id, u],
            )
            .unwrap();
        }
    }

    /// The audit's exact waste: a fresh persisted briefing + an auto trigger.
    /// Reuse returns the persisted briefing in the generation response shape,
    /// marked cached, with zero LLM involvement.
    #[test]
    fn auto_reuse_returns_fresh_briefing_in_response_shape() {
        let _guard = REUSE_TEST_LOCK.lock().unwrap();
        let db = crate::test_utils::test_db();
        db.save_briefing(
            "## Fresh brief",
            Some("claude-sonnet-4-6"),
            7,
            Some(6290),
            Some(32000),
        )
        .unwrap();

        let cached = try_reuse_recent_briefing(&db).expect("fresh briefing must be reused");
        assert_eq!(cached["success"], true);
        assert_eq!(cached["briefing"], "## Fresh brief");
        assert_eq!(cached["item_count"], 7);
        assert_eq!(cached["model"], "claude-sonnet-4-6");
        assert_eq!(cached["cached"], true);
        assert_eq!(cached["auto_triggered"], true);
        assert_eq!(
            crate::digest_config::get_latest_briefing_text().as_deref(),
            Some("## Fresh brief"),
            "in-memory cache (TTS/handoff) must track the reused content"
        );
    }

    /// Outside the window the auto path regenerates — reuse never serves a
    /// briefing older than BRIEFING_REUSE_WINDOW_HOURS.
    #[test]
    fn auto_reuse_declines_a_stale_briefing() {
        let db = crate::test_utils::test_db();
        db.save_briefing("## Old brief", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        backdate_latest_briefing(&db, BRIEFING_REUSE_WINDOW_HOURS + 0.5);
        assert!(try_reuse_recent_briefing(&db).is_none());
    }

    /// A critical-urgency arrival ('immediate' necessity) since the briefing
    /// forces regeneration; lower urgencies and unscored items do not.
    #[test]
    fn auto_reuse_declines_when_critical_items_arrived() {
        let _guard = REUSE_TEST_LOCK.lock().unwrap();
        let db = crate::test_utils::test_db();
        db.save_briefing("## Brief", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        backdate_latest_briefing(&db, 1.0);

        insert_item_with_urgency(&db, "aware-1", Some("awareness"));
        insert_item_with_urgency(&db, "unscored-1", None);
        assert!(
            try_reuse_recent_briefing(&db).is_some(),
            "non-critical arrivals must not bust the reuse window"
        );

        insert_item_with_urgency(&db, "critical-1", Some("immediate"));
        assert!(
            try_reuse_recent_briefing(&db).is_none(),
            "an 'immediate' arrival after the briefing forces regeneration"
        );
    }

    /// No persisted briefing (first run) and clock-skewed future briefings
    /// both fall through to regeneration.
    #[test]
    fn auto_reuse_declines_without_a_sane_briefing() {
        let db = crate::test_utils::test_db();
        assert!(try_reuse_recent_briefing(&db).is_none(), "empty table");

        db.save_briefing("## From the future", Some("m"), 3, Some(0), Some(0))
            .unwrap();
        backdate_latest_briefing(&db, -2.0); // 2 hours in the future
        assert!(
            try_reuse_recent_briefing(&db).is_none(),
            "negative age (clock skew) must regenerate, not reuse"
        );
    }
}
