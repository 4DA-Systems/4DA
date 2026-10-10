// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One process-wide knowledge-gap result, shared by every caller.
//!
//! Live 2026-10-07 the pass took 6 s idle and 111 s under load, and seven
//! callers (the Knowledge Gaps command, Blind Spots, stack health, the
//! weekly digest, the terminal API, ...) each recomputed it. The result only
//! changes when its inputs do, so it is keyed on them:
//!
//! - the latest `engine_runs.id` (new items, re-scored rows);
//! - the engagement count (`feedback` + `interactions` rows) — a click or a
//!   verdict changes what is unread, so every such write invalidates;
//! - the dependency count (a lockfile rescan).
//!
//! A refresh runs in the background after each engine cycle is recorded, so
//! the next read is a hit. A cold read computes once — callers that race on
//! it wait for that one computation instead of each starting their own.
//!
//! Across a restart (live 2026-10-10: the first read after launch took
//! 5.7 s, a warm one 19 ms) the previous run's result is persisted
//! (`restart_snapshot`) and comes back two ways:
//!
//! - computed for EXACTLY the current inputs, within [`MAX_AGE`]: it is the
//!   answer a recompute would give, so it seeds the cache for every caller;
//! - computed for other inputs (the usual case — an engine cycle ran since):
//!   only the Knowledge Gaps VIEW ([`knowledge_gaps_for_display`]) is served
//!   it, with its computed-at time, while this run's result computes in the
//!   background. Internal callers (Blind Spots, stack health, the digest)
//!   never see it: what they build from must be current.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{info, warn};

use crate::error::Result;
use crate::restart_snapshot::SnapshotFile;

use super::KnowledgeGap;

