// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use std::sync::atomic::AtomicUsize;

use super::*;
use crate::evidence::{Confidence, EvidenceItem, EvidenceKind, LensHints, Urgency};
use crate::restart_snapshot::{self as snapshot, Stamp};

fn item(id: &str, confidence: Confidence, urgency: Urgency) -> EvidenceItem {
    EvidenceItem {
        id: id.to_string(),
        kind: EvidenceKind::Alert,
        title: format!("test alert {id}"),
        explanation: "test".to_string(),
        confidence,
        urgency,
        reversibility: None,
        evidence: vec![],
        evidence_total: None,
        affected_projects: vec![],
        affected_deps: vec![],
        suggested_actions: vec![],
        precedents: vec![],
        refutation_condition: None,
        lens_hints: LensHints::preemption_only(),
        created_at: 0,
        expires_at: None,
    }
}

/// A Signal feed: an OSV-verified alert plus an LLM-assessed one (the part
/// a free user must never see). `marker` names the OSV item.
fn full_feed(marker: &str) -> EvidenceFeed {
    let mut feed = EvidenceFeed::from_items(vec![
        item(marker, Confidence::osv_verified(0.9), Urgency::Critical),
        item(
            "llm-signal-only",
            Confidence::llm_assessed(0.7),
            Urgency::High,
        ),
    ]);
    feed.tier_scope = Some(TierScope::Full);
    feed.computed_at = Some("2026-10-09T01:56:52+00:00".to_string());
    feed
}

fn floor_feed(marker: &str) -> EvidenceFeed {
    let mut feed = EvidenceFeed::from_items(vec![item(
        marker,
        Confidence::osv_verified(0.9),
        Urgency::Critical,
    )]);
    feed.tier_scope = Some(TierScope::FreeFloor);
    feed
}

fn first_id(feed: &EvidenceFeed) -> &str {
    feed.items.first().map_or("", |i| i.id.as_str())
}

fn leaked() -> &'static FeedCache {
    Box::leak(Box::new(FeedCache::new()))
}

fn wait(cache: &'static FeedCache) {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(Duration::from_secs(10), cache.wait_for_refresh())
            .await
            .expect("the refresh finished");
    });
}

// ─── Per-cycle cache (moved from preemption.rs) ──────────────────────────

#[test]
fn feed_cache_is_invalidated_and_rewarmed_per_engine_cycle() {
    let cache = FeedCache::new();
    let before = cache.generation();
    cache.store(full_feed("osv-7"), before, Origin::Computed, Instant::now());
    let got = cache
        .serve(TierScope::Full)
        .expect("a freshly stored feed must be served");
    assert_eq!(first_id(&got), "osv-7", "the exact stored feed");

    // A cycle is recorded: the old feed is stale. While its replacement is
    // being computed it still serves (no cold 23 s compute)...
    cache.invalidate();
    cache.refresh_in_flight.store(true, Ordering::SeqCst);
    assert_eq!(
        cache
            .serve(TierScope::Full)
            .map(|f| first_id(&f).to_string()),
        Some("osv-7".to_string()),
        "stale-while-revalidate: the previous cycle's feed serves during the refresh"
    );
    // ...the post-cycle refresh lands at the new generation and is served.
    cache.store(
        full_feed("osv-9"),
        cache.generation(),
        Origin::Computed,
        Instant::now(),
    );
    cache.refresh_in_flight.store(false, Ordering::SeqCst);
    assert_eq!(
        cache
            .serve(TierScope::Full)
            .map(|f| first_id(&f).to_string()),
        Some("osv-9".to_string())
    );

    // A refresh that failed leaves an old-generation feed and nothing in
    // flight: the next reader must recompute, not serve it forever.
    cache.invalidate();
    assert!(cache.serve(TierScope::Full).is_none());
}

#[test]
fn a_feed_outlives_the_analysis_cycle_interval() {
    // The bug: a 10-minute TTL under an ~11-minute cycle meant almost every
    // post-cycle open paid the cold compute. Freshness is now the cycle
    // generation; age only matters past the backstop.
    let eleven_min = Duration::from_mins(11);
    assert!(entry_servable(3, 3, eleven_min, false));
    assert!(!entry_servable(2, 3, eleven_min, false));
    assert!(entry_servable(2, 3, eleven_min, true));
    assert!(!entry_servable(3, 3, MAX_AGE, false));
}

#[test]
fn the_engine_cycle_pre_warms_the_preemption_cache() {
    let setup = include_str!("app_setup.rs");
    let record = setup
        .find("crate::engine_runs::record(receipt);")
        .expect("the successful-cycle receipt site");
    // `find` returns a char boundary, so split_at cannot panic here.
    let next: String = setup.split_at(record).1.chars().take(600).collect();
    assert!(
        next.contains("crate::preemption::refresh_preemption_cache_after_cycle();"),
        "the cycle-record site must pre-warm the preemption cache"
    );
}

