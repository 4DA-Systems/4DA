//! Hugging Face Hub source implementation
//!
//! Monitors trending AI/ML models and new model releases from the
//! Hugging Face Hub API. No authentication required for public data.

use std::collections::HashSet;

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Hugging Face API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct HfModel {
    /// Primary model identifier (e.g., "meta-llama/Llama-3-70B")
    #[serde(rename = "modelId")]
    model_id: Option<String>,
    /// Alternative field name the API sometimes uses
    id: Option<String>,
    author: Option<String>,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    pipeline_tag: Option<String>,
    downloads: Option<u64>,
    likes: Option<u64>,
}

impl HfModel {
    /// Resolve the model identifier, preferring `modelId` over `id`.
    fn resolved_id(&self) -> Option<&str> {
        self.model_id.as_deref().or(self.id.as_deref())
    }
}

// ============================================================================
// Hugging Face Source
// ============================================================================

const HF_API_BASE: &str = "https://huggingface.co/api/models";
const USER_AGENT: &str = "4DA-Developer-OS/1.0";

/// Hugging Face Hub source — surfaces trending and recently updated AI/ML models
pub struct HuggingFaceSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl HuggingFaceSource {
    /// Create a new Hugging Face source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 20,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
        }
    }

    /// Fetch models from a specific API endpoint
    async fn fetch_models(&self, url: &str, max: usize) -> SourceResult<Vec<SourceItem>> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Hugging Face rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Hugging Face forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Hugging Face API error: HTTP {}",
                status.as_u16()
            )));
        }

        let models: Vec<HfModel> = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let items: Vec<SourceItem> = models
            .into_iter()
            .filter_map(|model| {
                let model_id = model.resolved_id()?.to_string();
                // Build a descriptive title: "HF: org/model — task (Xk downloads)"
                let task_suffix = model
                    .pipeline_tag
                    .as_deref()
                    .map(|t| format!(" — {t}"))
                    .unwrap_or_default();
                let dl_suffix = model
                    .downloads
                    .filter(|&d| d > 1000)
                    .map(|d| format!(" ({:.0}k downloads)", d as f64 / 1000.0))
                    .unwrap_or_default();
                let title = format!("HF: {model_id}{task_suffix}{dl_suffix}");
                let url = format!("https://huggingface.co/{model_id}");

                // Build content from available fields
                let mut content_parts: Vec<String> = Vec::new();
                if let Some(ref tag) = model.pipeline_tag {
                    content_parts.push(format!("Task: {tag}"));
                }
                if let Some(ref tags) = model.tags {
                    if !tags.is_empty() {
                        content_parts.push(format!("Tags: {}", tags.join(", ")));
                    }
                }
                if let Some(dl) = model.downloads {
                    content_parts.push(format!("Downloads: {dl}"));
                }
                if let Some(lk) = model.likes {
                    content_parts.push(format!("Likes: {lk}"));
                }
                let content = content_parts.join(" | ");

                // Build metadata
                let mut metadata = serde_json::json!({});
                if let Some(ref author) = model.author {
                    metadata["author"] = serde_json::json!(author);
                }
                if let Some(ref tag) = model.pipeline_tag {
                    metadata["pipeline_tag"] = serde_json::json!(tag);
                }
                if let Some(dl) = model.downloads {
                    metadata["downloads"] = serde_json::json!(dl);
                }
                if let Some(lk) = model.likes {
                    metadata["likes"] = serde_json::json!(lk);
                }
                if let Some(ref modified) = model.last_modified {
                    metadata["lastModified"] = serde_json::json!(modified);
                }

                Some(
                    SourceItem::new("huggingface", &model_id, &title)
                        .with_url(Some(url))
                        .with_content(content)
                        .with_metadata(metadata),
                )
            })
            .take(max)
            .collect();

        Ok(items)
    }
}

