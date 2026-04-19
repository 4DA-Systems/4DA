// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Bluesky/AT Protocol source implementation
//!
//! Fetches developer-relevant posts from Bluesky's public search API.
//! No auth required for public API access.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Bluesky API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct BskySearchResponse {
    posts: Option<Vec<BskyPost>>,
    // getFeed returns {feed: [{post: BskyPost}]}
    feed: Option<Vec<BskyFeedItem>>,
    #[allow(dead_code)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BskyFeedItem {
    post: BskyPost,
}

#[derive(Debug, Deserialize)]
struct BskyPost {
    uri: String,
    #[allow(dead_code)]
    cid: Option<String>,
    author: BskyAuthor,
    record: BskyRecord,
    #[serde(rename = "likeCount")]
    like_count: Option<u32>,
    #[serde(rename = "replyCount")]
    reply_count: Option<u32>,
    #[serde(rename = "repostCount")]
    repost_count: Option<u32>,
    #[serde(rename = "indexedAt")]
    #[allow(dead_code)]
    indexed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BskyAuthor {
    handle: String,
    #[serde(rename = "displayName")]
    #[allow(dead_code)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BskyRecord {
    text: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
}

// ============================================================================
// Bluesky Source
// ============================================================================

/// Default search queries for developer content
const DEFAULT_QUERIES: &[&str] = &[
    "rust programming",
    "typescript",
    "react framework",
    "open source security",
    "developer tools",
    "machine learning",
    "web development",
];

/// Bluesky source — fetches developer posts from the AT Protocol network
pub struct BlueskySource {
    config: SourceConfig,
    client: reqwest::Client,
    queries: Vec<String>,
}

impl BlueskySource {
    /// Create a new Bluesky source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 25,
                fetch_interval_secs: 1800, // 30 minutes
                custom: None,
            },
            client: super::shared_client(),
            queries: DEFAULT_QUERIES.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Extract the rkey from an AT Protocol URI
    /// Format: at://did:plc:xxx/app.bsky.feed.post/rkey
    fn extract_rkey(uri: &str) -> Option<&str> {
        uri.rsplit('/').next()
    }

    /// Construct a web URL from author handle and post URI
    fn post_url(handle: &str, uri: &str) -> String {
        match Self::extract_rkey(uri) {
            Some(rkey) => format!("https://bsky.app/profile/{}/post/{}", handle, rkey),
            None => format!("https://bsky.app/profile/{}", handle),
        }
    }

    /// Truncate text to a maximum character length at a word boundary
    fn truncate_title(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }
        // Find last space before limit
        match text[..max_chars].rfind(' ') {
            Some(pos) => format!("{}...", &text[..pos]),
            None => format!("{}...", &text[..max_chars]),
        }
    }

    /// Fetch posts from Bluesky's "What's Hot" feed generator.
    /// The search API requires auth, so we use the public feed endpoint instead.
    async fn fetch_query(&self, _query: &str) -> SourceResult<Vec<SourceItem>> {
        // Use the "What's Hot" feed generator which is public and doesn't need auth
        let url = "https://public.api.bsky.app/xrpc/app.bsky.feed.getFeed?feed=at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot&limit=25".to_string();

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Bluesky rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Bluesky forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Bluesky API error: HTTP {}",
                status.as_u16()
            )));
        }

        let bsky_resp: BskySearchResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        // Handle both search (posts field) and feed (feed field) response formats
        let posts: Vec<BskyPost> = if let Some(feed_items) = bsky_resp.feed {
            feed_items.into_iter().map(|fi| fi.post).collect()
        } else {
            bsky_resp.posts.unwrap_or_default()
        };

        let items: Vec<SourceItem> = posts
            .into_iter()
            .filter_map(|post| {
                let text = post.record.text.as_deref().unwrap_or("").to_string();
                if text.is_empty() {
                    return None;
                }

                let title = Self::truncate_title(&text, 120);
                let url = Self::post_url(&post.author.handle, &post.uri);

                let mut metadata = serde_json::json!({
                    "author_handle": post.author.handle,
                });

                if let Some(likes) = post.like_count {
                    metadata["like_count"] = serde_json::json!(likes);
                }
                if let Some(replies) = post.reply_count {
                    metadata["reply_count"] = serde_json::json!(replies);
                }
                if let Some(reposts) = post.repost_count {
                    metadata["repost_count"] = serde_json::json!(reposts);
                }
                if let Some(created) = &post.record.created_at {
                    metadata["created_at"] = serde_json::json!(created);
                }

                // Use the AT URI as source_id (globally unique)
                Some(
                    SourceItem::new("bluesky", &post.uri, &title)
                        .with_url(Some(url))
                        .with_content(text)
                        .with_metadata(metadata),
                )
            })
            .collect();

        Ok(items)
    }
}

