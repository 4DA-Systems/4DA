// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! RSS/Atom source implementation
//!
//! Fetches items from configured RSS and Atom feeds.

use async_trait::async_trait;
use scraper::{Html, Selector};
use tracing::{debug, info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

/// Maximum items to parse from a single feed (prevents OOM from malicious feeds)
const MAX_ITEMS_PER_FEED: usize = 200;

/// Maximum content length per feed item (100KB)
const MAX_ITEM_CONTENT_LEN: usize = 100_000;

/// Maximum RSS response size (10MB) — prevents malicious feed flooding
const MAX_RSS_RESPONSE: u64 = 10 * 1024 * 1024;

// ============================================================================
// RSS Feed Entry
// ============================================================================

#[derive(Debug)]
pub(crate) struct FeedEntry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) link: String,
    pub(crate) description: String,
    pub(crate) pub_date: Option<String>,
    pub(crate) feed_title: String,
}

// ============================================================================
// RSS Source
// ============================================================================

/// RSS/Atom source - fetches items from configured feed URLs
pub struct RssSource {
    config: SourceConfig,
    client: reqwest::Client,
    /// Feed URLs to fetch
    feed_urls: Vec<String>,
    /// Per-feed errors recorded during the last `fetch_items()` call
    feed_errors: std::sync::Mutex<Vec<(String, String)>>,
}

