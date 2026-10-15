// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Keeping the advisory mirror current from the desktop app.
//!
//! The mirror used to sync at launch only, and only when dependencies were
//! already stored; the frontend never asked for a sync and the background
//! engine is opt-in and Windows-only. A fresh install therefore showed no
//! findings until a restart, and a long-running app on macOS/Linux (or on
//! Windows without the engine) never refreshed advisories at all
//! (fresh-profile audit E1, 2026-10-12: 8.5 h of "syncing" over 156
//! vulnerable packages).
//!
//! Now the app syncs when the inventory changes (a scan stored packages the
//! mirror has not seen) and when the scheduler finds the mirror stale or never
//! synced, on every platform. Every desktop sync runs through one gate, so a
//! scan, the scheduler, launch and "Check now" never run two syncs at once,
//! and [`sync_in_flight`] tells readiness whether "syncing" is true.
//!
//! The headless engine (another process, same database) keeps its own sync
//! step; the triggers here are enabled only in the GUI ([`enable_gui_triggers`])
//! so an engine scan never starts a second sync inside the engine. The two
//! processes coexist through the database: every due check re-reads the sync
//! status rows, so a mirror the engine just refreshed is not synced again, and
//! the sync's writes are upserts.

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::Result;
use crate::osv::types::SyncResult;

/// `kv_store` key: fingerprint of the inventory the last complete desktop
/// sync covered. The sync stores advisories only for the packages it was
/// given, so a package added since then has none until the next sync, however
/// fresh the ecosystem's sync time looks.
const INVENTORY_KV_KEY: &str = "osv.synced_inventory.v1";

/// After a failed sync the scheduler waits this long before retrying a merely
/// stale mirror (a changed inventory and "Check now" still run at once), so
/// an offline machine does not hammer OSV every few minutes.
const RETRY_AFTER_FAILURE_SECS: i64 = 30 * 60;

/// The scheduler's full due check (it reads the whole inventory) runs at most
/// this often; the cheap never-synced probe runs on every tick.
const FULL_CHECK_EVERY_SECS: i64 = 5 * 60;

static GUI_TRIGGERS: AtomicBool = AtomicBool::new(false);
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static LAST_FAILURE_AT: AtomicI64 = AtomicI64::new(0);
static LAST_FULL_CHECK_AT: AtomicI64 = AtomicI64::new(0);

fn gate() -> &'static Mutex<()> {
    static GATE: OnceLock<Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(()))
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Called once by the desktop app's setup. The headless engine never calls
/// it, so its scans do not start syncs beside its own sync step.
pub(crate) fn enable_gui_triggers() {
    GUI_TRIGGERS.store(true, Ordering::SeqCst);
}

/// A desktop sync is running now.
pub(crate) fn sync_in_flight() -> bool {
    IN_FLIGHT.load(Ordering::SeqCst)
}

/// What started a desktop sync check (logged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// The lockfile walk / scan stored dependencies.
    Scan,
    /// The scheduler's periodic freshness check.
    Scheduler,
}

/// What one gated attempt did.
#[derive(Debug)]
pub(crate) enum Outcome<T> {
    /// Another desktop sync holds the gate.
    Busy,
    /// Nothing to do: no inventory, or the mirror covers it and is fresh.
    NotDue,
    Ran(T),
}

/// Marks a sync as running for exactly as long as it lives.
struct InFlight<'a>(&'a AtomicBool);

impl<'a> InFlight<'a> {
    fn mark(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::SeqCst);
        Self(flag)
    }
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Single flight: run `run` only when the gate is free and `due()` holds,
/// with `flag` set while it runs. Never waits — a busy gate means a sync is
/// already doing the work.
pub(crate) async fn run_exclusive<D, F, Fut, T>(
    gate: &Mutex<()>,
    flag: &AtomicBool,
    due: D,
    run: F,
) -> Outcome<T>
where
    D: FnOnce() -> bool,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let Ok(_guard) = gate.try_lock() else {
        return Outcome::Busy;
    };
    if !due() {
        return Outcome::NotDue;
    }
    let _mark = InFlight::mark(flag);
    Outcome::Ran(run().await)
}

/// The due rule. Pure. A changed inventory is always due (its new packages
/// have no advisories stored); a stale or never-synced mirror is due unless a
/// sync failed within the retry window.
pub(crate) fn sync_due(
    stale: bool,
    synced_inventory: Option<&str>,
    inventory: &str,
    retry_blocked: bool,
) -> bool {
    let changed = synced_inventory.is_some_and(|synced| synced != inventory);
    changed || (stale && !retry_blocked)
}

/// Fingerprint of the auditable inventory (the packages the sync reads), or
/// `None` when it is empty or unreadable.
pub(crate) fn inventory_fingerprint(db: &Database) -> Option<String> {
    let mut deps = db.get_auditable_user_dependencies().ok()?;
    deps.extend(db.get_auditable_scanned_dependencies().ok()?);
    let mut keys: Vec<String> = deps
        .iter()
        .map(|d| {
            format!(
                "{}\t{}",
                d.ecosystem.to_lowercase(),
                d.package_name.to_lowercase()
            )
        })
        .collect();
    if keys.is_empty() {
        return None;
    }
    keys.sort();
    keys.dedup();
    let mut hasher = Sha256::new();
    for key in &keys {
        hasher.update(key.as_bytes());
        hasher.update(b"\n");
    }
    Some(hex::encode(hasher.finalize()))
}

