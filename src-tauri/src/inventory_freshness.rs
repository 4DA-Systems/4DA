// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Is the dependency inventory current for this session?
//!
//! At startup the stored inventory is the PREVIOUS session's: lockfiles may
//! have changed since (a merged bump, a `cargo update`). The startup ACE scan
//! re-reads them, but it waits for first light and a grace period, while the
//! brief and the startup morning-brief notification used to run before it.
//! Live 2026-10-18: brief #421 (09:51:10) told the user to bump ammonia to
//! 4.2.4 although Cargo.lock had carried 4.2.4 for a day; the lockfile rows
//! were only re-read at 09:51:40 and the scan finished at 09:54:16.
//!
//! This module is the one signal those readers gate on. [`begin_startup_refresh`]
//! shuts the gate and returns a [`RefreshGuard`] that the scan task owns: the
//! gate reopens when the guard drops (scan done, failed, or the task
//! PANICKED), or at a hard deadline if the scan hangs (an unreachable network
//! share), whichever comes first. A shut gate can therefore never outlive the
//! session's scan. With no scan scheduled the inventory reads as fresh.
//!
//! The same gate covers a MID-SESSION rescan (`dep_rescan`): live 2026-10-24,
//! a merged bump changed lockfiles hours after the startup scan and the GUI's
//! scheduled cycle never re-read them, so the brief kept naming advisories
//! the lockfiles had already cleared. [`try_begin_session_refresh`] begins a
//! refresh only when no other scan is in flight, so a rescan never overlaps
//! the startup scan or another rescan. The gate counts its holders: it reads
//! fresh only once every refresh holding it has released, so overlapping
//! refreshes can never reopen it early for one another.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio::sync::watch;

/// Event emitted when the startup inventory refresh finishes, so a brief the
/// UI was told to wait for can be generated then.
pub(crate) const INVENTORY_FRESH_EVENT: &str = "inventory-fresh";

/// Hard deadline for the startup scan to release the gate. Measured
/// 2026-10-18: ~8 minutes on the operator's 19-lockfile machine.
pub(crate) const STARTUP_SCAN_DEADLINE: Duration = Duration::from_mins(15);

/// Machine-readable error of a brief refused while the inventory refreshes.
pub(crate) const INVENTORY_REFRESHING: &str = "inventory_refreshing";

/// One refresh gate. The process uses [`GLOBAL`]; tests make their own.
pub(crate) struct Gate {
    /// Refreshes currently holding the gate shut. Fresh at zero.
    tx: watch::Sender<usize>,
    /// Scans in flight. Released only when a [`RefreshGuard`] drops, never at
    /// the deadline, so a hung scan still blocks a second one from starting.
    scans: AtomicUsize,
}

impl Gate {
    /// A gate that starts fresh (nothing scheduled).
    pub(crate) fn new() -> Self {
        let (tx, _rx) = watch::channel(0);
        Self {
            tx,
            scans: AtomicUsize::new(0),
        }
    }

    fn shut(&self) {
        self.tx.send_modify(|holders| *holders += 1);
    }

    fn open(&self) {
        self.tx
            .send_modify(|holders| *holders = holders.saturating_sub(1));
    }

    pub(crate) fn is_fresh(&self) -> bool {
        *self.tx.borrow() == 0
    }

    /// True while any scan (startup or mid-session) is still running.
    pub(crate) fn scan_in_flight(&self) -> bool {
        self.scans.load(Ordering::SeqCst) > 0
    }

    /// Wait for the refresh to finish, at most `timeout`. True when the
    /// inventory is fresh (already, or within the timeout).
    pub(crate) async fn wait_until_fresh(&self, timeout: Duration) -> bool {
        let mut rx = self.tx.subscribe();
        let waited = tokio::time::timeout(timeout, async {
            rx.wait_for(|holders| *holders == 0).await.is_ok()
        })
        .await;
        waited.unwrap_or(false)
    }
}

static GLOBAL: Lazy<Arc<Gate>> = Lazy::new(|| Arc::new(Gate::new()));

type OnFinish = Box<dyn FnOnce() + Send>;

/// The shared state of one refresh: reopening happens exactly once.
struct Refresh {
    gate: Arc<Gate>,
    on_finish: Mutex<Option<OnFinish>>,
}

impl Refresh {
    fn release(&self, deadline_hit: bool) {
        let Some(on_finish) = self.on_finish.lock().take() else {
            return;
        };
        self.gate.open();
        if deadline_hit {
            tracing::warn!(target: "4da::startup", "Dependency inventory refresh hit its deadline: dependency facts are released on the stored inventory");
        } else {
            tracing::info!(target: "4da::startup", "Dependency inventory refresh finished");
        }
        on_finish();
    }
}

