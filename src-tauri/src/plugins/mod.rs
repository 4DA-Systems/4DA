//! Source Plugin API — external binary plugins that provide content items.
//!
//! Plugins are standalone executables in the `plugins/` data directory.
//! They receive config on stdin (JSON) and output source items on stdout (JSON).
//! Same protocol as MCP tools — JSON-over-stdio.
//!
//! ## Directory Structure
//! ```text
//! data/plugins/
//!   my-plugin/
//!     manifest.json   # Plugin metadata (PluginManifest)
//!     my-plugin.exe   # Binary (or my-plugin on Unix)
//! ```
//!
//! ## Protocol
//! 1. 4DA sends `PluginConfig` as JSON on stdin
//! 2. Plugin outputs `PluginItem[]` as JSON on stdout
//! 3. Plugin exits (non-zero = error, stderr captured for logging)

use serde::{Deserialize, Serialize};

/// Plugin manifest — metadata about a plugin.
/// Stored as `manifest.json` in each plugin subdirectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Human-readable plugin name (also the subdirectory name)
    pub name: String,
    /// Semantic version string
    pub version: String,
    /// Short description of what this plugin provides
    pub description: String,
    /// Plugin author (optional)
    pub author: Option<String>,
    /// Binary name (relative to the plugin's subdirectory)
    pub binary: String,
    /// Default polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Maximum items to return per fetch
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

fn default_poll_interval() -> u64 {
    600 // 10 minutes
}

fn default_max_items() -> usize {
    50
}

/// Configuration sent to plugin on stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// User's tech stack (for filtering)
    pub tech_stack: Vec<String>,
    /// User's interests
    pub interests: Vec<String>,
    /// Maximum items to return
    pub max_items: usize,
    /// Custom config from user settings (plugin-specific)
    #[serde(default)]
    pub custom: serde_json::Value,
}

/// Item returned by a plugin (maps to SourceItem for the analysis pipeline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginItem {
    /// Item title
    pub title: String,
    /// URL to the content (optional)
    pub url: Option<String>,
    /// Main content text
    pub content: String,
    /// Source type identifier (e.g., "mastodon", "bluesky", "internal-wiki")
    pub source_type: String,
    /// Author name (optional)
    pub author: Option<String>,
    /// ISO 8601 publish timestamp (optional)
    pub published_at: Option<String>,
}

pub mod loader;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manifest_deserialize_with_defaults() {
        let json = r#"{
            "name": "test-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "binary": "test-plugin.exe"
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.poll_interval_secs, 600);
        assert_eq!(manifest.max_items, 50);
        assert!(manifest.author.is_none());
    }

    #[test]
    fn test_plugin_manifest_deserialize_full() {
        let json = r#"{
            "name": "mastodon-source",
            "version": "0.1.0",
            "description": "Mastodon timeline source",
            "author": "Community",
            "binary": "mastodon-source.exe",
            "poll_interval_secs": 300,
            "max_items": 100
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "mastodon-source");
        assert_eq!(manifest.author.as_deref(), Some("Community"));
        assert_eq!(manifest.poll_interval_secs, 300);
        assert_eq!(manifest.max_items, 100);
    }

    #[test]
    fn test_plugin_config_serialize() {
        let config = PluginConfig {
            tech_stack: vec!["rust".to_string(), "typescript".to_string()],
            interests: vec!["systems programming".to_string()],
            max_items: 30,
            custom: serde_json::json!({"instance": "mastodon.social"}),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("rust"));
        assert!(json.contains("mastodon.social"));
    }

    #[test]
    fn test_plugin_item_deserialize() {
        let json = r#"{
            "title": "New Rust Release",
            "url": "https://blog.rust-lang.org/2026/01/01/release.html",
            "content": "Rust 1.85 has been released with exciting new features.",
            "source_type": "rss-custom",
            "author": "Rust Team",
            "published_at": "2026-01-01T00:00:00Z"
        }"#;
        let item: PluginItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.title, "New Rust Release");
        assert_eq!(item.source_type, "rss-custom");
        assert!(item.url.is_some());
    }

    #[test]
    fn test_plugin_item_minimal() {
        let json = r#"{
            "title": "Untitled",
            "content": "Some content",
            "source_type": "custom",
            "url": null,
            "author": null,
            "published_at": null
        }"#;
        let item: PluginItem = serde_json::from_str(json).unwrap();
        assert!(item.url.is_none());
        assert!(item.author.is_none());
    }
}
