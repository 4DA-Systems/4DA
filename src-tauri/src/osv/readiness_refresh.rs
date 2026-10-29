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
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
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

/// Back-off after a failed or partial sync: 5, then 15, then 30 minutes
/// (the cap), reset by a clean sync. It applies to every automatic trigger,
/// a changed inventory included: a sync that keeps failing (OSV rate limits,
/// offline) would otherwise re-POST the whole inventory, and re-fetch the
/// ecosystem archives, on every full check (PR #961 review). "Check now"
/// bypasses it.
const RETRY_BACKOFF_SECS: [i64; 3] = [5 * 60, 15 * 60, 30 * 60];

/// The scheduler's full due check (it reads the whole inventory) runs at most
/// this often (stretched by the battery/CPU policy); the cheap never-synced
/// probe can run on every tick.
const FULL_CHECK_EVERY_SECS: i64 = 5 * 60;

static GUI_TRIGGERS: AtomicBool = AtomicBool::new(false);
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static LAST_FULL_CHECK_AT: AtomicI64 = AtomicI64::new(0);
/// The probe already led to a check that found nothing due (an inventory of
/// only uncovered ecosystems never writes a sync row, so the probe would fire
/// every minute forever). Re-armed by a scan.
static PROBE_SPENT: AtomicBool = AtomicBool::new(false);
static BACKOFF: Backoff = Backoff::new();

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
    /// Nothing to do: no inventory, the mirror covers it and is fresh, or the
    /// failure back-off is running.
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

/// The failure back-off: when the last failure was and how many in a row.
pub(crate) struct Backoff {
    failed_at: AtomicI64,
    streak: AtomicU32,
}

impl Backoff {
    pub(crate) const fn new() -> Self {
        Self {
            failed_at: AtomicI64::new(0),
            streak: AtomicU32::new(0),
        }
    }

    /// Seconds to wait after `streak` failures in a row (0 for none).
    pub(crate) fn delay_secs(streak: u32) -> i64 {
        let Some(i) = streak.checked_sub(1) else {
            return 0;
        };
        let i = usize::try_from(i).unwrap_or(usize::MAX);
        RETRY_BACKOFF_SECS[i.min(RETRY_BACKOFF_SECS.len() - 1)]
    }

    /// Inside the back-off window at `now`.
    pub(crate) fn blocked(&self, now: i64) -> bool {
        let streak = self.streak.load(Ordering::SeqCst);
        streak > 0 && now - self.failed_at.load(Ordering::SeqCst) < Self::delay_secs(streak)
    }

    /// A sync finished: clean resets, anything else extends the streak.
    pub(crate) fn record(&self, clean: bool, now: i64) {
        if clean {
            self.streak.store(0, Ordering::SeqCst);
            self.failed_at.store(0, Ordering::SeqCst);
        } else {
            let streak = self.streak.load(Ordering::SeqCst);
            self.streak
                .store(streak.saturating_add(1), Ordering::SeqCst);
            self.failed_at.store(now, Ordering::SeqCst);
        }
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

/// [`run_exclusive`] with a follow-up that runs only after the sync ran,
/// still holding the gate: launch's offline-archive refresh rewrites the
/// ecosystem ZIPs in place, so it must never overlap another sync.
pub(crate) async fn run_exclusive_then<D, F, Fut, T, A, AFut, U>(
    gate: &Mutex<()>,
    flag: &AtomicBool,
    due: D,
    run: F,
    after: A,
) -> Outcome<(T, U)>
where
    D: FnOnce() -> bool,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
    A: FnOnce() -> AFut,
    AFut: Future<Output = U>,
{
    run_exclusive(gate, flag, due, || async {
        let ran = run().await;
        (ran, after().await)
    })
    .await
}

/// "Check now" through the gate. A free gate runs at once. A busy gate is
/// waited for, then `due_after_wait()` decides: the sync it waited on may
/// already have been the check. `None` = waited, nothing left to do.
pub(crate) async fn run_check_now<D, F, Fut, T>(
    gate: &Mutex<()>,
    flag: &AtomicBool,
    due_after_wait: D,
    run: F,
) -> Option<T>
where
    D: FnOnce() -> bool,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let _guard = match gate.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            let guard = gate.lock().await;
            if !due_after_wait() {
                return None;
            }
            guard
        }
    };
    let _mark = InFlight::mark(flag);
    Some(run().await)
}