impl RssSource {
    /// Create a new RSS source with default feeds
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 1800, // 30 minutes
                custom: None,
            },
            client: super::shared_client(),
            // Default quality tech feeds — users can add more via settings
            feed_urls: vec![
                "https://feeds.arstechnica.com/arstechnica/technology-lab".to_string(),
                "https://www.theverge.com/rss/index.xml".to_string(),
                "https://techcrunch.com/feed/".to_string(),
                "https://blog.rust-lang.org/feed.xml".to_string(),
                "https://engineering.fb.com/feed/".to_string(),
                "https://netflixtechblog.com/feed".to_string(),
                "https://github.blog/feed/".to_string(),
                "https://blog.cloudflare.com/rss/".to_string(),
                "https://martinfowler.com/feed.atom".to_string(),
                "https://simonwillison.net/atom/everything/".to_string(),
                "https://jvns.ca/atom.xml".to_string(),
                "https://danluu.com/atom.xml".to_string(),
            ],
            feed_errors: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create with custom feed URLs (empty vec preserves defaults)
    pub fn with_feeds(feeds: Vec<String>) -> Self {
        let mut source = Self::new();
        if !feeds.is_empty() {
            source.feed_urls = feeds;
        }
        source
    }

    /// Add a feed URL
    pub fn add_feed(&mut self, url: String) {
        if !self.feed_urls.contains(&url) {
            self.feed_urls.push(url);
        }
    }

    /// Get configured feed URLs
    pub fn feed_urls(&self) -> &[String] {
        &self.feed_urls
    }

    /// Parse RSS 2.0 feed
    pub(crate) fn parse_rss_feed(&self, xml: &str, feed_url: &str) -> Vec<FeedEntry> {
        let mut entries = Vec::new();

        // Extract feed title
        let feed_title = Self::extract_tag(xml, "title").unwrap_or_else(|| feed_url.to_string());

        // Find <item> blocks (RSS 2.0)
        for item_block in xml.split("<item>").skip(1) {
            if entries.len() >= MAX_ITEMS_PER_FEED {
                break;
            }
            let item_end = item_block.find("</item>").unwrap_or(item_block.len());
            let item_xml = &item_block[..item_end];

            let title = Self::extract_tag(item_xml, "title")
                .map(|t| Self::decode_html_entities(&t))
                .unwrap_or_default();

            let link = Self::extract_tag(item_xml, "link")
                .or_else(|| Self::extract_guid(item_xml))
                .unwrap_or_default();

            let description = Self::extract_tag(item_xml, "description")
                .or_else(|| Self::extract_tag(item_xml, "content:encoded"))
                .map(|d| Self::strip_html(&Self::decode_html_entities(&d)))
                .unwrap_or_default();

            // Cap per-item content to prevent memory abuse
            let description = if description.len() > MAX_ITEM_CONTENT_LEN {
                description[..MAX_ITEM_CONTENT_LEN].to_string()
            } else {
                description
            };

            let pub_date = Self::extract_tag(item_xml, "pubDate")
                .or_else(|| Self::extract_tag(item_xml, "dc:date"));

            // Generate ID from link or title
            let id = if link.is_empty() {
                Self::generate_id(&title)
            } else {
                Self::generate_id(&link)
            };

            if !title.is_empty() && !link.is_empty() {
                entries.push(FeedEntry {
                    id,
                    title,
                    link,
                    description,
                    pub_date,
                    feed_title: feed_title.clone(),
                });
            }
        }

        entries
    }

    /// Parse Atom feed
    pub(crate) fn parse_atom_feed(&self, xml: &str, feed_url: &str) -> Vec<FeedEntry> {
        let mut entries = Vec::new();

        // Extract feed title
        let feed_title = Self::extract_tag(xml, "title").unwrap_or_else(|| feed_url.to_string());

        // Find <entry> blocks (Atom)
        for entry_block in xml.split("<entry>").skip(1) {
            if entries.len() >= MAX_ITEMS_PER_FEED {
                break;
            }
            let entry_end = entry_block.find("</entry>").unwrap_or(entry_block.len());
            let entry_xml = &entry_block[..entry_end];

            let title = Self::extract_tag(entry_xml, "title")
                .map(|t| Self::decode_html_entities(&t))
                .unwrap_or_default();

            // Atom links are in <link href="..." /> format
            let link = Self::extract_atom_link(entry_xml)
                .or_else(|| Self::extract_tag(entry_xml, "id"))
                .unwrap_or_default();

            let description = Self::extract_tag(entry_xml, "summary")
                .or_else(|| Self::extract_tag(entry_xml, "content"))
                .map(|d| Self::strip_html(&Self::decode_html_entities(&d)))
                .unwrap_or_default();

            // Cap per-item content to prevent memory abuse
            let description = if description.len() > MAX_ITEM_CONTENT_LEN {
                description[..MAX_ITEM_CONTENT_LEN].to_string()
            } else {
                description
            };

            let pub_date = Self::extract_tag(entry_xml, "published")
                .or_else(|| Self::extract_tag(entry_xml, "updated"));

            let id = Self::extract_tag(entry_xml, "id")
                .map_or_else(|| Self::generate_id(&link), |i| Self::generate_id(&i));

            if !title.is_empty() && !link.is_empty() {
                entries.push(FeedEntry {
                    id,
                    title,
                    link,
                    description,
                    pub_date,
                    feed_title: feed_title.clone(),
                });
            }
        }

        entries
    }

    /// Extract content from XML tag
    pub(crate) fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{tag}");
        let close_tag = format!("</{tag}>");

        let start_pos = xml.find(&open_tag)?;
        let content_start = xml[start_pos..].find('>')? + start_pos + 1;
        let end_pos = xml[content_start..].find(&close_tag)? + content_start;

        let content = xml[content_start..end_pos].trim();

        // Handle CDATA sections
        if content.starts_with("<![CDATA[") && content.ends_with("]]>") {
            Some(content[9..content.len() - 3].to_string())
        } else {
            Some(content.to_string())
        }
    }

    /// Extract guid element (RSS)
    fn extract_guid(xml: &str) -> Option<String> {
        Self::extract_tag(xml, "guid")
    }

    /// Extract link from Atom format (href attribute)
    fn extract_atom_link(xml: &str) -> Option<String> {
        // Look for <link href="..." rel="alternate" /> or just <link href="..." />
        // Prefer alternate link
        for link_match in xml.match_indices("<link") {
            let start = link_match.0;
            let end = xml[start..].find("/>").or_else(|| xml[start..].find('>'))?;
            let link_tag = &xml[start..start + end];

            // Skip non-alternate links if we find a rel attribute
            if link_tag.contains("rel=\"") && !link_tag.contains("rel=\"alternate\"") {
                continue;
            }

            // Extract href
            if let Some(href_pos) = link_tag.find("href=\"") {
                let href_start = href_pos + 6;
                if let Some(href_end) = link_tag[href_start..].find('"') {
                    return Some(link_tag[href_start..href_start + href_end].to_string());
                }
            }
        }
        None
    }

    /// Decode common HTML entities in text content.
    /// Note: For HTML content, prefer strip_html() which uses ammonia for
    /// proper sanitization including entity handling.
    pub(crate) fn decode_html_entities(text: &str) -> String {
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ")
    }

    /// Strip ALL HTML tags from content, producing plain text only.
    pub(crate) fn strip_html(html: &str) -> String {
        // ammonia::Builder with an empty tag set removes every HTML tag,
        // leaving only decoded plain text.
        let clean = ammonia::Builder::new()
            .tags(std::collections::HashSet::new())
            .clean(html)
            .to_string();
        // Collapse whitespace for consistent output
        clean.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Generate a stable ID from a string
    pub(crate) fn generate_id(input: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        format!("rss_{:x}", hasher.finish())
    }

    /// Detect feed type and parse accordingly
    pub(crate) fn parse_feed(&self, xml: &str, feed_url: &str) -> Vec<FeedEntry> {
        // Check if it's Atom or RSS
        if xml.contains("<feed") && xml.contains("xmlns=\"http://www.w3.org/2005/Atom\"") {
            debug!(feed_url, "Detected Atom feed");
            self.parse_atom_feed(xml, feed_url)
        } else if xml.contains("<entry>") && !xml.contains("<item>") {
            debug!(feed_url, "Detected Atom feed (by entry tag)");
            self.parse_atom_feed(xml, feed_url)
        } else {
            debug!(feed_url, "Treating as RSS feed");
            self.parse_rss_feed(xml, feed_url)
        }
    }
}

