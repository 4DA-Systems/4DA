// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack Overflow source implementation
//!
//! Fetches trending questions from Stack Overflow's public API.
//! No auth required for 300 requests/day quota.

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Stack Overflow API Types
// ============================================================================

// Unread JSON keys (`error_id`) carry no binding: serde skips unknown fields,
// and doctrine rule 8 says unused code is deleted rather than annotated into
// permanence.
#[derive(Debug, Deserialize)]
struct SoResponse {
    items: Option<Vec<SoQuestion>>,
    quota_remaining: Option<u32>,
    /// Stack Exchange sets this on a SUCCESSFUL response to demand a pause
    /// before the next call to the same method. Ignoring it is what escalates
    /// into a `throttle_violation`.
    backoff: Option<u64>,
}

/// Stack Exchange reports failures as HTTP 400 with a JSON body — NOT as 429.
/// Live capture, 2026-08-14:
/// `{"error_id":502,"error_message":"too many requests from this IP,
///    more requests available in 46472 seconds","error_name":"throttle_violation"}`
#[derive(Debug, Deserialize)]
struct SoError {
    error_message: Option<String>,
    error_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SoQuestion {
    question_id: u64,
    title: String,
    link: String,
    score: i32,
    answer_count: Option<u32>,
    view_count: Option<u64>,
    tags: Option<Vec<String>>,
    creation_date: Option<u64>,
    is_answered: Option<bool>,
}

// ============================================================================
// Stack Overflow Source
// ============================================================================

/// Default tags to fetch trending questions for
const DEFAULT_TAGS: &[&str] = &[
    "rust",
    "typescript",
    "react",
    "python",
    "docker",
    "kubernetes",
    "postgresql",
    "node.js",
];

/// Maximum tags to fetch per cycle (conservative rate limiting)
const MAX_TAGS_PER_FETCH: usize = 4;

/// Minimum quota remaining before stopping
const MIN_QUOTA: u32 = 10;

/// Cap on how long one throttle response may silence the source. Stack Exchange
/// has handed back deadlines of ~13 hours; honour them, but never longer than a
/// day, so a bogus value cannot disable the source permanently.
const MAX_THROTTLE_SECS: u64 = 86_400;

/// Fallback pause when Stack Exchange says "throttled" without a parseable
/// deadline. Long enough to actually break the hammering loop.
const DEFAULT_THROTTLE_SECS: u64 = 3_600;

/// Unix seconds until which Stack Exchange has told us to stay away.
///
/// PROCESS-GLOBAL on purpose. A fresh `StackOverflowSource` is constructed for
/// every fetch cycle, so per-instance state would forget the throttle the
/// instant it was learned — which is precisely how this source stayed pinned in
/// permanent violation: it could never read `quota_remaining` again (the 400
/// path returns before the body is parsed), so it never backed off, so the
/// throttle never expired.
static THROTTLED_UNTIL: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Where the throttle deadline is shared between processes.
///
/// Stack Exchange throttles by IP, but on this machine THREE processes drive the
/// pipeline against one data dir: the GUI, the `4DA Background Refresh` task
/// (`fourda --engine-once`, every 30min), and the ledger's `run-cycle.mjs`
/// (`fourda-engine --once`). An in-process breaker alone cannot help the two
/// short-lived ones — each new process would rediscover the ban by spending a
/// request into it. Persisting the deadline lets every process inherit it.
///
/// Disabled under `cfg(test)` so the unit tests exercise the in-memory logic
/// hermetically and never touch a real data directory.
#[cfg(not(test))]
fn throttle_file() -> Option<std::path::PathBuf> {
    Some(
        crate::runtime_paths::RuntimePaths::get()
            .data_dir
            .join(".stackoverflow_throttle"),
    )
}

/// Read a persisted deadline written by another process. Fail-soft: any error
/// (missing file, garbage, permissions) simply means "no known throttle".
#[cfg(not(test))]
fn load_persisted_deadline() -> u64 {
    throttle_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

#[cfg(not(test))]
fn persist_deadline(deadline: u64) {
    if let Some(path) = throttle_file() {
        let _ = std::fs::write(path, deadline.to_string());
    }
}

#[cfg(test)]
fn load_persisted_deadline() -> u64 {
    0
}

#[cfg(test)]
fn persist_deadline(_deadline: u64) {}

/// Seconds still remaining on an active throttle, if any.
///
/// Consults the shared on-disk deadline as well as this process's own, adopting
/// whichever is later, so a freshly-spawned `--once` run starts out already
/// aware of a ban another process discovered.
fn throttle_remaining() -> Option<u64> {
    let mut until = THROTTLED_UNTIL.load(Ordering::Relaxed);
    let persisted = load_persisted_deadline();
    if persisted > until {
        THROTTLED_UNTIL.fetch_max(persisted, Ordering::Relaxed);
        until = persisted;
    }
    until.checked_sub(now_secs()).filter(|&r| r > 0)
}

/// Arm the circuit breaker. Only ever extends the deadline — a shorter reading
/// must not shorten an existing longer pause.
fn arm_throttle(secs: u64) -> u64 {
    let clamped = secs.clamp(1, MAX_THROTTLE_SECS);
    let deadline = now_secs().saturating_add(clamped);
    let previous = THROTTLED_UNTIL.fetch_max(deadline, Ordering::Relaxed);
    if deadline > previous {
        persist_deadline(deadline);
    }
    clamped
}

/// Pull the retry delay out of a Stack Exchange throttle message, e.g.
/// "too many requests from this IP, more requests available in 46472 seconds".
fn parse_retry_after_secs(message: &str) -> Option<u64> {
    let tail = message.split(" in ").nth(1)?;
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().ok().filter(|&n| n > 0)
}

/// Classify a Stack Exchange error body into the right `SourceError`.
/// A throttle is NOT a bad request, and reporting it as one is what hid this
/// failure behind "HTTP 400 Bad Request" in the logs for so long.
fn classify_error(status: reqwest::StatusCode, body: &str) -> SourceError {
    let parsed: Option<SoError> = serde_json::from_str(body).ok();
    let name = parsed
        .as_ref()
        .and_then(|e| e.error_name.as_deref())
        .unwrap_or_default();
    let message = parsed
        .as_ref()
        .and_then(|e| e.error_message.as_deref())
        .unwrap_or(body);

    if name == "throttle_violation" || message.contains("too many requests") {
        let retry = parse_retry_after_secs(message).unwrap_or(DEFAULT_THROTTLE_SECS);
        let armed = arm_throttle(retry);
        warn!(
            retry_after_secs = armed,
            retry_after_hours = format!("{:.1}", armed as f64 / 3600.0),
            "Stack Exchange THROTTLE VIOLATION — circuit breaker armed, no further requests until it expires"
        );
        return SourceError::RateLimited(format!(
            "Stack Exchange throttled this IP; retry in {armed}s ({message})"
        ));
    }

    SourceError::Network(format!(
        "StackOverflow API error: HTTP {status} — {message}"
    ))
}

/// Stack Overflow source — fetches trending developer questions
pub struct StackOverflowSource {
    config: SourceConfig,
    client: reqwest::Client,
    tags: Vec<String>,
}

impl StackOverflowSource {
    /// Create a new Stack Overflow source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 20,
                fetch_interval_secs: 1800, // 30 minutes
                custom: None,
            },
            client: super::shared_client(),
            tags: DEFAULT_TAGS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Create a Stack Overflow source whose tags are shaped by the user's detected stack.
    /// Falls back to `DEFAULT_TAGS` when `tags` is empty (no stack signals / fresh install).
    pub fn with_tags(tags: Vec<String>) -> Self {
        let mut source = Self::new();
        if !tags.is_empty() {
            source.tags = tags;
        }
        source
    }

    /// Fetch questions for a single tag
    async fn fetch_tag(&self, tag: &str) -> SourceResult<SoFetchOutcome> {
        // Honour an armed circuit breaker BEFORE spending a request. Every call
        // made while throttled is both guaranteed to fail and liable to extend
        // the ban.
        if let Some(remaining) = throttle_remaining() {
            return Err(SourceError::RateLimited(format!(
                "Stack Exchange throttle active for another {remaining}s; request suppressed"
            )));
        }

        let url = format!(
            "https://api.stackexchange.com/2.3/questions?order=desc&sort=activity&site=stackoverflow&tagged={}&pagesize=10",
            urlencoding::encode(tag)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        // DELIBERATELY does not use `super::classify_http_status`, unlike every
        // sibling source. That helper decides from the STATUS CODE alone, and
        // Stack Exchange is the one upstream where the status code does not
        // carry the meaning: a throttle arrives as HTTP 400 with the reason in
        // the BODY. Classifying on status here is what disguised a 12.9-hour IP
        // ban as "Bad Request" and prevented the source from ever backing off.
        // If a cleanup pass centralises this call, the bug comes straight back.
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Stack Overflow forbidden (HTTP 403)".to_string(),
            ));
        }

