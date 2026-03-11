//! Source fetching functions extracted from lib.rs
//!
//! Contains: fetch_all_sources, fetch_all_sources_deep, fill_cache_background,
//! process_source_items, settings loader helpers, and self-healing retry logic.

mod fetcher;
mod processor;

// Re-export everything at the module level so `crate::source_fetching::X` paths still work
pub(crate) use fetcher::{fetch_all_sources, fetch_all_sources_deep};
pub(crate) use processor::fill_cache_background;

use crate::get_settings_manager;
use crate::sources::{SourceError, SourceItem, SourceResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

// ============================================================================
// Self-healing retry logic
// ============================================================================

/// Backoff delays for retry attempts (1s, 2s, 4s as specified).
const RETRY_BACKOFF_SECS: [u64; 3] = [1, 2, 4];

/// Maximum number of fetch attempts (1 initial + 2 retries = 3 total).
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Structured error returned after all retry attempts have been exhausted.
#[derive(Debug, Clone)]
pub(crate) struct RetryExhaustedError {
    /// Name of the adapter that failed
    pub adapter_name: String,
    /// Total number of attempts made
    pub attempts: usize,
    /// The last error encountered
    pub last_error: SourceError,
}

impl std::fmt::Display for RetryExhaustedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Adapter '{}' failed after {} attempts: {}",
            self.adapter_name, self.attempts, self.last_error
        )
    }
}

/// Check whether a SourceError is worth retrying.
/// Parse errors and Disabled errors are deterministic and will not succeed on retry.
fn is_retryable(err: &SourceError) -> bool {
    match err {
        SourceError::Network(_) => true,
        SourceError::RateLimited => true,
        SourceError::Other(_) => true,
        // Parse errors are deterministic — retrying won't help
        SourceError::Parse(_) => false,
        // Disabled is a config issue — retrying won't help
        SourceError::Disabled => false,
    }
}

/// Fetch items from a source adapter with exponential backoff retry.
///
/// Retries up to `MAX_RETRY_ATTEMPTS` times with delays of 1s, 2s, 4s.
/// Only retryable errors (Network, RateLimited, Other) trigger retries.
/// Non-retryable errors (Parse, Disabled) fail immediately.
///
/// On success, updates the failure tracker to reset the adapter's failure count.
/// On final failure, increments the failure tracker and returns a `RetryExhaustedError`.
pub(crate) async fn fetch_with_retry<F, Fut>(
    adapter_name: &str,
    tracker: &AdapterFailureTracker,
    fetch_fn: F,
) -> Result<Vec<SourceItem>, RetryExhaustedError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = SourceResult<Vec<SourceItem>>>,
{
    let mut last_error: Option<SourceError> = None;

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        match fetch_fn().await {
            Ok(items) => {
                if attempt > 1 {
                    info!(
                        target: "4da::retry",
                        adapter = adapter_name,
                        attempt,
                        "Fetch succeeded after retry"
                    );
                }
                tracker.record_success(adapter_name);
                return Ok(items);
            }
            Err(e) => {
                // Non-retryable errors fail immediately
                if !is_retryable(&e) {
                    warn!(
                        target: "4da::retry",
                        adapter = adapter_name,
                        attempt,
                        error = %e,
                        "Non-retryable error — failing immediately"
                    );
                    tracker.record_failure(adapter_name);
                    return Err(RetryExhaustedError {
                        adapter_name: adapter_name.to_string(),
                        attempts: attempt,
                        last_error: e,
                    });
                }

                if attempt < MAX_RETRY_ATTEMPTS {
                    let delay_secs = RETRY_BACKOFF_SECS.get(attempt - 1).copied().unwrap_or(4);
                    warn!(
                        target: "4da::retry",
                        adapter = adapter_name,
                        attempt,
                        max_attempts = MAX_RETRY_ATTEMPTS,
                        error = %e,
                        delay_secs,
                        "Fetch failed, retrying after backoff"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }

                last_error = Some(e);
            }
        }
    }

    // All attempts exhausted
    let err = last_error.unwrap_or_else(|| SourceError::Other("Unknown error".to_string()));
    tracker.record_failure(adapter_name);

    Err(RetryExhaustedError {
        adapter_name: adapter_name.to_string(),
        attempts: MAX_RETRY_ATTEMPTS,
        last_error: err,
    })
}

// ============================================================================
// Adapter failure tracker — per-adapter persistent failure counting
// ============================================================================

