// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Papers with Code source implementation
//!
//! Fetches latest research papers with linked code implementations
//! from the Papers with Code API.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Papers with Code API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct PwcResponse {
    results: Option<Vec<PwcPaper>>,
}

#[derive(Debug, Deserialize)]
struct PwcPaper {
    id: Option<String>,
    title: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    url_abs: Option<String>,
    url_pdf: Option<String>,
    published: Option<String>,
    authors: Option<Vec<String>>,
    tasks: Option<Vec<PwcTask>>,
    repositories: Option<Vec<PwcRepo>>,
}

#[derive(Debug, Deserialize)]
struct PwcTask {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PwcRepo {
    url: Option<String>,
    stars: Option<u32>,
}

// ============================================================================
// Papers with Code Source
// ============================================================================

/// Papers with Code source - fetches latest research papers with code
pub struct PapersWithCodeSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl PapersWithCodeSource {
    /// Create a new Papers with Code source with default config
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
}

impl Default for PapersWithCodeSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for PapersWithCodeSource {
    fn source_type(&self) -> &'static str {
        "papers_with_code"
    }

    fn name(&self) -> &'static str {
        "Papers with Code"
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
            default_content_type: "deep_dive",
            default_multiplier: 1.15,
            label: "PwC",
            color_hint: "indigo",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Papers with Code latest papers");