        // Read the body BEFORE deciding what the failure means. Stack Exchange
        // signals throttling as HTTP 400 with the reason in the payload, so
        // status-only classification cannot tell a throttle from a genuine bad
        // request — and the old code returned before the body was ever read.
        let body = response
            .text()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !status.is_success() {
            return Err(classify_error(status, &body));
        }

        let so_resp: SoResponse =
            serde_json::from_str(&body).map_err(|e| SourceError::Parse(e.to_string()))?;

        let quota_remaining = so_resp.quota_remaining;
        let backoff = so_resp.backoff;
        let questions = so_resp.items.unwrap_or_default();

        let items: Vec<SourceItem> = questions
            .into_iter()
            .map(|q| {
                let question_tags = q.tags.clone().unwrap_or_default();
                let answer_count = q.answer_count.unwrap_or(0);
                // Tags flow through metadata → extract_structured_tags() → extract_topics().
                // Content is empty because SO API doesn't return question body in list endpoints.
                let content = String::new();

                let mut metadata = serde_json::json!({
                    "score": q.score,
                    "answer_count": answer_count,
                    "tags": question_tags,
                    "source_name": "stackoverflow",
                });

                if let Some(is_answered) = q.is_answered {
                    metadata["is_answered"] = serde_json::json!(is_answered);
                }
                if let Some(view_count) = q.view_count {
                    metadata["view_count"] = serde_json::json!(view_count);
                }
                if let Some(created) = q.creation_date {
                    metadata["creation_date"] = serde_json::json!(created);
                }

                let source_id = format!("so-{}", q.question_id);

                SourceItem::new("stackoverflow", &source_id, &q.title)
                    .with_url(Some(q.link))
                    .with_content(content)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(SoFetchOutcome {
            items,
            quota_remaining,
            backoff,
        })
    }
}

/// One tag's worth of results, plus the two rate signals Stack Exchange hands
/// back. Both were previously discarded on the failure path, which is why the
/// source could never learn it was throttled.
struct SoFetchOutcome {
    items: Vec<SourceItem>,
    quota_remaining: Option<u32>,
    backoff: Option<u64>,
}

impl Default for StackOverflowSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for StackOverflowSource {
    fn source_type(&self) -> &'static str {
        "stackoverflow"
    }

