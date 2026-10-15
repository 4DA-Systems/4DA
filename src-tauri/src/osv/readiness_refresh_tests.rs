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
    // A scan added packages: due even when the ecosystems look fresh (the new
    // packages have nothing stored) — but not inside the failure back-off: a
    // sync that keeps failing must not re-POST the inventory every 5 minutes.
    assert!(sync_due(false, Some("a"), "b", false));
    assert!(!sync_due(false, Some("a"), "b", true));
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
    let b = Backoff::new();
    let due_now = |db: &crate::db::Database| due_at(db, &b, true, 1_000_000);
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

/// PR #961 review: the back-off is 5, 15, then 30 minutes (capped) after
/// consecutive failures, and a clean sync resets it.
#[test]
fn the_failure_backoff_grows_caps_and_resets() {
    assert_eq!(Backoff::delay_secs(0), 0);
    assert_eq!(Backoff::delay_secs(1), 5 * 60);
    assert_eq!(Backoff::delay_secs(2), 15 * 60);
    assert_eq!(Backoff::delay_secs(3), 30 * 60);
    assert_eq!(Backoff::delay_secs(40), 30 * 60, "capped");

    let b = Backoff::new();
    assert!(!b.blocked(1_000), "no failure yet");
    b.record(false, 1_000);
    assert!(b.blocked(1_000 + 4 * 60));
    assert!(!b.blocked(1_000 + 5 * 60), "first window is 5 minutes");
    b.record(false, 2_000);
    assert!(b.blocked(2_000 + 14 * 60), "second window is 15 minutes");
    assert!(!b.blocked(2_000 + 15 * 60));
    b.record(true, 3_000);
    assert!(!b.blocked(3_000), "a clean sync resets the streak");
    b.record(false, 4_000);
    assert!(!b.blocked(4_000 + 5 * 60), "back to the first window");
}

/// PR #961 review, the unbounded loop: a changed inventory whose sync keeps
/// failing (OSV 429, offline) is NOT due again inside the back-off, so the
/// scheduler's 5-minute full check does not re-POST the inventory each time.
/// "Check now" ignores the back-off.
#[test]
fn a_changed_inventory_after_a_failed_sync_waits_out_the_backoff() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/p", "serde", Some("1.0.200"), "rust", false, None)
        .unwrap();
    db.update_osv_sync_status("crates.io", 0, None).unwrap();
    db.set_kv(INVENTORY_KV_KEY, "an older inventory").unwrap();
    let now = chrono::Utc::now().timestamp();
    let b = Backoff::new();
    assert!(due_at(&db, &b, true, now), "changed inventory, no failure");

    b.record(false, now); // the sync of that inventory failed
    assert!(
        !due_at(&db, &b, true, now + 60),
        "inside the back-off: not due, however changed"
    );
    assert!(
        due_at(&db, &b, false, now + 60),
        "Check now bypasses the back-off"
    );
    assert!(
        due_at(&db, &b, true, now + 5 * 60),
        "due again when the window ends"
    );
}

/// "Check now" behind a running sync waits for it, then re-checks: nothing
/// left to do returns `None` without syncing; still due runs. A free gate
/// runs at once without asking.
#[tokio::test]
async fn check_now_waits_for_a_running_sync_then_rechecks() {
    use std::sync::Arc;
    for still_due in [false, true] {
        let gate = Arc::new(Mutex::new(()));
        let flag = Arc::new(AtomicBool::new(false));
        let asked = Arc::new(AtomicUsize::new(0));
        let held = gate.clone().lock_owned().await; // another sync holds it
        let (g, f, a) = (gate.clone(), flag.clone(), asked.clone());
        let task = tokio::spawn(async move {
            run_check_now(
                &g,
                &f,
                || {
                    a.fetch_add(1, Ordering::SeqCst);
                    still_due
                },
                || async { 9 },
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!task.is_finished(), "waits behind the running sync");
        assert_eq!(asked.load(Ordering::SeqCst), 0);
        drop(held);
        let out = task.await.unwrap();
        assert_eq!(asked.load(Ordering::SeqCst), 1, "re-checked once");
        assert_eq!(out, still_due.then_some(9));
    }
    let gate = Mutex::new(());
    let flag = AtomicBool::new(false);
    let out = run_check_now(&gate, &flag, || unreachable!(), || async { 3 }).await;
    assert_eq!(out, Some(3), "a free gate runs without re-checking");
}

/// The scheduler tick: the full check at most once per interval; the probe
/// in between, but not inside the back-off, not once spent, and nothing at
/// all while the battery policy holds background work.
#[test]
fn the_scheduler_tick_throttles_its_checks() {
    let every = 300;
    let probe_hit = || true;
    let probe_miss = || false;
    let never = || -> bool { unreachable!("probe must not run") };
    assert!(
        tick_wants_check(true, 300, every, false, false, probe_miss),
        "interval elapsed"
    );
    assert!(
        !tick_wants_check(true, 60, every, false, false, probe_miss),
        "throttled"
    );
    assert!(
        tick_wants_check(true, 60, every, false, false, probe_hit),
        "never synced: probe starts it"
    );
    assert!(
        !tick_wants_check(true, 60, every, true, false, never),
        "back-off skips the probe"
    );
    assert!(
        !tick_wants_check(true, 60, every, false, true, never),
        "spent probe is skipped"
    );
    assert!(
        tick_wants_check(true, 600, every, true, true, never),
        "full check still runs"
    );
    assert!(
        !tick_wants_check(false, 600, every, false, false, never),
        "low battery: nothing"
    );
}

/// PR #961 review: launch's offline-archive refresh rewrites the ZIPs in
/// place, so it runs inside the gate, never beside another sync.
#[tokio::test]
async fn the_follow_up_runs_inside_the_gate_only_after_a_sync() {
    let gate = Mutex::new(());
    let flag = AtomicBool::new(false);
    let out = run_exclusive_then(
        &gate,
        &flag,
        || true,
        || async { 1 },
        || async { gate.try_lock().is_err() && flag.load(Ordering::SeqCst) },
    )
    .await;
    assert!(matches!(out, Outcome::Ran((1, true))), "{out:?}");

    let followed = AtomicBool::new(false);
    let out = run_exclusive_then(
        &gate,
        &flag,
        || false,
        || async { 1 },
        || async { followed.store(true, Ordering::SeqCst) },
    )
    .await;
    assert!(matches!(out, Outcome::NotDue));
    assert!(!followed.load(Ordering::SeqCst), "no sync, no follow-up");
}
