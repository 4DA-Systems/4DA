//! Product Hunt source implementation
//!
//! Fetches featured products from Product Hunt RSS feed and extracts metadata.

use async_trait::async_trait;
use tracing::info;

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// Product Hunt RSS Feed Entry
// ============================================================================

#[derive(Debug)]
struct ProductHuntItem {
    title: String,
    link: String,
    description: String,
    pub_date: String,
    upvotes: Option<i32>,
    comments: Option<i32>,
}

// ============================================================================
// Product Hunt Source
// ============================================================================

/// Product Hunt source - fetches featured products from RSS feed
pub struct ProductHuntSource {
    config: SourceConfig,
    client: reqwest::Client,
    #[allow(dead_code)] // Future: category filtering
    categories: Vec<String>,
}

impl ProductHuntSource {
    /// Create a new Product Hunt source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
            categories: vec!["tech".into(), "developer-tools".into()],
        }
    }

    /// Create with custom categories
    #[allow(dead_code)] // Future: category filtering
    pub fn with_categories(categories: Vec<String>) -> Self {
        let mut source = Self::new();
        source.categories = categories;
        source
    }

    /// Parse Product Hunt RSS feed (XML)
    fn parse_feed(&self, xml: &str) -> Result<Vec<ProductHuntItem>, String> {
        let mut items = Vec::new();

        // Find all <item> tags
        let item_pattern = "<item>";
        let item_end = "</item>";

        let mut start = 0;
        while let Some(item_start) = xml[start..].find(item_pattern) {
            let item_offset = start + item_start;
            if let Some(item_end_pos) = xml[item_offset..].find(item_end) {
                let item_xml = &xml[item_offset..item_offset + item_end_pos + item_end.len()];

                // Extract fields
                let title = extract_tag(item_xml, "title").unwrap_or_default();
                let link = extract_tag(item_xml, "link").unwrap_or_default();
                let description = extract_tag(item_xml, "description").unwrap_or_default();
                let pub_date = extract_tag(item_xml, "pubDate").unwrap_or_default();

                // Extract upvotes/comments from description
                let upvotes = extract_upvotes(&description);
                let comments = extract_comments(&description);

                items.push(ProductHuntItem {
                    title,
                    link,
                    description,
                    pub_date,
                    upvotes,
                    comments,
                });

                start = item_offset + item_end_pos + item_end.len();
            } else {
                break;
            }
        }

        Ok(items)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract content from XML tag
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)?;

    let content = xml[start..start + end].trim();

    // Handle CDATA sections
    if content.starts_with("<![CDATA[") && content.ends_with("]]>") {
        Some(content[9..content.len() - 3].to_string())
    } else {
        Some(content.to_string())
    }
}

/// Extract upvotes from description text
fn extract_upvotes(description: &str) -> Option<i32> {
    // Product Hunt descriptions often include "👍 123 upvotes"
    if let Some(idx) = description.find("upvote") {
        let before = &description[..idx];
        // Find the last number before "upvote"
        if let Some(num_str) = before.split_whitespace().last() {
            return num_str.parse::<i32>().ok();
        }
    }
    None
}

/// Extract comments from description text
fn extract_comments(description: &str) -> Option<i32> {
    // Similar pattern for comments
    if let Some(idx) = description.find("comment") {
        let before = &description[..idx];
        if let Some(num_str) = before.split_whitespace().last() {
            return num_str.parse::<i32>().ok();
        }
    }
    None
}

impl Default for ProductHuntSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for ProductHuntSource {
    fn source_type(&self) -> &'static str {
        "producthunt"
    }

    fn name(&self) -> &'static str {
        "Product Hunt"
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

        info!("Fetching Product Hunt feed");

        let url = "https://www.producthunt.com/feed";

        let xml = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let ph_items = self.parse_feed(&xml).map_err(SourceError::Parse)?;

        info!(
            total = ph_items.len(),
            max_items = self.config.max_items,
            "Parsed Product Hunt items"
        );

        let items: Vec<SourceItem> = ph_items
            .into_iter()
            .take(self.config.max_items)
            .map(|ph_item| {
                // Build metadata with conditionally added fields
                let mut metadata = serde_json::json!({
                    "pub_date": ph_item.pub_date,
                });

                if let Some(upvotes) = ph_item.upvotes {
                    metadata["upvotes"] = serde_json::json!(upvotes);
                }

                if let Some(comments) = ph_item.comments {
                    metadata["comments"] = serde_json::json!(comments);
                }

                SourceItem::new("producthunt", &ph_item.link, &ph_item.title)
                    .with_url(Some(ph_item.link.clone()))
                    .with_content(ph_item.description.clone())
                    .with_metadata(metadata)
            })
            .collect();

        info!(items = items.len(), "Created Product Hunt source items");

        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // For Product Hunt, the description from RSS is usually sufficient
        // But we could optionally scrape the full page here
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
    fn test_producthunt_source_creation() {
        let source = ProductHuntSource::new();
        assert_eq!(source.source_type(), "producthunt");
        assert_eq!(source.name(), "Product Hunt");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_custom_categories() {
        let source = ProductHuntSource::with_categories(vec!["ai".into(), "devtools".into()]);
        assert_eq!(source.categories.len(), 2);
        assert_eq!(source.categories[0], "ai");
    }

    #[test]
    fn test_extract_tag() {
        let xml = "<title>Test Product</title>";
        assert_eq!(extract_tag(xml, "title"), Some("Test Product".to_string()));
    }

    #[test]
    fn test_extract_tag_with_cdata() {
        let xml = "<description><![CDATA[Test content]]></description>";
        assert_eq!(
            extract_tag(xml, "description"),
            Some("Test content".to_string())
        );
    }

    #[test]
    fn test_extract_upvotes() {
        let desc = "Great product with 123 upvotes and 45 comments";
        assert_eq!(extract_upvotes(desc), Some(123));
    }

    #[test]
    fn test_extract_comments() {
        let desc = "Great product with 123 upvotes and 45 comments";
        assert_eq!(extract_comments(desc), Some(45));
    }

    #[test]
    fn test_extract_upvotes_no_match() {
        let desc = "Great product";
        assert_eq!(extract_upvotes(desc), None);
    }

    #[test]
    fn test_parse_feed_sample() {
        let source = ProductHuntSource::new();
        let sample_xml = r#"
        <?xml version="1.0"?>
        <rss version="2.0">
        <channel>
            <title>Product Hunt Feed</title>
            <item>
                <title>Amazing AI Tool</title>
                <link>https://www.producthunt.com/posts/amazing-ai-tool</link>
                <description>Revolutionary AI tool with 250 upvotes and 32 comments</description>
                <pubDate>Mon, 03 Feb 2026 00:00:00 GMT</pubDate>
            </item>
        </channel>
        </rss>
        "#;

        let items = source.parse_feed(sample_xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Amazing AI Tool");
        assert!(items[0].link.contains("amazing-ai-tool"));
        assert_eq!(items[0].upvotes, Some(250));
        assert_eq!(items[0].comments, Some(32));
    }
}
