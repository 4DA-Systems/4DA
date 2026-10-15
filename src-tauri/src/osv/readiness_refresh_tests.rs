// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;

use super::*;

/// Two triggers at once (a scan finishing while the scheduler ticks): one
/// sync runs, the other sees the gate busy and does nothing. The in-flight
/// flag is set exactly while the sync runs.
#[tokio::test]
async fn concurrent_triggers_run_one_sync() {
    let gate = Mutex::new(());
    let flag = AtomicBool::new(false);
    let runs = AtomicUsize::new(0);
    let run = || async {
        runs.fetch_add(1, Ordering::SeqCst);
        assert!(flag.load(Ordering::SeqCst), "in flight while running");
        tokio::time::sleep(Duration::from_millis(50)).await;
        7
    };
    let (a, b) = tokio::join!(
        run_exclusive(&gate, &flag, || true, run),
        run_exclusive(&gate, &flag, || true, run)
    );
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    let ran = [&a, &b]
        .iter()
        .filter(|o| matches!(o, Outcome::Ran(7)))
        .count();
    let busy = [&a, &b]
        .iter()
        .filter(|o| matches!(o, Outcome::Busy))
        .count();
    assert_eq!((ran, busy), (1, 1), "{a:?} {b:?}");
    assert!(!flag.load(Ordering::SeqCst), "cleared after the sync");
}

#[tokio::test]
async fn nothing_due_runs_nothing_and_leaves_the_flag_clear() {
    let gate = Mutex::new(());
    let flag = AtomicBool::new(false);
    let out = run_exclusive(&gate, &flag, || false, || async { unreachable!() }).await;
    assert!(matches!(out, Outcome::<()>::NotDue));
    assert!(!flag.load(Ordering::SeqCst));
    // The gate is free again for the next trigger.
    let out = run_exclusive(&gate, &flag, || true, || async { 1 }).await;
    assert!(matches!(out, Outcome::Ran(1)));
}

#[test]
fn the_due_rule() {
    // Never synced / stale: due, unless a failure is inside the retry window.
    assert!(sync_due(true, None, "a", false));
    assert!(!sync_due(true, None, "a", true));
    // Fresh and covering the same inventory: not due.
    assert!(!sync_due(false, Some("a"), "a", false));
    // A scan added packages: due even when the ecosystems look fresh, and
    // even inside the retry window (the new packages have nothing stored).
    assert!(sync_due(false, Some("a"), "b", false));
    assert!(sync_due(false, Some("a"), "b", true));
    // No recorded inventory (an engine-only history): freshness decides.
    assert!(!sync_due(false, None, "a", false));
}

#[test]
fn the_inventory_fingerprint_follows_the_packages_not_their_order() {
    let db = crate::test_utils::test_db();
    assert_eq!(inventory_fingerprint(&db), None, "nothing stored");
    db.store_dependency("/p", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.store_dependency("/p", "tokio", Some("1.40.0"), "rust", false, None)
        .unwrap();
    let two = inventory_fingerprint(&db).expect("inventory");

    let other = crate::test_utils::test_db();
    other
        .store_dependency("/q", "Tokio", Some("1.41.0"), "rust", false, None)
        .unwrap();
    other
        .store_dependency("/q", "serde", Some("1.0.201"), "rust", false, None)
        .unwrap();
    assert_eq!(inventory_fingerprint(&other).as_deref(), Some(two.as_str()));

    db.store_dependency("/p", "axum", Some("0.7.0"), "rust", false, None)
        .unwrap();
    assert_ne!(inventory_fingerprint(&db).as_deref(), Some(two.as_str()));
}

/// The self-heal (E2): dependencies stored and never synced is due on the
/// next check; once a complete sync recorded this inventory and the status
/// rows are fresh, it is not.
#[test]
fn stored_dependencies_without_a_sync_are_due() {
    let db = crate::test_utils::test_db();
    assert!(!due_now(&db), "no inventory, nothing to sync");
    db.store_dependency("/p", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    assert!(due_now(&db), "never synced");

    db.update_osv_sync_status("crates.io", 0, None).unwrap();
    db.set_kv(INVENTORY_KV_KEY, &inventory_fingerprint(&db).unwrap())
        .unwrap();
    assert!(!due_now(&db), "fresh and covering the inventory");

    db.store_dependency("/p", "tokio", Some("1.40.0"), "rust", false, None)
        .unwrap();
    assert!(due_now(&db), "a scan added a package");
}
