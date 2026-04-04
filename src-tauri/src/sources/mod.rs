//! Source abstraction layer for 4DA
//!
//! This module defines the trait that all information sources must implement.
//! Sources include: Hacker News, arXiv, RSS feeds, GitHub, Reddit, etc.
//!
//! The abstraction enables:
//! - Easy addition of new sources
//! - Consistent interface for fetching and processing
//! - Unified caching strategy
//! - Parallel fetching across sources

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

pub mod arxiv;
pub mod bluesky;
pub mod crates_io;
pub mod cve;
pub(crate) mod cve_matching;
pub mod devto;
pub mod fallback;
pub mod freshness;
pub mod github;
pub mod go_modules;
pub mod hackernews;
pub mod huggingface;
pub mod lobsters;
pub mod npm_registry;
pub mod osv;
pub mod papers_with_code;
pub mod producthunt;
pub mod pypi;
pub mod rate_limiter;
pub mod reddit;
pub mod rss;
pub mod stackoverflow;
pub mod twitter;
pub mod youtube;

/// Get the shared HTTP client (clone is free - Arc-based).
/// Delegates to `crate::http_client::HTTP_CLIENT` — single connection pool for the whole app.
pub fn shared_client() -> reqwest::Client {
    crate::http_client::HTTP_CLIENT.clone()
}

// ============================================================================
// Source Item - Universal representation of content from any source
// ============================================================================

/// A single item from any source (HN story, arXiv paper, RSS entry, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceItem {
    /// Unique identifier within the source (e.g., HN id, arXiv id)
    pub source_id: String,

    /// The source type this came from
    pub source_type: String,

    /// Item title
    pub title: String,

    /// Optional URL to the content
    pub url: Option<String>,

    /// Scraped/extracted content (may be empty if scraping failed)
    pub content: String,

    /// Optional metadata (authors, tags, score, etc.)
    pub metadata: Option<serde_json::Value>,
}

impl SourceItem {
    /// Create a new source item
    pub fn new(source_type: &str, source_id: &str, title: &str) -> Self {
        Self {
            source_id: source_id.to_string(),
            source_type: source_type.to_string(),
            title: title.to_string(),
            url: None,
            content: String::new(),
            metadata: None,
        }
    }

    /// Set the URL
    pub fn with_url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }

    /// Set the content
    pub fn with_content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build text for embedding (title + content)
    pub fn embedding_text(&self) -> String {
        if self.content.is_empty() {
            self.title.clone()
        } else {
            format!("{}\n\n{}", self.title, self.content)
        }
    }
}

// ============================================================================
// Source Trait - Interface all sources must implement
// ============================================================================

/// Configuration for a source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Whether this source is enabled
    pub enabled: bool,

    /// Maximum items to fetch per update
    pub max_items: usize,

    /// Minimum seconds between fetches
    pub fetch_interval_secs: u64,

    /// Source-specific configuration (JSON)
    pub custom: Option<serde_json::Value>,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_items: 30,
            fetch_interval_secs: 300, // 5 minutes
            custom: None,
        }
    }
}

/// Result type for source operations
pub type SourceResult<T> = Result<T, SourceError>;

/// Errors that can occur during source operations
#[derive(Debug, Clone)]
pub enum SourceError {
    /// Network error fetching data
    Network(String),
    /// Error parsing response
    Parse(String),
    /// Rate limited by source (HTTP 429)
    RateLimited(String),
    /// Forbidden / auth error (HTTP 403) — not retryable
    Forbidden(String),
    /// Source is disabled
    Disabled,
    /// Other error
    Other(String),
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::Network(msg) => write!(f, "Network error: {msg}"),
            SourceError::Parse(msg) => write!(f, "Parse error: {msg}"),
            SourceError::RateLimited(msg) => write!(f, "Rate limited: {msg}"),
            SourceError::Forbidden(msg) => write!(f, "Forbidden: {msg}"),
            SourceError::Disabled => write!(f, "Source disabled"),
            SourceError::Other(msg) => write!(f, "Error: {msg}"),
        }
    }
}

impl std::error::Error for SourceError {}

// ============================================================================
// Source Manifest - Declarative metadata for scaling to 100+ sources
// ============================================================================

/// Category for grouping sources in the UI and filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCategory {
    Security,
    PackageRegistry,
    News,
    Social,
    Research,
    Community,
    General,
}

impl SourceCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Security => "Security",
            Self::PackageRegistry => "Packages",
            Self::News => "News",
            Self::Social => "Social",
            Self::Research => "Research",
            Self::Community => "Community",
            Self::General => "Other",
        }
    }
}

/// Declarative metadata for a source — category, default content type,
/// display hints. Sources declare this once; the system uses it for
/// classification, UI grouping, and registration without hardcoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceManifest {
    pub category: SourceCategory,
    /// Content type slug (e.g., "security_advisory", "release_notes", "discussion")
    pub default_content_type: &'static str,
    /// Scoring multiplier for this content type (e.g., 1.30 for security)
    pub default_multiplier: f32,
    /// Short display label (e.g., "HN", "CVE", "npm")
    pub label: &'static str,
    /// Color hint for UI badges (e.g., "orange", "red", "blue")
    pub color_hint: &'static str,
}