/// Owned by the scan task; reopens the gate when dropped (including during
/// a panic's unwind).
pub(crate) struct RefreshGuard(Arc<Refresh>);

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.0.release(false);
        self.0.gate.scans.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Shut `gate` until the returned guard drops or `deadline` passes.
/// `on_finish` runs once, on whichever comes first.
fn begin_refresh_on(
    gate: Arc<Gate>,
    deadline: Duration,
    on_finish: impl FnOnce() + Send + 'static,
) -> RefreshGuard {
    gate.scans.fetch_add(1, Ordering::SeqCst);
    start_refresh(gate, deadline, on_finish)
}

/// [`begin_refresh_on`], but only when no scan is in flight on `gate` (the
/// startup scan, or another rescan, even one past its deadline). `None`
/// means: skip, a scan is already re-reading the lockfiles.
pub(crate) fn try_begin_refresh_on(
    gate: Arc<Gate>,
    deadline: Duration,
    on_finish: impl FnOnce() + Send + 'static,
) -> Option<RefreshGuard> {
    gate.scans
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .ok()?;
    Some(start_refresh(gate, deadline, on_finish))
}

/// Shut the gate and arm the deadline watchdog; the caller has already
/// counted the scan in `gate.scans`.
fn start_refresh(
    gate: Arc<Gate>,
    deadline: Duration,
    on_finish: impl FnOnce() + Send + 'static,
) -> RefreshGuard {
    gate.shut();
    let refresh = Arc::new(Refresh {
        gate,
        on_finish: Mutex::new(Some(Box::new(on_finish))),
    });
    let watchdog = Arc::downgrade(&refresh);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(deadline).await;
        if let Some(refresh) = watchdog.upgrade() {
            refresh.release(true);
        }
    });
    RefreshGuard(refresh)
}

/// The startup scan is scheduled: dependency facts wait for the returned
/// guard (move it into the scan task) or [`STARTUP_SCAN_DEADLINE`].
pub(crate) fn begin_startup_refresh(on_finish: impl FnOnce() + Send + 'static) -> RefreshGuard {
    tracing::info!(target: "4da::startup", "Dependency inventory refresh scheduled: dependency facts wait for it");
    begin_refresh_on(GLOBAL.clone(), STARTUP_SCAN_DEADLINE, on_finish)
}

/// A mid-session rescan: dependency facts wait for the returned guard or
/// [`STARTUP_SCAN_DEADLINE`]. `None` when a scan is already in flight.
pub(crate) fn try_begin_session_refresh(
    on_finish: impl FnOnce() + Send + 'static,
) -> Option<RefreshGuard> {
    let guard = try_begin_refresh_on(GLOBAL.clone(), STARTUP_SCAN_DEADLINE, on_finish)?;
    tracing::info!(target: "4da::inventory", "Dependency inventory rescan started: dependency facts wait for it");
    Some(guard)
}

/// [`Gate::scan_in_flight`] on the process gate.
pub(crate) fn scan_in_flight() -> bool {
    GLOBAL.scan_in_flight()
}

pub(crate) fn is_fresh() -> bool {
    GLOBAL.is_fresh()
}

pub(crate) async fn wait_until_fresh(timeout: Duration) -> bool {
    GLOBAL.wait_until_fresh(timeout).await
}

/// The brief's gate: `None` when the inventory is fresh; otherwise, at once,
/// the response that tells the UI to generate on [`INVENTORY_FRESH_EVENT`].
/// No dependency fact is built or stated from a stale inventory, and no time
/// is taken from the model call's share of the UI's command timeout.
pub(crate) fn brief_gate(gate: &Gate) -> Option<serde_json::Value> {
    if gate.is_fresh() {
        return None;
    }
    tracing::info!(target: "4da::briefing", "Brief deferred: the dependency inventory is being refreshed");
    Some(serde_json::json!({
        "success": false,
        "error": INVENTORY_REFRESHING,
        "inventory_refreshing": true,
        "briefing": null
    }))
}

