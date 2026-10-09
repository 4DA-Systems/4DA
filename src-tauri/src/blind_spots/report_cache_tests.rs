// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for the per-cycle blind-spot report cache. Each test owns its own
//! leaked `CycleCache`, so they never race on shared statics.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;

fn leaked() -> &'static CycleCache<u32> {
    Box::leak(Box::new(CycleCache::new()))
}

/// Poll until `cond` holds (background refresh threads), failing after 10 s.
fn wait_for(what: &str, cond: impl Fn() -> bool) {
    let began = Instant::now();
    while !cond() {
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_report_survives_within_a_cycle() {
    let cache = CycleCache::<u32>::new();
    let runs = AtomicUsize::new(0);
    let compute = || {
        runs.fetch_add(1, Ordering::SeqCst);
        Ok(7)
    };
    assert_eq!(cache.get_or_compute(compute).unwrap(), 7);
    assert_eq!(cache.get_or_compute(compute).unwrap(), 7);
    assert_eq!(cache.get_or_compute(compute).unwrap(), 7);
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "same generation = one build"
    );
}

#[test]
fn a_cycle_bump_invalidates_when_nothing_is_rebuilding() {
    let cache = CycleCache::<u32>::new();
    cache.get_or_compute(|| Ok(7)).unwrap();
    cache.invalidate();
    assert!(
        cache.servable().is_none(),
        "an old-generation report with no rebuild in flight is not served"
    );
    assert_eq!(cache.get_or_compute(|| Ok(9)).unwrap(), 9);
    assert_eq!(cache.servable(), Some(9));
}

#[test]
fn the_old_report_outlives_the_old_five_minute_ttl() {
    // The bug: a 5-minute TTL meant nearly every tab visit paid the 18 s
    // cold build. Freshness is the cycle generation; age only matters past
    // the 1 h backstop.
    assert!(entry_servable(3, 3, Duration::from_mins(6), false));
    assert!(entry_servable(3, 3, Duration::from_mins(59), false));
    assert!(!entry_servable(2, 3, Duration::from_mins(6), false));
    assert!(entry_servable(2, 3, Duration::from_mins(6), true));
}

#[test]
fn the_one_hour_safety_net_expires_even_a_current_report() {
    assert!(!entry_servable(3, 3, MAX_AGE, false));
    assert!(
        !entry_servable(2, 3, MAX_AGE, true),
        "nor a stale one being rebuilt"
    );
    assert_eq!(MAX_AGE, Duration::from_hours(1));
}

static STALE_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static STALE_BUILDS: AtomicUsize = AtomicUsize::new(0);

fn gated_build() -> crate::error::Result<u32> {
    let _wait = STALE_GATE.lock();
    STALE_BUILDS.fetch_add(1, Ordering::SeqCst);
    Ok(9)
}

#[test]
fn the_previous_report_serves_while_the_next_one_builds() {
    let cache = leaked();
    cache.get_or_compute(|| Ok(7)).unwrap();

    let gate = STALE_GATE.lock(); // hold the background build mid-flight
    assert!(cache.invalidate_and_prewarm("test-cycle", true, gated_build));
    wait_for("the refresh to start", || {
        cache.refresh_in_flight.load(Ordering::SeqCst)
    });

    // A reader during the rebuild gets the previous cycle's report at once —
    // no cold compute, no waiting on the build.
    let served = cache
        .get_or_compute(|| panic!("a reader must not build while a refresh is in flight"))
        .unwrap();
    assert_eq!(served, 7, "stale-while-revalidate");

    drop(gate);
    wait_for("the pre-warmed report", || cache.servable() == Some(9));
    wait_for("the refresh to finish", || {
        !cache.refresh_in_flight.load(Ordering::SeqCst)
    });
    assert_eq!(STALE_BUILDS.load(Ordering::SeqCst), 1);
    assert_eq!(
        cache.get_or_compute(|| Ok(0)).unwrap(),
        9,
        "now a plain hit"
    );
}

static UNENTITLED_BUILDS: AtomicUsize = AtomicUsize::new(0);

fn counted_build() -> crate::error::Result<u32> {
    UNENTITLED_BUILDS.fetch_add(1, Ordering::SeqCst);
    Ok(1)
}

#[test]
fn the_prewarm_is_skipped_without_the_signal_feature() {
    let cache = leaked();
    let before = cache.generation();
    let started = cache.invalidate_and_prewarm("test-cycle", false, counted_build);
    assert!(!started, "no pre-warm for a user without the feature");
    assert_eq!(
        cache.generation(),
        before + 1,
        "the cycle still invalidates"
    );
    assert!(!cache.refresh_in_flight.load(Ordering::SeqCst));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        UNENTITLED_BUILDS.load(Ordering::SeqCst),
        0,
        "nothing was computed"
    );
}

