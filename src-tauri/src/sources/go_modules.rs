//! Go Module Proxy source implementation
//!
//! Monitors new Go module versions via the Go Module Index API.
//! Uses newline-delimited JSON from index.golang.org.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Go Module Index API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct GoModuleEntry {
    #[serde(rename = "Path")]
    path: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Timestamp")]
    timestamp: Option<String>,
}

// ============================================================================
// Go Modules Source
// ============================================================================

/// Go Modules source - monitors new Go module versions
pub struct GoModulesSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl GoModulesSource {
    /// Create a new Go Modules source with default config
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

impl Default for GoModulesSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for GoModulesSource {
    fn source_type(&self) -> &'static str {
        "go_modules"
    }

    fn name(&self) -> &'static str {
        "Go Modules"
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
            label: "Go",
            color_hint: "cyan",
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Fetching Go Module Index latest entries");

        let url = format!(
            "https://index.golang.org/index?limit={}",
            self.config.max_items
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
                "Go Module Index rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "Go Module Index forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "Go Module Index API error: HTTP {}",
                status.as_u16()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        // Parse newline-delimited JSON
        let items: Vec<SourceItem> = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| {
                match serde_json::from_str::<GoModuleEntry>(line) {
                    Ok(entry) => {
                        let source_id = format!("{}@{}", entry.path, entry.version);
                        let title = format!("Go: {} {}", entry.path, entry.version);
                        let pkg_url =
                            format!("https://pkg.go.dev/{}@{}", entry.path, entry.version);

                        // Build content from module info
                        let mut content_parts = vec![
                            format!("Module: {}", entry.path),
                            format!("Version: {}", entry.version),
                        ];
                        if let Some(ref ts) = entry.timestamp {
                            content_parts.push(format!("Published: {}", ts));
                        }
                        let content = content_parts.join("\n");

                        // Build metadata
                        let mut metadata = serde_json::json!({
                            "module_path": entry.path,
                            "version": entry.version,
                            "ecosystem": "go",
                        });
                        if let Some(ref ts) = entry.timestamp {
                            metadata["timestamp"] = serde_json::json!(ts);
                        }

                        Some(
                            SourceItem::new("go_modules", &source_id, &title)
                                .with_url(Some(pkg_url))
                                .with_content(content)
                                .with_metadata(metadata),
                        )
                    }
                    Err(e) => {
                        warn!(line = %line, error = %e, "Failed to parse Go module entry");
                        None
                    }
                }
            })
            .take(self.config.max_items)
            .collect();

        info!(items = items.len(), "Fetched Go Module Index items");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Module info is already populated from the index API
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
    fn test_go_modules_source_creation() {
        let source = GoModulesSource::new();
        assert_eq!(source.source_type(), "go_modules");
        assert_eq!(source.name(), "Go Modules");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_go_modules_source_default() {
        let source = GoModulesSource::default();
        assert_eq!(source.source_type(), "go_modules");
    }

    #[test]
    fn test_go_module_ndjson_parsing() {
        let ndjson = r#"{"Path":"github.com/example/module","Version":"v1.2.3","Timestamp":"2026-03-15T10:00:00Z"}
{"Path":"golang.org/x/text","Version":"v0.14.0","Timestamp":"2026-03-15T09:00:00Z"}
{"Path":"github.com/no-timestamp/pkg","Version":"v0.1.0"}"#;

        let entries: Vec<GoModuleEntry> = ndjson
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].path, "github.com/example/module");
        assert_eq!(entries[0].version, "v1.2.3");
        assert_eq!(
            entries[0].timestamp.as_deref(),
            Some("2026-03-15T10:00:00Z")
        );

        assert_eq!(entries[1].path, "golang.org/x/text");
        assert_eq!(entries[1].version, "v0.14.0");

        // Entry without timestamp
        assert_eq!(entries[2].path, "github.com/no-timestamp/pkg");
        assert!(entries[2].timestamp.is_none());
    }
}