/// Tracks consecutive failure counts per adapter for detecting persistent issues.
///
/// Thread-safe (Arc<Mutex>) so it can be shared across async tasks.
/// The circuit breaker in DB (5+ consecutive failures) provides the hard stop;
/// this tracker provides in-memory visibility for the current session.
#[derive(Debug, Clone)]
pub(crate) struct AdapterFailureTracker {
    failures: Arc<Mutex<HashMap<String, u32>>>,
}

impl AdapterFailureTracker {
    /// Create a new tracker with zero failure counts.
    pub fn new() -> Self {
        Self {
            failures: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a successful fetch, resetting the failure count for this adapter.
    pub fn record_success(&self, adapter_name: &str) {
        let mut map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(adapter_name.to_string(), 0);
    }

    /// Record a failed fetch, incrementing the failure count for this adapter.
    pub fn record_failure(&self, adapter_name: &str) {
        let mut map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        let count = map.entry(adapter_name.to_string()).or_insert(0);
        *count += 1;
    }

    /// Get the current consecutive failure count for an adapter.
    pub fn failure_count(&self, adapter_name: &str) -> u32 {
        let map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        map.get(adapter_name).copied().unwrap_or(0)
    }

    /// Get all adapters that have persistent failures (2+ consecutive).
    #[allow(dead_code)]
    pub fn persistent_failures(&self) -> Vec<(String, u32)> {
        let map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .filter(|(_, &count)| count >= 2)
            .map(|(name, &count)| (name.clone(), count))
            .collect()
    }
}

// ============================================================================
// Settings loader helpers
// ============================================================================

/// Load RSS feed URLs from settings
pub(crate) fn load_rss_feeds_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let feeds = settings.get_rss_feeds();
    drop(settings);
    feeds
}

/// Load Twitter handles and X API key from settings
pub(crate) fn load_twitter_settings() -> (Vec<String>, String) {
    let settings = get_settings_manager().lock();
    let handles = settings.get_twitter_handles();
    let api_key = settings.get_x_api_key();
    drop(settings);
    (handles, api_key)
}

/// Load YouTube channel IDs from settings
pub(crate) fn load_youtube_channels_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let channels = settings.get_youtube_channels();
    drop(settings);
    channels
}

/// Load GitHub languages from settings (defaults if empty)
pub(crate) fn load_github_languages_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let langs = settings.get_github_languages();
    drop(settings);
    if langs.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        langs
    }
}

