// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What a headless engine cycle needs to do this time (audit R7/O1).
//!
//! The scheduled cadence is 10 minutes. On a machine where nothing changed
//! (lockfiles untouched, advisory archives current, no interests turned on)
//! a cycle used to re-fetch every registry feed, re-score, and rebuild the
//! Upgrade Plan anyway: measured 2026-11-12 on a fresh 36-project profile,
//! 258 s wall and 267 CPU-seconds per "nothing changed" cycle in a debug
//! build. These gates decide which of those steps has anything to do, so a
//! quiet cycle is a few cheap checks and a receipt.
//!
//! Also here: the engine's daily retention pass, which only the GUI ran.

use std::time::Instant;

use tracing::{info, warn};

/// With every interest off, the always-on registry feeds (new releases of
/// the user's own dependencies) are polled at most this often. Advisories do
/// not depend on it: they come from the OSV mirror on its own cadence.
pub(super) const STACK_ONLY_FETCH_MINUTES: u64 = 60;

/// An Upgrade Plan snapshot whose inputs did not change is rebuilt at most
/// this often (it stays well inside its own `expires_at` horizon).
pub(super) const PLAN_REFRESH_MINUTES: u64 = 60;

/// Seconds between retention passes.
const RETENTION_INTERVAL_SECS: u64 = 86_400;

/// Whether this cycle should fetch sources.
///
/// `last_fetch_age_minutes` is `None` when nothing was ever fetched (or the
/// stamp is unreadable): fetch. With an interest on, the configured refresh
/// interval applies; with none on, only the registry feeds remain and they
/// wait for [`STACK_ONLY_FETCH_MINUTES`].
pub(super) fn fetch_due(
    interests_on: bool,
    last_fetch_age_minutes: Option<u64>,
    interval_minutes: u64,
) -> bool {
    let Some(age) = last_fetch_age_minutes else {
        return true;
    };
    let wait = if interests_on {
        interval_minutes
    } else {
        interval_minutes.max(STACK_ONLY_FETCH_MINUTES)
    };
    age >= wait
}

/// Whether the Upgrade Plan snapshot must be rebuilt this cycle: its inputs
/// (the inventory, the advisory mirror) changed, it is missing, or it is
/// older than [`PLAN_REFRESH_MINUTES`].
pub(super) fn plan_refresh_due(inputs_changed: bool, snapshot_age_minutes: Option<u64>) -> bool {
    inputs_changed || snapshot_age_minutes.is_none_or(|age| age >= PLAN_REFRESH_MINUTES)
}

/// Whether a retention pass is due, given the last one's unix time.
pub(super) fn retention_due(last_run_unix: u64, now_unix: u64) -> bool {
    now_unix.saturating_sub(last_run_unix) >= RETENTION_INTERVAL_SECS
}

/// Whether any opt-in interest source is turned on. Unreadable counts as on,
/// so a database hiccup never silences a source the user asked for.
pub(super) fn interests_enabled() -> bool {
    crate::get_database()
        .ok()
        .and_then(|db| db.get_all_sources().ok())
        .is_none_or(|rows| {
            rows.iter().any(|(source_type, _, enabled, _)| {
                *enabled && crate::sources::source_class::is_interest(source_type)
            })
        })
}

/// Minutes since the newest `sources.last_fetch` (UTC `YYYY-MM-DD HH:MM:SS`).
pub(super) fn last_fetch_age_minutes() -> Option<u64> {
    let last = crate::engine_runs::freshness_snapshot()
        .ok()?
        .last_source_fetch?;
    let naive = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S").ok()?;
    let age = chrono::Utc::now()
        .signed_duration_since(naive.and_utc())
        .num_minutes();
    u64::try_from(age).ok()
}

/// Minutes since the engine last rebuilt the plan snapshot, `None` if never.
pub(super) fn plan_snapshot_age_minutes() -> Option<u64> {
    let last = crate::scheduler_state::get_persisted_timestamp(
        crate::scheduler_state::jobs::PLAN_SNAPSHOT,
    );
    if last == 0 {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(now.saturating_sub(last) / 60)
}

/// The GUI's daily retention, for the engine: old source items (Saved items
/// are kept, `db::history::keep_saved_clause`) and the deep clean, which
/// includes the bookkeeping-table bounds (`db::retention`). It ran only in the
/// GUI's monitoring loop, so an engine-only deployment never pruned anything.
/// Once a day, stamped AFTER it ran; best-effort, never fails the cycle.
pub(super) fn run_daily_retention() {
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return;
    };
    let now = now.as_secs();
    let last =
        crate::scheduler_state::get_persisted_timestamp(crate::scheduler_state::jobs::RETENTION);
    if !retention_due(last, now) {
        return;
    }
    let Ok(db) = crate::get_database() else {
        return;
    };
    let max_age_days = crate::get_settings_manager()
        .lock()
        .get()
        .monitoring
        .retention_days();
    let started = Instant::now();
    let error = match retention_pass(db, max_age_days) {
        Ok(items) => {
            info!(target: "4da::headless", items, max_age_days, "Daily retention complete");
            crate::scheduler_state::persist_run(crate::scheduler_state::jobs::RETENTION, now);
            None
        }
        Err(e) => {
            warn!(target: "4da::headless", error = %e, "Daily retention failed — next cycle retries");
            Some(e.to_string())
        }
    };
    crate::scheduler_state::record_outcome(
        crate::scheduler_state::jobs::RETENTION,
        error.as_deref(),
        started.elapsed().as_millis() as u64,
    );
}

