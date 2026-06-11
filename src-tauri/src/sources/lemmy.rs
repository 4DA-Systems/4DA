// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lemmy source — the open-protocol substitute for Reddit.
//!
//! Lemmy is ActivityPub/federated: thousands of independent, self-hostable instances, no single
//! corporate gatekeeper that can wall it off the way Reddit did. It is the structural hedge in 4DA's
//! source-resilience model (see [`super::access`]): as the walled gardens enclose, the open-protocol
//! networks grow (refugee network effect), and they can't be throttled-to-monetize by one owner.
//!
//! 4DA reads a developer-focused instance (`programming.dev`) whose `All`/`Hot` view *federates* posts
//! from across the network. Lemmy declares the same resilient strategy list as Reddit —
//! `lemmy:api` preferred, `lemmy:rss` fallback — demonstrating the failover pattern on a different
//! trust model, not just a different endpoint.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;

use super::access::{resilient_fetch, AccessStrategy};
use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

/// Developer-focused Lemmy instance; its `All` view federates posts from across the network.
const LEMMY_INSTANCE: &str = "programming.dev";
const LEMMY_USER_AGENT: &str = "4DA/1.0 (+https://4da.ai)";

// ============================================================================
// Lemmy API (v3) types
// ============================================================================

#[derive(Debug, Deserialize)]
struct LemmyPostList {
    posts: Vec<LemmyPostView>,
}

#[derive(Debug, Deserialize)]
struct LemmyPostView {
    post: LemmyPost,
    #[serde(default)]
    creator: Option<LemmyActor>,
    #[serde(default)]
    community: Option<LemmyActor>,
    #[serde(default)]
    counts: Option<LemmyCounts>,
}

#[derive(Debug, Deserialize)]
struct LemmyPost {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    body: Option<String>,
    /// Canonical federated ActivityPub id (stable dedup key across instances).
    ap_id: String,
}

#[derive(Debug, Deserialize)]
struct LemmyActor {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LemmyCounts {
    #[serde(default)]
    score: i64,
    #[serde(default)]
    comments: i64,
}

// ============================================================================
// Access strategies
// ============================================================================

/// Fetch via the Lemmy HTTP API (`/api/v3/post/list`). `type_=All` pulls the federated firehose.
async fn fetch_api(
    client: &reqwest::Client,
    instance: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!(
        "https://{instance}/api/v3/post/list?type_=All&sort=Hot&limit={}",
        limit.clamp(1, 50)
    );
    let response = client
        .get(&url)
        .header("User-Agent", LEMMY_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Lemmy rate limited (HTTP 429)".to_string(),
        ));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(SourceError::Forbidden(
            "Lemmy forbidden (HTTP 403)".to_string(),
        ));
    }
    super::check_http_status(status, "Lemmy API")?;

    let list: LemmyPostList = response
        .json()
        .await
        .map_err(|e| SourceError::Parse(e.to_string()))?;

    Ok(list.posts.into_iter().map(post_view_to_item).collect())
}

fn post_view_to_item(view: LemmyPostView) -> SourceItem {
    let post = view.post;
    // External link if the post links out; otherwise the canonical federated discussion URL.
    let link = post.url.clone().unwrap_or_else(|| post.ap_id.clone());
    let content = post.body.clone().unwrap_or_default();
    SourceItem::new("lemmy", &post.ap_id, &post.name)
        .with_url(Some(link))
        .with_content(content)
        .with_metadata(serde_json::json!({
            "community": view.community.as_ref().map(|c| c.name.as_str()),
            "author": view.creator.as_ref().map(|c| c.name.as_str()),
            "score": view.counts.as_ref().map(|c| c.score).unwrap_or(0),
            "comments": view.counts.as_ref().map(|c| c.comments).unwrap_or(0),
            "is_self": post.url.is_none(),
            "via": "api",
        }))
}

/// Fetch via the instance RSS feed (`/feeds/all.xml`) — the credential-free fallback path.
async fn fetch_rss(
    client: &reqwest::Client,
    instance: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!("https://{instance}/feeds/all.xml?sort=Hot");
    let response = client
        .get(&url)
        .header("User-Agent", LEMMY_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Lemmy RSS rate limited (HTTP 429)".to_string(),
        ));
    }
    super::check_http_status(status, "Lemmy RSS")?;

    let body = response
        .text()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;
    Ok(parse_lemmy_rss(&body, limit))
}

/// Parse a Lemmy RSS 2.0 feed (`<item>` blocks) into `SourceItem`s.
fn parse_lemmy_rss(xml: &str, limit: usize) -> Vec<SourceItem> {
    xml.split("<item>")
        .skip(1)
        .take(limit)
        .filter_map(|block| {
            let item = &block[..block.find("</item>").unwrap_or(block.len())];
            let title = super::extract_tag(item, "title")?;
            let link =
                super::extract_tag(item, "link").or_else(|| super::extract_tag(item, "guid"))?;
            let content = super::extract_tag(item, "description").unwrap_or_default();
            let id = super::extract_tag(item, "guid").unwrap_or_else(|| link.clone());
            Some(
                SourceItem::new("lemmy", &id, &title)
                    .with_url(Some(link))
                    .with_content(content)
                    .with_metadata(serde_json::json!({ "via": "rss", "is_self": false })),
            )
        })
        .collect()
}

