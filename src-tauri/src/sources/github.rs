//! GitHub source implementation
//!
//! Fetches trending repositories from GitHub Search API and README content.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// GitHub API Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields deserialized from GitHub API JSON
struct GitHubRepo {
    id: u64,
    name: String,
    full_name: String,
    description: Option<String>,
    html_url: String,
    stargazers_count: i32,
    language: Option<String>,
    updated_at: String,
    #[serde(default)]
    topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubSearchResponse {
    #[allow(dead_code)]
    total_count: u32,
    items: Vec<GitHubRepo>,
}

#[derive(Debug, Deserialize)]
struct GitHubReadmeResponse {
    content: String,
}

// ============================================================================
// GitHub Source
// ============================================================================

/// GitHub source - fetches trending repositories and README content
pub struct GitHubSource {
    config: SourceConfig,
    client: reqwest::Client,
    languages: Vec<String>,
}

impl GitHubSource {
    /// Create a new GitHub source with default languages
    pub fn new() -> Self {
        Self::with_languages(vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ])
    }

    /// Create with custom language filters
    pub fn with_languages(languages: Vec<String>) -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("4DA/1.0") // GitHub requires User-Agent
                .build()
                .expect("Failed to create HTTP client"),
            languages,
        }
    }

    /// Build GitHub search query string
    fn build_search_query(&self) -> String {
        let today = chrono::Utc::now();
        let week_ago = today - chrono::Duration::days(7);
        let week_ago_str = week_ago.format("%Y-%m-%d").to_string();

        // Build language query: "language:rust OR language:typescript"
        let lang_query = self
            .languages
            .iter()
            .map(|lang| format!("language:{}", lang))
            .collect::<Vec<_>>()
            .join("+OR+");

        // Full query: languages + stars filter + recent activity
        format!("{}+stars:>1000+pushed:>{}", lang_query, week_ago_str)
    }
}

impl Default for GitHubSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for GitHubSource {
    fn source_type(&self) -> &'static str {
        "github"
    }

    fn name(&self) -> &'static str {
        "GitHub"
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

        info!(
            languages = ?self.languages,
            max_items = self.config.max_items,
            "Fetching trending GitHub repositories"
        );

        let query = self.build_search_query();
        let url = format!(
            "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page={}",
            query, self.config.max_items
        );

        info!(query = %query, "GitHub search query");

        // Fetch search results
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            warn!(status = %status, "GitHub API request failed");
            return Err(SourceError::Network(format!("HTTP {}", status)));
        }

        let search_result: GitHubSearchResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        info!(
            count = search_result.items.len(),
            "Fetched GitHub repositories"
        );

        // Convert to SourceItems
        let mut items = Vec::new();
        for repo in search_result.items {
            let title = format!(
                "{} (★{}{})",
                repo.full_name,
                repo.stargazers_count,
                repo.language
                    .as_ref()
                    .map(|l| format!(" • {}", l))
                    .unwrap_or_default()
            );

            let description = repo.description.unwrap_or_default();

            let mut item = SourceItem::new("github", &repo.id.to_string(), &title)
                .with_url(Some(repo.html_url.clone()))
                .with_content(description);

            // Add metadata
            let metadata = serde_json::json!({
                "stars": repo.stargazers_count,
                "language": repo.language,
                "updated_at": repo.updated_at,
                "topics": repo.topics,
                "full_name": repo.full_name,
            });
            item = item.with_metadata(metadata);

            items.push(item);
        }

        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Extract owner/repo from metadata
        let metadata = match &item.metadata {
            Some(m) => m,
            None => return Ok(item.content.clone()),
        };

        let full_name = match metadata.get("full_name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => return Ok(item.content.clone()),
        };

        info!(repo = %full_name, "Fetching README");

        // Fetch README via GitHub API
        let readme_url = format!("https://api.github.com/repos/{}/readme", full_name);

        let response = self
            .client
            .get(&readme_url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !response.status().is_success() {
            warn!(repo = %full_name, "README not found or rate limited");
            return Ok(item.content.clone()); // Return description as fallback
        }

        let readme_response: GitHubReadmeResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        // README content is base64 encoded
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&readme_response.content)
            .map_err(|e| SourceError::Parse(format!("Base64 decode failed: {}", e)))?;

        let readme_text = String::from_utf8_lossy(&decoded).to_string();

        // Truncate to reasonable length (keep first 5000 chars)
        let max_len = 5000;
        let truncated = if readme_text.len() > max_len {
            readme_text.chars().take(max_len).collect()
        } else {
            readme_text
        };

        info!(
            repo = %full_name,
            length = truncated.len(),
            "Fetched README"
        );

        Ok(truncated)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_source_creation() {
        let source = GitHubSource::new();
        assert_eq!(source.source_type(), "github");
        assert_eq!(source.name(), "GitHub");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_custom_languages() {
        let source = GitHubSource::with_languages(vec!["rust".to_string()]);
        assert_eq!(source.languages.len(), 1);
        assert_eq!(source.languages[0], "rust");
    }

    #[test]
    fn test_default_languages() {
        let source = GitHubSource::new();
        assert_eq!(source.languages.len(), 3);
        assert!(source.languages.contains(&"rust".to_string()));
        assert!(source.languages.contains(&"typescript".to_string()));
        assert!(source.languages.contains(&"python".to_string()));
    }

    #[test]
    fn test_search_query_format() {
        let source =
            GitHubSource::with_languages(vec!["rust".to_string(), "typescript".to_string()]);
        let query = source.build_search_query();

        // Should contain language filters
        assert!(query.contains("language:rust"));
        assert!(query.contains("language:typescript"));
        // Should contain stars filter
        assert!(query.contains("stars:>1000"));
        // Should contain date filter
        assert!(query.contains("pushed:>"));
    }

    #[test]
    fn test_disabled_source() {
        let mut source = GitHubSource::new();
        source.config.enabled = false;

        // Should be disabled
        assert!(!source.config().enabled);
    }
}
