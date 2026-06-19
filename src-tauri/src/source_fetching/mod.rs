// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source fetching functions extracted from lib.rs
//!
//! Contains: fetch_all_sources, fill_cache_background,
//! process_source_items, settings loader helpers, and self-healing retry logic.

mod fetcher;
mod processor;
mod yield_throttle;

// Re-export everything at the module level so `crate::source_fetching::X` paths still work
pub(crate) use fetcher::fetch_all_sources;
pub(crate) use processor::fill_cache_background;
pub(crate) use yield_throttle::{fetch_cap, SourceYield, RELEVANCE_FLOOR_PUB};

#[derive(Debug, Default)]
pub(crate) struct FetchSummary {
    pub succeeded: usize,
    pub failed: usize,
    pub skipped_disabled: usize,
    pub new_items: usize,
    pub cached_touches: usize,
}

use crate::get_settings_manager;
use crate::sources::rate_limiter::rate_limiter;
use crate::sources::{SourceError, SourceItem, SourceResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

// ============================================================================
// Feed origin extraction
// ============================================================================

/// Extract feed_origin from a SourceItem's metadata.
/// Checks keys in priority order: feed_url (RSS), channel_id (YouTube),
/// handle (Twitter), subreddit (Reddit), language (GitHub), source_name (single-endpoint).
pub(crate) fn extract_feed_origin(item: &crate::sources::SourceItem) -> Option<String> {
    item.metadata.as_ref().and_then(|m| {
        m.get("feed_url")
            .or_else(|| m.get("channel_id"))
            .or_else(|| m.get("handle"))
            .or_else(|| m.get("subreddit"))
            .or_else(|| m.get("language"))
            .or_else(|| m.get("source_name"))
            .and_then(|v| v.as_str().map(String::from))
    })
}

/// Extract structured tags from source item metadata and serialize as JSON.
///
/// Returns `None` if no tags found, or a JSON array string like `["rust","async"]`.
pub(crate) fn extract_source_tags(item: &crate::sources::SourceItem) -> Option<String> {
    let tags = item.extract_structured_tags();
    if tags.is_empty() {
        None
    } else {
        serde_json::to_string(&tags).ok()
    }
}

// ============================================================================
// Self-healing retry logic
// ============================================================================

/// Backoff delays for retry attempts (1s, 2s, 4s as specified).
const RETRY_BACKOFF_SECS: [u64; 3] = [1, 2, 4];

/// Extended backoff for rate-limited requests (seconds).
/// Much longer than normal backoff to respect API rate limits.
const RATE_LIMIT_BACKOFF_SECS: u64 = 30;

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
/// Parse errors, Disabled, and Forbidden errors are deterministic and will not succeed on retry.
fn is_retryable(err: &SourceError) -> bool {
    match err {
        SourceError::Network(_) => true,
        SourceError::RateLimited(_) => true,
        SourceError::Other(_) => true,
        // Parse errors are deterministic — retrying won't help
        SourceError::Parse(_) => false,
        // Disabled is a config issue — retrying won't help
        SourceError::Disabled => false,
        // Forbidden (403) means auth/permission issues — retrying won't help
        SourceError::Forbidden(_) => false,
    }
}

/// Check whether a SourceError indicates a rate-limit (HTTP 429) condition.
/// Matches the explicit RateLimited variant, plus common rate-limit indicators
/// in error strings (for errors tunnelled through Network/Other variants).
fn is_rate_limited(err: &SourceError) -> bool {
    match err {
        SourceError::RateLimited(_) => true,
        SourceError::Network(msg) | SourceError::Other(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("429")
                || lower.contains("rate limit")
                || lower.contains("too many requests")
        }
        _ => false,
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
        let _global_permit = rate_limiter().acquire_global_permit().await;
        let attempt_result =
            tokio::time::timeout(std::time::Duration::from_secs(90), fetch_fn()).await;

        let fetch_result = match attempt_result {
            Ok(r) => r,
            Err(_) => {
                warn!(
                    target: "4da::retry",
                    adapter = adapter_name,
                    attempt,
                    "Fetch attempt timed out after 90s"
                );
                Err(SourceError::Network(format!(
                    "{adapter_name}: fetch timed out after 90s"
                )))
            }
        };

        match fetch_result {
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
                    let rate_limited = is_rate_limited(&e);
                    let delay_secs = if rate_limited {
                        // Use extended backoff for rate-limited requests
                        RATE_LIMIT_BACKOFF_SECS
                    } else {
                        RETRY_BACKOFF_SECS.get(attempt - 1).copied().unwrap_or(4)
                    };

                    if rate_limited {
                        warn!(
                            target: "4da::retry",
                            adapter = adapter_name,
                            attempt,
                            max_attempts = MAX_RETRY_ATTEMPTS,
                            error = %e,
                            delay_secs,
                            "Rate limited (HTTP 429) — using extended backoff"
                        );
                    } else {
                        warn!(
                            target: "4da::retry",
                            adapter = adapter_name,
                            attempt,
                            max_attempts = MAX_RETRY_ATTEMPTS,
                            error = %e,
                            delay_secs,
                            "Fetch failed, retrying after backoff"
                        );
                    }
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
        let mut map = self
            .failures
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!(target: "4da::sources", "Mutex poisoned in failure tracker (record_success): {e}");
                e.into_inner()
            });
        map.insert(adapter_name.to_string(), 0);
    }

    /// Record a failed fetch, incrementing the failure count for this adapter.
    pub fn record_failure(&self, adapter_name: &str) {
        let mut map = self
            .failures
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!(target: "4da::sources", "Mutex poisoned in failure tracker (record_failure): {e}");
                e.into_inner()
            });
        let count = map.entry(adapter_name.to_string()).or_insert(0);
        *count += 1;
    }

    /// Get the current consecutive failure count for an adapter.
    pub fn failure_count(&self, adapter_name: &str) -> u32 {
        let map = self
            .failures
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!(target: "4da::sources", "Mutex poisoned in failure tracker (failure_count): {e}");
                e.into_inner()
            });
        map.get(adapter_name).copied().unwrap_or(0)
    }

    /// Get all adapters that have persistent failures (2+ consecutive).
    pub fn persistent_failures(&self) -> Vec<(String, u32)> {
        let map = self
            .failures
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!(target: "4da::sources", "Mutex poisoned in failure tracker (persistent_failures): {e}");
                e.into_inner()
            });
        map.iter()
            .filter(|(_, &count)| count >= 2)
            .map(|(name, &count)| (name.clone(), count))
            .collect()
    }
}

