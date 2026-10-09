// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

fn v(x: f32) -> Vec<f32> {
    vec![x; 4]
}

#[test]
fn cache_returns_hits_and_evicts_the_least_recent() {
    let mut cache = QueryEmbedCache::default();
    for i in 0..QUERY_EMBED_CACHE_MAX {
        cache.insert(format!("q{i}"), v(i as f32));
    }
    // Touch the oldest so it is no longer the eviction candidate.
    assert_eq!(cache.get("q0"), Some(v(0.0)));
    cache.insert("new".into(), v(9.0));
    assert_eq!(cache.len(), QUERY_EMBED_CACHE_MAX);
    assert_eq!(cache.get("q0"), Some(v(0.0)), "recently used survives");
    assert_eq!(cache.get("q1"), None, "least recently used was evicted");
    assert_eq!(cache.get("new"), Some(v(9.0)));
}

#[test]
fn reinserting_a_key_replaces_it_without_growing() {
    let mut cache = QueryEmbedCache::default();
    cache.insert("a".into(), v(1.0));
    cache.insert("a".into(), v(2.0));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get("a"), Some(v(2.0)));
}

#[test]
fn cache_keys_separate_models_and_embedding_spaces() {
    let base = cache_key("nomic-embed-text", 3, "rusqlite");
    assert_ne!(base, cache_key("other-model", 3, "rusqlite"));
    assert_ne!(
        base,
        cache_key("nomic-embed-text", 4, "rusqlite"),
        "a re-embed must miss"
    );
    assert_eq!(base, cache_key("nomic-embed-text", 3, "rusqlite"));
}

#[test]
fn warm_is_claimed_once_and_throttled() {
    let flag = AtomicBool::new(false);
    let now = Instant::now();
    assert!(try_begin_warm(now, None, &flag), "first warm runs");
    assert!(
        !try_begin_warm(now, None, &flag),
        "a second warm while one runs is skipped"
    );
    flag.store(false, Ordering::Release);
    assert!(
        !try_begin_warm(now, Some(now), &flag),
        "a warm that just finished suppresses another"
    );
    let long_ago = now.checked_sub(WARM_MIN_INTERVAL + Duration::from_secs(1));
    if let Some(earlier) = long_ago {
        assert!(
            try_begin_warm(now, Some(earlier), &flag),
            "an old warm does not"
        );
    }
}

#[tokio::test]
async fn a_duplicate_request_waits_for_the_running_embed() {
    let key = cache_key("test-model", u64::MAX, "dup-wait");
    IN_FLIGHT.lock().insert(key.clone());
    let filler_key = key.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        QUERY_EMBED_CACHE.lock().insert(filler_key.clone(), v(0.5));
        IN_FLIGHT.lock().remove(&filler_key);
    });
    let got = await_in_flight(&key, Duration::from_secs(5)).await;
    assert_eq!(got.source, EmbedSource::Cached);
    assert_eq!(got.vector, Some(v(0.5)));
}

#[tokio::test]
async fn a_duplicate_request_gives_up_at_the_budget() {
    let key = cache_key("test-model", u64::MAX, "dup-timeout");
    IN_FLIGHT.lock().insert(key.clone());
    let began = Instant::now();
    let got = await_in_flight(&key, Duration::from_millis(80)).await;
    IN_FLIGHT.lock().remove(&key);
    assert_eq!(got.source, EmbedSource::TimedOut);
    assert!(got.vector.is_none());
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "bounded by the budget"
    );
}

#[test]
fn the_in_flight_guard_clears_its_key_on_drop() {
    let key = cache_key("test-model", u64::MAX, "guard");
    IN_FLIGHT.lock().insert(key.clone());
    drop(InFlightGuard(key.clone()));
    assert!(!IN_FLIGHT.lock().contains(&key));
}

/// The search budget must stay below the "under ~2 s" target with room for the
/// keyword legs, which measured 0.3-0.7 s warm on the 2 GB corpus.
#[test]
fn the_embed_budget_leaves_room_for_the_keyword_legs() {
    assert!(SEARCH_EMBED_BUDGET <= Duration::from_millis(1_300));
    assert!(
        SEARCH_EMBED_BUDGET >= Duration::from_millis(500),
        "a warm embed fits"
    );
}

/// The pre-warm runs once after startup first-light, so a first search does not
/// pay the embedder load. Guarded at the source level: the startup hook is a
/// line in a 2,700-line setup function that no unit test executes.
#[test]
fn startup_schedules_a_search_pre_warm_after_first_light() {
    let setup = include_str!("app_setup.rs");
    assert!(
        setup.contains("natural_language_search::prewarm_search(\"startup\")"),
        "app_setup must pre-warm search after first-light"
    );
}

#[path = "natural_language_search_live.rs"]
mod live;
