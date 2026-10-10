// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Preemption feed cache: first paint, per engine cycle, and across restarts.
//!
//! `get_preemption_alerts` recomputes live OSV matching AND runs an
//! adversarial LLM deliberation (one call per Medium/Watch item), so an
//! uncached call costs 23 s idle and 74 s under startup contention (live
//! 2026-10-07 / 2026-10-10). The fully-deliberated `EvidenceFeed` is cached
//! in-process and served stale-while-revalidate:
//!
//! - **Per engine cycle** (audit 2026-10-07, wave 2c): each recorded cycle
//!   bumps the generation and pre-warms the next feed in the background
//!   ([`refresh_preemption_cache_after_cycle`]); the previous cycle's feed
//!   keeps serving while that refresh runs. [`MAX_AGE`] is only a backstop
//!   for a session where no cycle runs at all.
//! - **Across a restart** (wave 9e): the last good feed of each tier scope is
//!   persisted (`restart_snapshot`) and, on the first read of a new run,
//!   served at once while this run's feed computes in the background —
//!   instead of a cold compute on the request path (live 2026-10-10: more
//!   than 60 s after launch, because the startup warm waits for first-light
//!   plus a 3-minute debug grace). The feed carries `computed_at`, so the tab
//!   says how old it is.
//! - **Single flight**: the startup warm, the post-cycle refresh and the
//!   restart rebuild share one in-flight flag. A warm that finds a refresh
//!   running joins it — and queues one more pass only when that refresh
//!   started before the data the warm is for (an OSV sync) was in place.
//!
//! Tier safety: a snapshot is filed by tier scope, restored only for the
//! scope the caller is entitled to at serve time, rejected when its own scope
//! disagrees, and never served across tiers while it sits in the cache. Every
//! response still passes through `present_preemption_list`, which applies the
//! caller's dismissals (and snoozes, which are dismissals with a TTL) per
//! call — a snapshot written before a dismissal cannot resurrect the item.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio::sync::Notify;
use tracing::{debug, info, warn};

use crate::evidence::{EvidenceFeed, TierScope};
use crate::restart_snapshot::SnapshotFile;

use super::{
    compute_preemption_evidence_feed, compute_preemption_fast_full_feed,
    compute_preemption_free_floor_feed, free_floor_view,
};

type FeedResult = std::result::Result<EvidenceFeed, String>;
type FeedFuture = Pin<Box<dyn Future<Output = FeedResult> + Send>>;
/// How a refresh computes its feed — injected so tests can drive the cache.
pub(super) type Compute = fn() -> FeedFuture;

/// Backstop staleness bound when no engine cycle runs (monitoring off).
pub(super) const MAX_AGE: Duration = Duration::from_hours(1);

/// A persisted feed older than this is rebuilt cold (the Blind Spots horizon:
/// a week covers a weekend away).
const SNAPSHOT_MAX_AGE: Duration = Duration::from_hours(24 * 7);

static SNAPSHOT_FULL: SnapshotFile =
    SnapshotFile::new("preemption_feed_full.json", SNAPSHOT_MAX_AGE);
static SNAPSHOT_FREE_FLOOR: SnapshotFile =
    SnapshotFile::new("preemption_feed_free_floor.json", SNAPSHOT_MAX_AGE);

/// The snapshot file for one tier scope. Separate files: a free user's
/// restore can never even read the Signal feed.
pub(super) fn snapshot_for(scope: TierScope) -> &'static SnapshotFile {
    match scope {
        TierScope::Full => &SNAPSHOT_FULL,
        TierScope::FreeFloor => &SNAPSHOT_FREE_FLOOR,
    }
}

pub(super) fn scope_for(entitled: bool) -> TierScope {
    if entitled {
        TierScope::Full
    } else {
        TierScope::FreeFloor
    }
}

/// Where a cached feed came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    /// Computed by this run: the deliberated feed, or the free floor.
    Computed,
    /// The entitled cache-miss fast feed (no adversarial deliberation). It
    /// serves until the refresh started beside it lands.
    Provisional,
    /// The previous run's feed from disk — served only while its replacement
    /// computes, and only to the tier scope it was materialized for.
    Restored,
}

struct Entry {
    computed_at: Instant,
    /// When the computation that produced it started (for the warm's
    /// "already reflects the data" test).
    started_at: Instant,
    /// The engine-cycle generation the feed was computed for.
    generation: u64,
    origin: Origin,
    feed: EvidenceFeed,
}