fn retry_blocked(now: i64) -> bool {
    let failed_at = LAST_FAILURE_AT.load(Ordering::SeqCst);
    failed_at > 0 && now - failed_at < RETRY_AFTER_FAILURE_SECS
}

/// Whether a desktop sync is due now, read from the database.
fn due_now(db: &Database) -> bool {
    let Some(inventory) = inventory_fingerprint(db) else {
        return false;
    };
    let stale = crate::osv::sync::needs_sync(db, crate::osv::sync::osv_sync_max_age_hours())
        .unwrap_or(true);
    let synced = db.get_kv(INVENTORY_KV_KEY).ok().flatten();
    sync_due(
        stale,
        synced.as_deref(),
        &inventory,
        retry_blocked(now_secs()),
    )
}

/// Run the sync and record what it covered. The fingerprint is taken BEFORE
/// the sync reads the inventory, so packages a concurrent scan adds show as a
/// change and get the next sync.
async fn run_sync(db: &Database) -> Result<SyncResult> {
    let inventory = inventory_fingerprint(db);
    let result = crate::osv::sync::sync(db).await;
    match &result {
        Ok(r) if r.errors.is_empty() => {
            LAST_FAILURE_AT.store(0, Ordering::SeqCst);
            if let Some(inventory) = inventory {
                if let Err(e) = db.set_kv(INVENTORY_KV_KEY, &inventory) {
                    warn!(target: "4da::osv", error = %e, "Could not record the synced inventory");
                }
            }
        }
        _ => LAST_FAILURE_AT.store(now_secs(), Ordering::SeqCst),
    }
    result
}

/// One gated attempt: sync when due, without warming anything. For launch,
/// which warms Preemption on its own schedule (after first light).
pub(crate) async fn sync_if_due(db: &Database) -> Outcome<Result<SyncResult>> {
    run_exclusive(gate(), &IN_FLIGHT, || due_now(db), || run_sync(db)).await
}

/// One gated attempt, then a Preemption recompute so findings appear without
/// any user action.
async fn refresh_if_due(trigger: Trigger) {
    let Ok(db) = crate::get_database() else {
        return;
    };
    match sync_if_due(&db).await {
        Outcome::Busy => debug!(target: "4da::osv", ?trigger, "Advisory sync already running"),
        Outcome::NotDue => debug!(target: "4da::osv", ?trigger, "Advisory mirror current"),
        Outcome::Ran(Ok(r)) => {
            info!(
                target: "4da::osv",
                ?trigger,
                ecosystems = ?r.ecosystems_synced,
                stored = r.advisories_stored,
                errors = r.errors.len(),
                duration_ms = r.duration_ms,
                "Advisory sync complete"
            );
            crate::preemption::warm_preemption_cache().await;
        }
        Outcome::Ran(Err(e)) => {
            warn!(target: "4da::osv", ?trigger, error = %e, "Advisory sync failed; the scheduler retries");
        }
    }
}

/// Start a gated sync check in the background (desktop app only).
pub(crate) fn spawn_if_due(trigger: Trigger) {
    if !GUI_TRIGGERS.load(Ordering::SeqCst) || sync_in_flight() {
        return;
    }
    tauri::async_runtime::spawn(refresh_if_due(trigger));
}

/// The scheduler's per-tick hook. A cheap probe (dependencies stored, no
/// ecosystem ever synced) starts the first sync on the next tick; the full
/// due check, which reads the whole inventory, runs every few minutes.
pub(crate) fn on_scheduler_tick() {
    if !GUI_TRIGGERS.load(Ordering::SeqCst) || sync_in_flight() {
        return;
    }
    let now = now_secs();
    let full_check_due = now - LAST_FULL_CHECK_AT.load(Ordering::SeqCst) >= FULL_CHECK_EVERY_SECS;
    // Offline after a failure: the probe would only find the retry window
    // closed, so skip it until the window passes (the full check still runs).
    if full_check_due || (!retry_blocked(now) && never_synced_with_inventory()) {
        LAST_FULL_CHECK_AT.store(now, Ordering::SeqCst);
        spawn_if_due(Trigger::Scheduler);
    }
}

/// Dependencies are stored and no sync status row exists at all.
fn never_synced_with_inventory() -> bool {
    let Ok(conn) = crate::open_db_connection() else {
        return false;
    };
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM user_dependencies)
            AND NOT EXISTS(SELECT 1 FROM osv_sync_status)",
        [],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// "Check now": always syncs, through the same gate. When another sync held
/// the gate, it waits for it and then syncs only if that sync left the mirror
/// stale or the inventory changed — the sync it waited on was the check.
pub(crate) async fn sync_now(db: &Database) -> Result<SyncResult> {
    let (_guard, waited) = match gate().try_lock() {
        Ok(guard) => (guard, false),
        Err(_) => (gate().lock().await, true),
    };
    if waited && !due_now(db) {
        return Ok(SyncResult {
            ecosystems_synced: Vec::new(),
            advisories_stored: 0,
            advisories_matched: 0,
            duration_ms: 0,
            errors: Vec::new(),
        });
    }
    let _mark = InFlight::mark(&IN_FLIGHT);
    run_sync(db).await
}

#[cfg(test)]
#[path = "readiness_refresh_tests.rs"]
mod tests;