#[derive(Clone, Copy)]
enum AccessPath {
    Api,
    Rss,
}

struct LemmyStrategy {
    client: reqwest::Client,
    instance: &'static str,
    max_items: usize,
    path: AccessPath,
}

#[async_trait]
impl AccessStrategy for LemmyStrategy {
    fn label(&self) -> &str {
        match self.path {
            AccessPath::Api => "lemmy:api",
            AccessPath::Rss => "lemmy:rss",
        }
    }

    async fn fetch(&self) -> SourceResult<Vec<SourceItem>> {
        match self.path {
            AccessPath::Api => fetch_api(&self.client, self.instance, self.max_items).await,
            AccessPath::Rss => fetch_rss(&self.client, self.instance, self.max_items).await,
        }
    }
}

// ============================================================================
// Lemmy Source
// ============================================================================

/// Lemmy source — federated open-protocol developer discussion (Reddit substitute).
pub struct LemmySource {
    config: SourceConfig,
    client: reqwest::Client,
    instance: &'static str,
}

impl LemmySource {
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 40,
                fetch_interval_secs: 600,
                custom: None,
            },
            client: super::shared_client(),
            instance: LEMMY_INSTANCE,
        }
    }

    fn strategies(&self) -> Vec<Box<dyn AccessStrategy>> {
        vec![
            Box::new(LemmyStrategy {
                client: self.client.clone(),
                instance: self.instance,
                max_items: self.config.max_items,
                path: AccessPath::Api,
            }),
            Box::new(LemmyStrategy {
                client: self.client.clone(),
                instance: self.instance,
                max_items: self.config.max_items,
                path: AccessPath::Rss,
            }),
        ]
    }
}

impl Default for LemmySource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for LemmySource {
    fn source_type(&self) -> &'static str {
        "lemmy"
    }

    fn name(&self) -> &'static str {
        "Lemmy"
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
            label: "Lemmy",
            color_hint: "green",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        info!(instance = self.instance, "Fetching Lemmy (federated Hot)");
        resilient_fetch("lemmy", &self.strategies()).await
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        // One federated firehose; "deep" just widens the page.
        let mut deep = LemmySource::new();
        deep.config.max_items = (items_per_category * 10).clamp(1, 50);
        resilient_fetch("lemmy", &deep.strategies()).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lemmy_api_json() {
        let json = r#"{
            "posts": [
                {
                    "post": {
                        "name": "A federated post",
                        "url": "https://example.com/article",
                        "body": "some text",
                        "ap_id": "https://lemy.lol/post/123"
                    },
                    "creator": { "name": "dev" },
                    "community": { "name": "programming" },
                    "counts": { "score": 42, "comments": 7 }
                },
                {
                    "post": { "name": "Self post", "ap_id": "https://programming.dev/post/9" }
                }
            ],
            "next_page": "abc"
        }"#;
        let list: LemmyPostList = serde_json::from_str(json).unwrap();
        let items: Vec<_> = list.posts.into_iter().map(post_view_to_item).collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source_id, "https://lemy.lol/post/123");
        assert_eq!(items[0].url.as_deref(), Some("https://example.com/article"));
        // A post with no outbound url is self → links to the federated discussion.
        assert_eq!(
            items[1].url.as_deref(),
            Some("https://programming.dev/post/9")
        );
        let md = items[1].metadata.as_ref().unwrap();
        assert_eq!(md.get("is_self").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn parses_lemmy_rss() {
        let xml = r#"<rss><channel>
            <item><title>RSS post one</title><link>https://programming.dev/post/1</link>
              <description>body one</description><guid>https://programming.dev/post/1</guid></item>
            <item><title>RSS post two</title><link>https://programming.dev/post/2</link></item>
        </channel></rss>"#;
        let items = parse_lemmy_rss(xml, 40);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "RSS post one");
        assert_eq!(items[0].source_id, "https://programming.dev/post/1");
        assert!(items[1].content.is_empty(), "missing description tolerated");
    }

    #[test]
    fn lemmy_source_defaults() {
        let s = LemmySource::new();
        assert_eq!(s.source_type(), "lemmy");
        assert_eq!(s.name(), "Lemmy");
        assert!(s.config().enabled);
        assert_eq!(s.strategies().len(), 2, "api + rss");
    }

    /// LIVE: federated open-protocol fetch actually produces items (no credential, no gatekeeper).
    /// Run: `cargo test --lib sources::lemmy::tests::live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "network: verifies live Lemmy federated fetch"]
    async fn live_lemmy_produces_items() {
        let items = LemmySource::new()
            .fetch_items()
            .await
            .expect("Lemmy (open protocol) should serve federated items credential-free");
        assert!(!items.is_empty(), "expected federated Lemmy items");
        let via = items
            .iter()
            .filter_map(|i| i.metadata.as_ref()?.get("via")?.as_str())
            .next()
            .unwrap_or("unknown");
        println!("LIVE lemmy: {} items via {via}", items.len());
    }
}
