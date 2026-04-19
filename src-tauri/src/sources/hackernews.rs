// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Hacker News source implementation
//!
//! Fetches top stories from Hacker News API and scrapes article content.

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// HN API Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields deserialized from HN API JSON
struct HNStory {
    id: u64,
    title: Option<String>,
    url: Option<String>,
    text: Option<String>, // For Ask HN / Show HN posts
    score: Option<i32>,
    by: Option<String>,
}

// ============================================================================
// Hacker News Source
// ============================================================================

/// Hacker News source - fetches top stories and scrapes article content
pub struct HackerNewsSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl HackerNewsSource {
    /// Create a new HN source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 100, // Increased from 30 for better first-run experience
                fetch_interval_secs: 300,
                custom: None,
            },
            client: super::shared_client(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: SourceConfig) -> Self {
        Self {
            config,
            client: super::shared_client(),
        }
    }
}

impl Default for HackerNewsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl HackerNewsSource {
    /// Fetch story details by IDs (helper method)
    /// Uses buffer_unordered(5) for parallel fetching — reqwest::Client is Arc-backed.
    async fn fetch_stories_by_ids(
        &self,
        ids: Vec<u64>,
        max_items: usize,
    ) -> SourceResult<Vec<SourceItem>> {
        let owned_ids: Vec<u64> = ids.into_iter().take(max_items).collect();
        let items: Vec<SourceItem> = stream::iter(owned_ids)
            .map(|id| {
                let client = self.client.clone();
                async move {
                    // Per-request rate limiting — respects HN Firebase interval
                    super::rate_limiter::rate_limiter()
                        .wait_for_rate_limit("hackernews")
                        .await;

                    let url = format!("https://hacker-news.firebaseio.com/v0/item/{id}.json");
                    match client.get(&url).send().await {
                        Ok(response) => match response.json::<HNStory>().await {
                            Ok(story) => {
                                let title = story.title.unwrap_or_else(|| "[No title]".to_string());
                                let content = story.text.unwrap_or_default();

                                let mut item =
                                    SourceItem::new("hackernews", &id.to_string(), &title)
                                        .with_url(story.url.clone())
                                        .with_content(content);

                                if let (Some(score), Some(by)) = (story.score, story.by) {
                                    item = item.with_metadata(serde_json::json!({
                                        "score": score,
                                        "author": by,
                                    }));
                                }

                                Some(item)
                            }
                            Err(e) => {
                                warn!(story_id = id, error = %e, "Failed to parse story");
                                None
                            }
                        },
                        Err(e) => {
                            warn!(story_id = id, error = %e, "Failed to fetch story");
                            None
                        }
                    }
                }
            })
            // Limit concurrent HN API requests to avoid rate limiting
            .buffer_unordered(5)
            .filter_map(|opt| async { opt })
            .collect()
            .await;

        info!(count = items.len(), "Fetched stories");
        Ok(items)
    }
}

#[async_trait]
impl Source for HackerNewsSource {
    fn source_type(&self) -> &'static str {
        "hackernews"
    }

    fn name(&self) -> &'static str {
        "Hacker News"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::News,
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "HN",
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

        info!("Fetching top stories");

        let response = self
            .client
            .get("https://hacker-news.firebaseio.com/v0/topstories.json")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Hacker News rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Hacker News forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Hacker News API error: HTTP {}",
                status.as_u16()
            )));
        }

        let top_ids: Vec<u64> = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        info!(
            total = top_ids.len(),
            max_items = self.config.max_items,
            "Got story IDs, fetching top items"
        );

        self.fetch_stories_by_ids(top_ids, self.config.max_items)
            .await
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!(items_per_category, "Deep fetching from all HN categories");

        let endpoints = [
            (
                "topstories",
                "https://hacker-news.firebaseio.com/v0/topstories.json",
            ),
            (
                "newstories",
                "https://hacker-news.firebaseio.com/v0/newstories.json",
            ),
            (
                "beststories",
                "https://hacker-news.firebaseio.com/v0/beststories.json",
            ),
            (
                "askstories",
                "https://hacker-news.firebaseio.com/v0/askstories.json",
            ),
            (
                "showstories",
                "https://hacker-news.firebaseio.com/v0/showstories.json",
            ),
        ];

        let mut all_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut ordered_ids: Vec<u64> = Vec::new();

        for (category, url) in endpoints {
            match self.client.get(url).send().await {
                Ok(response) => match response.json::<Vec<u64>>().await {
                    Ok(ids) => {
                        let new_count = ids
                            .iter()
                            .take(items_per_category)
                            .filter(|id| all_ids.insert(**id))
                            .count();

                        for id in ids.into_iter().take(items_per_category) {
                            if !ordered_ids.contains(&id) {
                                ordered_ids.push(id);
                            }
                        }

                        info!(
                            category,
                            new = new_count,
                            total = all_ids.len(),
                            "Fetched category IDs"
                        );
                    }
                    Err(e) => {
                        warn!(category, error = %e, "Failed to parse category IDs");
                    }
                },
                Err(e) => {
                    warn!(category, error = %e, "Failed to fetch category");
                }
            }
        }

        info!(
            unique_ids = all_ids.len(),
            "Fetching unique stories from all categories"
        );
        let len = ordered_ids.len();
        self.fetch_stories_by_ids(ordered_ids, len).await
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // If item already has content (Ask HN / Show HN), return it
        if !item.content.is_empty() {
            return Ok(item.content.clone());
        }

        // If no URL, nothing to scrape
        let url = match &item.url {
            Some(u) => u,
            None => return Ok(String::new()),
        };

        // Skip non-HTTP URLs
        if !url.starts_with("http") {
            return Ok(String::new());
        }

        // Use shared client with per-request timeout for scraping
        let client = super::shared_client();

        // Fetch the page
        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            warn!(target: "4da::sources", url = %url, status = %status, "Scrape failed — returning empty content");
            return Ok(String::new());
        }

        let html = response
            .text()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        // Extract content using multiple selectors
        let document = Html::parse_document(&html);
        let selectors = [
            "article",
            "main",
            "[role='main']",
            ".post-content",
            ".article-content",
            ".entry-content",
            ".content",
            "body",
        ];

        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text: String = element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");

                    // Only use if we got meaningful content
                    if text.len() > 100 {
                        // Truncate to reasonable length
                        let max_len = 5000;
                        let truncated = if text.len() > max_len {
                            text.chars().take(max_len).collect()
                        } else {
                            text
                        };
                        return Ok(truncated);
                    }
                }
            }
        }

        Ok(String::new())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hn_source_defaults() {
        let source = HackerNewsSource::new();
        assert_eq!(source.source_type(), "hackernews");
        assert_eq!(source.name(), "Hacker News");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 100);
    }
}
