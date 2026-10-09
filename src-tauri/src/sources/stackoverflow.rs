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

/// A successful Stack Exchange response, read field by field rather than as one
/// strict struct. The rate signals must survive anything else in the payload
/// being off: on 2026-10-08 the API answered `"quota_remaining": -1`, the old
/// `Option<u32>` field rejected the WHOLE body as a parse error, the response's
/// rate signals went unread, and a 15-hour `throttle_violation` followed two
/// seconds later on the next tag.
#[derive(Debug)]
struct SoResponse {
    questions: Vec<SoQuestion>,
    /// Requests left in today's quota. Signed: Stack Exchange reports `-1`
    /// once the quota is overdrawn.
    quota_remaining: Option<i64>,
    /// Stack Exchange sets this on a SUCCESSFUL response to demand a pause
    /// before the next call to the same method. Ignoring it is what escalates
    /// into a `throttle_violation`.
    backoff: Option<u64>,
}

/// Parse a success body. Fails only when the body is not JSON at all; a
/// question that does not match [`SoQuestion`] is skipped, not fatal.
fn parse_response(body: &str) -> Result<SoResponse, SourceError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let quota_remaining = value.get("quota_remaining").and_then(|q| {
        q.as_i64()
            .or_else(|| q.as_f64().map(|f| f as i64))
            .or_else(|| q.as_str().and_then(|s| s.trim().parse().ok()))
    });
    let backoff = value.get("backoff").and_then(|b| {
        b.as_u64()
            .or_else(|| b.as_f64().filter(|f| *f > 0.0).map(|f| f.ceil() as u64))
            .or_else(|| b.as_str().and_then(|s| s.trim().parse().ok()))
    });
    let questions = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|q| serde_json::from_value::<SoQuestion>(q.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Ok(SoResponse {
        questions,
        quota_remaining,
        backoff,
    })
}

/// Seconds until the next UTC midnight, when Stack Exchange resets the daily
/// request quota.
fn secs_until_utc_midnight(now: u64) -> u64 {
    86_400 - (now % 86_400)
}

/// How long to stay away given the quota a response reported, or `None` to
/// carry on. Below [`MIN_QUOTA`] (including an overdrawn `-1`) the source
/// pauses until the quota resets. Only stopping the current cycle, as before,
/// let the next cycle 10 minutes later spend four more requests: measured on
/// 2026-10-08 the quota ran 9 -> 7 -> 4 -> 1 -> -1 across consecutive cycles,
/// and the request after -1 drew the 15-hour IP ban.
fn quota_pause_secs(quota_remaining: Option<i64>, now: u64) -> Option<u64> {
    (quota_remaining? < i64::from(MIN_QUOTA)).then(|| secs_until_utc_midnight(now))
}

/// Longest `backoff` honoured by sleeping inside the cycle. A longer demand
/// arms the breaker instead, so the next cycle cannot call early either.
const MAX_INLINE_BACKOFF_SECS: u64 = 30;

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
    /// Question body (HTML), present because the query asks for `filter=withbody`.
    body: Option<String>,
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

/// Write the deadline atomically: temp file + rename.
///
/// Three processes can arm the breaker concurrently, and a plain `fs::write` is
/// not atomic — a reader can observe a half-written file. That would parse as
/// garbage, be treated as "no throttle" (fail-soft), and cost a wasted request
/// into an active ban. `rename` over the same directory replaces in one step, so
/// a reader sees either the old deadline or the new one, never a torn value.
#[cfg(not(test))]
fn persist_deadline(deadline: u64) {
    let Some(path) = throttle_file() else {
        return;
    };
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, deadline.to_string()).is_ok() {
        if std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
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
        // Stack Exchange announces its cooldown in the JSON body rather than a
        // `Retry-After` header, but it is the same announcement — carry it so
        // the retry layer and the circuit breaker honour it like any other.
        return SourceError::rate_limited_after(
            format!("Stack Exchange throttled this IP; retry in {armed}s ({message})"),
            Some(armed),
        );
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
            return Err(SourceError::rate_limited(format!(
                "Stack Exchange throttle active for another {remaining}s; request suppressed"
            )));
        }

        let url = format!(
            "https://api.stackexchange.com/2.3/questions?order=desc&sort=activity&site=stackoverflow&tagged={}&pagesize=10&filter=withbody",
            urlencoding::encode(tag)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        // DELIBERATELY does not use `super::classify_http_response`, unlike every
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

        let so_resp = parse_response(&body)?;

        let quota_remaining = so_resp.quota_remaining;
        let backoff = so_resp.backoff;
        let questions = so_resp.questions;

        let items: Vec<SourceItem> = questions
            .into_iter()
            .map(|q| {
                let question_tags = q.tags.clone().unwrap_or_default();
                let answer_count = q.answer_count.unwrap_or(0);
                // Tags flow through metadata → extract_structured_tags() → extract_topics().
                // The body comes with the list call (`filter=withbody`, no extra
                // request). Without it every question was stored as a bare title.
                let content = q
                    .body
                    .as_deref()
                    .map(|html| crate::utils::html_to_text(html, crate::utils::MAX_CONTENT_LENGTH))
                    .unwrap_or_default();

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
    quota_remaining: Option<i64>,
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
            max_item_age_days: None,
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
        // A rotating window over the stack's tags: the fixed first
        // `MAX_TAGS_PER_FETCH` left every tag past the fourth permanently
        // unwatched (the stack can map to up to six).
        let tag_window = crate::source_fetching::rotating_window(
            "sources.stackoverflow.tag_cursor",
            &self.tags,
            MAX_TAGS_PER_FETCH,
        );
        let tags_to_fetch: Vec<&String> = tag_window.iter().collect();

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
                    // The quota is per IP and per day: below the floor, pause
                    // until it resets rather than just ending this cycle.
                    if let Some(pause) = quota_pause_secs(outcome.quota_remaining, now_secs()) {
                        let armed = arm_throttle(pause);
                        warn!(
                            remaining = ?outcome.quota_remaining,
                            pause_secs = armed,
                            "Stack Overflow daily quota nearly spent — pausing until it resets"
                        );
                        break;
                    }

                    if let Some(backoff) = outcome.backoff {
                        if backoff > MAX_INLINE_BACKOFF_SECS {
                            let armed = arm_throttle(backoff);
                            warn!(
                                backoff,
                                armed,
                                "Stack Exchange requested a long backoff — pausing the source"
                            );
                            break;
                        }
                        warn!(backoff, "Stack Exchange requested backoff — honouring it");
                        tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                    }
                }
                Err(e) => {
                    warn!(tag = %tag, error = ?e, "Failed to fetch SO questions for tag");
                    // A throttle applies to the IP, not the tag. Continuing the
                    // loop would spend three more doomed requests and can push
                    // the ban out further.
                    if matches!(
                        e,
                        SourceError::RateLimited { .. } | SourceError::Forbidden(_)
                    ) {
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
#[path = "stackoverflow_tests.rs"]
mod tests;
