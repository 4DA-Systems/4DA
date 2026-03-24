//! Reddit source implementation
//!
//! Fetches top posts from tech-related subreddits using Reddit's JSON API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Reddit API Types
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

// ============================================================================
// Reddit Source
// ============================================================================

/// Reddit source - fetches top posts from tech subreddits
pub struct RedditSource {
    config: SourceConfig,
    client: reqwest::Client,
    subreddits: Vec<&'static str>,
}

impl RedditSource {
    /// Create a new Reddit source with default config
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

    /// Fetch posts from a single subreddit
    async fn fetch_subreddit(
        &self,
        subreddit: &str,
        limit: usize,
    ) -> SourceResult<Vec<SourceItem>> {
        let url = format!("https://www.reddit.com/r/{subreddit}/hot.json?limit={limit}");

        let response = self
            .client
            .get(&url)
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
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Reddit API error: HTTP {}",
                status.as_u16()
            )));
        }

        let listing: RedditListing = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let items: Vec<SourceItem> = listing
            .data
            .children
            .into_iter()
            .map(|child| {
                let post = child.data;
                let content = post.selftext.unwrap_or_default();

                // For link posts, use the linked URL; for self posts, use Reddit URL
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
                    }))
            })
            .collect();

        Ok(items)
    }
}

impl Default for RedditSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RedditSource {
    /// Helper to fetch from specified subreddits
    async fn fetch_from_subreddits(
        &self,
        subreddits: &[&'static str],
        max_items: usize,
    ) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!(count = subreddits.len(), "Fetching from subreddits");

        let mut all_items = Vec::new();
        let items_per_sub = (max_items / subreddits.len()).max(3);

        for subreddit in subreddits {
            match self.fetch_subreddit(subreddit, items_per_sub).await {
                Ok(items) => {
                    info!(count = items.len(), subreddit, "Got posts from subreddit");
                    all_items.extend(items);
                }
                Err(e) => {
                    warn!(subreddit, error = ?e, "Failed to fetch subreddit");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        all_items.sort_by(|a, b| {
            let score_a = a
                .metadata
                .as_ref()
                .and_then(|m| m.get("score"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let score_b = b
                .metadata
                .as_ref()
                .and_then(|m| m.get("score"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            score_b.cmp(&score_a)
        });

        all_items.truncate(max_items);
        info!(count = all_items.len(), "Total posts across all subreddits");
        Ok(all_items)
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

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        self.fetch_from_subreddits(&self.subreddits, self.config.max_items)
            .await
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
            "ClaudeAI",
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
        ];

        info!(
            subreddit_count = deep_subreddits.len(),
            items_per = items_per_category,
            "Deep fetching Reddit"
        );
        // Multiplier of 15 gives ~1500 max items, ~36 per subreddit for comprehensive coverage
        self.fetch_from_subreddits(&deep_subreddits, items_per_category * 15)
            .await
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Self posts already have content
        if !item.content.is_empty() {
            return Ok(item.content.clone());
        }

        // For link posts, scrape the linked article content
        let is_self = item
            .metadata
            .as_ref()
            .and_then(|m| m.get("is_self"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);

        if is_self {
            return Ok(String::new());
        }

        // Get the link URL
        let url = match &item.url {
            Some(u) => u,
            None => return Ok(String::new()),
        };

        // Skip reddit internal URLs - they don't have article content to scrape
        if url.contains("reddit.com") || url.contains("redd.it") {
            return Ok(String::new());
        }

        // Skip non-HTTP URLs
        if !url.starts_with("http") {
            return Ok(String::new());
        }

        info!(url = %url, "Scraping linked article for Reddit post");

        // Use a 2 second timeout to avoid slowing down the pipeline
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::scrape_article_content(url),
        )
        .await
        {
            Ok(Some(content)) => {
                // Truncate to 5000 chars
                let truncated = if content.len() > 5000 {
                    content.chars().take(5000).collect()
                } else {
                    content
                };
                info!(
                    url = %url,
                    length = truncated.len(),
                    "Scraped linked article content"
                );
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
}
