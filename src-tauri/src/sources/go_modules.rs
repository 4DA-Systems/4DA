// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

/// Response of the module proxy `/<module>/@latest` endpoint.
#[derive(Debug, Deserialize)]
struct GoLatestInfo {
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Time")]
    time: Option<String>,
}

/// Encode a module path for the Go module proxy. The proxy lower-cases paths and escapes
/// each uppercase letter as `!<lower>` (e.g. `github.com/BurntSushi/toml` ->
/// `github.com/!burnt!sushi/toml`), so a case-sensitive path resolves correctly.
fn encode_module_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 4);
    for c in path.chars() {
        if c.is_ascii_uppercase() {
            out.push('!');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
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

    /// Strict manifest mode: fetch the latest version of ONLY the modules the stack's
    /// go.mod/go.sum pins, via the module proxy's per-module `/@latest` endpoint — never
    /// the whole-registry `index.golang.org` feed (which surfaces random modules the stack
    /// never imports). Returns an empty vec when the manifest has no Go modules tracked.
    async fn fetch_targeted(&self) -> SourceResult<Vec<SourceItem>> {
        let modules = crate::source_fetching::load_ace_packages_for_ecosystem("go");
        if modules.is_empty() {
            info!("Strict manifest mode: no Go modules in manifest — skipping Go fetch");
            return Ok(Vec::new());
        }
        info!(
            modules = modules.len(),
            "Strict manifest mode: fetching latest versions for manifest Go modules"
        );

        let mut items = Vec::new();
        for module in modules.iter().take(self.config.max_items) {
            match self.fetch_module_latest(module).await {
                Ok(Some(item)) => items.push(item),
                Ok(None) => {}
                Err(SourceError::RateLimited(msg)) => {
                    warn!(module = %module, "Rate limited by Go proxy: {msg}");
                    break;
                }
                Err(e) => {
                    warn!(module = %module, error = ?e, "Failed to fetch Go module latest");
                }
            }
            // Gentle pacing against proxy.golang.org.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        info!(items = items.len(), "Fetched targeted Go module items");
        Ok(items)
    }

    /// Fetch the latest version of a single module from `proxy.golang.org/<mod>/@latest`.
    async fn fetch_module_latest(&self, module: &str) -> SourceResult<Option<SourceItem>> {
        let url = format!(
            "https://proxy.golang.org/{}/@latest",
            encode_module_path(module)
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
                "Go module proxy rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
            // No published version for this module path — not an error.
            return Ok(None);
        }
        super::check_http_status(status, "Go module proxy")?;

        let info: GoLatestInfo = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let source_id = format!("{}@{}", module, info.version);
        let title = format!("Go: {} {}", module, info.version);
        let pkg_url = format!("https://pkg.go.dev/{}@{}", module, info.version);

        let mut content_parts = vec![
            format!("Module: {}", module),
            format!("Version: {}", info.version),
        ];
        if let Some(ref ts) = info.time {
            content_parts.push(format!("Published: {}", ts));
        }
        let content = content_parts.join("\n");

        let mut metadata = serde_json::json!({
            "module_path": module,
            "version": info.version,
            "ecosystem": "go",
            "source_name": "go_modules",
            "manifest_grounded": true,
        });
        if let Some(ref ts) = info.time {
            metadata["timestamp"] = serde_json::json!(ts);
        }

        Ok(Some(
            SourceItem::new("go_modules", &source_id, &title)
                .with_url(Some(pkg_url))
                .with_content(content)
                .with_metadata(metadata),
        ))
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
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode targets only the stack's pinned modules via the proxy;
        // the global index below is never queried.
        if crate::source_fetching::strict_manifest_mode() {
            return self.fetch_targeted().await;
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
        super::check_http_status(status, "Go modules proxy")?;

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
                            "source_name": "go_modules",
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
    fn test_encode_module_path_escapes_uppercase() {
        // The proxy lower-cases and !-escapes capitals so case-sensitive paths resolve.
        assert_eq!(
            encode_module_path("github.com/BurntSushi/toml"),
            "github.com/!burnt!sushi/toml"
        );
        // All-lowercase paths pass through unchanged.
        assert_eq!(encode_module_path("golang.org/x/net"), "golang.org/x/net");
        // Mixed: only capitals are escaped.
        assert_eq!(
            encode_module_path("github.com/gin-gonic/Gin"),
            "github.com/gin-gonic/!gin"
        );
    }

    #[test]
    fn test_go_latest_info_parsing() {
        let json = r#"{"Version":"v1.9.1","Time":"2026-02-01T00:00:00Z"}"#;
        let info: GoLatestInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "v1.9.1");
        assert_eq!(info.time.as_deref(), Some("2026-02-01T00:00:00Z"));

        // Time is optional.
        let info: GoLatestInfo = serde_json::from_str(r#"{"Version":"v0.1.0"}"#).unwrap();
        assert_eq!(info.version, "v0.1.0");
        assert!(info.time.is_none());
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