// ============================================================================
// Settings loader helpers
// ============================================================================

/// Load RSS feed URLs from settings — merges custom feeds on top of defaults,
/// filtering out any defaults the user has explicitly disabled.
pub(crate) fn load_rss_feeds_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let custom = settings.get_rss_feeds();
    let disabled = settings.get_disabled_default_rss_feeds();
    drop(settings);

    let mut feeds = load_default_rss_feeds();
    feeds.retain(|f| !disabled.contains(f));
    for url in custom {
        if !feeds.contains(&url) {
            feeds.push(url);
        }
    }

    // Filter out circuit-broken feeds (skip during DB init to avoid OnceCell deadlock)
    if let Some(db) = crate::try_get_database() {
        feeds.retain(|f| {
            if db.is_feed_circuit_open(f, "rss") {
                info!(target: "4da::sources", feed = %f, "Skipping circuit-broken RSS feed");
                false
            } else {
                true
            }
        });
    }

    feeds
}

/// Load Twitter handles and X API key from settings — merges custom handles
/// on top of defaults, filtering out any defaults the user has disabled.
pub(crate) fn load_twitter_settings() -> (Vec<String>, String) {
    let settings = get_settings_manager().lock();
    let custom = settings.get_twitter_handles();
    let disabled = settings.get_disabled_default_twitter_handles();
    let api_key = settings.get_x_api_key();
    drop(settings);

    let mut handles = load_default_twitter_handles();
    handles.retain(|h| !disabled.contains(h));
    for h in custom {
        if !handles.contains(&h) {
            handles.push(h);
        }
    }

    // Filter out circuit-broken handles (skip during DB init to avoid OnceCell deadlock)
    if let Some(db) = crate::try_get_database() {
        handles.retain(|h| {
            if db.is_feed_circuit_open(h, "twitter") {
                info!(target: "4da::sources", feed = %h, "Skipping circuit-broken Twitter handle");
                false
            } else {
                true
            }
        });
    }

    (handles, api_key)
}

