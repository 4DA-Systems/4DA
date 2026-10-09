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

use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{info, warn};

use crate::error::Result;

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

struct Entry {
    key: CacheKey,
    computed_at: Instant,
    gaps: Vec<KnowledgeGap>,
}

pub(super) struct GapsCache {
    slot: Mutex<Option<Entry>>,
    /// Single flight: held for the duration of one computation.
    compute: Mutex<()>,
}

impl GapsCache {
    pub(super) const fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            compute: Mutex::new(()),
        }
    }

    /// Fill an EMPTY slot with a result computed `age` ago for `key` (the
    /// previous run's, from disk). It then expires on the usual schedule.
    pub(super) fn seed(&self, key: CacheKey, gaps: Vec<KnowledgeGap>, age: Duration) -> bool {
        let mut slot = self.slot.lock();
        if slot.is_some() {
            return false;
        }
        let now = Instant::now();
        *slot = Some(Entry {
            key,
            computed_at: now.checked_sub(age).unwrap_or(now),
            gaps,
        });
        true
    }

    fn fresh(&self, key: CacheKey) -> Option<Vec<KnowledgeGap>> {
        let slot = self.slot.lock();
        slot.as_ref()
            .filter(|e| e.key == key && e.computed_at.elapsed() < MAX_AGE)
            .map(|e| e.gaps.clone())
    }

    /// The cached gaps for `key`, or `compute()`'s result stored under it.
    pub(super) fn get_or_compute(
        &self,
        key: CacheKey,
        compute: impl FnOnce() -> Result<Vec<KnowledgeGap>>,
    ) -> Result<Vec<KnowledgeGap>> {
        if let Some(hit) = self.fresh(key) {
            return Ok(hit);
        }
        let _flight = self.compute.lock();
        // Another caller may have finished the same computation while this
        // one waited for the flight lock.
        if let Some(hit) = self.fresh(key) {
            return Ok(hit);
        }
        let gaps = compute()?;
        *self.slot.lock() = Some(Entry {
            key,
            computed_at: Instant::now(),
            gaps: gaps.clone(),
        });
        Ok(gaps)
    }
}

static CACHE: GapsCache = GapsCache::new();

/// The persisted last result — see `blind_spots::report_snapshot`.
const SNAPSHOT_FILE: &str = "knowledge_gaps_snapshot.json";

/// A result on disk with the inputs it was computed for.
#[derive(serde::Serialize, serde::Deserialize)]
struct Persisted {
    key: CacheKey,
    gaps: Vec<KnowledgeGap>,
}

/// Knowledge gaps for every caller. Blocking on a cold cache — call it off
/// the UI thread (the Tauri command runs it in `spawn_blocking`).
///
/// After a restart, the previous run's result is reused when it was computed
/// for EXACTLY the current inputs (same engine run, engagement and dependency
/// counts, within [`MAX_AGE`]) — the answer a recompute would give, without
/// the pass. Any other input recomputes, as before.
pub fn cached_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>> {
    let key = CacheKey::read(conn);
    if let Some(hit) = CACHE.fresh(key) {
        return Ok(hit);
    }
    if let Some((gaps, age)) = restore_once(conn, key) {
        if CACHE.seed(key, gaps.clone(), age) {
            return Ok(gaps);
        }
    }
    CACHE.get_or_compute(key, || {
        let gaps = super::detect_knowledge_gaps(conn)?;
        persist(conn, key, &gaps);
        Ok(gaps)
    })
}

/// The previous run's result for `key`, read at most once per process.
fn restore_once(
    conn: &rusqlite::Connection,
    key: CacheKey,
) -> Option<(Vec<KnowledgeGap>, Duration)> {
    static ATTEMPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ATTEMPTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    use crate::blind_spots::report_snapshot as snapshot;
    let stamp = snapshot::Stamp::current(conn)?;
    let restored: snapshot::Restored<Persisted> =
        snapshot::load(&snapshot::snapshot_path(SNAPSHOT_FILE), &stamp, MAX_AGE)?;
    (restored.value.key == key).then(|| {
        info!(
            target: "4da::knowledge_decay",
            age_secs = restored.age.as_secs(),
            "Knowledge gaps restored from the previous run (same inputs)"
        );
        (restored.value.gaps, restored.age)
    })
}

