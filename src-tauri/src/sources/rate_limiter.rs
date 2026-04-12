//! Centralized rate limiter for source fetching.
//!
//! Tracks the last request timestamp per source/domain and enforces
//! minimum intervals between requests to the same source.
//! Thread-safe via `parking_lot::Mutex`.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::debug;

/// Default minimum interval between requests to the same source (2 seconds).
const DEFAULT_MIN_INTERVAL: Duration = Duration::from_secs(2);

/// Per-source rate limit overrides (seconds).
/// Sources with stricter or more lenient rate limits can be configured here.
fn source_interval(source: &str) -> Duration {
    match source {
        // Reddit is aggressive about rate limiting unauthenticated requests
        "reddit" => Duration::from_secs(3),
        // GitHub API has generous limits but best to be respectful
        "github" => Duration::from_secs(2),
        // HN Firebase API is very permissive
        "hackernews" => Duration::from_secs(1),
        // arXiv explicitly asks for politeness
        "arxiv" => Duration::from_secs(3),
        // Twitter/X API varies by tier
        "twitter" => Duration::from_secs(2),
        // Security feeds
        "cve" => Duration::from_secs(3),
        "osv" => Duration::from_secs(2),
        // Package registries (CDN-fronted, permissive)
        "npm_registry" => Duration::from_millis(500),
        "pypi" => Duration::from_millis(500),
        "crates_io" => Duration::from_secs(1),
        "go_modules" => Duration::from_secs(1),
        // Community/research
        "huggingface" => Duration::from_secs(2),
        "stackoverflow" => Duration::from_secs(2),
        "bluesky" => Duration::from_secs(2),
        "papers_with_code" => Duration::from_secs(2),
        // Default for everything else
        _ => DEFAULT_MIN_INTERVAL,
    }
}

/// Maximum concurrent HTTP requests across all sources.
/// Prevents overwhelming the network or hitting OS file-descriptor limits
/// when 100+ sources are fetching simultaneously.
const MAX_CONCURRENT_REQUESTS: usize = 15;

/// Centralized rate limiter that tracks per-source request timing
/// and enforces a global concurrency cap via semaphore.
pub struct RateLimiter {
    last_requests: Mutex<HashMap<String, Instant>>,
    global_permits: Arc<Semaphore>,
}

impl RateLimiter {
    /// Create a new rate limiter with no history and a global concurrency cap.
    pub fn new() -> Self {
        Self {
            last_requests: Mutex::new(HashMap::new()),
            global_permits: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        }
    }

    /// Acquire a global request permit. Returns a guard that releases on drop.
    /// This limits total concurrent HTTP requests across all sources to
    /// [`MAX_CONCURRENT_REQUESTS`] (15), preventing network saturation when
    /// many sources fetch in parallel.
    pub async fn acquire_global_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.global_permits
            .clone()
            .acquire_owned()
            .await
            .expect("global request semaphore closed")
    }

    /// Check whether a request to the given source is currently allowed
    /// without waiting. Returns `true` if the minimum interval has elapsed
    /// (or if there is no prior request recorded).
    pub fn check_rate_limit(&self, source: &str) -> bool {
        let map = self.last_requests.lock();
        match map.get(source) {
            None => true,
            Some(last) => last.elapsed() >= source_interval(source),
        }
    }

    /// Record that a request was just made to the given source.
    pub fn record_request(&self, source: &str) {
        let mut map = self.last_requests.lock();
        map.insert(source.to_string(), Instant::now());
    }

    /// Wait until the rate limit for the given source has elapsed, then
    /// record the request. This is the primary entry point for callers —
    /// it combines check + sleep + record in one call.
    ///
    /// If no prior request is recorded, returns immediately.
    pub async fn wait_for_rate_limit(&self, source: &str) {
        let sleep_duration = {
            let map = self.last_requests.lock();
            match map.get(source) {
                None => None,
                Some(last) => {
                    let elapsed = last.elapsed();
                    let interval = source_interval(source);
                    if elapsed < interval {
                        Some(interval.saturating_sub(elapsed))
                    } else {
                        None
                    }
                }
            }
        }; // MutexGuard dropped here — safe to await below

        if let Some(delay) = sleep_duration {
            debug!(
                target: "4da::rate_limit",
                source = source,
                delay_ms = delay.as_millis() as u64,
                "Rate limit: waiting before next request"
            );
            tokio::time::sleep(delay).await;
        }

        self.record_request(source);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global singleton
// ============================================================================

use once_cell::sync::Lazy;

/// Global rate limiter instance shared across all source fetchers.
static GLOBAL_RATE_LIMITER: Lazy<RateLimiter> = Lazy::new(RateLimiter::new);

/// Get the global rate limiter.
pub fn rate_limiter() -> &'static RateLimiter {
    &GLOBAL_RATE_LIMITER
}

// NOTE: Circuit breaker logic lives in the DB layer (db/sources.rs) via
// `is_circuit_open()` and `consecutive_failures` tracking. The DB-based
// approach is durable across restarts and already wired into the fetch pipeline.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_source_is_allowed() {
        let rl = RateLimiter::new();
        assert!(
            rl.check_rate_limit("hackernews"),
            "First request to a source should always be allowed"
        );
    }

    #[test]
    fn test_immediate_repeat_is_blocked() {
        let rl = RateLimiter::new();
        rl.record_request("reddit");
        assert!(
            !rl.check_rate_limit("reddit"),
            "Immediate repeat to same source should be blocked"
        );
    }

    #[test]
    fn test_different_sources_independent() {
        let rl = RateLimiter::new();
        rl.record_request("reddit");
        assert!(
            rl.check_rate_limit("hackernews"),
            "Different source should not be affected by another source's rate limit"
        );
    }

    #[test]
    fn test_source_intervals_are_positive() {
        // Verify all configured sources have reasonable intervals
        for source in &[
            "reddit",
            "github",
            "hackernews",
            "arxiv",
            "twitter",
            "rss",
            "devto",
            "lobsters",
        ] {
            let interval = source_interval(source);
            assert!(
                interval >= Duration::from_secs(1),
                "Source {} should have at least 1s interval, got {:?}",
                source,
                interval
            );
        }
    }

    #[test]
    fn test_default_interval_for_unknown_source() {
        let interval = source_interval("some_new_source");
        assert_eq!(
            interval, DEFAULT_MIN_INTERVAL,
            "Unknown source should use default interval"
        );
    }

    #[tokio::test]
    async fn test_wait_records_request() {
        let rl = RateLimiter::new();
        // First wait should return immediately (no prior request)
        rl.wait_for_rate_limit("devto").await;

        // Should now be recorded, so immediate check should fail
        assert!(
            !rl.check_rate_limit("devto"),
            "After wait_for_rate_limit, immediate check should be blocked"
        );
    }
}