impl Default for HuggingFaceSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for HuggingFaceSource {
    fn source_type(&self) -> &'static str {
        "huggingface"
    }

    fn name(&self) -> &'static str {
        "Hugging Face"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Research,
            default_content_type: "release_notes",
            default_multiplier: 1.15,
            label: "HF",
            color_hint: "yellow",
            min_title_words: 2, // Model IDs like "meta-llama/Llama-3" are OK (2 parts after split)
            require_user_language: false,
            require_dev_relevance: false, // All HF models are dev-relevant by nature
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Hugging Face trending models");

        let trending_url = format!("{HF_API_BASE}?sort=likes&direction=-1&limit=15");
        let items = self.fetch_models(&trending_url, 15).await?;

        info!(items = items.len(), "Fetched Hugging Face trending models");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Deep fetching Hugging Face (trending + recently updated)");

        let trending_url = format!("{HF_API_BASE}?sort=likes&direction=-1&limit=15");
        let trending_result = self.fetch_models(&trending_url, 15).await;

        // 2-second delay between requests to be polite
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let recent_url = format!("{HF_API_BASE}?sort=lastModified&direction=-1&limit=10");
        let recent_result = self.fetch_models(&recent_url, 10).await;

        let mut all_items = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        match trending_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Hugging Face trending");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Hugging Face trending");
            }
        }

        match recent_result {
            Ok(items) => {
                info!(count = items.len(), "Fetched Hugging Face recently updated");
                for item in items {
                    if seen_ids.insert(item.source_id.clone()) {
                        all_items.push(item);
                    }
                }
            }
            Err(e) => {
                warn!(error = ?e, "Failed to fetch Hugging Face recently updated");
            }
        }

        info!(
            total = all_items.len(),
            "Total Hugging Face items after dedup"
        );
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Model metadata is already populated as content during fetch;
        // no additional scraping needed for model cards.
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
    fn test_huggingface_source_creation() {
        let source = HuggingFaceSource::new();
        assert_eq!(source.source_type(), "huggingface");
        assert_eq!(source.name(), "Hugging Face");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_huggingface_source_default() {
        let source = HuggingFaceSource::default();
        assert_eq!(source.source_type(), "huggingface");
    }

    #[test]
    fn test_hf_model_json_parsing() {
        let json = r#"[
            {
                "modelId": "meta-llama/Llama-3-70B",
                "author": "meta-llama",
                "lastModified": "2026-03-15T10:00:00.000Z",
                "tags": ["pytorch", "llama", "text-generation"],
                "pipeline_tag": "text-generation",
                "downloads": 1500000,
                "likes": 4200
            },
            {
                "id": "openai/whisper-large-v3",
                "author": "openai",
                "lastModified": "2026-02-28T08:30:00.000Z",
                "tags": ["pytorch", "whisper"],
                "pipeline_tag": "automatic-speech-recognition",
                "downloads": 800000,
                "likes": 2100
            }
        ]"#;

        let models: Vec<HfModel> = serde_json::from_str(json).unwrap();
        assert_eq!(models.len(), 2);

        // First model uses modelId
        assert_eq!(models[0].resolved_id(), Some("meta-llama/Llama-3-70B"));
        assert_eq!(models[0].author.as_deref(), Some("meta-llama"));
        assert_eq!(models[0].pipeline_tag.as_deref(), Some("text-generation"));
        assert_eq!(models[0].downloads, Some(1_500_000));
        assert_eq!(models[0].likes, Some(4200));
        assert_eq!(
            models[0].tags.as_ref().unwrap(),
            &vec!["pytorch", "llama", "text-generation"]
        );

        // Second model uses id field
        assert_eq!(models[1].resolved_id(), Some("openai/whisper-large-v3"));
        assert_eq!(models[1].model_id, None);
    }

    #[test]
    fn test_hf_model_missing_fields() {
        let json = r#"[{
            "modelId": "user/model",
            "author": null,
            "tags": null,
            "pipeline_tag": null,
            "downloads": null,
            "likes": null
        }]"#;

        let models: Vec<HfModel> = serde_json::from_str(json).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].resolved_id(), Some("user/model"));
        assert!(models[0].author.is_none());
        assert!(models[0].tags.is_none());
        assert!(models[0].pipeline_tag.is_none());
    }
}