impl Default for BlueskySource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for BlueskySource {
    fn source_type(&self) -> &'static str {
        "bluesky"
    }

    fn name(&self) -> &'static str {
        "Bluesky"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Social,
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "Bsky",
            color_hint: "blue",
            min_title_words: 4,
            require_user_language: true,
            require_dev_relevance: true, // "What's Hot" is general audience — must filter
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Bluesky developer posts");

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for (i, query) in self.queries.iter().enumerate() {
            // 1-second delay between query requests (skip first)
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            match self.fetch_query(query).await {
                Ok(items) => {
                    info!(query = %query, count = items.len(), "Fetched Bluesky posts");

                    for item in items {
                        if seen_ids.insert(item.source_id.clone()) {
                            all_items.push(item);
                        }
                    }
                }
                Err(e) => {
                    warn!(query = %query, error = ?e, "Failed to fetch Bluesky posts for query");
                }
            }
        }

        // Respect max_items limit
        all_items.truncate(self.config.max_items);

        info!(total = all_items.len(), "Total Bluesky items fetched");
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Bluesky posts already have full text from the API
        Ok(item.content.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluesky_source_creation() {
        let source = BlueskySource::new();
        assert_eq!(source.source_type(), "bluesky");
        assert_eq!(source.name(), "Bluesky");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 25);
        assert_eq!(source.config().fetch_interval_secs, 1800);
        assert_eq!(source.queries.len(), 7);
    }

    #[test]
    fn test_bluesky_source_default() {
        let source = BlueskySource::default();
        assert_eq!(source.source_type(), "bluesky");
    }

    #[test]
    fn test_bluesky_url_construction() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789";
        let url = BlueskySource::post_url("alice.bsky.social", uri);
        assert_eq!(
            url,
            "https://bsky.app/profile/alice.bsky.social/post/xyz789"
        );
    }

    #[test]
    fn test_bluesky_rkey_extraction() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/3kqrs7abc";
        assert_eq!(BlueskySource::extract_rkey(uri), Some("3kqrs7abc"));
    }

    #[test]
    fn test_bluesky_title_truncation() {
        let short = "Short post";
        assert_eq!(BlueskySource::truncate_title(short, 120), "Short post");

        let long = "This is a very long post that exceeds the maximum character limit and should be truncated at a word boundary to keep things tidy";
        let truncated = BlueskySource::truncate_title(long, 80);
        assert!(truncated.len() <= 83); // 80 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_bluesky_json_parsing() {
        let json = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:abc/app.bsky.feed.post/xyz",
                    "cid": "bafyabc",
                    "author": {
                        "handle": "dev.bsky.social",
                        "displayName": "Dev Person"
                    },
                    "record": {
                        "text": "Just shipped a new Rust crate for async error handling!",
                        "createdAt": "2026-03-15T10:30:00.000Z"
                    },
                    "likeCount": 42,
                    "replyCount": 5,
                    "repostCount": 12,
                    "indexedAt": "2026-03-15T10:30:01.000Z"
                }
            ],
            "cursor": "next_page"
        }"#;

        let resp: BskySearchResponse = serde_json::from_str(json).unwrap();
        let posts = resp.posts.unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].uri, "at://did:plc:abc/app.bsky.feed.post/xyz");
        assert_eq!(posts[0].author.handle, "dev.bsky.social");
        assert_eq!(
            posts[0].record.text.as_deref(),
            Some("Just shipped a new Rust crate for async error handling!")
        );
        assert_eq!(posts[0].like_count, Some(42));
        assert_eq!(posts[0].reply_count, Some(5));
        assert_eq!(posts[0].repost_count, Some(12));
    }
}
