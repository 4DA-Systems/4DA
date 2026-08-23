// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Bluesky/AT Protocol source implementation
//!
//! Fetches developer-relevant posts from Bluesky's public search API.
//! No auth required for public API access.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Bluesky API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct BskySearchResponse {
    posts: Option<Vec<BskyPost>>,
    // getFeed returns {feed: [{post: BskyPost}]}
    feed: Option<Vec<BskyFeedItem>>,
}

#[derive(Debug, Deserialize)]
struct BskyFeedItem {
    post: BskyPost,
}

#[derive(Debug, Deserialize)]
struct BskyPost {
    uri: String,
    author: BskyAuthor,
    record: BskyRecord,
    #[serde(rename = "likeCount")]
    like_count: Option<u32>,
    #[serde(rename = "replyCount")]
    reply_count: Option<u32>,
    #[serde(rename = "repostCount")]
    repost_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct BskyAuthor {
    handle: String,
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

/// Bluesky source — fetches developer posts from the AT Protocol network
pub struct BlueskySource {
    config: SourceConfig,
    client: reqwest::Client,
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
    fn truncate_title(text: &str, max_bytes: usize) -> String {
        if text.len() <= max_bytes {
            return text.to_string();
        }
        let boundary = text.floor_char_boundary(max_bytes);
        match text[..boundary].rfind(' ') {
            Some(pos) => format!("{}...", &text[..pos]),
            None => format!("{}...", &text[..boundary]),
        }
    }

    /// Fetch posts from Bluesky's public "What's Hot" feed generator.
    ///
    /// SINGLE fetch by design: `app.bsky.feed.searchPosts` requires an authenticated
    /// AT Protocol session, and 4DA is BYOK/no-account for social reads — so there is
    /// no query to vary. The feed generator is the only credential-free surface that
    /// returns developer-adjacent posts, and it takes no search term. Anything
    /// stack-shaping wants to do here has to wait for an auth story; per-query
    /// plumbing without it just issues the same request N times.
    async fn fetch_feed(&self) -> SourceResult<Vec<SourceItem>> {
        let url = "https://public.api.bsky.app/xrpc/app.bsky.feed.getFeed?feed=at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot&limit=25";

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        super::classify_http_status(response.status(), "Bluesky API")?;

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

        let items: Vec<SourceItem> = posts.into_iter().filter_map(Self::post_to_item).collect();

        Ok(items)
    }

    /// Convert one API post into a `SourceItem`. Returns `None` for text-less posts.
    fn post_to_item(post: BskyPost) -> Option<SourceItem> {
        let text = post.record.text.as_deref().unwrap_or("").to_string();
        if text.is_empty() {
            return None;
        }

        let title = Self::truncate_title(&text, 120);
        let url = Self::post_url(&post.author.handle, &post.uri);

        let mut metadata = serde_json::json!({
            "author_handle": post.author.handle,
            "source_name": "bluesky",
        });

        // Engagement contract (scoring::pipeline_v2::extract_community_signal):
        // the community-signal reader consumes "likes" for bluesky, so the API's
        // likeCount is written under that key (the legacy "like_count" spelling
        // is kept alongside for existing readers). Written ONLY when the API
        // returned a count — "likes": 0 means MEASURED zero engagement
        // (metadata present), while an absent likeCount writes no key at all
        // (engagement unknown). The two must stay distinguishable.
        if let Some(likes) = post.like_count {
            metadata["likes"] = serde_json::json!(likes);
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

        // Propagate the error rather than reporting `Ok(vec![])` — with one fetch there
        // is nothing to partially succeed, and an honest failure lets the retry/health
        // layer see it instead of recording a silent empty cycle.
        let mut all_items = self.fetch_feed().await?;

        // Respect max_items limit
        all_items.truncate(self.config.max_items);

        info!(total = all_items.len(), "Total Bluesky items fetched");
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
    fn test_bluesky_source_creation() {
        let source = BlueskySource::new();
        assert_eq!(source.source_type(), "bluesky");
        assert_eq!(source.name(), "Bluesky");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 25);
        assert_eq!(source.config().fetch_interval_secs, 1800);
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

    /// Test fixture: a post with controllable engagement counts.
    fn bsky_post(text: &str, likes: Option<u32>) -> BskyPost {
        BskyPost {
            uri: "at://did:plc:abc/app.bsky.feed.post/xyz".to_string(),
            author: BskyAuthor {
                handle: "dev.bsky.social".to_string(),
            },
            record: BskyRecord {
                text: Some(text.to_string()),
                created_at: None,
            },
            like_count: likes,
            reply_count: None,
            repost_count: None,
        }
    }

    #[test]
    fn test_bluesky_engagement_flows_to_metadata() {
        // likeCount lands under "likes" — the key the community-signal reader
        // consumes — with the legacy "like_count" spelling kept alongside.
        let item = BlueskySource::post_to_item(bsky_post("Shipped a new Rust crate!", Some(42)))
            .expect("text post converts");
        let md = item.metadata.as_ref().unwrap();
        assert_eq!(md.get("likes").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(md.get("like_count").and_then(|v| v.as_i64()), Some(42));
    }

    #[test]
    fn test_bluesky_zero_engagement_distinct_from_missing() {
        // likeCount: 0 is MEASURED zero engagement — the key is present with
        // value 0. The scoring pipeline's UGC cliff fires on NO metadata, so
        // measured zero must stay distinguishable from an absent count.
        let zero = BlueskySource::post_to_item(bsky_post("A post with zero likes yet", Some(0)))
            .expect("text post converts");
        let md = zero.metadata.as_ref().unwrap();
        assert_eq!(md.get("likes").and_then(|v| v.as_i64()), Some(0));

        // No likeCount at all → engagement UNKNOWN → no engagement keys.
        let unknown = BlueskySource::post_to_item(bsky_post("A post with no counts", None))
            .expect("text post converts");
        let md = unknown.metadata.as_ref().unwrap();
        assert!(md.get("likes").is_none(), "missing count must not fake 0");
        assert!(md.get("like_count").is_none());
    }

    #[test]
    fn test_bluesky_textless_post_dropped() {
        let mut post = bsky_post("", Some(9));
        post.record.text = None;
        assert!(BlueskySource::post_to_item(post).is_none());
    }
}