/// One retention pass; returns the source items pruned (the other tables'
/// counts are logged by `run_maintenance` itself).
pub(super) fn retention_pass(
    db: &crate::db::Database,
    max_age_days: u32,
) -> rusqlite::Result<usize> {
    let items = db.cleanup_old_items(max_age_days)?;
    let deep = db.run_maintenance(i64::from(max_age_days))?;
    Ok(items + deep.deleted_items)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 10-minute cadence with every interest off must not re-poll the
    /// registry feeds each time (audit R7/O1).
    #[test]
    fn registry_feeds_alone_wait_an_hour() {
        assert!(!fetch_due(false, Some(10), 10));
        assert!(!fetch_due(false, Some(59), 10));
        assert!(fetch_due(false, Some(60), 10));
        assert!(
            fetch_due(true, Some(10), 10),
            "an interest keeps the configured cadence"
        );
        assert!(!fetch_due(true, Some(9), 10));
        assert!(fetch_due(false, None, 10), "never fetched: fetch");
        assert!(
            fetch_due(false, Some(90), 90),
            "a longer interval still applies"
        );
        assert!(!fetch_due(false, Some(80), 90));
    }

    #[test]
    fn plan_rebuilds_only_when_its_inputs_or_age_say_so() {
        assert!(!plan_refresh_due(false, Some(10)));
        assert!(
            plan_refresh_due(true, Some(10)),
            "inventory or mirror changed"
        );
        assert!(plan_refresh_due(false, None), "never built");
        assert!(plan_refresh_due(false, Some(PLAN_REFRESH_MINUTES)));
    }

    #[test]
    fn retention_runs_daily() {
        assert!(retention_due(0, 1_000_000));
        assert!(!retention_due(1_000_000, 1_000_000 + 3_600));
        assert!(retention_due(1_000_000, 1_000_000 + 86_400));
        assert!(
            !retention_due(2_000_000, 1_000_000),
            "clock went back: not due"
        );
    }

    /// The plan is built before the cycle's receipt exists; the stamp names
    /// the run afterwards and changes nothing else.
    #[test]
    fn an_early_plan_is_attributed_to_its_run_afterwards() {
        let db = crate::test_utils::test_db();
        let (_, generated_at) = super::super::refresh_plan_snapshot_at(&db, None).expect("plan");
        let generated_at = generated_at.expect("written");
        let before = db.get_kv("upgrade_plan_snapshot").unwrap().unwrap();
        assert!(before.contains(r#""engine_run_id":null"#), "{before}");
        assert!(crate::evidence::stamp_plan_engine_run(
            &db,
            7,
            &generated_at
        ));
        let after = db.get_kv("upgrade_plan_snapshot").unwrap().unwrap();
        assert!(after.contains(r#""engine_run_id":7"#), "{after}");
        let strip = |s: &str| {
            let mut v: serde_json::Value = serde_json::from_str(s).unwrap();
            v.as_object_mut().unwrap().remove("engine_run_id");
            v
        };
        assert_eq!(strip(&before), strip(&after), "only the run id changes");
    }

    /// A plan the app wrote while the engine cycle ran is not the engine's:
    /// it must not be credited to the engine run.
    #[test]
    fn a_plan_replaced_during_the_cycle_is_not_attributed() {
        let db = crate::test_utils::test_db();
        let (_, engine_plan) = super::super::refresh_plan_snapshot_at(&db, None).expect("plan");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let app_plan =
            crate::evidence::persist_upgrade_plan(&db, &[], &[], 0, None).expect("written");
        assert_ne!(engine_plan.as_deref(), Some(app_plan.as_str()));
        assert!(!crate::evidence::stamp_plan_engine_run(
            &db,
            9,
            &engine_plan.unwrap()
        ));
        let stored = db.get_kv("upgrade_plan_snapshot").unwrap().unwrap();
        assert!(stored.contains(r#""engine_run_id":null"#), "{stored}");
    }

    /// The engine's pass prunes old items and keeps a Saved one (#1119).
    #[test]
    fn engine_retention_prunes_old_items_and_keeps_saved() {
        let db = crate::test_utils::test_db();
        crate::ace::db::migrate(&db.conn).expect("ACE schema");
        let old = crate::test_utils::insert_test_item(&db, "hackernews", "r1", "Old", "body");
        let saved = crate::test_utils::insert_test_item(&db, "hackernews", "r2", "Saved", "body");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET last_seen = datetime('now', '-400 days') WHERE id IN (?1, ?2)",
                rusqlite::params![old, saved],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO interactions (item_id, action_type, signal_strength) VALUES (?1, 'save', 1.0)",
                [saved],
            )
            .unwrap();
        }
        let pruned = retention_pass(&db, 30).unwrap();
        assert_eq!(pruned, 1);
        let left: Vec<i64> = {
            let conn = db.conn.lock();
            let mut stmt = conn.prepare("SELECT id FROM source_items").unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .flatten()
                .collect()
        };
        assert_eq!(left, vec![saved]);
    }
}
