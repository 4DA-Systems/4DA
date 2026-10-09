// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Blind-spot report cache — one report per engine cycle, pre-warmed.
//!
//! Live 2026-10-08 a cold `get_blind_spots` took 17.7 s on an idle app (21 s
//! during an analysis) and a cached one ~100 ms. The old cache had a
//! 5-minute TTL and only refilled on a miss, so almost every visit to the
//! paid Blind Spots tab paid the cold build. The report only changes when
//! its inputs do — a recorded engine cycle, a foreground analysis, a user
//! action on the stack — so freshness is now a cycle GENERATION, mirroring
//! the Preemption feed cache (audit 2026-10-07, wave 2c → this is wave 2e):
//!
//! - every engine cycle bumps the generation and rebuilds in the background
//!   ([`CycleCache::invalidate_and_prewarm`]), off every caller's path;
//! - while that rebuild runs, the previous cycle's report keeps serving
//!   (stale-while-revalidate) — never a cold 18 s compute on tab open;
//! - a reader that does miss joins the in-flight build (single flight)
//!   instead of starting a second one;
//! - [`MAX_AGE`] is only a backstop for a session where no cycle runs.
//!
//! The pre-warm is gated on the Signal feature the command itself requires
//! (`require_signal_feature("get_blind_spots")`): a free user never pays the
//! build in the background. The build is LLM-free — it reads stored rows
//! only (the Tier-2 judged items are read per call from `llm_judgments`,
//! never generated here), so a pre-warm cannot spend money.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{debug, info, warn};

use crate::error::Result;

/// Backstop staleness bound when no engine cycle runs (monitoring off).
pub(super) const MAX_AGE: Duration = Duration::from_hours(1);

struct Entry<T> {
    computed_at: Instant,
    /// The cycle generation the value was computed for.
    generation: u64,
    value: T,
}

/// A value computed once per engine cycle. Generic so the tests can drive it
/// with a sentinel instead of a real report.
pub(super) struct CycleCache<T> {
    slot: Mutex<Option<Entry<T>>>,
    /// Single flight: held for the duration of one computation.
    compute: Mutex<()>,
    generation: AtomicU64,
    refresh_in_flight: AtomicBool,
    /// An invalidation arrived while a refresh was running — run once more
    /// when it finishes, so the request is never silently dropped.
    refresh_again: AtomicBool,
}

/// Whether a cached entry may be served. Current-generation entries are
/// fresh; a previous-generation entry keeps serving only while its
/// replacement is being computed — if that refresh failed, the next reader
/// recomputes rather than serving an old cycle forever.
pub(super) fn entry_servable(
    entry_generation: u64,
    current_generation: u64,
    age: Duration,
    refresh_in_flight: bool,
) -> bool {
    age < MAX_AGE && (entry_generation == current_generation || refresh_in_flight)
}