/// Load YouTube channel IDs from settings — merges custom channels on top
/// of defaults, filtering out any defaults the user has disabled.
pub(crate) fn load_youtube_channels_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let custom = settings.get_youtube_channels();
    let disabled = settings.get_disabled_default_youtube_channels();
    drop(settings);

    let mut channels = load_default_youtube_channels();
    channels.retain(|c| !disabled.contains(c));
    for ch in custom {
        if !channels.contains(&ch) {
            channels.push(ch);
        }
    }

    // Filter out circuit-broken channels (skip during DB init to avoid OnceCell deadlock)
    if let Some(db) = crate::try_get_database() {
        channels.retain(|c| {
            if db.is_feed_circuit_open(c, "youtube") {
                info!(target: "4da::sources", feed = %c, "Skipping circuit-broken YouTube channel");
                false
            } else {
                true
            }
        });
    }

    channels
}

/// Default YouTube channel IDs from the YouTube source adapter
pub(crate) fn load_default_youtube_channels() -> Vec<String> {
    crate::sources::youtube::default_channel_ids()
}

/// Default Twitter handles from the Twitter source adapter
pub(crate) fn load_default_twitter_handles() -> Vec<String> {
    crate::sources::twitter::default_handle_list()
}

/// Load GitHub languages with precedence: explicit user settings > ACE-detected stack >
/// hard-coded defaults. The stack fallback keeps the GitHub-trending firehose shaped to the
/// user's actual languages (a Go/Java/etc. dev no longer gets only rust/ts/python trending).
pub(crate) fn load_github_languages_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let langs = settings.get_github_languages();
    drop(settings);
    if !langs.is_empty() {
        return langs;
    }
    let stack = crate::sources::stack_signals::detect().github_languages();
    if !stack.is_empty() {
        return stack;
    }
    vec![
        "rust".to_string(),
        "typescript".to_string(),
        "python".to_string(),
    ]
}

