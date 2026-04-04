//! npm registry source adapter
//!
//! Monitors npm packages for new versions, deprecations, and breaking changes.
//! Phase 1: checks a curated list of popular packages + user-configured packages.
//! Phase 2 (future): integrates with ACE to read package.json dependencies.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// npm Registry API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct NpmPackageInfo {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "dist-tags")]
    dist_tags: Option<NpmDistTags>,
    time: Option<HashMap<String, String>>,
    deprecated: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NpmDistTags {
    latest: Option<String>,
    #[allow(dead_code)]
    next: Option<String>,
}

// ============================================================================
// Default package lists
// ============================================================================

/// Popular packages monitored by default (Phase 1).
const DEFAULT_PACKAGES: &[&str] = &[
    "react",
    "next",
    "typescript",
    "vite",
    "eslint",
    "tailwindcss",
    "express",
    "fastify",
    "prisma",
    "drizzle-orm",
    "zod",
    "trpc",
];

/// Extended list used during deep fetch.
const EXTENDED_PACKAGES: &[&str] = &[
    "svelte",
    "vue",
    "angular",
    "remix",
    "astro",
    "esbuild",
    "webpack",
    "turbo",
    "vitest",
    "playwright",
    "tanstack-query",
    "zustand",
];

/// Delay between individual package requests to avoid hammering the registry.
const REQUEST_DELAY_MS: u64 = 500;

// ============================================================================
// npm Registry Source
// ============================================================================

/// npm registry source — monitors packages for new versions and deprecations.
pub struct NpmRegistrySource {
    config: SourceConfig,
    client: reqwest::Client,
    packages: Vec<String>,
}

impl NpmRegistrySource {
    /// Create a new npm registry source with default popular packages.
    pub fn new() -> Self {
        // Use real user deps from ACE if available, fall back to popular defaults
        let ace_packages = crate::source_fetching::load_ace_packages_for_ecosystem("npm");
        let packages = if ace_packages.is_empty() {
            DEFAULT_PACKAGES.iter().map(|s| s.to_string()).collect()
        } else {
            ace_packages
        };
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
            packages,
        }
    }

    /// Create with a custom package list.
    pub fn with_packages(packages: Vec<String>) -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600,
                custom: None,
            },
            client: super::shared_client(),
            packages,
        }
    }

    /// Fetch metadata for a single package from the npm registry.
    async fn fetch_package(&self, package: &str) -> SourceResult<Option<SourceItem>> {
        let url = format!("https://registry.npmjs.org/{}", package);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.npm.install-v1+json")
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "npm registry rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            warn!(package = %package, "npm package not found, skipping");
            return Ok(None);
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "npm registry forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "npm registry error: HTTP {}",
                status.as_u16()
            )));
        }

        let info: NpmPackageInfo = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        Ok(Some(package_to_source_item(package, &info)))
    }

    /// Fetch items for a given package list with rate-limiting delays.
    async fn fetch_package_list(
        &self,
        packages: &[String],
        max: usize,
    ) -> SourceResult<Vec<SourceItem>> {
        let mut items = Vec::new();

        for (i, package) in packages.iter().enumerate() {
            if items.len() >= max {
                break;
            }

            // Rate-limit: delay between requests (skip delay before first)
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(REQUEST_DELAY_MS)).await;
            }

            match self.fetch_package(package).await {
                Ok(Some(item)) => items.push(item),
                Ok(None) => {} // 404 — skip
                Err(SourceError::RateLimited(msg)) => {
                    warn!(error = %msg, "Rate limited, stopping npm fetch early");
                    break;
                }
                Err(e) => {
                    warn!(package = %package, error = ?e, "Failed to fetch npm package");
                }
            }
        }

        Ok(items)
    }
}

impl Default for NpmRegistrySource {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Source Trait Implementation
// ============================================================================

#[async_trait]
impl Source for NpmRegistrySource {
    fn source_type(&self) -> &'static str {
        "npm_registry"
    }