/// A cached Signal feed narrows losslessly for a free caller (trial ended
/// mid-session); a cached floor is a miss for an entitled one.
#[test]
fn computed_feeds_serve_each_tier_its_own_view() {
    let cache = FeedCache::new();
    cache.store(full_feed("osv-1"), 0, Origin::Computed, Instant::now());
    let floor = cache.serve(TierScope::FreeFloor).expect("narrowed");
    assert_eq!(floor.tier_scope, Some(TierScope::FreeFloor));
    assert!(floor.items.iter().all(|i| i.id != "llm-signal-only"));
    assert_eq!(
        floor.computed_at,
        full_feed("x").computed_at,
        "keeps its computed-at"
    );

    let cache = FeedCache::new();
    cache.store(floor_feed("osv-2"), 0, Origin::Computed, Instant::now());
    assert!(cache.serve(TierScope::Full).is_none());
}

// ─── Restart: serve the persisted feed while it rebuilds ─────────────────

static COLD_REBUILDS: AtomicUsize = AtomicUsize::new(0);
fn cold_rebuild() -> FeedFuture {
    Box::pin(async {
        COLD_REBUILDS.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(150)).await;
        Ok(full_feed("osv-rebuilt"))
    })
}

#[test]
fn a_restored_feed_is_served_on_a_cold_cache_while_it_rebuilds() {
    let cache = leaked();
    assert!(
        cache.serve(TierScope::Full).is_none(),
        "cold after a restart"
    );
    assert!(cache.serve_restored(full_feed("osv-restored"), cold_rebuild));
    let served = cache.serve(TierScope::Full).expect("served at once");
    assert_eq!(first_id(&served), "osv-restored");
    assert!(served.computed_at.is_some(), "says when it was computed");
    assert!(
        cache.refresh_in_flight(),
        "the rebuild started in the background"
    );
    wait(cache);
    assert_eq!(COLD_REBUILDS.load(Ordering::SeqCst), 1);
    assert_eq!(
        cache
            .serve(TierScope::Full)
            .map(|f| first_id(&f).to_string()),
        Some("osv-rebuilt".to_string()),
        "the rebuilt feed replaces the snapshot"
    );
    assert!(
        !cache.serve_restored(full_feed("osv-late"), cold_rebuild),
        "a snapshot never displaces a feed this run computed"
    );
}

static FAILED_REBUILDS: AtomicUsize = AtomicUsize::new(0);
fn failing_rebuild() -> FeedFuture {
    Box::pin(async {
        FAILED_REBUILDS.fetch_add(1, Ordering::SeqCst);
        Err("database unavailable".to_string())
    })
}

#[test]
fn a_failed_rebuild_stops_the_snapshot_serving() {
    let cache = leaked();
    assert!(cache.serve_restored(full_feed("osv-restored"), failing_rebuild));
    wait(cache);
    assert_eq!(FAILED_REBUILDS.load(Ordering::SeqCst), 1);
    assert!(
        cache.serve(TierScope::Full).is_none(),
        "the next reader computes rather than serving the old run's feed"
    );
}

static TIER_REBUILDS: AtomicUsize = AtomicUsize::new(0);
fn tier_rebuild() -> FeedFuture {
    Box::pin(async {
        TIER_REBUILDS.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(floor_feed("osv-floor-rebuilt"))
    })
}

/// The restored Signal feed is never served to a free caller in ANY form —
/// not even narrowed — and a restored floor never answers a Signal caller.
#[test]
fn a_restored_feed_is_never_served_across_tiers() {
    let cache = leaked();
    assert!(cache.serve_restored(full_feed("osv-restored"), tier_rebuild));
    assert!(
        cache.serve(TierScope::FreeFloor).is_none(),
        "a Signal snapshot is never served to a free caller"
    );
    assert!(cache.serve(TierScope::Full).is_some());
    wait(cache);

    let cache = leaked();
    assert!(cache.serve_restored(floor_feed("osv-floor"), tier_rebuild));
    assert!(cache.serve(TierScope::Full).is_none());
    assert!(cache.serve(TierScope::FreeFloor).is_some());
    wait(cache);

    // The restore itself only accepts a feed filed for the caller's scope.
    assert!(restored_feed_fits(&full_feed("a"), TierScope::Full));
    assert!(!restored_feed_fits(&full_feed("a"), TierScope::FreeFloor));
    assert!(!restored_feed_fits(&floor_feed("a"), TierScope::Full));
    let mut unscoped = full_feed("a");
    unscoped.tier_scope = None;
    assert!(!restored_feed_fits(&unscoped, TierScope::FreeFloor));
}

#[test]
fn snapshots_are_filed_per_tier_scope_beside_the_database() {
    let full = snapshot_for(TierScope::Full).path();
    let floor = snapshot_for(TierScope::FreeFloor).path();
    assert_ne!(full, floor, "one file per tier scope");
    for path in [&full, &floor] {
        assert_eq!(path.parent(), crate::state::get_db_path().parent());
        // `data/*.json` is gitignored; so is the `.json.tmp` staging file.
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("json"));
    }
    assert_eq!(scope_for(true), TierScope::Full);
    assert_eq!(scope_for(false), TierScope::FreeFloor);
}