        // Papers with Code API now redirects to HuggingFace.
        // Use HuggingFace daily papers endpoint directly.
        let url = "https://huggingface.co/api/daily_papers".to_string();

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "Papers with Code rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Papers with Code forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Papers with Code API error: HTTP {}",
                status.as_u16()
            )));
        }

        // Try HuggingFace daily_papers format first (flat array of {paper: {...}})
        // Fall back to original PwC format ({results: [...]})
        let body = response
            .text()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let papers: Vec<PwcPaper> =
            if let Ok(daily_papers) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
                daily_papers
                    .into_iter()
                    .filter_map(|dp| {
                        let paper = dp.get("paper")?;
                        Some(PwcPaper {
                            id: paper.get("id").and_then(|v| v.as_str()).map(String::from),
                            title: paper
                                .get("title")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            abstract_text: paper
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            url_abs: paper
                                .get("id")
                                .and_then(|v| v.as_str())
                                .map(|id| format!("https://huggingface.co/papers/{id}")),
                            url_pdf: None,
                            published: paper
                                .get("publishedAt")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            authors: paper.get("authors").and_then(|v| v.as_array()).map(|a| {
                                a.iter()
                                    .filter_map(|author| {
                                        author
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .map(String::from)
                                    })
                                    .collect()
                            }),
                            tasks: None,
                            repositories: None,
                        })
                    })
                    .collect()
            } else if let Ok(pwc_response) = serde_json::from_str::<PwcResponse>(&body) {
                pwc_response.results.unwrap_or_default()
            } else {
                Vec::new()
            };

        let items: Vec<SourceItem> = papers
            .into_iter()
            .take(self.config.max_items)
            .filter_map(|paper| {
                let title = paper.title.as_deref().unwrap_or("").trim();
                if title.is_empty() {
                    return None;
                }

                let source_id = paper
                    .id
                    .clone()
                    .unwrap_or_else(|| title.to_lowercase().replace(' ', "-"));

                // Build URL: prefer url_abs, fall back to paperswithcode URL
                let paper_url = paper
                    .url_abs
                    .clone()
                    .unwrap_or_else(|| format!("https://paperswithcode.com/paper/{}", source_id));

                // Build content from abstract + authors + tasks
                let mut content_parts = Vec::new();
                if let Some(ref abs) = paper.abstract_text {
                    let trimmed = abs.trim();
                    if !trimmed.is_empty() {
                        content_parts.push(trimmed.to_string());
                    }
                }
                if let Some(ref authors) = paper.authors {
                    if !authors.is_empty() {
                        content_parts.push(format!("Authors: {}", authors.join(", ")));
                    }
                }
                if let Some(ref tasks) = paper.tasks {
                    let task_names: Vec<&str> =
                        tasks.iter().filter_map(|t| t.name.as_deref()).collect();
                    if !task_names.is_empty() {
                        content_parts.push(format!("Tasks: {}", task_names.join(", ")));
                    }
                }
                let content = content_parts.join("\n\n");

                // Build metadata
                let mut metadata = serde_json::json!({});
                if let Some(ref authors) = paper.authors {
                    metadata["authors"] = serde_json::json!(authors);
                }
                if let Some(ref tasks) = paper.tasks {
                    let task_names: Vec<&str> =
                        tasks.iter().filter_map(|t| t.name.as_deref()).collect();
                    metadata["tasks"] = serde_json::json!(task_names);
                }
                if let Some(ref published) = paper.published {
                    metadata["published_date"] = serde_json::json!(published);
                }
                if let Some(ref url_pdf) = paper.url_pdf {
                    metadata["url_pdf"] = serde_json::json!(url_pdf);
                }

                // Aggregate repository stats
                if let Some(ref repos) = paper.repositories {
                    let total_stars: u32 = repos.iter().filter_map(|r| r.stars).sum();
                    metadata["stars"] = serde_json::json!(total_stars);
                    metadata["repository_count"] = serde_json::json!(repos.len());
                }

                Some(
                    SourceItem::new("papers_with_code", &source_id, title)
                        .with_url(Some(paper_url))
                        .with_content(content)
                        .with_metadata(metadata),
                )
            })
            .collect();

        info!(items = items.len(), "Fetched Papers with Code items");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Papers already have abstract content from the API
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
    fn test_papers_with_code_source_creation() {
        let source = PapersWithCodeSource::new();
        assert_eq!(source.source_type(), "papers_with_code");
        assert_eq!(source.name(), "Papers with Code");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_papers_with_code_source_default() {
        let source = PapersWithCodeSource::default();
        assert_eq!(source.source_type(), "papers_with_code");
    }

    #[test]
    fn test_pwc_json_parsing() {
        let json = r#"{
            "count": 100,
            "results": [
                {
                    "id": "attention-is-all-you-need",
                    "title": "Attention Is All You Need",
                    "abstract": "The dominant sequence transduction models are based on complex recurrent or convolutional neural networks.",
                    "url_abs": "https://arxiv.org/abs/1706.03762",
                    "url_pdf": "https://arxiv.org/pdf/1706.03762",
                    "published": "2017-06-12",
                    "authors": ["Ashish Vaswani", "Noam Shazeer"],
                    "tasks": [{"name": "Machine Translation"}, {"name": "Language Modelling"}],
                    "repositories": [
                        {"url": "https://github.com/tensorflow/tensor2tensor", "stars": 14000},
                        {"url": "https://github.com/jadore801120/attention-is-all-you-need-pytorch", "stars": 7000}
                    ]
                },
                {
                    "id": "empty-paper",
                    "title": "",
                    "abstract": null,
                    "url_abs": null,
                    "url_pdf": null,
                    "published": null,
                    "authors": null,
                    "tasks": null,
                    "repositories": null
                }
            ]
        }"#;

        let response: PwcResponse = serde_json::from_str(json).unwrap();
        let results = response.results.unwrap();
        assert_eq!(results.len(), 2);

        let paper = &results[0];
        assert_eq!(paper.id.as_deref(), Some("attention-is-all-you-need"));
        assert_eq!(paper.title.as_deref(), Some("Attention Is All You Need"));
        assert!(paper.abstract_text.as_ref().unwrap().contains("dominant"));
        assert_eq!(paper.authors.as_ref().unwrap().len(), 2);
        assert_eq!(paper.tasks.as_ref().unwrap().len(), 2);
        assert_eq!(paper.repositories.as_ref().unwrap().len(), 2);
        assert_eq!(paper.repositories.as_ref().unwrap()[0].stars, Some(14000));

        // Empty title paper should have None/empty fields
        let empty = &results[1];
        assert_eq!(empty.title.as_deref(), Some(""));
        assert!(empty.abstract_text.is_none());
    }
}
