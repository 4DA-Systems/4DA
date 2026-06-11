// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Reddit source implementation.
//!
//! Reddit is the canonical case for 4DA's source-resilience model (see [`super::access`]): its
//! official `.json` API is now aggressively throttled (requests hang to a 90s timeout), while its
//! `.rss` endpoint still answers HTTP 200. So Reddit declares an ordered list of access strategies —
//! `reddit:json` preferred, `reddit:rss` as the credential-free fallback — and [`resilient_fetch`]
//! routes around whichever path is currently walled off. A future `reddit:oauth` strategy (gated on a
//! user-supplied credential) slots in as depth without changing this shape.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::access::{resilient_fetch, AccessStrategy};
use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Reddit JSON API types
// ============================================================================

#[derive(Debug, Deserialize)]
struct RedditListing {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    children: Vec<RedditChild>,
}

#[derive(Debug, Deserialize)]
struct RedditChild {
    data: RedditPost,
}

#[derive(Debug, Deserialize)]
struct RedditPost {
    id: String,
    title: String,
    selftext: Option<String>,
    url: Option<String>,
    permalink: String,
    score: i32,
    author: String,
    subreddit: String,
    num_comments: i32,
    is_self: bool,
}

const REDDIT_USER_AGENT: &str = "4DA:com.4da.app:1.0 (by /u/4da-desktop)";

// ============================================================================
// Per-subreddit fetchers (one per access path)
// ============================================================================

/// Fetch a subreddit via the official JSON API (`/r/<sub>/hot.json`).
async fn fetch_subreddit_json(
    client: &reqwest::Client,
    subreddit: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!("https://www.reddit.com/r/{subreddit}/hot.json?limit={limit}");

    let response = client
        .get(&url)
        .header("User-Agent", REDDIT_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Reddit rate limited (HTTP 429)".to_string(),
        ));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(SourceError::Forbidden(
            "Reddit forbidden (HTTP 403)".to_string(),
        ));
    }
    super::check_http_status(status, "Reddit API")?;

    let listing: RedditListing = response
        .json()
        .await
        .map_err(|e| SourceError::Parse(e.to_string()))?;

    let items = listing
        .data
        .children
        .into_iter()
        .map(|child| {
            let post = child.data;
            let content = post.selftext.unwrap_or_default();
            let url = if post.is_self {
                format!("https://reddit.com{}", post.permalink)
            } else {
                post.url
                    .unwrap_or_else(|| format!("https://reddit.com{}", post.permalink))
            };
            SourceItem::new("reddit", &post.id, &post.title)
                .with_url(Some(url))
                .with_content(content)
                .with_metadata(serde_json::json!({
                    "score": post.score,
                    "author": post.author,
                    "subreddit": post.subreddit,
                    "comments": post.num_comments,
                    "is_self": post.is_self,
                    "via": "json",
                }))
        })
        .collect();

    Ok(items)
}

/// Fetch a subreddit via its Atom feed (`/r/<sub>/.rss`) — the credential-free fallback that survives
/// when the JSON API is throttled. Reddit serves Atom; we extract one item per `<entry>`.
async fn fetch_subreddit_rss(
    client: &reqwest::Client,
    subreddit: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!("https://www.reddit.com/r/{subreddit}/.rss?limit={limit}");

    let response = client
        .get(&url)
        .header("User-Agent", REDDIT_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Reddit RSS rate limited (HTTP 429)".to_string(),
        ));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(SourceError::Forbidden(
            "Reddit RSS forbidden (HTTP 403)".to_string(),
        ));
    }
    super::check_http_status(status, "Reddit RSS")?;

    let body = response
        .text()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    Ok(parse_reddit_atom(&body, subreddit, limit))
}

/// Parse Reddit's Atom feed into `SourceItem`s. Reused via the shared `super::extract_tag` for simple
/// tags; the `<link>` href is an attribute, so it gets a dedicated extractor.
fn parse_reddit_atom(xml: &str, subreddit: &str, limit: usize) -> Vec<SourceItem> {
    xml.split("<entry>")
        .skip(1)
        .take(limit)
        .filter_map(|block| {
            let entry = &block[..block.find("</entry>").unwrap_or(block.len())];
            let title = super::extract_tag(entry, "title")?;
            // Reddit ids look like "t3_abc123"; strip the kind prefix to match the JSON path's id so
            // the two strategies dedup against each other.
            let id = super::extract_tag(entry, "id")
                .map(|raw| raw.rsplit('_').next().unwrap_or(&raw).to_string())
                .unwrap_or_else(|| title.clone());
            let link = extract_link_href(entry)
                .unwrap_or_else(|| format!("https://www.reddit.com/r/{subreddit}/"));
            let content = super::extract_tag(entry, "content").unwrap_or_default();
            Some(
                SourceItem::new("reddit", &id, &title)
                    .with_url(Some(link))
                    .with_content(content)
                    .with_metadata(serde_json::json!({
                        "subreddit": subreddit,
                        // RSS gives no score; mark self so scrape_content keeps the feed content
                        // rather than trying to scrape the reddit comments page.
                        "is_self": true,
                        "via": "rss",
                    })),
            )
        })
        .collect()
}

