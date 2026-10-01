// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lobste.rs source implementation
//!
//! Fetches hottest and newest stories from Lobste.rs JSON API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Lobste.rs API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct LobstersStory {
    short_id: String,
    title: String,
    url: Option<String>,
    #[serde(default)]
    description: String,
    created_at: Option<String>,
    score: Option<i32>,
    comment_count: Option<i32>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    submitter_user: Option<LobstersSubmitter>,
}

/// The submitter arrives either as a bare username string — the shape the live
/// API returns as of 2026-08-14 — or as an object carrying a `username` field,
/// which is what it used to return. Accept both.
///
/// This is not defensive padding: the object-only binding silently took the
/// whole source offline. `#[serde(default)]` did not save it, because the field
/// was PRESENT with the wrong type, and `default` only covers ABSENT fields. A
/// single field's type change zeroed every Lobste.rs fetch, and the unit test
/// kept passing because it asserted the stale shape.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LobstersSubmitter {
    Name(String),
    Object { username: String },
}

impl LobstersSubmitter {
    fn username(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Object { username } => username,
        }
    }
}

/// Oldest story `hottest.json` / `newest.json` can legitimately list. Both
/// feeds only ever show the last few days; anything older is an archive page.
///
/// Since 2026-09-23 Lobste.rs has intermittently served cached archive pages at
/// the plain feed URLs (its page cache keys on the path and ignores `?page=`,
/// so a `newest.json?page=N` request that lands on an expired cache poisons
/// the shared entry). 3,460 stories from 2012-2025 were ingested that way and
/// 24 reached the Signal feed — "Tauri 2.0 Stable Release", "Announcing Rust
/// 1.88.0" — as if they were today's news.
const MAX_STORY_AGE_DAYS: i64 = 14;

/// True when the story's `created_at` is older than [`MAX_STORY_AGE_DAYS`]
/// relative to `now`. A missing or unparseable timestamp is NOT stale — the
/// gate drops only what it can prove is old.
fn is_stale_story(created_at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> bool {
    created_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|t| now.signed_duration_since(t) > chrono::Duration::days(MAX_STORY_AGE_DAYS))
}

// ============================================================================
// Lobste.rs Source
// ============================================================================

/// Lobste.rs source - fetches hottest and newest tech stories
pub struct LobstersSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl LobstersSource {
    /// Create a new Lobste.rs source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 600, // 10 minutes
                custom: None,
            },
            client: super::shared_client(),
        }
    }

    /// Fetch stories from a specific endpoint
    async fn fetch_endpoint(&self, url: &str, max: usize) -> SourceResult<Vec<SourceItem>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        super::classify_http_response(&response, "Lobste.rs API")?;

        // Decode the envelope and each story SEPARATELY. Deserializing straight
        // into `Vec<LobstersStory>` is all-or-nothing: one record whose shape
        // drifted upstream takes the entire batch to zero, which is exactly how
        // this source went dark. Per-record decoding degrades to "lost one
        // story" instead of "lost the source".
        let raw: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;
        let raw_count = raw.len();

        let mut stories: Vec<LobstersStory> = Vec::with_capacity(raw_count.min(max));
        let mut skipped: Vec<String> = Vec::new();
        for value in raw {
            if stories.len() >= max {
                break;
            }
            match serde_json::from_value::<LobstersStory>(value) {
                Ok(story) => stories.push(story),
                Err(e) => skipped.push(e.to_string()),
            }
        }

        if !skipped.is_empty() {
            warn!(
                url = %url,
                skipped = skipped.len(),
                raw_count,
                first_error = %skipped[0],
                "Lobste.rs: skipped unparseable stories (upstream shape drift?)"
            );
        }

        // Records arrived but NONE decoded — that is a contract break, not an
        // empty feed. Surface it as an error so the source shows as failing
        // rather than quietly reporting zero items forever.
        if stories.is_empty() && raw_count > 0 {
            return Err(SourceError::Parse(format!(
                "all {raw_count} Lobste.rs stories failed to decode; first error: {}",
                skipped
                    .first()
                    .map_or("<none>", std::string::String::as_str)
            )));
        }

        let decoded = stories.len();
        let now = chrono::Utc::now();
        stories.retain(|story| !is_stale_story(story.created_at.as_deref(), now));
        let stale = decoded - stories.len();
        if stale > 0 {
            warn!(
                url = %url,
                stale,
                decoded,
                max_age_days = MAX_STORY_AGE_DAYS,
                "Lobste.rs: dropped archive stories from a live feed (upstream page-cache poisoning?)"
            );
        }

        let items: Vec<SourceItem> = stories
            .into_iter()
            .map(|story| {
                // Use description as content; for stories without description,
                // content will be populated later via scrape_content
                let content = story.description.clone();

                // Build the canonical URL: story URL if available, otherwise lobste.rs link
                let item_url = story
                    .url
                    .clone()
                    .unwrap_or_else(|| format!("https://lobste.rs/s/{}", story.short_id));

                let mut metadata = serde_json::json!({
                    "tags": story.tags,
                    "source_name": "lobsters",
                });

                if let Some(score) = story.score {
                    metadata["score"] = serde_json::json!(score);
                }
                if let Some(comments) = story.comment_count {
                    metadata["comments"] = serde_json::json!(comments);
                }
                if let Some(created) = &story.created_at {
                    metadata["created_at"] = serde_json::json!(created);
                }
                if let Some(user) = &story.submitter_user {
                    metadata["author"] = serde_json::json!(user.username());
                }

                SourceItem::new("lobsters", &story.short_id, &story.title)
                    .with_url(Some(item_url))
                    .with_content(content)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(items)
    }
}

