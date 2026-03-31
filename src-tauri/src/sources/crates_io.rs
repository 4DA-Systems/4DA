//! crates.io source implementation
//!
//! Monitors Rust crates for new versions, yanked crates, and recently updated
//! packages. Combines monitored-crate checks with discovery of trending crates.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// crates.io API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    krate: Option<CrateInfo>,
    versions: Option<Vec<CrateVersion>>,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    max_version: Option<String>,
    downloads: Option<u64>,
    updated_at: Option<String>,
    #[serde(default)]
    categories: Option<Vec<String>>,
    #[serde(default)]
    keywords: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CrateVersion {
    num: String,
    yanked: bool,
    #[allow(dead_code)]
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CratesSearchResponse {
    crates: Vec<CrateInfo>,
}

// ============================================================================
// crates.io Source
// ============================================================================

const USER_AGENT: &str = "4DA-Developer-OS/1.0 (https://4da.ai)";
const API_BASE: &str = "https://crates.io/api/v1";

/// Default crates to monitor — popular Rust ecosystem crates
const DEFAULT_CRATES: &[&str] = &[
    "tokio",
    "serde",
    "reqwest",
    "axum",
    "clap",
    "anyhow",
    "thiserror",
    "tracing",
    "sqlx",
    "diesel",
    "warp",
    "actix-web",
];

/// crates.io source — monitors Rust crates for new versions and discovery
pub struct CratesIoSource {
    config: SourceConfig,
    client: reqwest::Client,
    crates: Vec<String>,
}

impl CratesIoSource {
    /// Create a new crates.io source — uses real deps from ACE if available
    pub fn new() -> Self {
        let ace_crates = crate::source_fetching::load_ace_packages_for_ecosystem("crates.io");
        let crates = if ace_crates.is_empty() {
            DEFAULT_CRATES.iter().map(|s| s.to_string()).collect()
        } else {
            ace_crates
        };
        Self::with_crates(crates)
    }

    /// Create a crates.io source with a custom crate list
    pub fn with_crates(crates: Vec<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|e| {
                warn!("Failed to build crates.io HTTP client: {e}, using default");
                reqwest::Client::new()
            });

        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600, // 1 hour — crates don't update that fast
                custom: None,
            },
            client,
            crates,
        }
    }

    /// Fetch info for a single crate from the API
    async fn fetch_crate(&self, name: &str) -> SourceResult<SourceItem> {
        let url = format!("{API_BASE}/crates/{name}");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "crates.io rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "crates.io forbidden (HTTP 403)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(SourceError::Other(format!("Crate not found: {name}")));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "crates.io API error: HTTP {}",
                status.as_u16()
            )));
        }

        let data: CratesIoResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let krate = data
            .krate
            .ok_or_else(|| SourceError::Parse(format!("Missing crate data for {name}")))?;

        let version = krate
            .max_version
            .as_deref()
            .unwrap_or("unknown")
            .to_string();

        let title = format!("crates.io: {} v{}", krate.name, version);
        let crate_url = format!("https://crates.io/crates/{}", krate.name);

        // Check for yanked versions (security signal)
        let has_yanked = data
            .versions
            .as_ref()
            .map(|vs| vs.iter().any(|v| v.yanked))
            .unwrap_or(false);

        let yanked_versions: Vec<String> = data
            .versions
            .as_ref()
            .map(|vs| {
                vs.iter()
                    .filter(|v| v.yanked)
                    .map(|v| v.num.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Build content from description + stats
        let description = krate.description.as_deref().unwrap_or("No description");
        let downloads = krate.downloads.unwrap_or(0);
        let categories = krate.categories.as_deref().unwrap_or(&[]);
        let keywords = krate.keywords.as_deref().unwrap_or(&[]);

        let mut content_parts = vec![description.to_string()];
        content_parts.push(format!("Downloads: {downloads}"));
        if !categories.is_empty() {
            content_parts.push(format!("Categories: {}", categories.join(", ")));
        }
        if !keywords.is_empty() {
            content_parts.push(format!("Keywords: {}", keywords.join(", ")));
        }
        if has_yanked {
            content_parts.push(format!("Yanked versions: {}", yanked_versions.join(", ")));
        }
        let content = content_parts.join("\n");

        let mut metadata = serde_json::json!({
            "version": version,
            "downloads": downloads,
            "yanked": has_yanked,
            "ecosystem": "crates.io",
        });
        if !categories.is_empty() {
            metadata["categories"] = serde_json::json!(categories);
        }
        if !keywords.is_empty() {
            metadata["keywords"] = serde_json::json!(keywords);
        }
        if let Some(updated) = &krate.updated_at {
            metadata["updated_at"] = serde_json::json!(updated);
        }
        if !yanked_versions.is_empty() {
            metadata["yanked_versions"] = serde_json::json!(yanked_versions);
        }

        Ok(
            SourceItem::new("crates_io", &format!("crate-{}", krate.name), &title)
                .with_url(Some(crate_url))
                .with_content(content)
                .with_metadata(metadata),
        )
    }

    /// Fetch recently updated crates for discovery
    async fn fetch_recent(&self, max: usize) -> SourceResult<Vec<SourceItem>> {
        let url = format!("{API_BASE}/crates?sort=recent-updates&per_page={max}");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "crates.io rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "crates.io forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "crates.io API error: HTTP {}",
                status.as_u16()
            )));
        }

        let data: CratesSearchResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let items = data
            .crates
            .into_iter()
            .take(max)
            .map(|krate| {
                let version = krate
                    .max_version
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_string();
                let title = format!("crates.io: {} v{}", krate.name, version);
                let crate_url = format!("https://crates.io/crates/{}", krate.name);
                let description = krate
                    .description
                    .as_deref()
                    .unwrap_or("No description")
                    .to_string();
                let downloads = krate.downloads.unwrap_or(0);

                let mut metadata = serde_json::json!({
                    "version": version,
                    "downloads": downloads,
                    "ecosystem": "crates.io",
                    "discovery": true,
                });
                if let Some(updated) = &krate.updated_at {
                    metadata["updated_at"] = serde_json::json!(updated);
                }

                SourceItem::new("crates_io", &format!("crate-{}", krate.name), &title)
                    .with_url(Some(crate_url))
                    .with_content(format!("{description}\nDownloads: {downloads}"))
                    .with_metadata(metadata)
            })
            .collect();

        Ok(items)
    }
}