/// Clears the in-flight flag even if the computation panics, so a crashed
/// refresh cannot pin a stale report as "being replaced" forever.
struct InFlightGuard<'a>(&'a AtomicBool);

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl<T: Clone + Send + 'static> CycleCache<T> {
    pub(super) const fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            compute: Mutex::new(()),
            generation: AtomicU64::new(0),
            refresh_in_flight: AtomicBool::new(false),
            refresh_again: AtomicBool::new(false),
        }
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// The cached value if it is servable. Clones under the lock.
    pub(super) fn servable(&self) -> Option<T> {
        let slot = self.slot.lock();
        slot.as_ref()
            .filter(|e| {
                entry_servable(
                    e.generation,
                    self.generation(),
                    e.computed_at.elapsed(),
                    self.refresh_in_flight.load(Ordering::SeqCst),
                )
            })
            .map(|e| e.value.clone())
    }

    /// The slot already holds a within-backstop value for `generation`.
    fn holds_generation(&self, generation: u64) -> bool {
        self.slot
            .lock()
            .as_ref()
            .is_some_and(|e| e.generation == generation && e.computed_at.elapsed() < MAX_AGE)
    }

    fn store(&self, value: T, generation: u64) {
        *self.slot.lock() = Some(Entry {
            computed_at: Instant::now(),
            generation,
            value,
        });
    }

    /// The servable value, or `compute()`'s result stored under the current
    /// generation. A miss that races an in-flight computation (a background
    /// refresh, or another reader) waits for it instead of duplicating it.
    pub(super) fn get_or_compute(&self, compute: impl FnOnce() -> Result<T>) -> Result<T> {
        if let Some(hit) = self.servable() {
            return Ok(hit);
        }
        let _flight = self.compute.lock();
        if let Some(hit) = self.servable() {
            return Ok(hit);
        }
        let generation = self.generation();
        let value = compute()?;
        self.store(value.clone(), generation);
        Ok(value)
    }

    /// Mark the cached value stale (bump the generation). It keeps serving
    /// only while a refresh is in flight.
    pub(super) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Drop the cached value outright: the next reader must see a fresh
    /// one, never the stale one (an explicit user setting changed).
    pub(super) fn clear(&self) {
        self.invalidate();
        *self.slot.lock() = None;
    }

    /// Invalidate, then pre-warm in the background when `entitled`. Returns
    /// whether a pre-warm was started or queued. Never blocks the caller.
    pub(super) fn invalidate_and_prewarm(
        &'static self,
        reason: &'static str,
        entitled: bool,
        compute: fn() -> Result<T>,
    ) -> bool {
        self.invalidate();
        if !entitled {
            debug!(
                target: "4da::blind_spots",
                reason,
                "blind-spot pre-warm skipped — Signal feature not available"
            );
            return false;
        }
        self.refresh_in_background(reason, compute)
    }

    /// Recompute on a background thread so the next reader hits a warm
    /// cache. Coalesces: a request during a running refresh queues exactly
    /// one more pass.
    fn refresh_in_background(
        &'static self,
        reason: &'static str,
        compute: fn() -> Result<T>,
    ) -> bool {
        if self.refresh_in_flight.swap(true, Ordering::SeqCst) {
            self.refresh_again.store(true, Ordering::SeqCst);
            debug!(
                target: "4da::blind_spots",
                reason,
                "blind-spot refresh already in flight — queued one more pass"
            );
            return true;
        }
        let spawned = std::thread::Builder::new()
            .name("blind-spots-refresh".to_string())
            .spawn(move || self.run_refresh(reason, compute));
        if let Err(e) = spawned {
            self.refresh_in_flight.store(false, Ordering::SeqCst);
            warn!(target: "4da::blind_spots", error = %e, "Could not spawn blind-spot refresh");
            return false;
        }
        true
    }

    /// The refresh loop. The caller has already set `refresh_in_flight`.
    fn run_refresh(&self, reason: &'static str, compute: fn() -> Result<T>) {
        let _in_flight = InFlightGuard(&self.refresh_in_flight);
        loop {
            let started = Instant::now();
            let result = {
                let _flight = self.compute.lock();
                let generation = self.generation();
                if self.holds_generation(generation) {
                    // A reader computed this generation while we queued for
                    // the flight lock — nothing left to do.
                    Ok(false)
                } else {
                    compute()
                        .map(|value| self.store(value, generation))
                        .map(|()| true)
                }
            };
            match result {
                Ok(false) => debug!(
                    target: "4da::blind_spots",
                    reason,
                    "blind-spot refresh skipped — current generation already cached"
                ),
                Ok(true) => info!(
                    target: "4da::blind_spots",
                    reason,
                    generation = self.generation(),
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Blind-spot report cache refreshed"
                ),
                Err(e) => warn!(
                    target: "4da::blind_spots",
                    reason,
                    error = %e,
                    "Blind-spot report refresh failed (next read computes)"
                ),
            }
            if !self.refresh_again.swap(false, Ordering::SeqCst) {
                break;
            }
        }
    }
}

/// Per-phase wall time of one uncached report build. Logged at debug level
/// (target `4da::blind_spots::profile`); the live-snapshot profile test reads
/// the phases directly.
pub(super) struct PhaseClock {
    started: Instant,
    mark: Instant,
    phases: Vec<(&'static str, u64)>,
}

impl PhaseClock {
    pub(super) fn start() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            mark: now,
            phases: Vec::with_capacity(16),
        }
    }

    /// Close the phase that ran since the previous lap.
    pub(super) fn lap(&mut self, phase: &'static str) {
        let now = Instant::now();
        self.phases
            .push((phase, now.duration_since(self.mark).as_millis() as u64));
        self.mark = now;
    }

    pub(super) fn finish(self) -> Vec<(&'static str, u64)> {
        let total_ms = self.started.elapsed().as_millis() as u64;
        let phases = self
            .phases
            .iter()
            .map(|(p, ms)| format!("{p}={ms}"))
            .collect::<Vec<_>>()
            .join(" ");
        debug!(
            target: "4da::blind_spots::profile",
            total_ms,
            phases = %phases,
            "blind-spot report build phases (ms)"
        );
        self.phases
    }
}

#[cfg(test)]
#[path = "report_cache_tests.rs"]
mod tests;