impl Default for SourceManifest {
    fn default() -> Self {
        Self {
            category: SourceCategory::General,
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "",
            color_hint: "gray",
        }
    }
}

/// The trait that all information sources must implement
#[async_trait]
pub trait Source: Send + Sync {
    /// Unique identifier for this source type (e.g., "hackernews", "arxiv")
    fn source_type(&self) -> &'static str;

    /// Human-readable name for this source
    fn name(&self) -> &'static str;

    /// Get the current configuration
    fn config(&self) -> &SourceConfig;

    /// Update the configuration
    fn set_config(&mut self, config: SourceConfig);

    /// Fetch new items from this source
    ///
    /// Returns a list of items without content populated.
    /// Content scraping happens separately for better parallelization.
    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>>;

    /// Deep fetch for comprehensive initial scans
    ///
    /// Sources can override this to fetch from multiple endpoints/categories.
    /// Default implementation just calls fetch_items().
    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        // Default: just use regular fetch
        self.fetch_items().await
    }

    /// Scrape/extract content for a single item
    ///
    /// This is called separately from fetch_items to allow parallel scraping.
    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String>;

    /// Declarative metadata — category, default content type, display hints.
    /// Override this to declare your source's identity. Default: General/Discussion.
    fn manifest(&self) -> SourceManifest {
        SourceManifest::default()
    }

    /// Check if enough time has passed since last fetch
    fn should_fetch(&self, last_fetch: Option<std::time::SystemTime>) -> bool {
        match last_fetch {
            None => true,
            Some(last) => {
                let elapsed = last.elapsed().unwrap_or_default();
                elapsed.as_secs() >= self.config().fetch_interval_secs
            }
        }
    }
}

// ============================================================================
// Source Registry - Manages all available sources
// ============================================================================

/// Registry of all available sources
pub struct SourceRegistry {
    sources: Vec<Box<dyn Source>>,
}

impl SourceRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Register a source
    pub fn register(&mut self, source: Box<dyn Source>) {
        info!(
            name = source.name(),
            source_type = source.source_type(),
            "Registered source"
        );
        self.sources.push(source);
    }

    /// Get all registered sources
    pub fn sources(&self) -> &[Box<dyn Source>] {
        &self.sources
    }

    /// Get enabled sources only
    pub fn enabled_sources(&self) -> Vec<&dyn Source> {
        self.sources
            .iter()
            .filter(|s| s.config().enabled)
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    /// Get a source by type
    pub fn get_source(&self, source_type: &str) -> Option<&dyn Source> {
        self.sources
            .iter()
            .find(|s| s.source_type() == source_type)
            .map(std::convert::AsRef::as_ref)
    }

    /// Get source count
    pub fn count(&self) -> usize {
        self.sources.len()
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Canonical Source Factory — THE ONLY place sources are instantiated
// ============================================================================

/// Build all source adapters. This is the single source of truth for
/// source instantiation. Adding a new source: create the file, add one
/// line here. Everything else (registry, DB, frontend) follows automatically.
pub fn build_all_sources() -> Vec<Box<dyn Source>> {
    use crate::source_fetching::{
        load_github_languages_from_settings, load_rss_feeds_from_settings,
        load_twitter_settings, load_youtube_channels_from_settings,
    };

    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();
    let github_languages = load_github_languages_from_settings();

    vec![
        // News
        Box::new(hackernews::HackerNewsSource::new()),
        Box::new(github::GitHubSource::with_languages(github_languages)),
        Box::new(rss::RssSource::with_feeds(rss_feeds)),
        Box::new(youtube::YouTubeSource::with_channels(youtube_channels)),
        Box::new(producthunt::ProductHuntSource::new()),
        // Research
        Box::new(arxiv::ArxivSource::new()),
        Box::new(huggingface::HuggingFaceSource::new()),
        Box::new(papers_with_code::PapersWithCodeSource::new()),
        // Community
        Box::new(reddit::RedditSource::new()),
        Box::new(lobsters::LobstersSource::new()),
        Box::new(devto::DevtoSource::new()),
        Box::new(stackoverflow::StackOverflowSource::new()),
        // Social
        Box::new(twitter::TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key)),
        Box::new(bluesky::BlueskySource::new()),
        // Security
        Box::new(cve::CveSource::new()),
        Box::new(osv::OsvSource::new()),
        // Package Registries (ACE-integrated)
        Box::new(npm_registry::NpmRegistrySource::new()),
        Box::new(pypi::PypiSource::new()),
        Box::new(crates_io::CratesIoSource::new()),
        Box::new(go_modules::GoModulesSource::new()),
    ]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod adapter_resilience_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_item_embedding_text() {
        let item = SourceItem::new("test", "123", "Test Title");
        assert_eq!(item.embedding_text(), "Test Title");

        let item_with_content = item.with_content("Some content here".to_string());
        assert_eq!(
            item_with_content.embedding_text(),
            "Test Title\n\nSome content here"
        );
    }

    #[test]
    fn test_source_config_default() {
        let config = SourceConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_items, 30);
        assert_eq!(config.fetch_interval_secs, 300);
    }
}