fn persist(conn: &rusqlite::Connection, key: CacheKey, gaps: &[KnowledgeGap]) {
    use crate::blind_spots::report_snapshot as snapshot;
    let Some(stamp) = snapshot::Stamp::current(conn) else {
        return;
    };
    // Same JSON shape as `Persisted`, without copying the gaps.
    #[derive(serde::Serialize)]
    struct PersistedRef<'a> {
        key: CacheKey,
        gaps: &'a [KnowledgeGap],
    }
    let value = PersistedRef { key, gaps };
    if let Err(e) = snapshot::save(&snapshot::snapshot_path(SNAPSHOT_FILE), &stamp, &value) {
        warn!(target: "4da::knowledge_decay", error = %e, "could not persist knowledge gaps");
    }
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn one_gap() -> Vec<KnowledgeGap> {
        vec![KnowledgeGap {
            dependency: "chrono".to_string(),
            version: Some("0.4.44".to_string()),
            project_path: "d:/runyourempire/victauri".to_string(),
            projects: vec!["d:/runyourempire/victauri".to_string()],
            basis: super::super::GapBasis::Release,
            latest_release: Some("0.4.45".to_string()),
            missed_items: Vec::new(),
            gap_severity: super::super::GapSeverity::Low,
            days_since_last_engagement: 999,
        }]
    }

    #[test]
    fn a_cache_hit_returns_without_recomputing() {
        let cache = GapsCache::new();
        let runs = AtomicUsize::new(0);
        let compute = || {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(one_gap())
        };
        let key = CacheKey::new(4261, 18, 5605);
        let first = cache.get_or_compute(key, compute).unwrap();
        let second = cache.get_or_compute(key, compute).unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 1, "the second read is a hit");
        assert_eq!(first.len(), second.len());
        assert_eq!(second[0].dependency, "chrono");
    }

    #[test]
    fn every_input_change_recomputes() {
        let cache = GapsCache::new();
        let runs = AtomicUsize::new(0);
        let compute = || {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(one_gap())
        };
        cache
            .get_or_compute(CacheKey::new(1, 0, 10), compute)
            .unwrap();
        cache
            .get_or_compute(CacheKey::new(2, 0, 10), compute)
            .unwrap(); // engine cycle
        cache
            .get_or_compute(CacheKey::new(2, 1, 10), compute)
            .unwrap(); // a click
        cache
            .get_or_compute(CacheKey::new(2, 1, 11), compute)
            .unwrap(); // a rescan
        assert_eq!(runs.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn a_failed_computation_is_not_cached() {
        let cache = GapsCache::new();
        let key = CacheKey::new(1, 0, 0);
        let failed = cache.get_or_compute(key, || {
            Err(crate::error::FourDaError::Internal("boom".into()))
        });
        assert!(failed.is_err());
        let runs = AtomicUsize::new(0);
        cache
            .get_or_compute(key, || {
                runs.fetch_add(1, Ordering::SeqCst);
                Ok(one_gap())
            })
            .unwrap();
        assert_eq!(
            runs.load(Ordering::SeqCst),
            1,
            "the failure left the slot empty"
        );
    }

    /// The app-wide freeze of 2026-10-07: a NON-async Tauri command runs on
    /// the UI thread. Fails if `get_knowledge_gaps` ever loses `async`.
    #[test]
    fn get_knowledge_gaps_stays_an_async_command() {
        let src = include_str!("knowledge_decay.rs");
        let compact: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            compact.contains("#[tauri::command] pub async fn get_knowledge_gaps("),
            "get_knowledge_gaps must be an async Tauri command"
        );
        assert!(!compact.contains("pub fn get_knowledge_gaps("));
        assert!(
            compact.contains("spawn_blocking(knowledge_gaps_feed)"),
            "its body must run on the blocking pool"
        );
    }

    /// A restored result answers its own key until the usual expiry, and
    /// never displaces a result this run computed.
    #[test]
    fn a_seeded_result_serves_its_key_and_ages_like_any_other() {
        let cache = GapsCache::new();
        let key = CacheKey::new(7, 1, 3);
        assert!(cache.seed(key, one_gap(), Duration::from_mins(10)));
        let runs = AtomicUsize::new(0);
        let got = cache
            .get_or_compute(key, || {
                runs.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            })
            .unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 0, "served without the pass");
        assert_eq!(got[0].dependency, "chrono");
        assert!(
            cache.fresh(CacheKey::new(8, 1, 3)).is_none(),
            "other inputs recompute"
        );
        assert!(
            !cache.seed(key, Vec::new(), Duration::ZERO),
            "never displaces"
        );

        let stale = GapsCache::new();
        assert!(stale.seed(key, one_gap(), MAX_AGE + Duration::from_secs(1)));
        assert!(stale.fresh(key).is_none(), "past the expiry it recomputes");
    }

    /// The persisted shape round-trips through the snapshot store.
    #[test]
    fn the_persisted_result_round_trips() {
        use crate::blind_spots::report_snapshot as snapshot;
        let dir = std::env::temp_dir().join(format!("4da-gaps-snapshot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gaps.json");
        let stamp = snapshot::Stamp::with_schema(124);
        let value = Persisted {
            key: CacheKey::new(4261, 18, 5605),
            gaps: one_gap(),
        };
        snapshot::save(&path, &stamp, &value).unwrap();
        let back: snapshot::Restored<Persisted> = snapshot::load(&path, &stamp, MAX_AGE).unwrap();
        assert_eq!(back.value.key, value.key);
        assert_eq!(back.value.gaps.len(), 1);
        assert_eq!(back.value.gaps[0].latest_release.as_deref(), Some("0.4.45"));
    }

    #[test]
    fn the_key_reads_zero_from_an_empty_database() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert_eq!(CacheKey::read(&conn), CacheKey::new(0, 0, 0));
    }

    /// The feed carries the dependency universe, so the panel can tell a
    /// clean result from "no lockfile read yet".
    #[test]
    fn the_feed_reports_how_many_dependencies_it_covered() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let empty =
            with_tracked_dependencies(crate::evidence::EvidenceFeed::from_items(vec![]), &conn);
        assert_eq!(empty.total_tracked, Some(0), "no tables: nothing known");

        conn.execute_batch(
            "CREATE TABLE user_dependencies (package_name TEXT);
             CREATE TABLE project_dependencies (package_name TEXT);
             INSERT INTO user_dependencies VALUES ('serde');
             INSERT INTO project_dependencies VALUES ('react'), ('vite');",
        )
        .unwrap();
        assert_eq!(known_dependency_count(&conn), 3);
        let feed =
            with_tracked_dependencies(crate::evidence::EvidenceFeed::from_items(vec![]), &conn);
        assert_eq!(feed.total_tracked, Some(3));
    }
}