/// Belt and braces: even with an unchanged key (a stalled engine), the
/// 30-day windows and advisory mirror move on.
const MAX_AGE: Duration = Duration::from_hours(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct CacheKey {
    engine_run: i64,
    engagement: i64,
    dependencies: i64,
}

impl CacheKey {
    #[cfg(test)]
    pub(super) fn new(engine_run: i64, engagement: i64, dependencies: i64) -> Self {
        Self {
            engine_run,
            engagement,
            dependencies,
        }
    }

    /// Cheap aggregate reads; a missing table counts as zero.
    pub(super) fn read(conn: &rusqlite::Connection) -> Self {
        Self {
            engine_run: scalar(conn, "SELECT MAX(id) FROM engine_runs"),
            engagement: scalar(conn, "SELECT COUNT(*) FROM feedback")
                + scalar(conn, "SELECT COUNT(*) FROM interactions"),
            dependencies: known_dependency_count(conn),
        }
    }
}

fn scalar(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get::<_, Option<i64>>(0))
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// Dependency rows a lockfile scan has stored. Zero means 4DA has read no
/// lockfile yet, so no surface may claim the user's dependencies are clear
/// or current: there is nothing to have checked. A missing table counts as
/// zero.
pub fn known_dependency_count(conn: &rusqlite::Connection) -> i64 {
    scalar(conn, "SELECT COUNT(*) FROM user_dependencies")
        + scalar(conn, "SELECT COUNT(*) FROM project_dependencies")
}

/// The gaps feed with the dependency universe it was computed over, so the
/// panel can tell "no gaps across N dependencies" from "no dependencies
/// known" (fresh-profile E2E 2026-10-09: a user who skipped the project scan
/// was told "No gaps detected — your knowledge is current").
pub(super) fn with_tracked_dependencies(
    mut feed: crate::evidence::EvidenceFeed,
    conn: &rusqlite::Connection,
) -> crate::evidence::EvidenceFeed {
    feed.total_tracked = Some(usize::try_from(known_dependency_count(conn)).unwrap_or(0));
    feed
}

/// Gaps with the moment they were computed (RFC 3339 UTC), so a surface can
/// say how old its picture is.
#[derive(Debug, Clone)]
pub(super) struct Timed {
    pub gaps: Vec<KnowledgeGap>,
    pub computed_at: String,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

struct Entry {
    key: CacheKey,
    /// When this process stored the entry. A restored result's earlier age
    /// rides in `age_at_store`: `Instant` counts from boot on Windows, so
    /// "now minus a 6 h old result" is not representable on a machine up
    /// for less than that, and must not collapse to "just computed".
    stored_at: Instant,
    age_at_store: Duration,
    computed_wall: String,
    gaps: Vec<KnowledgeGap>,
}

impl Entry {
    fn age(&self) -> Duration {
        self.age_at_store.saturating_add(self.stored_at.elapsed())
    }
}

pub(super) struct GapsCache {
    slot: Mutex<Option<Entry>>,
    /// Single flight: held for the duration of one computation.
    compute: Mutex<()>,
    /// The previous run's result for OTHER inputs — the view's answer while
    /// [`Self::revalidating`]. Cleared by the next computation, whatever its
    /// outcome: a failed rebuild must not be papered over with an old result.
    stale: Mutex<Option<Timed>>,
    revalidating: AtomicBool,
}

/// Clears the revalidation flag even if the computation panics.
struct RevalidatingGuard<'a>(&'a AtomicBool);

impl Drop for RevalidatingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl GapsCache {
    pub(super) const fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            compute: Mutex::new(()),
            stale: Mutex::new(None),
            revalidating: AtomicBool::new(false),
        }
    }

    /// Fill an EMPTY slot with a result computed `age` ago for `key` (the
    /// previous run's, from disk). It then expires on the usual schedule.
    pub(super) fn seed(
        &self,
        key: CacheKey,
        gaps: Vec<KnowledgeGap>,
        age: Duration,
        computed_wall: String,
    ) -> bool {
        let mut slot = self.slot.lock();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Entry {
            key,
            stored_at: Instant::now(),
            age_at_store: age,
            computed_wall,
            gaps,
        });
        true
    }

    /// Hold a previous run's result for other inputs, for the view to serve
    /// while this run's result computes. Never displaces a value of this run.
    pub(super) fn keep_stale(&self, stale: Timed) -> bool {
        if self.slot.lock().is_some() {
            return false;
        }
        let mut held = self.stale.lock();
        if held.is_some() {
            return false;
        }
        *held = Some(stale);
        true
    }

    fn fresh_timed(&self, key: CacheKey) -> Option<Timed> {
        let slot = self.slot.lock();
        slot.as_ref()
            .filter(|e| e.key == key && e.age() < MAX_AGE)
            .map(|e| Timed {
                gaps: e.gaps.clone(),
                computed_at: e.computed_wall.clone(),
            })
    }

    fn fresh(&self, key: CacheKey) -> Option<Vec<KnowledgeGap>> {
        self.fresh_timed(key).map(|t| t.gaps)
    }

    /// The cached gaps for `key`, or `compute()`'s result stored under it.
    pub(super) fn get_or_compute(
        &self,
        key: CacheKey,
        compute: impl FnOnce() -> Result<Vec<KnowledgeGap>>,
    ) -> Result<Vec<KnowledgeGap>> {
        self.get_or_compute_timed(key, compute).map(|t| t.gaps)
    }

    fn get_or_compute_timed(
        &self,
        key: CacheKey,
        compute: impl FnOnce() -> Result<Vec<KnowledgeGap>>,
    ) -> Result<Timed> {
        if let Some(hit) = self.fresh_timed(key) {
            return Ok(hit);
        }
        let _flight = self.compute.lock();
        // Another caller may have finished the same computation while this
        // one waited for the flight lock.
        if let Some(hit) = self.fresh_timed(key) {
            return Ok(hit);
        }
        let result = compute();
        // Whatever the outcome, the previous run's result has had its turn.
        *self.stale.lock() = None;
        let gaps = result?;
        let computed_wall = now_rfc3339();
        *self.slot.lock() = Some(Entry {
            key,
            stored_at: Instant::now(),
            age_at_store: Duration::ZERO,
            computed_wall: computed_wall.clone(),
            gaps: gaps.clone(),
        });
        Ok(Timed {
            gaps,
            computed_at: computed_wall,
        })
    }

    /// The held previous-run result, served only while its replacement is
    /// computing: the first call starts that computation in the background
    /// (`revalidate`). `None` when nothing is held or no computation could
    /// start — the caller then computes itself.
    pub(super) fn stale_while_revalidating(&'static self, revalidate: fn()) -> Option<Timed> {
        let stale = self.stale.lock().clone()?;
        if self.revalidating.swap(true, Ordering::SeqCst) {
            return Some(stale);
        }
        let spawned = std::thread::Builder::new()
            .name("knowledge-gaps-revalidate".to_string())
            .spawn(move || {
                let _guard = RevalidatingGuard(&self.revalidating);
                revalidate();
                // A revalidation that never reached a computation (no
                // database) must not leave the old result serving.
                *self.stale.lock() = None;
            });
        if let Err(e) = spawned {
            self.revalidating.store(false, Ordering::SeqCst);
            warn!(target: "4da::knowledge_decay", error = %e, "Could not spawn knowledge-gap revalidation");
            return None;
        }
        Some(stale)
    }
}

static CACHE: GapsCache = GapsCache::new();

/// The persisted last result. Restored for exactly-matching inputs within
/// [`MAX_AGE`]; otherwise served to the view while it recomputes, up to a
/// week old (the Blind Spots horizon).
static SNAPSHOT: SnapshotFile =
    SnapshotFile::new("knowledge_gaps_snapshot.json", Duration::from_hours(24 * 7));

