// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::restart_snapshot::Restored;

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

const YESTERDAY: &str = "2026-10-09T01:56:52+00:00";

fn stale_gaps() -> Timed {
    Timed {
        gaps: one_gap(),
        computed_at: YESTERDAY.to_string(),
    }
}

fn leaked() -> &'static GapsCache {
    Box::leak(Box::new(GapsCache::new()))
}

fn wait_for_revalidation(cache: &GapsCache) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while cache.revalidating.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "revalidation never finished");
        std::thread::sleep(Duration::from_millis(5));
    }
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
    assert!(cache.seed(
        key,
        one_gap(),
        Duration::from_mins(10),
        YESTERDAY.to_string()
    ));
    let runs = AtomicUsize::new(0);
    let got = cache
        .get_or_compute(key, || {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        })
        .unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 0, "served without the pass");
    assert_eq!(got[0].dependency, "chrono");
    assert_eq!(
        cache.fresh_timed(key).map(|t| t.computed_at).as_deref(),
        Some(YESTERDAY),
        "a seeded result keeps the time it was computed"
    );
    assert!(
        cache.fresh(CacheKey::new(8, 1, 3)).is_none(),
        "other inputs recompute"
    );
    assert!(
        !cache.seed(key, Vec::new(), Duration::ZERO, String::new()),
        "never displaces"
    );

    let stale = GapsCache::new();
    assert!(stale.seed(
        key,
        one_gap(),
        MAX_AGE + Duration::from_secs(1),
        String::new()
    ));
    assert!(stale.fresh(key).is_none(), "past the expiry it recomputes");
}

/// A restored result older than the machine's uptime is still old. On
/// Windows `Instant` counts from boot, so `now - age` underflows; that once
/// fell back to "now" and served an expired result as fresh after a reboot
/// (and failed the test above on fresh CI runners).
#[test]
fn a_restored_result_older_than_uptime_is_expired() {
    let key = CacheKey::new(7, 1, 3);
    let cache = GapsCache::new();
    let ten_years = Duration::from_secs(10 * 365 * 24 * 60 * 60);
    assert!(cache.seed(key, one_gap(), ten_years, String::new()));
    assert!(
        cache.fresh(key).is_none(),
        "an age the clock cannot represent is expired"
    );
}

/// The persisted shape round-trips through the snapshot store.
#[test]
fn the_persisted_result_round_trips() {
    use crate::restart_snapshot as snapshot;
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
    let empty = with_tracked_dependencies(crate::evidence::EvidenceFeed::from_items(vec![]), &conn);
    assert_eq!(empty.total_tracked, Some(0), "no tables: nothing known");

    conn.execute_batch(
        "CREATE TABLE user_dependencies (package_name TEXT);
         CREATE TABLE project_dependencies (package_name TEXT);
         INSERT INTO user_dependencies VALUES ('serde');
         INSERT INTO project_dependencies VALUES ('react'), ('vite');",
    )
    .unwrap();
    assert_eq!(known_dependency_count(&conn), 3);
    let feed = with_tracked_dependencies(crate::evidence::EvidenceFeed::from_items(vec![]), &conn);
    assert_eq!(feed.total_tracked, Some(3));
}

// ─── Restart: serve the previous run's result while revalidating ─────────

fn restored(key: CacheKey, age: Duration) -> Restored<Persisted> {
    Restored {
        value: Persisted {
            key,
            gaps: one_gap(),
        },
        age,
        saved_at: chrono::Utc::now().timestamp() - age.as_secs() as i64,
    }
}

/// Identical inputs keep #897's fast path: the restored result IS the
/// answer, for every caller. Any other inputs — or identical inputs past
/// the cache's own expiry — only reach the view's stale slot.
#[test]
fn a_restored_result_seeds_only_for_identical_inputs() {
    let key = CacheKey::new(4261, 18, 5605);

    let exact = GapsCache::new();
    place_restored(&exact, restored(key, Duration::from_mins(20)), key);
    assert!(exact.fresh(key).is_some(), "same inputs: a cache hit");
    assert!(exact.stale.lock().is_none());

    let moved_on = GapsCache::new();
    place_restored(
        &moved_on,
        restored(CacheKey::new(4260, 18, 5605), Duration::from_mins(20)),
        key,
    );
    assert!(moved_on.fresh(key).is_none(), "an engine cycle ran since");
    assert!(moved_on.stale.lock().is_some(), "held for the view");

    let too_old = GapsCache::new();
    place_restored(
        &too_old,
        restored(key, MAX_AGE + Duration::from_mins(1)),
        key,
    );
    assert!(too_old.fresh(key).is_none());
    assert!(too_old.stale.lock().is_some());
}