/// Whether a cached feed may be served. Current-generation feeds are fresh;
/// a previous-generation feed keeps serving only while its replacement is
/// being computed (stale-while-revalidate) — if that refresh failed, the next
/// reader recomputes rather than serving an old cycle forever.
pub(super) fn entry_servable(
    entry_generation: u64,
    current_generation: u64,
    age: Duration,
    refresh_in_flight: bool,
) -> bool {
    age < MAX_AGE && (entry_generation == current_generation || refresh_in_flight)
}

pub(super) struct FeedCache {
    slot: Mutex<Option<Entry>>,
    /// Bumped once per recorded engine cycle; an older-generation feed is stale.
    generation: AtomicU64,
    refresh_in_flight: AtomicBool,
    /// A refresh was requested while one was running — run once more when it
    /// finishes, so a request is never silently dropped.
    refresh_again: AtomicBool,
    /// When the running refresh pass started computing.
    pass_started: Mutex<Option<Instant>>,
    /// Signalled whenever a refresh run ends (joined warms wait on it).
    refresh_done: Notify,
}

/// Ends a refresh run even if its computation panics or is cancelled, so a
/// crashed refresh cannot pin a stale feed as "being replaced" forever.
struct InFlightGuard<'a>(&'a FeedCache);

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        *self.0.pass_started.lock() = None;
        self.0.refresh_in_flight.store(false, Ordering::SeqCst);
        self.0.refresh_done.notify_waiters();
    }
}