impl Default for RssSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for RssSource {
    fn source_type(&self) -> &'static str {
        "rss"
    }

    fn name(&self) -> &'static str {
        "RSS Feeds"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::News,
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "RSS",
            color_hint: "amber",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        if self.feed_urls.is_empty() {
            info!("No RSS feeds configured");
            return Ok(Vec::new());
        }

        // Clear per-feed errors from previous run
        *self.feed_errors.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();

        info!(feed_count = self.feed_urls.len(), "Fetching RSS feeds");

        let mut all_items = Vec::new();

        for feed_url in &self.feed_urls {
            debug!(url = feed_url, "Fetching feed");

            match self
                .client
                .get(feed_url)
                .header("User-Agent", "Mozilla/5.0 (compatible; 4DA/1.0)")
                .send()
                .await
            {
                Ok(response) => {
                    if !response.status().is_success() {
                        let err_msg = format!("HTTP {}", response.status());
                        warn!(url = feed_url, status = %response.status(), "Feed returned error status");
                        self.feed_errors.lock().unwrap_or_else(|e| e.into_inner())
                            .push((feed_url.clone(), err_msg));
                        continue;
                    }

                    // Cap RSS response at 10MB to prevent malicious feed flooding
                    if let Some(len) = response.content_length() {
                        if len > MAX_RSS_RESPONSE {
                            warn!(
                                target: "4da::sources::rss",
                                url = %feed_url,
                                size = len,
                                "RSS feed too large — skipping"
                            );
                            self.feed_errors.lock().unwrap_or_else(|e| e.into_inner())
                                .push((feed_url.clone(), format!("Feed too large: {} bytes", len)));
                            continue;
                        }
                    }

                    match response.text().await {
                        Ok(xml) => {
                            let entries = self.parse_feed(&xml, feed_url);
                            debug!(url = feed_url, count = entries.len(), "Parsed entries");

                            for entry in entries {
                                let mut item = SourceItem::new("rss", &entry.id, &entry.title)
                                    .with_url(Some(entry.link.clone()))
                                    .with_content(entry.description);

                                // Add metadata
                                item = item.with_metadata(serde_json::json!({
                                    "feed_title": entry.feed_title,
                                    "feed_url": feed_url,
                                    "pub_date": entry.pub_date,
                                }));

                                all_items.push(item);
                            }
                        }
                        Err(e) => {
                            warn!(url = feed_url, error = %e, "Failed to read feed body");
                            self.feed_errors.lock().unwrap_or_else(|e| e.into_inner())
                                .push((feed_url.clone(), e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    warn!(url = feed_url, error = %e, "Failed to fetch feed");
                    self.feed_errors.lock().unwrap_or_else(|e| e.into_inner())
                        .push((feed_url.clone(), e.to_string()));
                }
            }
        }

        // Limit total items
        all_items.truncate(self.config.max_items);

        info!(count = all_items.len(), "Fetched RSS items");
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // If item already has content from feed description, return it
        if !item.content.is_empty() && item.content.len() > 100 {
            return Ok(item.content.clone());
        }

        // If no URL, nothing to scrape
        let url = match &item.url {
            Some(u) => u,
            None => return Ok(item.content.clone()),
        };

        // Skip non-HTTP URLs
        if !url.starts_with("http") {
            return Ok(item.content.clone());
        }

        // Use shared client with per-request timeout for scraping
        let client = super::shared_client();

        // Fetch the page
        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !response.status().is_success() {
            warn!(target: "4da::sources", url = %url, status = %response.status(), "Scrape failed — returning empty content");
            return Ok(item.content.clone());
        }

        let html = response
            .text()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        // Extract content using common selectors
        let document = Html::parse_document(&html);
        let selectors = [
            "article",
            "main",
            "[role='main']",
            ".post-content",
            ".article-content",
            ".entry-content",
            ".content",
        ];

        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text: String = element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");

                    // Only use if we got meaningful content
                    if text.len() > 100 {
                        let max_len = 5000;
                        let truncated = if text.len() > max_len {
                            text.chars().take(max_len).collect()
                        } else {
                            text
                        };
                        return Ok(truncated);
                    }
                }
            }
        }

        // Fall back to feed description
        Ok(item.content.clone())
    }

    fn feed_errors(&self) -> Vec<(String, String)> {
        self.feed_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rss_source_defaults() {
        let source = RssSource::new();
        assert_eq!(source.source_type(), "rss");
        assert_eq!(source.name(), "RSS Feeds");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 30);
    }

    #[test]
    fn test_parse_rss_sample() {
        let source = RssSource::new();
        let sample_xml = r#"
        <?xml version="1.0"?>
        <rss version="2.0">
        <channel>
            <title>Test Feed</title>
            <item>
                <title>Test Article</title>
                <link>https://example.com/article1</link>
                <description>This is the article description.</description>
                <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
            </item>
        </channel>
        </rss>
        "#;

        let entries = source.parse_rss_feed(sample_xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Test Article");
        assert_eq!(entries[0].link, "https://example.com/article1");
        assert_eq!(entries[0].feed_title, "Test Feed");
    }

    #[test]
    fn test_parse_atom_sample() {
        let source = RssSource::new();
        let sample_xml = r#"
        <?xml version="1.0"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <title>Test Atom Feed</title>
            <entry>
                <title>Atom Article</title>
                <link href="https://example.com/atom1" rel="alternate"/>
                <id>urn:uuid:1234</id>
                <summary>Atom article summary.</summary>
                <published>2024-01-01T00:00:00Z</published>
            </entry>
        </feed>
        "#;

        let entries = source.parse_atom_feed(sample_xml, "https://example.com/atom");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Atom Article");
        assert_eq!(entries[0].link, "https://example.com/atom1");
    }

    #[test]
    fn test_decode_html_entities() {
        assert_eq!(
            RssSource::decode_html_entities("Hello &amp; World"),
            "Hello & World"
        );
        assert_eq!(RssSource::decode_html_entities("&lt;tag&gt;"), "<tag>");
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(
            RssSource::strip_html("<p>Hello <strong>World</strong></p>"),
            "Hello World"
        );
    }

    #[test]
    fn test_with_feeds() {
        let source = RssSource::with_feeds(vec![
            "https://example.com/feed1".to_string(),
            "https://example.com/feed2".to_string(),
        ]);
        assert_eq!(source.feed_urls().len(), 2);
    }

    #[test]
    fn test_parse_rss_max_items_limit() {
        let source = RssSource::new();
        // Generate XML with 500 items
        let mut xml = String::from("<rss><channel><title>Test</title>");
        for i in 0..500 {
            xml.push_str(&format!(
                "<item><title>Item {}</title><link>https://example.com/{}</link><description>Desc</description></item>",
                i, i
            ));
        }
        xml.push_str("</channel></rss>");

        let entries = source.parse_rss_feed(&xml, "https://test.com/feed");
        assert!(entries.len() <= 200, "Should limit to MAX_ITEMS_PER_FEED");
    }
}