/// Internal callers (Blind Spots, stack health, the digest) build from the
/// gaps they read, so they must never be handed the previous run's: they
/// compute, and that computation retires the stale result.
#[test]
fn internal_callers_never_see_the_stale_result() {
    let cache = GapsCache::new();
    assert!(cache.keep_stale(stale_gaps()));
    let key = CacheKey::new(9, 9, 9);
    let got = cache.get_or_compute(key, || Ok(Vec::new())).unwrap();
    assert!(got.is_empty(), "computed for the current inputs");
    assert!(cache.stale.lock().is_none(), "retired by the computation");
}

/// A failed recompute must not leave the old result serving forever.
#[test]
fn a_failed_recompute_retires_the_stale_result() {
    let cache = GapsCache::new();
    assert!(cache.keep_stale(stale_gaps()));
    let failed = cache.get_or_compute(CacheKey::new(1, 1, 1), || {
        Err(crate::error::FourDaError::Internal("boom".into()))
    });
    assert!(failed.is_err());
    assert!(cache.stale.lock().is_none());
}

/// A stale result never displaces this run's own, and only one is held.
#[test]
fn a_stale_result_never_displaces_a_current_one() {
    let cache = GapsCache::new();
    cache
        .get_or_compute(CacheKey::new(1, 0, 0), || Ok(one_gap()))
        .unwrap();
    assert!(!cache.keep_stale(stale_gaps()));

    let empty = GapsCache::new();
    assert!(empty.keep_stale(stale_gaps()));
    assert!(!empty.keep_stale(stale_gaps()), "the first restore wins");
}

static REVALIDATIONS: AtomicUsize = AtomicUsize::new(0);

fn slow_revalidation() {
    REVALIDATIONS.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(150));
}

/// The view's read after a restart: the previous result at once, with its
/// computed-at time, on every read while ONE background revalidation runs;
/// once it finishes, the stale result is gone.
#[test]
fn the_view_is_served_the_stale_result_while_one_revalidation_runs() {
    let cache = leaked();
    assert!(cache.keep_stale(stale_gaps()));
    let first = cache
        .stale_while_revalidating(slow_revalidation)
        .expect("served at once");
    let second = cache
        .stale_while_revalidating(slow_revalidation)
        .expect("still served while revalidating");
    assert_eq!(first.computed_at, YESTERDAY, "carries when it was computed");
    assert_eq!(second.gaps[0].dependency, "chrono");
    wait_for_revalidation(cache);
    assert_eq!(
        REVALIDATIONS.load(Ordering::SeqCst),
        1,
        "single flight: one revalidation for both reads"
    );
    assert!(
        cache.stale_while_revalidating(slow_revalidation).is_none(),
        "after the revalidation the view reads the current result"
    );
}

/// The first Knowledge Gaps read after a restart on a SNAPSHOT of a real
/// database (never the live file), when an engine cycle ran since the last
/// result was persisted — the usual case, which #897's exact-input reuse
/// could not serve:
///
/// ```text
/// FOURDA_DB_PATH=<snapshot>/4da.db cargo test --lib \
///     live_knowledge_gaps_restart -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_knowledge_gaps_restart_serves_the_previous_result() {
    if std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    let conn = crate::open_db_connection().expect("open snapshot");
    let key = CacheKey::read(&conn);
    let _ = std::fs::remove_file(SNAPSHOT.path());

    let _ = super::super::detect_knowledge_gaps(&conn); // page-cache warm-up
    let t = Instant::now();
    let gaps = super::super::detect_knowledge_gaps(&conn).expect("detect");
    let cold_ms = t.elapsed().as_millis();

    // What the previous run persisted, one engine cycle ago.
    let previous = CacheKey {
        engine_run: key.engine_run - 1,
        ..key
    };
    persist(&conn, previous, &gaps);

    let t = Instant::now();
    let served = knowledge_gaps_for_display(&conn).expect("served");
    let restart_ms = t.elapsed().as_millis();
    assert_eq!(
        serde_json::to_value(&served.gaps).unwrap(),
        serde_json::to_value(&gaps).unwrap(),
        "the previous run's result, served while it recomputes"
    );
    wait_for_revalidation(&CACHE);
    let t = Instant::now();
    let fresh = knowledge_gaps_for_display(&conn).expect("fresh");
    let warm_ms = t.elapsed().as_millis();
    assert_eq!(
        serde_json::to_value(&fresh.gaps).unwrap(),
        serde_json::to_value(&gaps).unwrap(),
        "the revalidated result is the one a cold read computes"
    );
    println!(
        "knowledge gaps: BEFORE cold detect {cold_ms} ms; AFTER restart read {restart_ms} ms \
         (computed_at {}); revalidated read {warm_ms} ms; {} gaps",
        served.computed_at,
        gaps.len()
    );
    let _ = std::fs::remove_file(SNAPSHOT.path());
}

#[test]
fn nothing_held_means_the_view_computes() {
    let cache = leaked();
    assert!(cache.stale_while_revalidating(slow_revalidation).is_none());
    assert!(!cache.revalidating.load(Ordering::SeqCst));
}