impl FeedCache {
    pub(super) fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            generation: AtomicU64::new(0),
            refresh_in_flight: AtomicBool::new(false),
            refresh_again: AtomicBool::new(false),
            pass_started: Mutex::new(None),
            refresh_done: Notify::new(),
        }
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(super) fn refresh_in_flight(&self) -> bool {
        self.refresh_in_flight.load(Ordering::SeqCst)
    }

    /// Mark the cached feed stale (a new engine cycle).
    pub(super) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// The cached feed for a caller entitled to `scope`, if servable:
    ///
    /// - a free caller gets any cached feed narrowed to the floor (a full
    ///   feed narrows losslessly), EXCEPT a restored Signal snapshot, which a
    ///   free caller is never served in any form;
    /// - an entitled caller gets a full feed only — a cached floor (e.g. a
    ///   trial started this session) is a miss.
    pub(super) fn serve(&self, scope: TierScope) -> Option<EvidenceFeed> {
        let (feed, age, generation) = {
            let slot = self.slot.lock();
            let e = slot.as_ref()?;
            let age = e.computed_at.elapsed();
            if !entry_servable(
                e.generation,
                self.generation(),
                age,
                self.refresh_in_flight(),
            ) {
                return None;
            }
            if e.origin == Origin::Restored && e.feed.tier_scope != Some(scope) {
                return None;
            }
            (e.feed.clone(), age, e.generation)
        };
        let served = match scope {
            TierScope::FreeFloor => free_floor_view(feed),
            TierScope::Full if feed.tier_scope == Some(TierScope::Full) => feed,
            TierScope::Full => return None,
        };
        info!(
            target: "4da::preemption",
            age_secs = age.as_secs(),
            generation,
            "preemption feed served from cache"
        );
        Some(served)
    }

    pub(super) fn store(
        &self,
        feed: EvidenceFeed,
        generation: u64,
        origin: Origin,
        started_at: Instant,
    ) {
        *self.slot.lock() = Some(Entry {
            computed_at: Instant::now(),
            started_at,
            generation,
            origin,
            feed,
        });
    }

    /// The cache already holds a feed of this run, for the current cycle and
    /// `scope`, whose computation started at or after `since`.
    fn holds_computed_since(&self, scope: TierScope, since: Instant) -> bool {
        self.slot.lock().as_ref().is_some_and(|e| {
            e.origin == Origin::Computed
                && e.generation == self.generation()
                && e.feed.tier_scope == Some(scope)
                && e.started_at >= since
                && e.computed_at.elapsed() < MAX_AGE
        })
    }

    /// Seed an EMPTY cache with the previous run's feed and start its
    /// replacement in the background (or ride the refresh already running —
    /// e.g. the startup warm). The seed sits one generation behind, so it is
    /// served only while that refresh is in flight; if the refresh fails the
    /// next reader computes. Returns whether the seed is being served.
    pub(super) fn serve_restored(&'static self, feed: EvidenceFeed, compute: Compute) -> bool {
        {
            let mut slot = self.slot.lock();
            if slot.is_some() {
                return false;
            }
            let now = Instant::now();
            *slot = Some(Entry {
                computed_at: now,
                started_at: now,
                generation: self.generation().wrapping_sub(1),
                origin: Origin::Restored,
                feed,
            });
        }
        self.refresh_in_flight() || self.refresh_in_background("restored-snapshot", compute)
    }

    /// Recompute on the async runtime so the next reader hits a warm cache.
    /// Coalesces: a request during a running refresh queues one more pass.
    pub(super) fn refresh_in_background(
        &'static self,
        reason: &'static str,
        compute: Compute,
    ) -> bool {
        if self.refresh_in_flight.swap(true, Ordering::SeqCst) {
            self.refresh_again.store(true, Ordering::SeqCst);
            debug!(
                target: "4da::preemption",
                reason,
                "preemption refresh already in flight — queued one more pass"
            );
            return true;
        }
        tauri::async_runtime::spawn(async move { self.run_refresh(reason, compute).await });
        true
    }

    /// The refresh loop. The caller has already set `refresh_in_flight`.
    async fn run_refresh(&self, reason: &'static str, compute: Compute) {
        let _in_flight = InFlightGuard(self);
        loop {
            let generation = self.generation();
            let started = Instant::now();
            *self.pass_started.lock() = Some(started);
            match compute().await {
                Ok(feed) => {
                    info!(
                        target: "4da::preemption",
                        reason, generation, items = feed.items.len(), scope = ?feed.tier_scope,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "Preemption feed cache refreshed"
                    );
                    self.store(feed, generation, Origin::Computed, started);
                }
                Err(e) => warn!(
                    target: "4da::preemption",
                    reason, error = %e,
                    "Preemption cache refresh failed (next read computes)"
                ),
            }
            if !self.refresh_again.swap(false, Ordering::SeqCst) {
                break;
            }
        }
    }

    async fn wait_for_refresh(&self) {
        loop {
            // Registered before the check: a refresh that ends in between
            // still wakes this waiter.
            let done = self.refresh_done.notified();
            if !self.refresh_in_flight() {
                return;
            }
            done.await;
        }
    }

    /// Make sure the cache holds a feed computed from the data in place at
    /// `data_ready_at`, computing it only if no refresh already covers it.
    /// Returns once that feed is stored (or the attempt failed), so callers
    /// can sequence other heavy work after it.
    pub(super) async fn warm(
        &'static self,
        reason: &'static str,
        scope: TierScope,
        data_ready_at: Instant,
        compute: Compute,
    ) {
        // Bounded: join at most twice, then compute once ourselves.
        for _ in 0..3 {
            if self.holds_computed_since(scope, data_ready_at) {
                info!(
                    target: "4da::preemption",
                    reason,
                    "Preemption warm skipped — this run already computed the current feed"
                );
                return;
            }
            if !self.refresh_in_flight.swap(true, Ordering::SeqCst) {
                self.run_refresh(reason, compute).await;
                return;
            }
            // A refresh is running (the restart rebuild, a post-cycle pass).
            // If its pass started before this warm's data was in place, it
            // cannot reflect it: queue exactly one more pass.
            let stale_pass = self
                .pass_started
                .lock()
                .is_some_and(|started| started < data_ready_at);
            if stale_pass {
                self.refresh_again.store(true, Ordering::SeqCst);
            }
            info!(
                target: "4da::preemption",
                reason,
                queued_pass = stale_pass,
                "Preemption warm joined the refresh already in flight"
            );
            self.wait_for_refresh().await;
        }
    }
}

static CACHE: Lazy<FeedCache> = Lazy::new(FeedCache::new);

/// The tier-correct feed for the cache: Signal/trial gets the full
/// deliberated feed, free tier the deterministic OSV floor. Blocking parts
/// run on the blocking pool so a background refresh never stalls an async
/// worker. Every success is persisted for the next launch.
async fn compute_feed_for_cache() -> FeedResult {
    let feed = if crate::settings::is_signal() {
        compute_preemption_evidence_feed().await?
    } else {
        crate::ipc_blocking::off_ui_thread(
            "preemption free-floor feed",
            compute_preemption_free_floor_feed,
        )
        .await?
    };
    let to_persist = feed.clone();
    // Best-effort; a failed write only costs the next launch its warm start.
    let _ = crate::ipc_blocking::off_ui_thread_infallible("preemption feed snapshot", move || {
        persist(&to_persist);
    })
    .await;
    Ok(feed)
}