/// The due rule. Pure. A changed inventory (its new packages have no
/// advisories stored) or a stale / never-synced mirror is due — unless the
/// failure back-off is running.
pub(crate) fn sync_due(
    stale: bool,
    synced_inventory: Option<&str>,
    inventory: &str,
    retry_blocked: bool,
) -> bool {
    let changed = synced_inventory.is_some_and(|synced| synced != inventory);
    (changed || stale) && !retry_blocked
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

/// Whether a desktop sync is due at `now`, read from the database. With
/// `honor_backoff` false ("Check now") a recent failure does not block it.
pub(crate) fn due_at(db: &Database, backoff: &Backoff, honor_backoff: bool, now: i64) -> bool {
    let Some(inventory) = inventory_fingerprint(db) else {
        return false;
    };
    let stale = crate::osv::sync::needs_sync(db, crate::osv::sync::osv_sync_max_age_hours())
        .unwrap_or(true);
    let synced = db.get_kv(INVENTORY_KV_KEY).ok().flatten();
    let blocked = honor_backoff && backoff.blocked(now);
    sync_due(stale, synced.as_deref(), &inventory, blocked)
}

/// Run the sync and record what it covered. The fingerprint is taken BEFORE
/// the sync reads the inventory, so packages a concurrent scan adds show as a
/// change and get the next sync.
async fn run_sync(db: &Database) -> Result<SyncResult> {
    let inventory = inventory_fingerprint(db);
    let result = crate::osv::sync::sync(db).await;
    let clean = matches!(&result, Ok(r) if r.errors.is_empty());
    BACKOFF.record(clean, now_secs());
    if clean {
        if let Some(inventory) = inventory {
            if let Err(e) = db.set_kv(INVENTORY_KV_KEY, &inventory) {
                warn!(target: "4da::osv", error = %e, "Could not record the synced inventory");
            }
        }
    }
    result
}

/// One gated attempt: sync when due, without warming anything.
pub(crate) async fn sync_if_due(db: &Database) -> Outcome<Result<SyncResult>> {
    run_exclusive(
        gate(),
        &IN_FLIGHT,
        || due_at(db, &BACKOFF, true, now_secs()),
        || run_sync(db),
    )
    .await
}

/// Launch's attempt: sync when due, then refresh the offline archive caches,
/// both inside the gate. Launch warms Preemption on its own schedule (after
/// first light).
pub(crate) async fn sync_with_caches_if_due(db: &Database) -> Outcome<(SyncOutcome, CacheOutcome)> {
    run_exclusive_then(
        gate(),
        &IN_FLIGHT,
        || due_at(db, &BACKOFF, true, now_secs()),
        || run_sync(db),
        || crate::osv::cache::update_all_caches(db),
    )
    .await
}

/// A sync's result.
pub(crate) type SyncOutcome = Result<SyncResult>;
/// An offline-archive refresh's result.
pub(crate) type CacheOutcome = Result<crate::osv::types::CacheUpdateResult>;

/// One gated attempt, then a Preemption recompute so findings appear without
/// any user action.
async fn refresh_if_due(trigger: Trigger) {
    let Ok(db) = crate::get_database() else {
        return;
    };
    match sync_if_due(&db).await {
        Outcome::Busy => debug!(target: "4da::osv", ?trigger, "Advisory sync already running"),
        Outcome::NotDue => {
            if trigger == Trigger::Scheduler {
                PROBE_SPENT.store(true, Ordering::SeqCst);
            }
            debug!(target: "4da::osv", ?trigger, "Advisory sync not due");
        }
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
            warn!(target: "4da::osv", ?trigger, error = %e, "Advisory sync failed; retried after a back-off");
        }
    }
}

/// Start a gated sync check in the background (desktop app only).
pub(crate) fn spawn_if_due(trigger: Trigger) {
    if !GUI_TRIGGERS.load(Ordering::SeqCst) || sync_in_flight() {
        return;
    }
    if trigger == Trigger::Scan {
        PROBE_SPENT.store(false, Ordering::SeqCst);
    }
    tauri::async_runtime::spawn(refresh_if_due(trigger));
}

/// The scheduler tick's decision. Pure. Nothing while the battery/CPU policy
/// holds background work; the full check once per `full_every`; in between,
/// the never-synced `probe` — skipped inside the failure back-off and once it
/// has led to a check that found nothing due.
pub(crate) fn tick_wants_check(
    policy_allows: bool,
    since_full_check: i64,
    full_every: i64,
    backoff_blocked: bool,
    probe_spent: bool,
    probe: impl FnOnce() -> bool,
) -> bool {
    policy_allows
        && (since_full_check >= full_every || (!backoff_blocked && !probe_spent && probe()))
}

/// The scheduler's per-tick hook.
pub(crate) fn on_scheduler_tick() {
    if !GUI_TRIGGERS.load(Ordering::SeqCst) || sync_in_flight() {
        return;
    }
    let now = now_secs();
    let full_every = i64::try_from(crate::scheduler_gate::effective_interval(
        FULL_CHECK_EVERY_SECS.unsigned_abs(),
    ))
    .unwrap_or(FULL_CHECK_EVERY_SECS);
    let wants = tick_wants_check(
        crate::scheduler_gate::should_run_job(crate::scheduler_gate::JobPriority::High),
        now - LAST_FULL_CHECK_AT.load(Ordering::SeqCst),
        full_every,
        BACKOFF.blocked(now),
        PROBE_SPENT.load(Ordering::SeqCst),
        never_synced_with_inventory,
    );
    if wants {
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

/// "Check now": an explicit request, so the failure back-off does not apply.
/// After waiting behind another sync it runs only if that one left the
/// mirror stale or the inventory changed (a failed sync leaves it stale, so
/// this runs) — never a silent no-op behind a failure.
pub(crate) async fn sync_now(db: &Database) -> Result<SyncResult> {
    let ran = run_check_now(
        gate(),
        &IN_FLIGHT,
        || due_at(db, &BACKOFF, false, now_secs()),
        || run_sync(db),
    )
    .await;
    ran.unwrap_or_else(|| {
        Ok(SyncResult {
            ecosystems_synced: Vec::new(),
            advisories_stored: 0,
            advisories_matched: 0,
            duration_ms: 0,
            errors: Vec::new(),
        })
    })
}

#[cfg(test)]
#[path = "readiness_refresh_tests.rs"]
mod tests;
