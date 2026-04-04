//! PyPI source implementation
//!
//! Monitors Python packages for new versions and metadata.
//! Matched against the user's requirements.txt/pyproject.toml dependencies.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// PyPI API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct PypiPackageInfo {
    info: PypiInfo,
}

#[derive(Debug, Deserialize)]
struct PypiInfo {
    name: String,
    version: String,
    summary: Option<String>,
    author: Option<String>,
    home_page: Option<String>,
    #[allow(dead_code)]
    project_url: Option<String>,
    requires_python: Option<String>,
    classifiers: Option<Vec<String>>,
    project_urls: Option<HashMap<String, String>>,
}

// ============================================================================
// Default packages to monitor
// ============================================================================

const DEFAULT_PACKAGES: &[&str] = &[
    "django",
    "flask",
    "fastapi",
    "numpy",
    "pandas",
    "pytorch",
    "tensorflow",
    "requests",
    "sqlalchemy",
    "pydantic",
    "celery",
    "boto3",
];

// ============================================================================
// PyPI Source
// ============================================================================

/// PyPI source - monitors Python packages for new versions and metadata
pub struct PypiSource {
    config: SourceConfig,
    client: reqwest::Client,
    packages: Vec<String>,
}

impl PypiSource {
    /// Create a new PyPI source — uses real deps from ACE if available
    pub fn new() -> Self {
        let ace_packages = crate::source_fetching::load_ace_packages_for_ecosystem("pypi");
        let packages = if ace_packages.is_empty() {
            DEFAULT_PACKAGES.iter().map(|s| s.to_string()).collect()
        } else {
            ace_packages
        };
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 20,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
            packages,
        }
    }

    /// Create a PyPI source with a custom list of packages to monitor
    pub fn with_packages(packages: Vec<String>) -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 20,
                fetch_interval_secs: 3600,
                custom: None,
            },
            client: super::shared_client(),
            packages,
        }
    }

    /// Fetch metadata for a single package from the PyPI JSON API
    async fn fetch_package(&self, package: &str) -> SourceResult<SourceItem> {
        let url = format!("https://pypi.org/pypi/{}/json", package);

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
                "PyPI rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "PyPI forbidden (HTTP 403)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(SourceError::Other(format!(
                "PyPI package not found: {package}"
            )));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "PyPI API error: HTTP {}",
                status.as_u16()
            )));
        }

        let pkg: PypiPackageInfo = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let info = &pkg.info;
        let title = format!("PyPI: {} v{}", info.name, info.version);
        let source_id = format!("{}-{}", info.name, info.version);

        // Build content from summary + author + requires_python + classifiers
        let mut content_parts: Vec<String> = Vec::new();
        if let Some(ref summary) = info.summary {
            content_parts.push(summary.clone());
        }
        if let Some(ref author) = info.author {
            content_parts.push(format!("Author: {author}"));
        }
        if let Some(ref req) = info.requires_python {
            content_parts.push(format!("Requires Python: {req}"));
        }
        if let Some(ref classifiers) = info.classifiers {
            let top: Vec<&String> = classifiers.iter().take(10).collect();
            if !top.is_empty() {
                content_parts.push(format!(
                    "Classifiers: {}",
                    top.iter()
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let content = content_parts.join("\n");

        // Build the project URL: prefer Homepage from project_urls, then home_page
        let project_url = info
            .project_urls
            .as_ref()
            .and_then(|urls| {
                urls.get("Homepage")
                    .or_else(|| urls.get("homepage"))
                    .or_else(|| urls.get("Home"))
                    .or_else(|| urls.get("Source"))
                    .or_else(|| urls.get("Repository"))
                    .cloned()
            })
            .or_else(|| info.home_page.clone())
            .unwrap_or_else(|| format!("https://pypi.org/project/{}/", info.name));

        // Build metadata
        let mut metadata = serde_json::json!({
            "version": info.version,
            "ecosystem": "pypi",
        });
        if let Some(ref author) = info.author {
            metadata["author"] = serde_json::json!(author);
        }
        if let Some(ref req) = info.requires_python {
            metadata["requires_python"] = serde_json::json!(req);
        }

        Ok(SourceItem::new("pypi", &source_id, &title)
            .with_url(Some(project_url))
            .with_content(content)
            .with_metadata(metadata))
    }
}

impl Default for PypiSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for PypiSource {
    fn source_type(&self) -> &'static str {
        "pypi"
    }

    fn name(&self) -> &'static str {
        "PyPI"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::PackageRegistry,
            default_content_type: "release_notes",
            default_multiplier: 1.15,
            label: "PyPI",
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

        info!(
            packages = self.packages.len(),
            "Fetching PyPI package metadata"
        );

        let mut items = Vec::new();
        let max = self.config.max_items.min(self.packages.len());

        for package in self.packages.iter().take(max) {
            match self.fetch_package(package).await {
                Ok(item) => items.push(item),
                Err(e) => {
                    warn!(package = %package, error = %e, "Failed to fetch PyPI package");
                }
            }
            // 500ms delay between requests to be polite to PyPI CDN
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        info!(items = items.len(), "Fetched PyPI package items");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        // Deep fetch: query all monitored packages (same as regular but logs differently)
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!(
            target: "4da::sources",
            packages = self.packages.len(),
            "PyPI deep scan: fetching all monitored packages"
        );

        let mut items = Vec::new();
        for package in &self.packages {
            match self.fetch_package(package).await {
                Ok(item) => items.push(item),
                Err(SourceError::RateLimited(_)) => break,
                Err(e) => {
                    warn!(
                        target: "4da::sources",
                        package = %package,
                        error = %e,
                        "PyPI: Failed to fetch package in deep scan"
                    );
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        info!(target: "4da::sources", items = items.len(), "PyPI deep scan complete");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // PyPI items already have full content from the JSON API
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
    fn test_pypi_source_creation() {
        let source = PypiSource::new();
        assert_eq!(source.source_type(), "pypi");
        assert_eq!(source.name(), "PyPI");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        assert_eq!(source.config().fetch_interval_secs, 3600);
        assert_eq!(source.packages.len(), DEFAULT_PACKAGES.len());
    }

    #[test]
    fn test_pypi_with_custom_packages() {
        let packages = vec!["httpx".to_string(), "rich".to_string()];
        let source = PypiSource::with_packages(packages.clone());
        assert_eq!(source.packages, packages);
        assert_eq!(source.source_type(), "pypi");
    }

    #[test]
    fn test_pypi_json_parsing() {
        let json = r#"{
            "info": {
                "name": "fastapi",
                "version": "0.115.0",
                "summary": "FastAPI framework, high performance, easy to learn",
                "author": "Sebastián Ramírez",
                "home_page": "https://github.com/tiangolo/fastapi",
                "project_url": null,
                "requires_python": ">=3.8",
                "classifiers": [
                    "Framework :: FastAPI",
                    "Programming Language :: Python :: 3"
                ],
                "project_urls": {
                    "Homepage": "https://fastapi.tiangolo.com",
                    "Source": "https://github.com/tiangolo/fastapi"
                }
            }
        }"#;

        let pkg: PypiPackageInfo = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.info.name, "fastapi");
        assert_eq!(pkg.info.version, "0.115.0");
        assert_eq!(
            pkg.info.summary.as_deref(),
            Some("FastAPI framework, high performance, easy to learn")
        );
        assert_eq!(pkg.info.author.as_deref(), Some("Sebastián Ramírez"));
        assert_eq!(pkg.info.requires_python.as_deref(), Some(">=3.8"));
        assert_eq!(pkg.info.classifiers.as_ref().unwrap().len(), 2);

        let urls = pkg.info.project_urls.as_ref().unwrap();
        assert_eq!(
            urls.get("Homepage").unwrap(),
            "https://fastapi.tiangolo.com"
        );
    }
}