#[test]
fn the_production_prewarm_uses_the_commands_own_gate() {
    let src = include_str!("../blind_spots.rs");
    let compact: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        compact.contains(
            "fn blind_spot_prewarm_entitled() -> bool { crate::settings::require_signal_feature(\"get_blind_spots\").is_ok() }"
        ),
        "the pre-warm must decide entitlement exactly as get_blind_spots does"
    );
    for hook in [
        "pub(crate) fn refresh_blind_spot_cache_after_cycle()",
        "pub fn invalidate_blind_spot_cache()",
    ] {
        let at = compact.find(hook).expect(hook);
        let body: String = compact.split_at(at).1.chars().take(260).collect();
        assert!(
            body.contains("blind_spot_prewarm_entitled()"),
            "{hook} must pass the Signal gate to the pre-warm"
        );
    }
    let warm = compact
        .find("async fn warm_blind_spot_cache_after_startup()")
        .expect("startup warm");
    let body: String = compact.split_at(warm).1.chars().take(160).collect();
    assert!(body.contains("if !blind_spot_prewarm_entitled() { return; }"));
}

static FAILING_BUILDS: AtomicUsize = AtomicUsize::new(0);

fn failing_build() -> crate::error::Result<u32> {
    FAILING_BUILDS.fetch_add(1, Ordering::SeqCst);
    Err(crate::error::FourDaError::Internal("boom".into()))
}

#[test]
fn a_failed_rebuild_stops_serving_the_old_report() {
    let cache = leaked();
    cache.get_or_compute(|| Ok(7)).unwrap();
    assert!(cache.invalidate_and_prewarm("test-cycle", true, failing_build));
    wait_for("the failed refresh to finish", || {
        FAILING_BUILDS.load(Ordering::SeqCst) == 1
            && !cache.refresh_in_flight.load(Ordering::SeqCst)
    });
    assert!(
        cache.servable().is_none(),
        "with nothing rebuilding, the old cycle's report is not served forever"
    );
    assert_eq!(cache.get_or_compute(|| Ok(8)).unwrap(), 8);
}

#[test]
fn clear_never_serves_the_old_report_even_mid_rebuild() {
    let cache = CycleCache::<u32>::new();
    cache.get_or_compute(|| Ok(7)).unwrap();
    cache.clear();
    cache.refresh_in_flight.store(true, Ordering::SeqCst);
    assert!(cache.servable().is_none());
    cache.refresh_in_flight.store(false, Ordering::SeqCst);
    assert_eq!(cache.get_or_compute(|| Ok(3)).unwrap(), 3);
}

static COALESCE_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static COALESCE_BUILDS: AtomicUsize = AtomicUsize::new(0);
static COALESCE_ENTERED: AtomicUsize = AtomicUsize::new(0);

fn coalesced_build() -> crate::error::Result<u32> {
    COALESCE_ENTERED.fetch_add(1, Ordering::SeqCst);
    let _wait = COALESCE_GATE.lock();
    Ok(COALESCE_BUILDS.fetch_add(1, Ordering::SeqCst) as u32 + 100)
}

