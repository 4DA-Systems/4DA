// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `dep_rescan`: the shared due-logic (moved from `headless.rs`) and the
//! gated rescan's skip-when-shut / no-double-run behaviour.

use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::inventory_freshness::{try_begin_refresh_on, Gate};

#[test]
fn parse_dep_scan_max_age_defaults_and_overrides() {
    assert_eq!(parse_dep_scan_max_age(None), 24);
    assert_eq!(parse_dep_scan_max_age(Some("".into())), 24);
    assert_eq!(parse_dep_scan_max_age(Some("  ".into())), 24);
    assert_eq!(parse_dep_scan_max_age(Some("abc".into())), 24);
    assert_eq!(parse_dep_scan_max_age(Some("0".into())), 24);
    assert_eq!(parse_dep_scan_max_age(Some("-5".into())), 24);
    assert_eq!(parse_dep_scan_max_age(Some("6".into())), 6);
    assert_eq!(parse_dep_scan_max_age(Some(" 12 ".into())), 12);
}

#[test]
fn dep_scan_due_never_scanned_or_stale() {
    // Never scanned -> always due.
    assert!(dep_scan_due(&[], None));
    // Unparseable timestamp -> due (fail safe, don't wedge).
    assert!(dep_scan_due(&[], Some("not-a-date")));
    // Far older than the 24h budget -> due.
    assert!(dep_scan_due(&[], Some("2000-01-01 00:00:00")));
}

#[test]
fn dep_scan_due_fresh_and_unchanged_is_not_due() {
    // Just scanned, and no dirs to find a changed manifest in -> not due.
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    assert!(!dep_scan_due(&[], Some(&now)));
}

/// Live 2026-10-24: a lockfile rewritten after the last scan makes the scan due,
/// within the max-age budget (the case the GUI cycle never caught).
#[test]
fn dep_scan_due_when_a_lockfile_changed_after_the_last_scan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let an_hour_ago = (chrono::Utc::now() - chrono::Duration::hours(1))
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let dirs = vec![dir.path().to_path_buf()];
    assert!(!dep_scan_due(&dirs, Some(&an_hour_ago)), "empty tree");
    fs::write(dir.path().join("Cargo.lock"), "version = 4\n").expect("write lockfile");
    assert!(dep_scan_due(&dirs, Some(&an_hour_ago)));
}

#[test]
fn manifest_changed_since_detects_recent_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname='x'\n").expect("write");
    // Anything modified after the epoch counts as "changed since" 0.
    assert!(manifest_changed_since(dir.path(), 0, 0));
    // Nothing is newer than a far-future cutoff.
    assert!(!manifest_changed_since(dir.path(), 9_999_999_999, 0));
}

#[test]
fn manifest_changed_since_ignores_non_manifests_and_vendored_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A non-manifest file is not watched.
    fs::write(dir.path().join("README.md"), "hi").expect("write");
    assert!(!manifest_changed_since(dir.path(), 0, 0));
    // A manifest buried in a skipped vendored dir is ignored.
    let nm = dir.path().join("node_modules").join("dep");
    fs::create_dir_all(&nm).expect("mkdir");
    fs::write(nm.join("package.json"), "{}").expect("write");
    assert!(!manifest_changed_since(dir.path(), 0, 0));
    // But a real nested manifest IS found.
    let pkg = dir.path().join("crates").join("inner");
    fs::create_dir_all(&pkg).expect("mkdir");
    fs::write(pkg.join("Cargo.toml"), "[package]\nname='y'\n").expect("write");
    assert!(manifest_changed_since(dir.path(), 0, 0));
}

const LONG: Duration = Duration::from_hours(1);

fn counting_scan(
    runs: &Arc<AtomicUsize>,
    takes: Duration,
) -> impl Future<Output = ()> + Send + 'static {
    let runs = runs.clone();
    async move {
        runs.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(takes).await;
    }
}

/// While the startup scan holds the gate, the scheduled cycle's rescan does not run.
#[tokio::test]
async fn a_rescan_is_skipped_while_the_startup_scan_holds_the_gate() {
    let gate = Arc::new(Gate::new());
    let startup = try_begin_refresh_on(gate.clone(), LONG, || {}).expect("startup scan");
    let runs = Arc::new(AtomicUsize::new(0));
    let g = gate.clone();
    let outcome = run_gated_rescan(
        move || try_begin_refresh_on(g, LONG, || {}),
        counting_scan(&runs, Duration::ZERO),
        LONG,
    )
    .await;
    assert_eq!(outcome, RescanOutcome::Skipped);
    assert_eq!(runs.load(Ordering::SeqCst), 0, "the scan never ran");
    drop(startup);
}

/// Two overlapping cycles: the lockfiles are re-read once, not twice.
#[tokio::test]
async fn overlapping_rescans_run_the_scan_once() {
    let gate = Arc::new(Gate::new());
    let runs = Arc::new(AtomicUsize::new(0));
    let (g1, g2) = (gate.clone(), gate.clone());
    let first = run_gated_rescan(
        move || try_begin_refresh_on(g1, LONG, || {}),
        counting_scan(&runs, Duration::from_millis(50)),
        LONG,
    );
    let second = run_gated_rescan(
        move || try_begin_refresh_on(g2, LONG, || {}),
        counting_scan(&runs, Duration::ZERO),
        LONG,
    );
    let (a, b) = tokio::join!(first, second);
    assert_eq!(a, RescanOutcome::Finished);
    assert_eq!(b, RescanOutcome::Skipped);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

/// During the rescan the brief reads a shut gate; on completion the gate reopens and the
/// finish hook (Preemption rebuild + inventory-fresh emit, in the app) runs once.
#[tokio::test]
async fn the_gate_is_shut_during_the_rescan_and_reopens_with_the_finish_hook() {
    let gate = Arc::new(Gate::new());
    let finished = Arc::new(AtomicUsize::new(0));
    let shut_during_scan = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (g, f, probe, seen) = (
        gate.clone(),
        finished.clone(),
        gate.clone(),
        shut_during_scan.clone(),
    );
    let scan = async move {
        seen.store(!probe.is_fresh(), Ordering::SeqCst);
    };
    let outcome = run_gated_rescan(
        move || {
            try_begin_refresh_on(g, LONG, move || {
                f.fetch_add(1, Ordering::SeqCst);
            })
        },
        scan,
        LONG,
    )
    .await;
    assert_eq!(outcome, RescanOutcome::Finished);
    assert!(
        shut_during_scan.load(Ordering::SeqCst),
        "dependency facts wait while lockfiles are re-read"
    );
    assert!(gate.is_fresh());
    assert!(!gate.scan_in_flight());
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

/// A scan that outlives the cycle's wait keeps its slot: the next cycle skips.
#[tokio::test]
async fn a_slow_rescan_keeps_its_slot_past_the_cycle_wait() {
    let gate = Arc::new(Gate::new());
    let runs = Arc::new(AtomicUsize::new(0));
    let g = gate.clone();
    let outcome = run_gated_rescan(
        move || try_begin_refresh_on(g, LONG, || {}),
        counting_scan(&runs, Duration::from_millis(300)),
        Duration::from_millis(10),
    )
    .await;
    assert_eq!(outcome, RescanOutcome::StillRunning);
    assert!(gate.scan_in_flight());
    let g = gate.clone();
    let next = run_gated_rescan(
        move || try_begin_refresh_on(g, LONG, || {}),
        counting_scan(&runs, Duration::ZERO),
        LONG,
    )
    .await;
    assert_eq!(next, RescanOutcome::Skipped);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}