impl Default for CratesIoSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for CratesIoSource {
    fn source_type(&self) -> &'static str {
        "crates_io"
    }

    fn name(&self) -> &'static str {
        "crates.io"
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
            crates = self.crates.len(),
            "Fetching crates.io monitored crates"
        );

        let mut items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // 1. Check monitored crates (primary value)
        for name in &self.crates {
            match self.fetch_crate(name).await {
                Ok(item) => {
                    if seen_ids.insert(item.source_id.clone()) {
                        items.push(item);
                    }
                }
                Err(SourceError::RateLimited(msg)) => {
                    warn!(crate_name = %name, "Rate limited by crates.io: {msg}");
                    break; // Stop hitting the API
                }
                Err(e) => {
                    warn!(crate_name = %name, error = ?e, "Failed to fetch crate");
                }
            }
            // Respect crates.io rate limit: ~1 req/sec
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        // 2. Fetch recently updated for discovery (cap at 10)
        let remaining = self.config.max_items.saturating_sub(items.len());
        let discovery_count = remaining.min(10);
        if discovery_count > 0 {
            match self.fetch_recent(discovery_count).await {
                Ok(recent) => {
                    info!(count = recent.len(), "Fetched recently updated crates");
                    for item in recent {
                        if seen_ids.insert(item.source_id.clone()) {
                            items.push(item);
                        }
                    }
                }
                Err(e) => {
                    warn!(error = ?e, "Failed to fetch recently updated crates");
                }
            }
        }

        info!(total = items.len(), "Total crates.io items");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        // Deep fetch is the same as regular — we already fetch all monitored crates
        self.fetch_items().await
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // crates.io items already have full content from the API response —
        // no scraping needed
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
    fn test_crates_io_source_creation() {
        let source = CratesIoSource::new();
        assert_eq!(source.source_type(), "crates_io");
        assert_eq!(source.name(), "crates.io");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 3600);
        assert_eq!(source.crates.len(), DEFAULT_CRATES.len());
    }

    #[test]
    fn test_crates_io_source_with_custom_crates() {
        let crates = vec!["serde".to_string(), "tokio".to_string()];
        let source = CratesIoSource::with_crates(crates.clone());
        assert_eq!(source.crates, crates);
        assert_eq!(source.crates.len(), 2);
    }

    #[test]
    fn test_crates_io_source_default() {
        let source = CratesIoSource::default();
        assert_eq!(source.source_type(), "crates_io");
        assert_eq!(source.crates.len(), DEFAULT_CRATES.len());
    }

    #[test]
    fn test_crate_response_parsing() {
        let json = r#"{
            "crate": {
                "name": "serde",
                "description": "A generic serialization/deserialization framework",
                "max_version": "1.0.215",
                "downloads": 350000000,
                "updated_at": "2026-03-15T10:00:00Z",
                "categories": ["encoding", "no-std"],
                "keywords": ["serde", "serialization", "no_std"]
            },
            "versions": [
                { "num": "1.0.215", "yanked": false, "created_at": "2026-03-15T10:00:00Z" },
                { "num": "1.0.214", "yanked": false, "created_at": "2026-03-01T10:00:00Z" },
                { "num": "1.0.100", "yanked": true, "created_at": "2023-01-01T10:00:00Z" }
            ]
        }"#;

        let data: CratesIoResponse = serde_json::from_str(json).unwrap();
        let krate = data.krate.unwrap();
        assert_eq!(krate.name, "serde");
        assert_eq!(krate.max_version.as_deref(), Some("1.0.215"));
        assert_eq!(krate.downloads, Some(350_000_000));
        assert_eq!(
            krate.description.as_deref(),
            Some("A generic serialization/deserialization framework")
        );

        let versions = data.versions.unwrap();
        assert_eq!(versions.len(), 3);
        assert!(!versions[0].yanked);
        assert!(versions[2].yanked);
        assert_eq!(versions[2].num, "1.0.100");
    }

    #[test]
    fn test_search_response_parsing() {
        let json = r#"{
            "crates": [
                {
                    "name": "tokio",
                    "description": "An event-driven, non-blocking I/O platform",
                    "max_version": "1.42.0",
                    "downloads": 200000000,
                    "updated_at": "2026-03-20T08:00:00Z"
                },
                {
                    "name": "axum",
                    "description": "Web framework built on top of tokio and hyper",
                    "max_version": "0.8.1",
                    "downloads": 15000000,
                    "updated_at": "2026-03-18T12:00:00Z"
                }
            ]
        }"#;

        let data: CratesSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.crates.len(), 2);
        assert_eq!(data.crates[0].name, "tokio");
        assert_eq!(data.crates[1].name, "axum");
        assert_eq!(data.crates[1].max_version.as_deref(), Some("0.8.1"));
    }
}