/// Whether the engine runs in strict manifest mode (`FOURDA_STRICT_MANIFEST=1`).
///
/// In strict mode the engine surfaces ONLY manifest-grounded release/vulnerability items:
/// registry fetchers never fall back to global default package lists, crates.io's
/// recent-updates discovery feed is disabled, the Go fetcher targets the stack's modules
/// instead of the whole-registry index, and vulnerabilities are routed through
/// dependency matching (`osv::matching`) rather than the global popular-package flow.
///
/// This is the mode the 4DA receipts ledger runs the headless engine in (set alongside
/// `FOURDA_DATA_DIR` per fixture). Read once at startup; with the flag unset, every gated
/// path below is skipped and desktop/product behavior is byte-for-byte unchanged.
pub(crate) fn strict_manifest_mode() -> bool {
    static STRICT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *STRICT.get_or_init(|| {
        std::env::var("FOURDA_STRICT_MANIFEST")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

/// Canonical ecosystem token used to compare a dependency's stored ecosystem/language
/// (e.g. "rust", "javascript", "PyPI", "go") against a registry source's ecosystem key
/// (e.g. "crates.io", "npm", "pypi", "go"). Returns `None` for unknown ecosystems.
fn ecosystem_token(eco: &str) -> Option<&'static str> {
    Some(match eco.trim().to_lowercase().as_str() {
        "npm" | "javascript" | "typescript" | "node" | "js" | "ts" => "npm",
        "crates.io" | "rust" | "cargo" | "crates" => "crates",
        "pypi" | "python" | "pip" | "py" => "pypi",
        "go" | "golang" => "go",
        "maven" | "java" | "gradle" => "maven",
        "nuget" | "csharp" | "c#" | "dotnet" => "nuget",
        "rubygems" | "ruby" | "gem" => "rubygems",
        "packagist" | "php" | "composer" => "packagist",
        _ => return None,
    })
}

/// Manifest-grounded package names for `ecosystem`, read directly from the dependency
/// tables the ACE full-scan populates (`user_dependencies` + `project_dependencies`)
/// WITHOUT the git-relevance gate. Strict-mode only; non-dev runtime deps.
///
/// Uses the caller's RAW connection — deliberately never `get_database()`. This runs inside
/// `build_all_sources()` (crates/npm/pypi constructors call `load_ace_packages_for_ecosystem`),
/// which executes within the one-time database + source-registry init (`SOURCES_REGISTERED`
/// `Once` in state.rs). Re-entering that init via the shared `Database` OnceCell/`Once` would
/// deadlock — the exact hazard documented at state.rs:371.
fn strict_manifest_packages_for(conn: &rusqlite::Connection, ecosystem: &str) -> Vec<String> {
    let Some(want) = ecosystem_token(ecosystem) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();

    // Both tables store (package_name, ecosystem-or-language, is_dev). user_dependencies uses
    // an explicit `ecosystem` column; project_dependencies stores the ecosystem in `language`.
    for sql in [
        "SELECT package_name, ecosystem, is_dev FROM user_dependencies",
        "SELECT package_name, language, is_dev FROM project_dependencies",
    ] {
        let Ok(mut stmt) = conn.prepare(sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2).unwrap_or(0) != 0,
            ))
        }) else {
            continue;
        };
        for (name, eco, is_dev) in rows.flatten() {
            if !is_dev && ecosystem_token(&eco) == Some(want) {
                out.push(name);
            }
        }
    }

    out
}

/// Load user's actual dependency names from ACE for a specific ecosystem.
/// Returns package names extracted from project manifests (Cargo.toml, package.json, etc.).
/// Falls back to empty vec if no deps are tracked yet.
pub(crate) fn load_ace_packages_for_ecosystem(ecosystem: &str) -> Vec<String> {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let manifest_types: Vec<&str> = match ecosystem {
        "npm" => vec!["PackageJson"],
        "crates.io" | "rust" => vec!["CargoToml"],
        "pypi" | "python" => vec!["PyprojectToml", "RequirementsTxt"],
        "go" => vec!["GoMod"],
        "maven" | "java" => vec!["PomXml", "BuildGradle"],
        "nuget" | "csharp" => vec!["Csproj"],
        "rubygems" | "ruby" => vec!["Gemfile"],
        "packagist" | "php" => vec!["ComposerJson"],
        "pub" | "dart" => vec!["PubspecYaml"],
        _ => return Vec::new(),
    };

    let mut packages: Vec<String> = match crate::temporal::get_all_dependencies(&conn) {
        Ok(deps) => deps
            .into_iter()
            .filter(|d| manifest_types.contains(&d.manifest_type.as_str()) && !d.is_dev)
            .map(|d| d.package_name)
            .collect(),
        Err(e) => {
            tracing::debug!(target: "4da::sources", error = %e, ecosystem = ecosystem, "No ACE deps available");
            Vec::new()
        }
    };

    // Strict manifest mode (ledger/headless): `project_dependencies` is gated by a
    // git-recency relevance score and is empty for the ledger's fixture stacks (plain
    // dirs, no git history), so the loop above yields nothing and the registry sources
    // would fall back to global defaults. Union in the manifest deps the ACE full-scan
    // persists to `user_dependencies` (+ `project_dependencies`, ungated) so the fetchers
    // target the stack's REAL pinned packages. Gated: desktop monitoring set is unchanged
    // when the flag is unset.
    if strict_manifest_mode() {
        packages.extend(strict_manifest_packages_for(&conn, ecosystem));
    }

    packages.sort();
    packages.dedup();
    packages
}