#[test]
fn invalidations_during_a_rebuild_queue_exactly_one_more_pass() {
    let cache = leaked();
    let gate = COALESCE_GATE.lock();
    assert!(cache.invalidate_and_prewarm("a", true, coalesced_build));
    // Wait until the first pass is INSIDE its build (its generation taken).
    wait_for("the first build to start", || {
        COALESCE_ENTERED.load(Ordering::SeqCst) == 1
    });
    // Three more inputs change mid-build (dismiss, watch, foreground run).
    for reason in ["b", "c", "d"] {
        assert!(cache.invalidate_and_prewarm(reason, true, coalesced_build));
    }
    drop(gate);
    wait_for("the queued pass to finish", || {
        !cache.refresh_in_flight.load(Ordering::SeqCst)
    });
    assert_eq!(
        COALESCE_BUILDS.load(Ordering::SeqCst),
        2,
        "the in-flight build plus one queued pass — not one per request"
    );
    assert_eq!(
        cache.servable(),
        Some(101),
        "the queued pass ran at the latest generation and is current"
    );
}

#[test]
fn the_engine_cycle_pre_warms_the_blind_spot_cache() {
    let setup = include_str!("../app_setup.rs");
    let record = setup
        .find("crate::engine_runs::record(receipt);")
        .expect("the successful-cycle receipt site");
    // `find` returns a char boundary, so split_at cannot panic here.
    let next: String = setup.split_at(record).1.chars().take(800).collect();
    assert!(
        next.contains("crate::blind_spots::refresh_blind_spot_cache_after_cycle();"),
        "each recorded engine cycle must bump the blind-spot generation and pre-warm"
    );
    let warm = setup
        .find("crate::preemption::warm_preemption_cache().await;")
        .expect("the startup Preemption warm");
    let after: String = setup.split_at(warm).1.chars().take(300).collect();
    assert!(
        after.contains("crate::blind_spots::warm_blind_spot_cache_after_startup().await;"),
        "the startup warm runs after first-light, sequenced after Preemption's"
    );
}

#[test]
fn the_phase_clock_records_phases_in_order() {
    let mut clock = PhaseClock::start();
    clock.lap("first");
    std::thread::sleep(Duration::from_millis(15));
    clock.lap("second");
    let phases = clock.finish();
    assert_eq!(
        phases.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert!(phases[1].1 >= 10, "the second lap measured the sleep");
}

/// Where a cold build spends its time, on a SNAPSHOT (the opens migrate it;
/// never point it at the live file):
///
/// ```text
/// FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<scratch> cargo test --lib \
///     live_blind_spot_report_phase_profile -- --ignored --nocapture
/// ```
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_blind_spot_report_phase_profile() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    // The per-dep consequence lookups read the test seam in test builds.
    let seam = rusqlite::Connection::open(&path).expect("open snapshot");
    super::super::test_support::install_test_conn(seam);

    let began = Instant::now();
    let (report, phases) =
        super::super::generate_blind_spot_report_profiled().expect("report on snapshot");
    let build_ms = began.elapsed().as_millis();
    for (phase, ms) in &phases {
        println!("phase {phase:<28} {ms:>7} ms");
    }
    println!(
        "build total {build_ms} ms — uncovered={} stale={} missed={} recs={}",
        report.uncovered_dependencies.len(),
        report.stale_topics.len(),
        report.missed_signals.len(),
        report.recommendations.len()
    );

    // Built once per report and cached with it (no breakdown memo in tests).
    let t = Instant::now();
    let items = super::super::blind_spot_report_items(&report);
    println!(
        "cached report_items {} ms ({} items)",
        t.elapsed().as_millis(),
        items.len()
    );
    // Per-call work a cache HIT still pays.
    let t = Instant::now();
    let feed = super::super::feed_from_report_items(items, &report);
    println!(
        "per-call feed_from_report_items {} ms ({} items)",
        t.elapsed().as_millis(),
        feed.items.len()
    );
    for pass in ["first (opens the DB singleton)", "warm"] {
        let t = Instant::now();
        let judged = super::super::llm_judged_blind_spot_items();
        println!(
            "per-call llm_judged_items, {pass}: {} ms ({} items, stored verdicts only)",
            t.elapsed().as_millis(),
            judged.len()
        );
    }
}