/// The persisted feed comes back whole (items, scope, computed-at) under the
/// same stamp, and not at all after a version change.
#[test]
fn the_persisted_feed_round_trips_and_is_discarded_on_version_change() {
    let dir = std::env::temp_dir().join(format!("4da-preemption-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("preemption_feed_full.json");
    let feed = full_feed("osv-1");
    snapshot::save(&path, &Stamp::with_schema(125), &feed).unwrap();
    let back: snapshot::Restored<EvidenceFeed> =
        snapshot::load(&path, &Stamp::with_schema(125), SNAPSHOT_MAX_AGE).unwrap();
    assert_eq!(back.value, feed);
    assert!(
        snapshot::load::<EvidenceFeed>(&path, &Stamp::with_schema(126), SNAPSHOT_MAX_AGE).is_none(),
        "a schema migration discards it"
    );
}

/// Dismissals (and snoozes — the view stores both as dismissals with a TTL)
/// are applied per call by `present_preemption_list`, so one made after the
/// snapshot was written is honoured, and the counts follow.
#[test]
fn dismissals_made_after_the_snapshot_are_honoured() {
    let cache = leaked();
    assert!(cache.serve_restored(full_feed("osv-restored"), tier_rebuild));
    let served = cache.serve(TierScope::Full).expect("restored");
    assert_eq!(served.critical_count, 1);
    let shown =
        crate::evidence::present_preemption_list(served, &["osv-restored".to_string()], false);
    assert!(shown.items.iter().all(|i| i.id != "osv-restored"));
    assert_eq!(shown.critical_count, 0, "counts follow the dismissal");
    assert_eq!(shown.total, 1);
    wait(cache);
}

// ─── Single flight with the startup warm ─────────────────────────────────

static JOIN_COMPUTES: AtomicUsize = AtomicUsize::new(0);
fn join_compute() -> FeedFuture {
    Box::pin(async {
        JOIN_COMPUTES.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(full_feed("osv-joined"))
    })
}

/// The tab opened during the first-light grace: the restored feed kicked a
/// rebuild. The startup warm for data already in place then JOINS that
/// rebuild — one compute, not two — and returns only once it landed.
#[test]
fn the_startup_warm_joins_the_restart_rebuild() {
    let cache = leaked();
    let data_ready_at = Instant::now();
    assert!(cache.serve_restored(full_feed("osv-restored"), join_compute));
    tauri::async_runtime::block_on(cache.warm(
        "startup-warm",
        TierScope::Full,
        data_ready_at,
        join_compute,
    ));
    assert_eq!(JOIN_COMPUTES.load(Ordering::SeqCst), 1, "single flight");
    assert!(
        !cache.refresh_in_flight(),
        "the warm returned after the rebuild"
    );
    assert_eq!(
        cache
            .serve(TierScope::Full)
            .map(|f| first_id(&f).to_string()),
        Some("osv-joined".to_string())
    );
    // A second warm for the same data finds it already computed.
    tauri::async_runtime::block_on(cache.warm(
        "startup-warm",
        TierScope::Full,
        data_ready_at,
        join_compute,
    ));
    assert_eq!(JOIN_COMPUTES.load(Ordering::SeqCst), 1);
}

static NEWER_DATA_COMPUTES: AtomicUsize = AtomicUsize::new(0);
fn newer_data_compute() -> FeedFuture {
    Box::pin(async {
        NEWER_DATA_COMPUTES.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(full_feed("osv-newer"))
    })
}

/// An OSV sync finished AFTER the running rebuild started: that pass cannot
/// reflect the new advisories, so the warm queues exactly one more pass —
/// still never two computations at once.
#[test]
fn a_warm_for_newer_data_queues_exactly_one_more_pass() {
    let cache = leaked();
    assert!(cache.serve_restored(full_feed("osv-restored"), newer_data_compute));
    std::thread::sleep(Duration::from_millis(50)); // the pass is under way
    let synced_at = Instant::now();
    tauri::async_runtime::block_on(cache.warm(
        "osv-sync-complete",
        TierScope::Full,
        synced_at,
        newer_data_compute,
    ));
    assert_eq!(NEWER_DATA_COMPUTES.load(Ordering::SeqCst), 2);
    assert!(cache.holds_computed_since(TierScope::Full, synced_at));
}

static LONE_WARMS: AtomicUsize = AtomicUsize::new(0);
fn lone_warm() -> FeedFuture {
    Box::pin(async {
        LONE_WARMS.fetch_add(1, Ordering::SeqCst);
        Ok(full_feed("osv-warm"))
    })
}

/// No restart rebuild running (nobody opened the tab): the warm computes
/// once, as before, under the shared in-flight flag.
#[test]
fn a_warm_with_nothing_running_computes_once() {
    let cache = leaked();
    tauri::async_runtime::block_on(cache.warm(
        "startup-warm",
        TierScope::Full,
        Instant::now(),
        lone_warm,
    ));
    assert_eq!(LONE_WARMS.load(Ordering::SeqCst), 1);
    assert!(!cache.refresh_in_flight());
    assert!(cache.serve(TierScope::Full).is_some());
}