/// Load ACE-tracked packages WITH their installed versions for version-aware
/// OSV querying. Returns (package_name, version) pairs for non-dev runtime deps.
pub(crate) fn load_ace_packages_with_versions(ecosystem: &str) -> Vec<(String, Option<String>)> {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let manifest_types: Vec<&str> = match ecosystem {
        "npm" => vec!["PackageJson"],
        "crates.io" | "rust" => vec!["CargoToml"],
        "pypi" | "python" => vec!["PyprojectToml", "RequirementsTxt"],
        "go" => vec!["GoMod"],
        "maven" | "java" => vec!["PomXml", "BuildGradle"],
        "nuget" | "csharp" => vec!["Csproj"],
        "rubygems" | "ruby" => vec!["Gemfile"],
        "packagist" | "php" => vec!["ComposerJson"],
        "pub" | "dart" => vec!["PubspecYaml"],
        _ => return Vec::new(),
    };

    match crate::temporal::get_all_dependencies(&conn) {
        Ok(deps) => {
            let mut packages: Vec<(String, Option<String>)> = deps
                .into_iter()
                .filter(|d| manifest_types.contains(&d.manifest_type.as_str()) && !d.is_dev)
                .map(|d| (d.package_name, d.version))
                .collect();
            packages.sort_by(|a, b| a.0.cmp(&b.0));
            packages.dedup_by(|a, b| a.0 == b.0);
            packages
        }
        Err(e) => {
            tracing::debug!(target: "4da::sources", error = %e, ecosystem = ecosystem, "No ACE deps with versions available");
            Vec::new()
        }
    }
}

/// Load default RSS feeds from the curated feed registry.
/// All curated feeds are enabled by default — users opt out via disabled_curated_feeds.
pub(crate) fn load_default_rss_feeds() -> Vec<String> {
    crate::curated_feeds::get_curated_registry()
        .all_feeds()
        .iter()
        .map(|f| f.url.clone())
        .collect()
}

// ============================================================================
// Tests for strict manifest mode helpers
// ============================================================================

#[cfg(test)]
mod strict_manifest_tests {
    use super::*;

    #[test]
    fn ecosystem_token_canonicalizes_aliases() {
        // Registry source keys map to the same token as the deps' stored ecosystem/language.
        assert_eq!(ecosystem_token("crates.io"), ecosystem_token("rust"));
        assert_eq!(ecosystem_token("crates.io"), Some("crates"));
        assert_eq!(ecosystem_token("npm"), ecosystem_token("javascript"));
        assert_eq!(ecosystem_token("npm"), ecosystem_token("typescript"));
        assert_eq!(ecosystem_token("pypi"), ecosystem_token("python"));
        assert_eq!(ecosystem_token("pypi"), ecosystem_token("pip"));
        assert_eq!(ecosystem_token("go"), ecosystem_token("golang"));
    }

    #[test]
    fn ecosystem_token_is_case_and_whitespace_insensitive() {
        assert_eq!(ecosystem_token("  Rust "), Some("crates"));
        assert_eq!(ecosystem_token("PyPI"), Some("pypi"));
    }

    #[test]
    fn ecosystem_token_unknown_is_none_and_never_cross_matches() {
        assert_eq!(ecosystem_token("brainfuck"), None);
        // Two distinct unknowns must not be treated as the same ecosystem.
        assert_ne!(ecosystem_token("rust"), ecosystem_token("unknown-eco"));
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
        assert!(is_retryable(&SourceError::RateLimited(
            "test rate limit".into()
        )));
    }