impl Default for LobstersSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for LobstersSource {
    fn source_type(&self) -> &'static str {
        "lobsters"
    }

    fn name(&self) -> &'static str {
        "Lobste.rs"
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
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "Lobsters",
            color_hint: "red",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
            max_item_age_days: super::freshness::news_max_item_age_days("lobsters"),
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Lobste.rs hottest stories");

        let items = self
            .fetch_endpoint("https://lobste.rs/hottest.json", self.config.max_items)
            .await?;

        info!(items = items.len(), "Fetched Lobste.rs hottest items");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Deep fetching Lobste.rs (hottest + newest)");

        let (hottest_result, newest_result) = tokio::join!(
            self.fetch_endpoint("https://lobste.rs/hottest.json", 30),
            self.fetch_endpoint("https://lobste.rs/newest.json", 30),
        );

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        match hottest_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Lobste.rs hottest");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Lobste.rs hottest");
            }
        }

        match newest_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Lobste.rs newest");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Lobste.rs newest");
            }
        }

        info!(total = all_items.len(), "Total Lobste.rs items after dedup");
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // If item already has meaningful content from description, return it
        if item.content.len() > 50 {
            return Ok(item.content.clone());
        }

        // Try to scrape the linked URL
        let url = match &item.url {
            Some(u) => u,
            None => return Ok(item.content.clone()),
        };

        // Skip non-HTTP URLs
        if !url.starts_with("http") {
            return Ok(item.content.clone());
        }

        info!(url = %url, "Scraping article content for Lobste.rs story");

        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::scrape_article_content(url),
        )
        .await
        {
            Ok(Some(content)) => {
                let truncated = if content.len() > 5000 {
                    content.chars().take(5000).collect()
                } else {
                    content
                };
                info!(url = %url, length = truncated.len(), "Scraped article content");
                Ok(truncated)
            }
            Ok(None) => {
                warn!(url = %url, "Failed to extract content from article");
                Ok(item.content.clone())
            }
            Err(_) => {
                warn!(url = %url, "Timed out scraping article (2s limit)");
                Ok(item.content.clone())
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lobsters_source_creation() {
        let source = LobstersSource::new();
        assert_eq!(source.source_type(), "lobsters");
        assert_eq!(source.name(), "Lobste.rs");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 600);
    }

    #[test]
    fn test_lobsters_source_default() {
        let source = LobstersSource::default();
        assert_eq!(source.source_type(), "lobsters");
    }

    #[test]
    fn test_lobsters_json_parsing() {
        let json = r#"[
            {
                "short_id": "abc123",
                "title": "Rust async patterns",
                "url": "https://example.com/rust-async",
                "description": "A deep dive into async Rust programming patterns",
                "created_at": "2026-02-10T12:00:00.000-06:00",
                "score": 42,
                "comment_count": 15,
                "tags": ["rust", "programming"],
                "submitter_user": { "username": "rustdev" }
            },
            {
                "short_id": "def456",
                "title": "SQLite internals",
                "url": null,
                "description": "",
                "created_at": "2026-02-09T08:30:00.000-06:00",
                "score": 28,
                "comment_count": 7,
                "tags": ["databases"]
            }
        ]"#;

        let stories: Vec<LobstersStory> = serde_json::from_str(json).unwrap();
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].short_id, "abc123");
        assert_eq!(stories[0].title, "Rust async patterns");
        assert_eq!(
            stories[0].url,
            Some("https://example.com/rust-async".to_string())
        );
        assert_eq!(stories[0].score, Some(42));
        assert_eq!(stories[0].comment_count, Some(15));
        assert_eq!(stories[0].tags, vec!["rust", "programming"]);
        assert_eq!(
            stories[0].submitter_user.as_ref().unwrap().username(),
            "rustdev"
        );

        // Story without URL should have None
        assert!(stories[1].url.is_none());
        assert!(stories[1].description.is_empty());
        assert!(stories[1].submitter_user.is_none());
    }

    /// VERBATIM record from `https://lobste.rs/hottest.json`, captured
    /// 2026-08-14. `submitter_user` is a BARE STRING here. The previous binding
    /// required an object and so failed this exact payload — every fetch, both
    /// endpoints — while `test_lobsters_json_parsing` above stayed green because
    /// it asserted the old shape. Keep this fixture byte-faithful to the wire.
    #[test]
    fn test_lobsters_parses_live_bare_string_submitter() {
        let json = r#"[{
            "short_id":"tssf5y",
            "created_at":"2026-08-13T12:43:13.111-05:00",
            "title":"I want extern \"fil-c\"",
            "url":"https://domenkozar.com/2026/08/13/i-want-extern-fil-c/",
            "score":42,
            "flags":1,
            "comment_count":13,
            "description":"",
            "description_plain":"",
            "submitter_user":"fzakaria",
            "user_is_author":false,
            "tags":["c","rust"],
            "short_id_url":"https://lobste.rs/s/tssf5y",
            "comments_url":"https://lobste.rs/s/tssf5y/i_want_extern_fil_c"
        }]"#;

        let stories: Vec<LobstersStory> =
            serde_json::from_str(json).expect("live Lobste.rs payload must decode");
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].short_id, "tssf5y");
        assert_eq!(stories[0].tags, vec!["c", "rust"]);
        assert_eq!(
            stories[0].submitter_user.as_ref().unwrap().username(),
            "fzakaria"
        );
    }

    /// Both submitter shapes must decode to the same username.
    #[test]
    fn test_lobsters_submitter_accepts_both_shapes() {
        let bare: LobstersSubmitter = serde_json::from_str(r#""alice""#).unwrap();
        assert_eq!(bare.username(), "alice");

        let object: LobstersSubmitter = serde_json::from_str(r#"{"username":"bob"}"#).unwrap();
        assert_eq!(object.username(), "bob");
    }

    /// One malformed record must cost one story, not the whole batch. This is
    /// the property whose absence turned a single field's type change into a
    /// total source blackout.
    #[test]
    fn test_lobsters_one_bad_record_does_not_zero_the_batch() {
        let json = r#"[
            {"short_id":"ok1","title":"Good one","tags":["rust"]},
            {"title":"Missing short_id — undecodable"},
            {"short_id":"ok2","title":"Good two","tags":["c"]}
        ]"#;

        let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
        let decoded: Vec<LobstersStory> = raw
            .into_iter()
            .filter_map(|v| serde_json::from_value::<LobstersStory>(v).ok())
            .collect();

        assert_eq!(decoded.len(), 2, "the two well-formed stories must survive");
        assert_eq!(decoded[0].short_id, "ok1");
        assert_eq!(decoded[1].short_id, "ok2");
    }

    fn at(rfc3339: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    /// Real poisoned-feed records (ingested 2026-10-01 from /newest.json):
    /// a 2017 and a 2025 story must be dropped, a same-day story kept.
    #[test]
    fn test_archive_stories_are_stale() {
        let now = at("2026-10-01T14:21:05Z");
        assert!(is_stale_story(Some("2017-10-24T12:49:10.000-05:00"), now));
        assert!(is_stale_story(Some("2025-06-26T13:58:39.000-05:00"), now));
        assert!(!is_stale_story(Some("2026-10-01T09:12:00.000-05:00"), now));
    }

    #[test]
    fn test_stale_boundary_is_fourteen_days() {
        let now = at("2026-10-01T00:00:00Z");
        assert!(!is_stale_story(Some("2026-09-17T00:00:00Z"), now));
        assert!(is_stale_story(Some("2026-09-16T23:59:59Z"), now));
    }

    /// The gate only drops what it can prove is old.
    #[test]
    fn test_missing_or_garbled_timestamp_is_kept() {
        let now = at("2026-10-01T00:00:00Z");
        assert!(!is_stale_story(None, now));
        assert!(!is_stale_story(Some("yesterday"), now));
    }
}
