//! Dev.to source implementation
//!
//! Fetches top articles from Dev.to (dev.to) using their public API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Dev.to API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct DevtoArticle {
    id: u64,
    title: String,
    url: String,
    #[serde(default)]
    description: String,
    published_at: Option<String>,
    positive_reactions_count: Option<i32>,
    comments_count: Option<i32>,
    #[serde(default)]
    tag_list: Vec<String>,
    user: Option<DevtoUser>,
    #[serde(default)]
    reading_time_minutes: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct DevtoUser {
    name: String,
    #[serde(default)]
    username: String,
}

// ============================================================================
// Dev.to Source
// ============================================================================

/// Dev.to source - fetches top developer articles from the Dev.to community
pub struct DevtoSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl DevtoSource {
    /// Create a new Dev.to source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 900, // 15 minutes
                custom: None,
            },
            client: super::shared_client(),
        }
    }

    /// Fetch articles from a specific Dev.to API endpoint
    async fn fetch_endpoint(&self, url: &str, max: usize) -> SourceResult<Vec<SourceItem>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Dev.to rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Dev.to forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Dev.to API error: HTTP {}",
                status.as_u16()
            )));
        }

        let articles: Vec<DevtoArticle> = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let items: Vec<SourceItem> = articles
            .into_iter()
            .take(max)
            .map(|article| {
                let content = article.description.clone();

                let mut metadata = serde_json::json!({
                    "tags": article.tag_list,
                });

                if let Some(reactions) = article.positive_reactions_count {
                    metadata["reactions"] = serde_json::json!(reactions);
                }
                if let Some(comments) = article.comments_count {
                    metadata["comments"] = serde_json::json!(comments);
                }
                if let Some(published) = &article.published_at {
                    metadata["published_at"] = serde_json::json!(published);
                }
                if let Some(user) = &article.user {
                    metadata["author"] = serde_json::json!(user.name);
                    metadata["author_username"] = serde_json::json!(user.username);
                }
                if let Some(reading_time) = article.reading_time_minutes {
                    metadata["reading_time_minutes"] = serde_json::json!(reading_time);
                }

                SourceItem::new("devto", &article.id.to_string(), &article.title)
                    .with_url(Some(article.url.clone()))
                    .with_content(content)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(items)
    }
}

impl Default for DevtoSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for DevtoSource {
    fn source_type(&self) -> &'static str {
        "devto"
    }

    fn name(&self) -> &'static str {
        "Dev.to"
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
            label: "Dev.to",
            color_hint: "green",
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Dev.to top articles (last 7 days)");

        let items = self
            .fetch_endpoint(
                "https://dev.to/api/articles?per_page=30&top=7",
                self.config.max_items,
            )
            .await?;

        info!(items = items.len(), "Fetched Dev.to articles");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Deep fetching Dev.to (top + programming + webdev tags)");

        let (top_result, programming_result, webdev_result) = tokio::join!(
            self.fetch_endpoint("https://dev.to/api/articles?per_page=30&top=7", 30),
            self.fetch_endpoint(
                "https://dev.to/api/articles?per_page=30&tag=programming",
                30
            ),
            self.fetch_endpoint("https://dev.to/api/articles?per_page=30&tag=webdev", 30),
        );

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        match top_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Dev.to top articles");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Dev.to top articles");
            }
        }

        match programming_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Dev.to programming tag");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Dev.to programming tag");
            }
        }

        match webdev_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Dev.to webdev tag");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Dev.to webdev tag");
            }
        }

        info!(total = all_items.len(), "Total Dev.to items after dedup");
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Dev.to descriptions are usually sufficient summaries
        // The full article content would require the articles/:id endpoint
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
    fn test_devto_source_creation() {
        let source = DevtoSource::new();
        assert_eq!(source.source_type(), "devto");
        assert_eq!(source.name(), "Dev.to");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 900);
    }

    #[test]
    fn test_devto_source_default() {
        let source = DevtoSource::default();
        assert_eq!(source.source_type(), "devto");
    }

    #[test]
    fn test_devto_json_parsing() {
        let json = r#"[
            {
                "id": 123456,
                "title": "Building a Tauri App with React",
                "url": "https://dev.to/someuser/building-a-tauri-app-123",
                "description": "Learn how to build a desktop app with Tauri and React",
                "published_at": "2026-02-10T14:30:00Z",
                "positive_reactions_count": 85,
                "comments_count": 12,
                "tag_list": ["tauri", "react", "rust", "tutorial"],
                "user": {
                    "name": "Some Developer",
                    "username": "someuser"
                },
                "reading_time_minutes": 8
            },
            {
                "id": 789012,
                "title": "Understanding SQLite WAL Mode",
                "url": "https://dev.to/dbexpert/sqlite-wal-789",
                "description": "Deep dive into SQLite Write-Ahead Logging",
                "published_at": "2026-02-09T10:00:00Z",
                "positive_reactions_count": 120,
                "comments_count": 25,
                "tag_list": ["database", "sqlite"],
                "user": {
                    "name": "DB Expert",
                    "username": "dbexpert"
                },
                "reading_time_minutes": 12
            }
        ]"#;

        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].id, 123456);
        assert_eq!(articles[0].title, "Building a Tauri App with React");
        assert_eq!(articles[0].positive_reactions_count, Some(85));
        assert_eq!(articles[0].comments_count, Some(12));
        assert_eq!(
            articles[0].tag_list,
            vec!["tauri", "react", "rust", "tutorial"]
        );
        assert_eq!(articles[0].user.as_ref().unwrap().name, "Some Developer");
        assert_eq!(articles[0].user.as_ref().unwrap().username, "someuser");
        assert_eq!(articles[0].reading_time_minutes, Some(8));

        assert_eq!(articles[1].id, 789012);
        assert_eq!(articles[1].positive_reactions_count, Some(120));
    }

    #[test]
    fn test_devto_json_parsing_minimal() {
        // Test with minimal fields (optional fields missing)
        let json = r#"[
            {
                "id": 999,
                "title": "Quick Tip",
                "url": "https://dev.to/user/quick-tip-999",
                "description": ""
            }
        ]"#;

        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].id, 999);
        assert!(articles[0].description.is_empty());
        assert!(articles[0].published_at.is_none());
        assert!(articles[0].positive_reactions_count.is_none());
        assert!(articles[0].comments_count.is_none());
        assert!(articles[0].tag_list.is_empty());
        assert!(articles[0].user.is_none());
        assert!(articles[0].reading_time_minutes.is_none());
    }
}
