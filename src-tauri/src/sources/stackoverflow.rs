//! Stack Overflow source implementation
//!
//! Fetches trending questions from Stack Overflow's public API.
//! No auth required for 300 requests/day quota.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Stack Overflow API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct SoResponse {
    items: Option<Vec<SoQuestion>>,
    #[allow(dead_code)]
    has_more: Option<bool>,
    quota_remaining: Option<u32>,
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

    /// Fetch questions for a single tag
    async fn fetch_tag(&self, tag: &str) -> SourceResult<(Vec<SourceItem>, Option<u32>)> {
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

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Stack Overflow rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Stack Overflow forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Stack Overflow API error: HTTP {}",
                status.as_u16()
            )));
        }

        let so_resp: SoResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let quota_remaining = so_resp.quota_remaining;
        let questions = so_resp.items.unwrap_or_default();

        let items: Vec<SourceItem> = questions
            .into_iter()
            .map(|q| {
                let question_tags = q.tags.clone().unwrap_or_default();
                let answer_count = q.answer_count.unwrap_or(0);
                let content = format!(
                    "Tags: {} | Score: {} | Answers: {}",
                    question_tags.join(", "),
                    q.score,
                    answer_count
                );

                let mut metadata = serde_json::json!({
                    "score": q.score,
                    "answer_count": answer_count,
                    "tags": question_tags,
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

        Ok((items, quota_remaining))
    }
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

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
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
                Ok((items, quota)) => {
                    info!(tag = %tag, count = items.len(), quota = ?quota, "Fetched SO questions");

                    for item in items {
                        if seen_ids.insert(item.source_id.clone()) {
                            all_items.push(item);
                        }
                    }

                    // Stop if quota is running low
                    if let Some(remaining) = quota {
                        if remaining < MIN_QUOTA {
                            warn!(remaining, "Stack Overflow quota low, stopping early");
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!(tag = %tag, error = ?e, "Failed to fetch SO questions for tag");
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

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // SO items already have useful content from the API
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
}