/// [`brief_gate`] on the process gate.
pub(crate) fn brief_gate_global() -> Option<serde_json::Value> {
    brief_gate(&GLOBAL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counter() -> (Arc<AtomicUsize>, impl FnOnce() + Send + 'static) {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        (count, move || {
            c.fetch_add(1, Ordering::SeqCst);
        })
    }

    #[tokio::test]
    async fn a_fresh_gate_never_waits() {
        let gate = Gate::new();
        assert!(gate.is_fresh());
        assert!(gate.wait_until_fresh(Duration::from_millis(1)).await);
    }

    #[tokio::test]
    async fn a_pending_refresh_times_out_as_not_fresh_and_reopens_on_drop() {
        let gate = Arc::new(Gate::new());
        let (count, on_finish) = counter();
        let guard = begin_refresh_on(gate.clone(), Duration::from_hours(1), on_finish);
        assert!(!gate.wait_until_fresh(Duration::from_millis(20)).await);
        drop(guard);
        assert!(gate.is_fresh());
        assert_eq!(count.load(Ordering::SeqCst), 1, "finish runs once");
    }

    #[tokio::test]
    async fn a_waiter_wakes_when_the_refresh_finishes() {
        let gate = Arc::new(Gate::new());
        let guard = begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        let waiter = {
            let gate = gate.clone();
            tokio::spawn(async move { gate.wait_until_fresh(Duration::from_secs(10)).await })
        };
        tokio::task::yield_now().await;
        drop(guard);
        assert!(waiter.await.expect("waiter"));
    }

    /// Review B1: a panicking scan task must not leave the gate shut.
    #[test]
    fn a_guard_dropped_by_a_panic_reopens_the_gate() {
        let gate = Arc::new(Gate::new());
        let (count, on_finish) = counter();
        let guard = begin_refresh_on(gate.clone(), Duration::from_hours(1), on_finish);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owned = guard;
            panic!("scan task panicked");
        }));
        assert!(result.is_err());
        assert!(gate.is_fresh(), "the unwind dropped the guard");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    /// Review B1: a hung scan releases the gate at the deadline, once.
    #[tokio::test]
    async fn a_hung_scan_releases_the_gate_at_the_deadline() {
        let gate = Arc::new(Gate::new());
        let (count, on_finish) = counter();
        let guard = begin_refresh_on(gate.clone(), Duration::from_millis(30), on_finish);
        assert!(gate.wait_until_fresh(Duration::from_secs(5)).await);
        // `release` opens the gate BEFORE it runs `on_finish` (the
        // inventory-fresh emit must find the gate open), and the watchdog
        // runs on another runtime, so wait for the callback rather than
        // reading the count the instant the gate opens.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        drop(guard); // the late finish is a no-op
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    /// Live 2026-10-18 (brief #421): a brief requested while the startup
    /// rescan is pending states no facts; it is deferred at once.
    #[test]
    fn a_brief_requested_during_the_rescan_states_no_facts() {
        let gate = Arc::new(Gate::new());
        let guard = begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        let refused = brief_gate(&gate).expect("refused while refreshing");
        assert_eq!(refused["success"], false);
        assert_eq!(refused["error"], INVENTORY_REFRESHING);
        assert!(refused["briefing"].is_null(), "no brief text, so no facts");
        drop(guard);
        assert!(brief_gate(&gate).is_none());
    }

    /// A mid-session rescan never starts while the startup scan holds the gate.
    #[test]
    fn a_session_refresh_is_refused_while_the_startup_scan_runs() {
        let gate = Arc::new(Gate::new());
        let startup = begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        assert!(gate.scan_in_flight());
        assert!(try_begin_refresh_on(gate.clone(), Duration::from_hours(1), || {}).is_none());
        drop(startup);
        assert!(!gate.scan_in_flight());
        let session = try_begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        assert!(session.is_some(), "free once the startup scan finished");
        assert!(!gate.is_fresh(), "the rescan shuts the gate");
        drop(session);
        assert!(gate.is_fresh());
    }

    /// Two rescans never overlap, even when the first is past its deadline
    /// (the gate reopened, but the scan itself still runs).
    #[tokio::test]
    async fn a_hung_rescan_past_its_deadline_still_blocks_a_second_one() {
        let gate = Arc::new(Gate::new());
        let first = try_begin_refresh_on(gate.clone(), Duration::from_millis(20), || {})
            .expect("first rescan");
        assert!(gate.wait_until_fresh(Duration::from_secs(5)).await);
        assert!(try_begin_refresh_on(gate.clone(), Duration::from_hours(1), || {}).is_none());
        drop(first);
        assert!(try_begin_refresh_on(gate.clone(), Duration::from_hours(1), || {}).is_some());
    }

    /// Overlapping holders: the gate reads fresh only once BOTH released.
    #[test]
    fn overlapping_refreshes_never_reopen_the_gate_for_each_other() {
        let gate = Arc::new(Gate::new());
        let a = begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        let b = begin_refresh_on(gate.clone(), Duration::from_hours(1), || {});
        drop(a);
        assert!(!gate.is_fresh(), "b still holds the gate");
        assert!(brief_gate(&gate).is_some());
        drop(b);
        assert!(gate.is_fresh());
        assert!(!gate.scan_in_flight());
    }
}