/// A result on disk with the inputs it was computed for.
#[derive(serde::Serialize, serde::Deserialize)]
struct Persisted {
    key: CacheKey,
    gaps: Vec<KnowledgeGap>,
}

/// Knowledge gaps for every caller. Blocking on a cold cache — call it off
/// the UI thread (the Tauri command runs it in `spawn_blocking`). Always
/// current: a previous run's result is reused only for identical inputs.
pub fn cached_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>> {
    let key = CacheKey::read(conn);
    if let Some(hit) = CACHE.fresh(key) {
        return Ok(hit);
    }
    restore_once(conn, key);
    CACHE.get_or_compute(key, || compute_and_persist(conn, key))
}

/// The Knowledge Gaps view's read: [`cached_knowledge_gaps`], except that
/// after a restart the previous run's result is served at once (with when it
/// was computed) while this run's result computes in the background.
pub(super) fn knowledge_gaps_for_display(conn: &rusqlite::Connection) -> Result<Timed> {
    let key = CacheKey::read(conn);
    if let Some(hit) = CACHE.fresh_timed(key) {
        return Ok(hit);
    }
    restore_once(conn, key);
    if let Some(hit) = CACHE.fresh_timed(key) {
        return Ok(hit);
    }
    if let Some(stale) = CACHE.stale_while_revalidating(revalidate) {
        info!(
            target: "4da::knowledge_decay",
            computed_at = %stale.computed_at,
            "Serving the previous run's knowledge gaps while they recompute"
        );
        return Ok(stale);
    }
    CACHE.get_or_compute_timed(key, || compute_and_persist(conn, key))
}

fn compute_and_persist(conn: &rusqlite::Connection, key: CacheKey) -> Result<Vec<KnowledgeGap>> {
    let gaps = super::detect_knowledge_gaps(conn)?;
    persist(conn, key, &gaps);
    Ok(gaps)
}

/// The background half of [`knowledge_gaps_for_display`].
fn revalidate() {
    let started = Instant::now();
    match crate::open_db_connection().and_then(|conn| cached_knowledge_gaps(&conn)) {
        Ok(gaps) => info!(
            target: "4da::knowledge_decay",
            gaps = gaps.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "Knowledge gaps revalidated after restart"
        ),
        Err(e) => {
            warn!(target: "4da::knowledge_decay", error = %e, "Knowledge-gap revalidation failed")
        }
    }
}

/// The previous run's result, read at most once per process: seeded for
/// every caller when its inputs match, held for the view otherwise.
fn restore_once(conn: &rusqlite::Connection, key: CacheKey) {
    if let Some(restored) = SNAPSHOT.restore_once::<Persisted>(conn) {
        place_restored(&CACHE, restored, key);
    }
}

/// Where a restored result goes: the cache proper for identical inputs
/// still inside [`MAX_AGE`], the view-only stale slot for anything else.
fn place_restored(
    cache: &GapsCache,
    restored: crate::restart_snapshot::Restored<Persisted>,
    key: CacheKey,
) {
    let computed_at = restored.computed_at();
    if restored.value.key == key && restored.age < MAX_AGE {
        if cache.seed(key, restored.value.gaps, restored.age, computed_at) {
            info!(
                target: "4da::knowledge_decay",
                age_secs = restored.age.as_secs(),
                "Knowledge gaps restored from the previous run (same inputs)"
            );
        }
        return;
    }
    cache.keep_stale(Timed {
        gaps: restored.value.gaps,
        computed_at,
    });
}

fn persist(conn: &rusqlite::Connection, key: CacheKey, gaps: &[KnowledgeGap]) {
    // Same JSON shape as `Persisted`, without copying the gaps.
    #[derive(serde::Serialize)]
    struct PersistedRef<'a> {
        key: CacheKey,
        gaps: &'a [KnowledgeGap],
    }
    SNAPSHOT.persist(conn, &PersistedRef { key, gaps });
}

/// Recompute after an engine cycle is recorded, on a background thread, so
/// the next reader hits a warm cache. Never blocks the caller.
pub fn refresh_knowledge_gaps_in_background() {
    let spawned = std::thread::Builder::new()
        .name("knowledge-gaps-refresh".to_string())
        .spawn(|| {
            let started = Instant::now();
            let result = crate::open_db_connection().and_then(|conn| cached_knowledge_gaps(&conn));
            match result {
                Ok(gaps) => info!(
                    target: "4da::knowledge_decay",
                    gaps = gaps.len(),
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Knowledge-gap cache refreshed"
                ),
                Err(e) => warn!(target: "4da::knowledge_decay", error = %e, "Knowledge-gap refresh failed"),
            }
        });
    if let Err(e) = spawned {
        warn!(target: "4da::knowledge_decay", error = %e, "Could not spawn knowledge-gap refresh");
    }
}

#[cfg(test)]
#[path = "knowledge_gaps_cache_tests.rs"]
mod tests;