/// Extract the `href` of the first `<link ... href="...">` in an Atom entry.
fn extract_link_href(entry: &str) -> Option<String> {
    let link_start = entry.find("<link")?;
    let rest = &entry[link_start..];
    let href_start = rest.find("href=\"")? + 6;
    let href = &rest[href_start..];
    let href_end = href.find('"')?;
    Some(href[..href_end].to_string())
}

/// Aggregate per-subreddit results into one ranked batch. Returns `Ok` if ANY subreddit produced
/// items; if every subreddit failed, bubbles the most actionable error so [`resilient_fetch`] can try
/// the next access strategy. A reachable-but-empty set (all `Ok`, no items) returns `Ok(empty)`.
fn aggregate(
    results: Vec<(&str, SourceResult<Vec<SourceItem>>)>,
    max_items: usize,
) -> SourceResult<Vec<SourceItem>> {
    let mut all = Vec::new();
    let mut errors = Vec::new();
    let mut any_ok = false;

    for (sub, res) in results {
        match res {
            Ok(items) => {
                any_ok = true;
                all.extend(items);
            }
            Err(e) => {
                match &e {
                    SourceError::Forbidden(_) | SourceError::RateLimited(_) => {
                        debug!(subreddit = sub, error = %e, "Skipped subreddit (auth/rate-limit)");
                    }
                    _ => warn!(subreddit = sub, error = %e, "Failed to fetch subreddit"),
                }
                errors.push(e);
            }
        }
    }

    if all.is_empty() {
        if errors.is_empty() && any_ok {
            return Ok(Vec::new()); // reached every subreddit, genuinely nothing right now
        }
        return Err(errors
            .into_iter()
            .max_by_key(super::access::actionability)
            .unwrap_or_else(|| {
                SourceError::Other("reddit: no subreddits configured".to_string())
            }));
    }

    all.sort_by(|a, b| item_score(b).cmp(&item_score(a)));
    all.truncate(max_items);
    Ok(all)
}

fn item_score(item: &SourceItem) -> i64 {
    item.metadata
        .as_ref()
        .and_then(|m| m.get("score"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0)
}

// ============================================================================
// Access strategies
// ============================================================================

struct RedditJsonStrategy {
    client: reqwest::Client,
    subreddits: Vec<&'static str>,
    max_items: usize,
}

struct RedditRssStrategy {
    client: reqwest::Client,
    subreddits: Vec<&'static str>,
    max_items: usize,
}

#[async_trait]
impl AccessStrategy for RedditJsonStrategy {
    fn label(&self) -> &str {
        "reddit:json"
    }

    async fn fetch(&self) -> SourceResult<Vec<SourceItem>> {
        let per_sub = (self.max_items / self.subreddits.len().max(1)).max(3);
        let mut results = Vec::with_capacity(self.subreddits.len());
        for sub in &self.subreddits {
            results.push((*sub, fetch_subreddit_json(&self.client, sub, per_sub).await));
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        aggregate(results, self.max_items)
    }
}

#[async_trait]
impl AccessStrategy for RedditRssStrategy {
    fn label(&self) -> &str {
        "reddit:rss"
    }

    async fn fetch(&self) -> SourceResult<Vec<SourceItem>> {
        let per_sub = (self.max_items / self.subreddits.len().max(1)).max(3);
        let mut results = Vec::with_capacity(self.subreddits.len());
        for sub in &self.subreddits {
            results.push((*sub, fetch_subreddit_rss(&self.client, sub, per_sub).await));
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        aggregate(results, self.max_items)
    }
}

/// Build the ordered access-strategy list for a subreddit set: JSON first (richest), RSS fallback.
fn reddit_strategies(
    client: &reqwest::Client,
    subreddits: Vec<&'static str>,
    max_items: usize,
) -> Vec<Box<dyn AccessStrategy>> {
    vec![
        Box::new(RedditJsonStrategy {
            client: client.clone(),
            subreddits: subreddits.clone(),
            max_items,
        }),
        Box::new(RedditRssStrategy {
            client: client.clone(),
            subreddits,
            max_items,
        }),
    ]
}

// ============================================================================
// Reddit Source
// ============================================================================

/// Reddit source — fetches top posts from tech subreddits via a resilient access-strategy list.
pub struct RedditSource {
    config: SourceConfig,
    client: reqwest::Client,
    subreddits: Vec<&'static str>,
}

impl RedditSource {
    /// Create a new Reddit source with default config.
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 600, // 10 minutes
                custom: None,
            },
            client: super::shared_client(),
            subreddits: vec![
                "programming",
                "technology",
                "machinelearning",
                "rust",
                "typescript",
                "webdev",
                "datascience",
            ],
        }
    }
}