// ============================================================================
// Tests for self-healing retry logic
// ============================================================================

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ---------- AdapterFailureTracker unit tests ----------

    #[test]
    fn tracker_starts_at_zero() {
        let tracker = AdapterFailureTracker::new();
        assert_eq!(tracker.failure_count("hackernews"), 0);
        assert_eq!(tracker.failure_count("nonexistent"), 0);
    }

    #[test]
    fn tracker_records_failures_incrementally() {
        let tracker = AdapterFailureTracker::new();
        tracker.record_failure("hackernews");
        assert_eq!(tracker.failure_count("hackernews"), 1);
        tracker.record_failure("hackernews");
        assert_eq!(tracker.failure_count("hackernews"), 2);
        tracker.record_failure("hackernews");
        assert_eq!(tracker.failure_count("hackernews"), 3);
    }

    #[test]
    fn tracker_success_resets_count() {
        let tracker = AdapterFailureTracker::new();
        tracker.record_failure("reddit");
        tracker.record_failure("reddit");
        assert_eq!(tracker.failure_count("reddit"), 2);

        tracker.record_success("reddit");
        assert_eq!(tracker.failure_count("reddit"), 0);
    }

    #[test]
    fn tracker_isolates_adapters() {
        let tracker = AdapterFailureTracker::new();
        tracker.record_failure("hackernews");
        tracker.record_failure("hackernews");
        tracker.record_failure("reddit");

        assert_eq!(tracker.failure_count("hackernews"), 2);
        assert_eq!(tracker.failure_count("reddit"), 1);
        assert_eq!(tracker.failure_count("arxiv"), 0);
    }

    #[test]
    fn tracker_persistent_failures_threshold() {
        let tracker = AdapterFailureTracker::new();
        tracker.record_failure("hackernews");
        tracker.record_failure("hackernews");
        tracker.record_failure("reddit");

        let persistent = tracker.persistent_failures();
        // Only hackernews has 2+ failures
        assert_eq!(persistent.len(), 1);
        assert_eq!(persistent[0].0, "hackernews");
        assert_eq!(persistent[0].1, 2);
    }

    #[test]
    fn tracker_clone_shares_state() {
        let tracker = AdapterFailureTracker::new();
        let tracker2 = tracker.clone();

        tracker.record_failure("hackernews");
        // Clone should see the failure (Arc-based sharing)
        assert_eq!(tracker2.failure_count("hackernews"), 1);
    }

    // ---------- is_retryable tests ----------

    #[test]
    fn network_error_is_retryable() {
        assert!(is_retryable(&SourceError::Network("timeout".into())));
    }

    #[test]
    fn rate_limited_is_retryable() {
        assert!(is_retryable(&SourceError::RateLimited));
    }

    #[test]
    fn other_error_is_retryable() {
        assert!(is_retryable(&SourceError::Other("transient".into())));
    }

    #[test]
    fn parse_error_is_not_retryable() {
        assert!(!is_retryable(&SourceError::Parse("bad json".into())));
    }

    #[test]
    fn disabled_error_is_not_retryable() {
        assert!(!is_retryable(&SourceError::Disabled));
    }

    // ---------- fetch_with_retry async tests ----------

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("test-source", &tracker, || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(vec![SourceItem::new("test", "1", "Item 1")])
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(tracker.failure_count("test-source"), 0);
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("test-source", &tracker, || {
            let cc = cc.clone();
            async move {
                let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == 1 {
                    Err(SourceError::Network("timeout".into()))
                } else {
                    Ok(vec![SourceItem::new("test", "1", "Item 1")])
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        // Success resets the tracker
        assert_eq!(tracker.failure_count("test-source"), 0);
    }

    #[tokio::test]
    async fn retry_succeeds_on_third_attempt() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("test-source", &tracker, || {
            let cc = cc.clone();
            async move {
                let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    Err(SourceError::Network("connection reset".into()))
                } else {
                    Ok(vec![SourceItem::new("test", "1", "Item 1")])
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(tracker.failure_count("test-source"), 0);
    }

    #[tokio::test]
    async fn retry_exhausted_after_max_attempts() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("failing-source", &tracker, || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Err(SourceError::Network("server down".into()))
            }
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.adapter_name, "failing-source");
        assert_eq!(err.attempts, MAX_RETRY_ATTEMPTS);
        assert!(err.to_string().contains("server down"));
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(tracker.failure_count("failing-source"), 1);
    }

    #[tokio::test]
    async fn non_retryable_error_fails_immediately() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("parse-fail", &tracker, || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Err(SourceError::Parse("invalid JSON".into()))
            }
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.adapter_name, "parse-fail");
        assert_eq!(
            err.attempts, 1,
            "Should fail on first attempt without retrying"
        );
        assert!(err.to_string().contains("invalid JSON"));
        // Only called once — no retries for parse errors
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(tracker.failure_count("parse-fail"), 1);
    }

    #[tokio::test]
    async fn disabled_error_fails_immediately() {
        let tracker = AdapterFailureTracker::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let result = fetch_with_retry("disabled-source", &tracker, || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Err(SourceError::Disabled)
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_exhausted_error_display_format() {
        let err = RetryExhaustedError {
            adapter_name: "hackernews".to_string(),
            attempts: 3,
            last_error: SourceError::Network("connection refused".into()),
        };

        let msg = format!("{}", err);
        assert!(msg.contains("hackernews"));
        assert!(msg.contains("3 attempts"));
        assert!(msg.contains("connection refused"));
    }

    #[tokio::test]
    async fn tracker_accumulates_across_multiple_failures() {
        let tracker = AdapterFailureTracker::new();

        // Simulate multiple failed fetch_with_retry calls
        for _ in 0..3 {
            let _ = fetch_with_retry("flaky-source", &tracker, || async {
                Err(SourceError::Network("timeout".into()))
            })
            .await;
        }

        assert_eq!(tracker.failure_count("flaky-source"), 3);

        // One success resets it
        let _ = fetch_with_retry("flaky-source", &tracker, || async {
            Ok(vec![SourceItem::new("test", "1", "Item")])
        })
        .await;

        assert_eq!(tracker.failure_count("flaky-source"), 0);
    }

    // ---------- Backoff constant tests ----------

    #[test]
    fn backoff_constants_are_exponential() {
        assert_eq!(RETRY_BACKOFF_SECS, [1, 2, 4]);
        // Each value is double the previous
        assert_eq!(RETRY_BACKOFF_SECS[1], RETRY_BACKOFF_SECS[0] * 2);
        assert_eq!(RETRY_BACKOFF_SECS[2], RETRY_BACKOFF_SECS[1] * 2);
    }

    #[test]
    fn max_retry_attempts_matches_backoff_array() {
        assert_eq!(MAX_RETRY_ATTEMPTS, RETRY_BACKOFF_SECS.len());
    }
}