    #[test]
    fn forbidden_is_not_retryable() {
        assert!(!is_retryable(&SourceError::Forbidden(
            "auth failure".into()
        )));
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

// ============================================================================
// Tests for merge logic and feed_origin extraction
// ============================================================================

#[cfg(test)]
mod merge_and_origin_tests {
    use super::*;
    use crate::sources::SourceItem;

    // ===== extract_feed_origin tests =====

    #[test]
    fn extract_feed_origin_from_rss_metadata() {
        let item = SourceItem::new("rss", "1", "Test")
            .with_metadata(serde_json::json!({"feed_url": "https://blog.rust-lang.org/feed.xml"}));
        assert_eq!(
            extract_feed_origin(&item),
            Some("https://blog.rust-lang.org/feed.xml".to_string())
        );
    }

    #[test]
    fn extract_feed_origin_from_youtube_metadata() {
        let item = SourceItem::new("youtube", "1", "Test")
            .with_metadata(serde_json::json!({"channel_id": "UCsBjURrPoezykLs9EqgamOA"}));
        assert_eq!(
            extract_feed_origin(&item),
            Some("UCsBjURrPoezykLs9EqgamOA".to_string())
        );
    }

    #[test]
    fn extract_feed_origin_from_twitter_metadata() {
        let item = SourceItem::new("twitter", "1", "Test")
            .with_metadata(serde_json::json!({"handle": "rustlang"}));
        assert_eq!(extract_feed_origin(&item), Some("rustlang".to_string()));
    }

    #[test]
    fn extract_feed_origin_no_metadata() {
        let item = SourceItem::new("hackernews", "1", "Test");
        assert_eq!(extract_feed_origin(&item), None);
    }

    #[test]
    fn extract_feed_origin_empty_metadata() {
        let item = SourceItem::new("hackernews", "1", "Test").with_metadata(serde_json::json!({}));
        assert_eq!(extract_feed_origin(&item), None);
    }

    #[test]
    fn extract_feed_origin_priority_order() {
        // If multiple keys present, feed_url wins over channel_id wins over handle
        let item = SourceItem::new("rss", "1", "Test").with_metadata(serde_json::json!({
            "feed_url": "https://example.com/feed",
            "channel_id": "UC123",
            "handle": "test"
        }));
        assert_eq!(
            extract_feed_origin(&item),
            Some("https://example.com/feed".to_string())
        );
    }

    #[test]
    fn extract_feed_origin_non_string_value_skipped() {
        let item = SourceItem::new("rss", "1", "Test")
            .with_metadata(serde_json::json!({"feed_url": 12345}));
        assert_eq!(extract_feed_origin(&item), None);
    }

    #[test]
    fn extract_feed_origin_subreddit_key() {
        let item = SourceItem::new("reddit", "1", "Test")
            .with_metadata(serde_json::json!({"subreddit": "rust"}));
        assert_eq!(extract_feed_origin(&item), Some("rust".to_string()));
    }

    #[test]
    fn extract_feed_origin_language_key() {
        let item = SourceItem::new("github", "1", "Test")
            .with_metadata(serde_json::json!({"language": "Rust", "stars": 100}));
        assert_eq!(extract_feed_origin(&item), Some("Rust".to_string()));
    }

    #[test]
    fn extract_feed_origin_source_name_key() {
        let item = SourceItem::new("hackernews", "1", "Test")
            .with_metadata(serde_json::json!({"source_name": "hackernews", "score": 42}));
        assert_eq!(extract_feed_origin(&item), Some("hackernews".to_string()));
    }

    // ===== Merge algorithm tests (testing the pure logic) =====

    /// Tests the merge algorithm that all three loader functions use:
    /// defaults - disabled + custom (deduplicated)
    fn merge_sources(
        defaults: Vec<String>,
        disabled: Vec<String>,
        custom: Vec<String>,
    ) -> Vec<String> {
        let mut result = defaults;
        result.retain(|f| !disabled.contains(f));
        for item in custom {
            if !result.contains(&item) {
                result.push(item);
            }
        }
        result
    }

    #[test]
    fn merge_custom_appends_to_defaults() {
        let defaults = vec!["a".into(), "b".into()];
        let custom = vec!["c".into()];
        let result = merge_sources(defaults, vec![], custom);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn merge_custom_never_replaces_defaults() {
        let defaults = vec!["a".into(), "b".into(), "c".into()];
        let custom = vec!["d".into()];
        let result = merge_sources(defaults, vec![], custom);
        assert_eq!(result.len(), 4);
        assert!(result.contains(&"a".to_string()));
        assert!(result.contains(&"b".to_string()));
        assert!(result.contains(&"c".to_string()));
        assert!(result.contains(&"d".to_string()));
    }

    #[test]
    fn merge_disabled_removes_from_defaults() {
        let defaults = vec!["a".into(), "b".into(), "c".into()];
        let disabled = vec!["b".into()];
        let result = merge_sources(defaults, disabled, vec![]);
        assert_eq!(result, vec!["a", "c"]);
    }

    #[test]
    fn merge_duplicate_custom_deduped() {
        let defaults = vec!["a".into(), "b".into()];
        let custom = vec!["a".into(), "c".into()]; // "a" is already in defaults
        let result = merge_sources(defaults, vec![], custom);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn merge_all_defaults_disabled() {
        let defaults = vec!["a".into(), "b".into()];
        let disabled = vec!["a".into(), "b".into()];
        let custom = vec!["c".into()];
        let result = merge_sources(defaults, disabled, custom);
        assert_eq!(result, vec!["c"]);
    }

    #[test]
    fn merge_empty_custom_returns_defaults() {
        let defaults = vec!["a".into(), "b".into()];
        let result = merge_sources(defaults, vec![], vec![]);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn merge_empty_defaults_returns_custom() {
        let result = merge_sources(vec![], vec![], vec!["x".into()]);
        assert_eq!(result, vec!["x"]);
    }

    #[test]
    fn merge_preserves_order_defaults_first() {
        let defaults = vec!["d1".into(), "d2".into()];
        let custom = vec!["c1".into(), "c2".into()];
        let result = merge_sources(defaults, vec![], custom);
        assert_eq!(result, vec!["d1", "d2", "c1", "c2"]);
    }

    #[test]
    fn merge_disabled_nonexistent_is_harmless() {
        let defaults = vec!["a".into()];
        let disabled = vec!["z".into()]; // "z" not in defaults
        let result = merge_sources(defaults, disabled, vec![]);
        assert_eq!(result, vec!["a"]);
    }

    // ===== Default source list sanity checks =====

    #[test]
    fn default_rss_feeds_not_empty() {
        let feeds = load_default_rss_feeds();
        assert!(
            feeds.len() >= 10,
            "Should have at least 10 default RSS feeds"
        );
        for feed in &feeds {
            assert!(
                feed.starts_with("https://"),
                "Default RSS feed should be HTTPS: {}",
                feed
            );
        }
    }

    #[test]
    fn default_youtube_channels_not_empty() {
        let channels = load_default_youtube_channels();
        assert!(!channels.is_empty(), "Should have default YouTube channels");
    }

    #[test]
    fn default_twitter_handles_not_empty() {
        let handles = load_default_twitter_handles();
        assert!(!handles.is_empty(), "Should have default Twitter handles");
        for handle in &handles {
            assert!(
                !handle.starts_with('@'),
                "Default handle should not start with @: {}",
                handle
            );
        }
    }

    #[test]
    fn default_rss_feeds_no_duplicates() {
        let feeds = load_default_rss_feeds();
        let mut seen = std::collections::HashSet::new();
        for feed in &feeds {
            assert!(seen.insert(feed), "Duplicate default RSS feed: {}", feed);
        }
    }
}
