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
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::info;

pub mod arxiv;
pub mod devto;
pub mod github;
pub mod hackernews;
pub mod lobsters;
pub mod producthunt;
pub mod reddit;
pub mod rss;
pub mod twitter;
pub mod youtube;

/// Shared HTTP client for all source adapters.
/// reqwest::Client uses Arc internally, so cloning is free.
/// Avoids creating 7+ clients per analysis run (TLS setup, connection pool init).
static SHARED_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("4DA/1.0")
        .build()
        .unwrap_or_else(|e| {
            tracing::error!("Failed to create configured HTTP client, using default: {e}");
            reqwest::Client::new()
        })
});

/// Get the shared HTTP client (clone is free - Arc-based)
pub fn shared_client() -> reqwest::Client {
    SHARED_CLIENT.clone()
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
    #[allow(dead_code)] // Future: embedding generation
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
    /// Rate limited by source
    #[allow(dead_code)]
    RateLimited,
    /// Source is disabled
    Disabled,
    /// Other error
    Other(String),
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::Network(msg) => write!(f, "Network error: {}", msg),
            SourceError::Parse(msg) => write!(f, "Parse error: {}", msg),
            SourceError::RateLimited => write!(f, "Rate limited"),
            SourceError::Disabled => write!(f, "Source disabled"),
            SourceError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for SourceError {}

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
    #[allow(dead_code)] // Future: dynamic source configuration
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

    /// Check if enough time has passed since last fetch
    #[allow(dead_code)] // Future: rate limiting
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
    #[allow(dead_code)] // Future: source filtering
    pub fn enabled_sources(&self) -> Vec<&dyn Source> {
        self.sources
            .iter()
            .filter(|s| s.config().enabled)
            .map(|s| s.as_ref())
            .collect()
    }

    /// Get a source by type
    #[allow(dead_code)] // Future: source lookup
    pub fn get_source(&self, source_type: &str) -> Option<&dyn Source> {
        self.sources
            .iter()
            .find(|s| s.source_type() == source_type)
            .map(|s| s.as_ref())
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
// Tests
// ============================================================================

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