    fn name(&self) -> &'static str {
        "Stack Overflow"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Community,
            default_content_type: "question",
            default_multiplier: 1.0,
            label: "SO",
            color_hint: "orange",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // One check for the whole cycle, so a live throttle produces a single
        // honest line instead of four identical "Bad Request" warnings.
        if let Some(remaining) = throttle_remaining() {
            warn!(
                remaining_secs = remaining,
                remaining_hours = format!("{:.1}", remaining as f64 / 3600.0),
                "Stack Overflow SKIPPED — Stack Exchange throttle still active"
            );
            return Ok(Vec::new());
        }

        info!("Fetching Stack Overflow trending questions");

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        let tags_to_fetch: Vec<&String> = self.tags.iter().take(MAX_TAGS_PER_FETCH).collect();

        for (i, tag) in tags_to_fetch.iter().enumerate() {
            // 2-second delay between tag requests (skip first)
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }

            match self.fetch_tag(tag).await {
                Ok(outcome) => {
                    info!(
                        tag = %tag,
                        count = outcome.items.len(),
                        quota = ?outcome.quota_remaining,
                        backoff = ?outcome.backoff,
                        "Fetched SO questions"
                    );

                    for item in outcome.items {
                        if seen_ids.insert(item.source_id.clone()) {
                            all_items.push(item);
                        }
                    }

                    // Stack Exchange demands this pause before the next call to
                    // the same method. Ignoring it is what escalates a polite
                    // slowdown into a multi-hour IP ban.
                    if let Some(backoff) = outcome.backoff {
                        let wait = backoff.min(30);
                        warn!(
                            backoff,
                            wait, "Stack Exchange requested backoff — honouring it"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    }

                    // Stop if quota is running low
                    if let Some(remaining) = outcome.quota_remaining {
                        if remaining < MIN_QUOTA {
                            warn!(remaining, "Stack Overflow quota low, stopping early");
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!(tag = %tag, error = ?e, "Failed to fetch SO questions for tag");
                    // A throttle applies to the IP, not the tag. Continuing the
                    // loop would spend three more doomed requests and can push
                    // the ban out further.
                    if matches!(e, SourceError::RateLimited(_) | SourceError::Forbidden(_)) {
                        warn!("Aborting Stack Overflow cycle — rate limit applies to the whole IP");
                        break;
                    }
                }
            }
        }

        // Respect max_items limit
        all_items.truncate(self.config.max_items);

        info!(
            total = all_items.len(),
            "Total Stack Overflow items fetched"
        );
        Ok(all_items)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stackoverflow_source_creation() {
        let source = StackOverflowSource::new();
        assert_eq!(source.source_type(), "stackoverflow");
        assert_eq!(source.name(), "Stack Overflow");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        assert_eq!(source.config().fetch_interval_secs, 1800);
        assert_eq!(source.tags.len(), 8);
    }

    #[test]
    fn test_stackoverflow_source_default() {
        let source = StackOverflowSource::default();
        assert_eq!(source.source_type(), "stackoverflow");
    }

    #[test]
    fn test_stackoverflow_json_parsing() {
        let json = r#"{
            "items": [
                {
                    "question_id": 12345678,
                    "title": "How to handle async errors in Rust?",
                    "link": "https://stackoverflow.com/questions/12345678",
                    "score": 15,
                    "answer_count": 3,
                    "view_count": 1200,
                    "tags": ["rust", "async-await", "error-handling"],
                    "creation_date": 1709251200,
                    "is_answered": true
                },
                {
                    "question_id": 87654321,
                    "title": "TypeScript generic constraints",
                    "link": "https://stackoverflow.com/questions/87654321",
                    "score": 7,
                    "answer_count": null,
                    "view_count": null,
                    "tags": ["typescript", "generics"],
                    "creation_date": null,
                    "is_answered": false
                }
            ],
            "has_more": true,
            "quota_remaining": 295
        }"#;

        let resp: SoResponse = serde_json::from_str(json).unwrap();
        let items = resp.items.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].question_id, 12345678);
        assert_eq!(items[0].title, "How to handle async errors in Rust?");
        assert_eq!(items[0].score, 15);
        assert_eq!(items[0].answer_count, Some(3));
        assert_eq!(items[0].view_count, Some(1200));
        assert!(items[0].is_answered.unwrap());
        assert_eq!(resp.quota_remaining, Some(295));

