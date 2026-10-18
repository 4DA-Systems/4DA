// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Is the dependency inventory current for this session?
//!
//! At startup the stored inventory is the PREVIOUS session's: lockfiles may
//! have changed since (a merged bump, a `cargo update`). The startup ACE scan
//! re-reads them, but it waits for first light and a grace period, while the
//! brief and the startup morning-brief notification used to run before it.
//! Live 2026-10-18: brief #421 (09:51:10) told the user to bump ammonia to
//! 4.2.4 although Cargo.lock had carried 4.2.4 for a day; the lockfile rows
//! were only re-read at 09:51:49.
//!
//! This module is the one signal those readers gate on: `begin_refresh` when
//! the startup scan is scheduled, `finish_refresh` when it has written the
//! inventory, and `wait_until_fresh` (bounded, event-driven, no polling) for
//! anything that states dependency facts. With no scan scheduled (no context
//! directories) the inventory is what it is and reads as fresh.

use std::time::Duration;

use once_cell::sync::Lazy;
use tokio::sync::watch;

/// Event emitted when the startup inventory refresh finishes, so a brief the
/// UI was told to wait for can be generated then.
pub(crate) const INVENTORY_FRESH_EVENT: &str = "inventory-fresh";

/// One refresh gate. The process uses [`GLOBAL`]; tests make their own.
pub(crate) struct Gate {
    tx: watch::Sender<bool>,
}

impl Gate {
    /// A gate that starts fresh (nothing scheduled).
    pub(crate) fn new() -> Self {
        let (tx, _rx) = watch::channel(true);
        Self { tx }
    }

    pub(crate) fn begin_refresh(&self) {
        self.tx.send_replace(false);
    }

    pub(crate) fn finish_refresh(&self) {
        self.tx.send_replace(true);
    }

    pub(crate) fn is_fresh(&self) -> bool {
        *self.tx.borrow()
    }

    /// Wait for the refresh to finish, at most `timeout`. True when the
    /// inventory is fresh (already, or within the timeout).
    pub(crate) async fn wait_until_fresh(&self, timeout: Duration) -> bool {
        let mut rx = self.tx.subscribe();
        let waited =
            tokio::time::timeout(timeout, async { rx.wait_for(|fresh| *fresh).await.is_ok() })
                .await;
        waited.unwrap_or(false)
    }
}

static GLOBAL: Lazy<Gate> = Lazy::new(Gate::new);

/// How long a brief request waits for the startup rescan before answering
/// "the inventory is being refreshed". Below the UI's 120 s command timeout
/// with room for the model call that follows a successful wait.
pub(crate) const BRIEF_INVENTORY_WAIT: Duration = Duration::from_secs(45);

/// Machine-readable error of a brief refused while the inventory refreshes.
pub(crate) const INVENTORY_REFRESHING: &str = "inventory_refreshing";

/// The brief's gate: `None` when the inventory is fresh (already, or within
/// `timeout`); otherwise the response that tells the UI to wait for
/// [`INVENTORY_FRESH_EVENT`]. No dependency fact is built or stated from a
/// stale inventory.
pub(crate) async fn brief_gate(gate: &Gate, timeout: Duration) -> Option<serde_json::Value> {
    if gate.wait_until_fresh(timeout).await {
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
pub(crate) async fn brief_gate_global() -> Option<serde_json::Value> {
    brief_gate(&GLOBAL, BRIEF_INVENTORY_WAIT).await
}

/// The startup scan has been scheduled: the stored inventory may be stale.
pub(crate) fn begin_refresh() {
    tracing::info!(target: "4da::startup", "Dependency inventory refresh scheduled: dependency facts wait for it");
    GLOBAL.begin_refresh();
}

/// The startup scan has written the inventory (or ended without one).
pub(crate) fn finish_refresh() {
    GLOBAL.finish_refresh();
    tracing::info!(target: "4da::startup", "Dependency inventory refresh finished");
}

pub(crate) fn is_fresh() -> bool {
    GLOBAL.is_fresh()
}

pub(crate) async fn wait_until_fresh(timeout: Duration) -> bool {
    GLOBAL.wait_until_fresh(timeout).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_fresh_gate_never_waits() {
        let gate = Gate::new();
        assert!(gate.is_fresh());
        assert!(gate.wait_until_fresh(Duration::from_millis(1)).await);
    }

    #[tokio::test]
    async fn a_pending_refresh_times_out_as_not_fresh() {
        let gate = Gate::new();
        gate.begin_refresh();
        assert!(!gate.wait_until_fresh(Duration::from_millis(20)).await);
    }

    #[tokio::test]
    async fn a_waiter_wakes_when_the_refresh_finishes() {
        let gate = std::sync::Arc::new(Gate::new());
        gate.begin_refresh();
        let waiter = {
            let gate = gate.clone();
            tokio::spawn(async move { gate.wait_until_fresh(Duration::from_secs(10)).await })
        };
        tokio::task::yield_now().await;
        gate.finish_refresh();
        assert!(waiter.await.expect("waiter"));
    }

    /// Live 2026-10-18 (brief #421): a brief built while the startup rescan
    /// was pending stated a bump the lockfile already carried. While the
    /// refresh is pending the brief is refused with `inventory_refreshing`
    /// (no facts built); once it finishes the brief proceeds.
    #[tokio::test]
    async fn a_brief_requested_during_the_rescan_states_no_facts() {
        let gate = Gate::new();
        gate.begin_refresh();
        let refused = brief_gate(&gate, Duration::from_millis(20))
            .await
            .expect("refused while refreshing");
        assert_eq!(refused["success"], false);
        assert_eq!(refused["error"], INVENTORY_REFRESHING);
        assert!(refused["briefing"].is_null(), "no brief text, so no facts");

        gate.finish_refresh();
        assert!(brief_gate(&gate, Duration::from_millis(20)).await.is_none());
    }
}