fn compute_for_cache() -> FeedFuture {
    Box::pin(compute_feed_for_cache())
}

/// Persist a feed under its own tier scope. A feed without a scope is never
/// persisted (it could not be filed safely).
fn persist(feed: &EvidenceFeed) {
    if let Some(scope) = feed.tier_scope {
        snapshot_for(scope).persist_detached(feed);
    }
}

/// A restored feed may answer a caller entitled to `scope` only when it was
/// materialized for exactly that scope.
pub(super) fn restored_feed_fits(feed: &EvidenceFeed, scope: TierScope) -> bool {
    feed.tier_scope == Some(scope)
}

/// The previous run's feed for `scope`, read at most once per process.
fn restore_snapshot(scope: TierScope) -> Option<EvidenceFeed> {
    let restored = snapshot_for(scope).restore_once_detached::<EvidenceFeed>()?;
    let computed_at = restored.computed_at();
    let mut feed = restored.value;
    if !restored_feed_fits(&feed, scope) {
        warn!(
            target: "4da::preemption",
            ?scope,
            found = ?feed.tier_scope,
            "persisted preemption feed is for another tier scope — not served"
        );
        return None;
    }
    feed.computed_at.get_or_insert(computed_at);
    Some(feed)
}

/// Engine-cycle hook: invalidate the cached feed's generation and pre-warm
/// the next one in the background, so the first tab open after a cycle is
/// cache-served. Never blocks the caller.
pub(crate) fn refresh_preemption_cache_after_cycle() {
    CACHE.invalidate();
    CACHE.refresh_in_background("post-cycle", compute_for_cache);
}

/// Pre-compute and cache the Preemption feed against the data in place now
/// (an OSV sync just finished). Best-effort: errors are logged, never
/// propagated.
pub async fn warm_preemption_cache() {
    warm_preemption_cache_since(Instant::now()).await;
}

/// Pre-compute and cache the Preemption feed for data that was in place at
/// `data_ready_at` — single flight with any refresh already running.
///
/// Tier-aware: Signal/trial warms the full deliberated feed; free tier warms
/// only the deterministic OSV floor — never spend LLM deliberating items a
/// free user won't be served.
pub async fn warm_preemption_cache_since(data_ready_at: Instant) {
    let scope = scope_for(crate::settings::is_signal());
    CACHE
        .warm("startup-warm", scope, data_ready_at, compute_for_cache)
        .await;
}

/// The tier-correct FULL feed backing both the list response and the item
/// detail path: cache-served when fresh, the previous run's feed on the first
/// read after a restart (while this run's computes), computed and stored on a
/// miss. Entitlement is read HERE, per call — never taken from the cache.
///
/// Blocking on a miss (corpus-scale OSV matching, 23 s cold measured
/// 2026-10-07) — the commands reach it through
/// [`current_tier_feed_off_thread`] so it runs on the blocking pool.
fn current_tier_feed() -> FeedResult {
    let entitled = crate::settings::is_signal();
    let scope = scope_for(entitled);
    let generation = CACHE.generation();
    if let Some(feed) = CACHE.serve(scope) {
        return Ok(feed);
    }
    // First read of this run, before any feed was computed: answer with the
    // last run's feed for this scope while this run's computes.
    if let Some(feed) = restore_snapshot(scope) {
        if CACHE.serve_restored(feed.clone(), compute_for_cache) {
            info!(
                target: "4da::preemption",
                ?scope,
                computed_at = feed.computed_at.as_deref().unwrap_or(""),
                "Serving the persisted preemption feed while it rebuilds"
            );
            return Ok(feed);
        }
    }
    let started = Instant::now();
    let feed = if entitled {
        // The fast path skips adversarial deliberation (a miss must not wait
        // on an LLM); the refresh started beside it replaces it.
        let feed = compute_preemption_fast_full_feed()?;
        CACHE.store(feed.clone(), generation, Origin::Provisional, started);
        CACHE.refresh_in_background("entitled-cache-miss", compute_for_cache);
        feed
    } else {
        let feed = compute_preemption_free_floor_feed()?;
        CACHE.store(feed.clone(), generation, Origin::Computed, started);
        persist(&feed);
        feed
    };
    Ok(feed)
}

pub(super) async fn current_tier_feed_off_thread() -> FeedResult {
    crate::ipc_blocking::off_ui_thread("preemption feed", current_tier_feed).await
}

#[cfg(test)]
#[path = "preemption_feed_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "preemption_feed_cache_live_tests.rs"]
mod live_tests;