    fn name(&self) -> &'static str {
        "npm Registry"
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
            label: "npm",
            color_hint: "red",
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!(count = self.packages.len(), "Fetching npm registry updates");

        let items = self
            .fetch_package_list(&self.packages, self.config.max_items)
            .await?;

        info!(items = items.len(), "Fetched npm registry items");
        Ok(items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!("Deep fetching npm registry (default + extended packages)");

        // Combine default + extended, dedup
        let mut all_packages = self.packages.clone();
        let mut seen: std::collections::HashSet<String> = all_packages.iter().cloned().collect();
        for pkg in EXTENDED_PACKAGES {
            let s = pkg.to_string();
            if seen.insert(s.clone()) {
                all_packages.push(s);
            }
        }

        let items = self
            .fetch_package_list(&all_packages, self.config.max_items * 2)
            .await?;

        info!(total = items.len(), "Total npm registry items (deep)");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Content is already populated during fetch
        Ok(item.content.clone())
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

/// Convert npm package metadata into a SourceItem.
fn package_to_source_item(package_name: &str, info: &NpmPackageInfo) -> SourceItem {
    let name = info.name.as_deref().unwrap_or(package_name);
    let description = info.description.as_deref().unwrap_or("No description");
    let latest_version = info
        .dist_tags
        .as_ref()
        .and_then(|dt| dt.latest.as_deref())
        .unwrap_or("unknown");

    // Check if the package-level deprecated field is set
    let is_deprecated = info.deprecated.as_ref().is_some_and(|v| !v.is_null());
    let deprecation_msg = info
        .deprecated
        .as_ref()
        .and_then(|v| v.as_str())
        .map(String::from);

    // Find the publish date for the latest version
    let published_date = info
        .time
        .as_ref()
        .and_then(|t| t.get(latest_version).cloned())
        .or_else(|| info.time.as_ref().and_then(|t| t.get("modified").cloned()));

    let version_count = info
        .time
        .as_ref()
        .map(|t| {
            // Subtract non-version keys: "created", "modified"
            t.len().saturating_sub(2)
        })
        .unwrap_or(0);

    // Build title
    let title = if is_deprecated {
        format!("[DEPRECATED] {name}")
    } else {
        format!("npm: {name} v{latest_version}")
    };

    // Build content
    let mut content = description.to_string();
    if let Some(date) = &published_date {
        content.push_str(&format!("\nLast published: {date}"));
    }
    if version_count > 0 {
        content.push_str(&format!("\nTotal versions: {version_count}"));
    }
    if let Some(msg) = &deprecation_msg {
        content.push_str(&format!("\nDeprecation notice: {msg}"));
    }

    // Build metadata
    let mut metadata = serde_json::json!({
        "latest_version": latest_version,
        "deprecated": is_deprecated,
        "ecosystem": "npm",
    });
    if let Some(date) = &published_date {
        metadata["published_date"] = serde_json::json!(date);
    }
    if version_count > 0 {
        metadata["version_count"] = serde_json::json!(version_count);
    }

    let npm_url = format!("https://www.npmjs.com/package/{name}");
    let source_id = format!("{name}@{latest_version}");

    SourceItem::new("npm_registry", &source_id, &title)
        .with_url(Some(npm_url))
        .with_content(content)
        .with_metadata(metadata)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npm_source_creation() {
        let source = NpmRegistrySource::new();
        assert_eq!(source.source_type(), "npm_registry");
        assert_eq!(source.name(), "npm Registry");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 3600);
        assert_eq!(source.packages.len(), DEFAULT_PACKAGES.len());
    }

    #[test]
    fn test_npm_source_with_packages() {
        let source = NpmRegistrySource::with_packages(vec!["lodash".into(), "axios".into()]);
        assert_eq!(source.packages, vec!["lodash", "axios"]);
        assert_eq!(source.source_type(), "npm_registry");
    }

    #[test]
    fn test_npm_source_default_trait() {
        let source = NpmRegistrySource::default();
        assert_eq!(source.source_type(), "npm_registry");
    }

    #[test]
    fn test_package_to_source_item_normal() {
        let info = NpmPackageInfo {
            name: Some("vite".to_string()),
            description: Some("Next generation frontend tooling".to_string()),
            dist_tags: Some(NpmDistTags {
                latest: Some("6.2.0".to_string()),
                next: None,
            }),
            time: Some(HashMap::from([
                ("created".to_string(), "2020-04-21T00:00:00Z".to_string()),
                ("modified".to_string(), "2026-03-28T10:00:00Z".to_string()),
                ("6.2.0".to_string(), "2026-03-25T12:00:00Z".to_string()),
                ("6.1.0".to_string(), "2026-02-10T08:00:00Z".to_string()),
            ])),
            deprecated: None,
        };

        let item = package_to_source_item("vite", &info);

        assert_eq!(item.source_type, "npm_registry");
        assert_eq!(item.source_id, "vite@6.2.0");
        assert_eq!(item.title, "npm: vite v6.2.0");
        assert_eq!(
            item.url,
            Some("https://www.npmjs.com/package/vite".to_string())
        );
        assert!(item.content.contains("Next generation frontend tooling"));
        assert!(item
            .content
            .contains("Last published: 2026-03-25T12:00:00Z"));
        assert!(item.content.contains("Total versions: 2"));

        let metadata = item.metadata.unwrap();
        assert_eq!(metadata["latest_version"], "6.2.0");
        assert_eq!(metadata["deprecated"], false);
        assert_eq!(metadata["ecosystem"], "npm");
        assert_eq!(metadata["published_date"], "2026-03-25T12:00:00Z");
        assert_eq!(metadata["version_count"], 2);
    }

    #[test]
    fn test_package_to_source_item_deprecated() {
        let info = NpmPackageInfo {
            name: Some("request".to_string()),
            description: Some("Simplified HTTP request client".to_string()),
            dist_tags: Some(NpmDistTags {
                latest: Some("2.88.2".to_string()),
                next: None,
            }),
            time: None,
            deprecated: Some(serde_json::json!("Use 'got' or 'node-fetch' instead")),
        };

        let item = package_to_source_item("request", &info);

        assert_eq!(item.title, "[DEPRECATED] request");
        assert!(item
            .content
            .contains("Deprecation notice: Use 'got' or 'node-fetch' instead"));

        let metadata = item.metadata.unwrap();
        assert_eq!(metadata["deprecated"], true);
    }

    #[test]
    fn test_package_to_source_item_minimal() {
        let info = NpmPackageInfo {
            name: None,
            description: None,
            dist_tags: None,
            time: None,
            deprecated: None,
        };

        let item = package_to_source_item("mystery-pkg", &info);

        assert_eq!(item.source_id, "mystery-pkg@unknown");
        assert_eq!(item.title, "npm: mystery-pkg vunknown");
        assert!(item.content.contains("No description"));
        assert_eq!(
            item.url,
            Some("https://www.npmjs.com/package/mystery-pkg".to_string())
        );
    }

    #[test]
    fn test_npm_json_parsing() {
        let json = r#"{
            "name": "zod",
            "description": "TypeScript-first schema validation",
            "dist-tags": {
                "latest": "3.23.0",
                "next": "4.0.0-beta.1"
            },
            "time": {
                "created": "2020-03-07T00:00:00Z",
                "modified": "2026-03-20T00:00:00Z",
                "3.23.0": "2026-03-18T10:30:00Z"
            }
        }"#;

        let info: NpmPackageInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name.as_deref(), Some("zod"));
        assert_eq!(
            info.description.as_deref(),
            Some("TypeScript-first schema validation")
        );
        assert_eq!(
            info.dist_tags.as_ref().unwrap().latest.as_deref(),
            Some("3.23.0")
        );
        assert_eq!(
            info.dist_tags.as_ref().unwrap().next.as_deref(),
            Some("4.0.0-beta.1")
        );
        assert!(info.time.as_ref().unwrap().contains_key("3.23.0"));
        assert!(info.deprecated.is_none());
    }
}