        // Second item with null optional fields
        assert_eq!(items[1].question_id, 87654321);
        assert!(items[1].answer_count.is_none());
        assert!(items[1].view_count.is_none());
    }

    /// `THROTTLED_UNTIL` is process-global, so any test that arms it must hold
    /// this lock — cargo runs tests in parallel threads inside one process and
    /// a leaked deadline would make unrelated tests observe a throttle.
    static THROTTLE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn reset_throttle() {
        THROTTLED_UNTIL.store(0, Ordering::Relaxed);
    }

    /// The exact message Stack Exchange returned on 2026-08-14.
    #[test]
    fn test_parses_retry_after_from_live_throttle_message() {
        assert_eq!(
            parse_retry_after_secs(
                "too many requests from this IP, more requests available in 46472 seconds"
            ),
            Some(46_472)
        );
    }

    #[test]
    fn test_retry_after_absent_when_unparseable() {
        assert_eq!(parse_retry_after_secs("no deadline here"), None);
        assert_eq!(parse_retry_after_secs("available in zero seconds"), None);
        assert_eq!(parse_retry_after_secs("available in 0 seconds"), None);
    }

    /// The live 400 body must classify as RateLimited (NOT a bad request) and
    /// must arm the breaker. Misclassifying this as `Network`/"HTTP 400" is what
    /// disguised a 13-hour IP ban as a malformed query.
    #[test]
    fn test_throttle_body_classifies_as_ratelimited_and_arms_breaker() {
        let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_throttle();

        let body = r#"{"error_id":502,"error_message":"too many requests from this IP, more requests available in 46472 seconds","error_name":"throttle_violation"}"#;
        let err = classify_error(reqwest::StatusCode::BAD_REQUEST, body);

        assert!(
            matches!(err, SourceError::RateLimited(_)),
            "throttle must not be reported as a generic bad request, got {err:?}"
        );

        let remaining = throttle_remaining().expect("breaker must be armed");
        assert!(
            remaining > 46_000 && remaining <= 46_472,
            "breaker should hold the upstream deadline, got {remaining}"
        );

        reset_throttle();
    }

    /// A real malformed-query 400 must stay a normal error and must NOT arm the
    /// breaker — otherwise one bad tag would silence the source for an hour.
    #[test]
    fn test_genuine_bad_request_does_not_arm_breaker() {
        let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_throttle();

        let body =
            r#"{"error_id":400,"error_message":"site is required","error_name":"bad_parameter"}"#;
        let err = classify_error(reqwest::StatusCode::BAD_REQUEST, body);

        assert!(
            matches!(err, SourceError::Network(_)),
            "a genuine bad request must not be classed as a rate limit, got {err:?}"
        );
        assert!(
            throttle_remaining().is_none(),
            "breaker must stay disarmed for non-throttle errors"
        );
    }

    /// A throttle with no parseable deadline still has to break the loop.
    #[test]
    fn test_throttle_without_deadline_uses_default_pause() {
        let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_throttle();

        let err = classify_error(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error_name":"throttle_violation","error_message":"too many requests"}"#,
        );
        assert!(matches!(err, SourceError::RateLimited(_)));

        let remaining = throttle_remaining().expect("breaker must be armed");
        assert!(
            remaining > DEFAULT_THROTTLE_SECS - 60 && remaining <= DEFAULT_THROTTLE_SECS,
            "expected the default pause, got {remaining}"
        );

        reset_throttle();
    }

    /// An absurd upstream value must be clamped, never allowed to disable the
    /// source forever; and a shorter reading must not shorten a longer pause.
    #[test]
    fn test_throttle_is_clamped_and_only_extends() {
        let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_throttle();

        assert_eq!(arm_throttle(u64::MAX), MAX_THROTTLE_SECS);
        let long = throttle_remaining().expect("armed");

        // A shorter subsequent throttle must not pull the deadline in.
        arm_throttle(5);
        let after = throttle_remaining().expect("still armed");
        assert!(
            after >= long - 5,
            "a shorter reading must not shorten an active pause: {long} -> {after}"
        );

        reset_throttle();
        assert!(throttle_remaining().is_none());
    }

    /// `backoff` is now captured off the success path; it previously had no
    /// binding at all, so Stack Exchange's own slow-down request was discarded.
    #[test]
    fn test_success_response_captures_backoff() {
        let json = r#"{"items":[],"has_more":false,"quota_remaining":42,"backoff":10}"#;
        let resp: SoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.backoff, Some(10));
        assert_eq!(resp.quota_remaining, Some(42));
    }
}