impl Default for RedditSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for RedditSource {
    fn source_type(&self) -> &'static str {
        "reddit"
    }

    fn name(&self) -> &'static str {
        "Reddit"
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
            label: "Reddit",
            color_hint: "blue",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        let strategies =
            reddit_strategies(&self.client, self.subreddits.clone(), self.config.max_items);
        resilient_fetch("reddit", &strategies).await
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        let deep_subreddits: Vec<&'static str> = vec![
            "programming",
            "rust",
            "golang",
            "python",
            "typescript",
            "javascript",
            "java",
            "cpp",
            "csharp",
            "swift",
            "kotlin",
            "webdev",
            "frontend",
            "reactjs",
            "node",
            "nextjs",
            "svelte",
            "learnprogramming",
            "cscareerquestions",
            "machinelearning",
            "deeplearning",
            "LanguageTechnology",
            "artificial",
            "LocalLLaMA",
            "ChatGPT",
            "datascience",
            "dataengineering",
            "datasets",
            "devops",
            "kubernetes",
            "docker",
            "aws",
            "selfhosted",
            "homelab",
            "linux",
            "sysadmin",
            "netsec",
            "cybersecurity",
            "technology",
            "startups",
            "SideProject",
            "opensource",
            "tauri",
            "electronjs",
            "flutter",
            "php",
        ];

        info!(
            subreddit_count = deep_subreddits.len(),
            items_per = items_per_category,
            "Deep fetching Reddit"
        );
        // Multiplier of 15 gives ~1500 max items, ~36 per subreddit for comprehensive coverage.
        let strategies = reddit_strategies(&self.client, deep_subreddits, items_per_category * 15);
        resilient_fetch("reddit", &strategies).await
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Self posts (and RSS-sourced items) already carry content.
        if !item.content.is_empty() {
            return Ok(item.content.clone());
        }

        let is_self = item
            .metadata
            .as_ref()
            .and_then(|m| m.get("is_self"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);

        if is_self {
            return Ok(String::new());
        }

        let url = match &item.url {
            Some(u) => u,
            None => return Ok(String::new()),
        };

        // Reddit-internal URLs have no article body to scrape.
        if url.contains("reddit.com") || url.contains("redd.it") {
            return Ok(String::new());
        }
        if !url.starts_with("http") {
            return Ok(String::new());
        }

        info!(url = %url, "Scraping linked article for Reddit post");

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
                info!(url = %url, length = truncated.len(), "Scraped linked article content");
                Ok(truncated)
            }
            Ok(None) => {
                warn!(url = %url, "Failed to extract content from linked article");
                Ok(String::new())
            }
            Err(_) => {
                warn!(url = %url, "Timed out scraping linked article (2s limit)");
                Ok(String::new())
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
    fn test_reddit_source_defaults() {
        let source = RedditSource::new();
        assert_eq!(source.source_type(), "reddit");
        assert_eq!(source.name(), "Reddit");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
    }

    #[test]
    fn parses_reddit_atom_entries() {
        // Minimal shape of Reddit's /.rss Atom feed.
        let xml = r#"<feed>
            <entry>
              <author><name>/u/dev</name></author>
              <content type="html">&lt;p&gt;body&lt;/p&gt;</content>
              <id>t3_abc123</id>
              <link href="https://www.reddit.com/r/rust/comments/abc123/title/" />
              <title>A Rust post</title>
            </entry>
            <entry>
              <id>t3_def456</id>
              <link href="https://www.reddit.com/r/rust/comments/def456/other/" />
              <title>Another post</title>
            </entry>
        </feed>"#;

        let items = parse_reddit_atom(xml, "rust", 30);
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].source_id, "abc123",
            "id should strip the t3_ kind prefix"
        );
        assert_eq!(items[0].title, "A Rust post");
        assert_eq!(
            items[0].url.as_deref(),
            Some("https://www.reddit.com/r/rust/comments/abc123/title/")
        );
        assert!(items[1].content.is_empty(), "missing content tolerated");
    }

    #[test]
    fn reddit_atom_respects_limit() {
        let entry = "<entry><id>t3_x</id><title>t</title><link href=\"http://x\" /></entry>";
        let xml = format!("<feed>{}</feed>", entry.repeat(10));
        assert_eq!(parse_reddit_atom(&xml, "rust", 3).len(), 3);
    }

    #[test]
    fn extract_link_href_reads_attribute() {
        assert_eq!(
            extract_link_href("<link href=\"https://example.com/x\" />").as_deref(),
            Some("https://example.com/x")
        );
        assert_eq!(extract_link_href("<title>no link</title>"), None);
    }
}
