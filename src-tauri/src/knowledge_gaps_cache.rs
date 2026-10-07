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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        let scalar = |sql: &str| -> i64 {
            conn.query_row(sql, [], |r| r.get::<_, Option<i64>>(0))
                .ok()
                .flatten()
                .unwrap_or(0)
        };
        Self {
            engine_run: scalar("SELECT MAX(id) FROM engine_runs"),
            engagement: scalar("SELECT COUNT(*) FROM feedback")
                + scalar("SELECT COUNT(*) FROM interactions"),
            dependencies: scalar("SELECT COUNT(*) FROM user_dependencies")
                + scalar("SELECT COUNT(*) FROM project_dependencies"),
        }
    }
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

/// Knowledge gaps for every caller. Blocking on a cold cache — call it off
/// the UI thread (the Tauri command runs it in `spawn_blocking`).
pub fn cached_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>> {
    CACHE.get_or_compute(CacheKey::read(conn), || super::detect_knowledge_gaps(conn))
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

    #[test]
    fn the_key_reads_zero_from_an_empty_database() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert_eq!(CacheKey::read(&conn), CacheKey::new(0, 0, 0));
    }
}
