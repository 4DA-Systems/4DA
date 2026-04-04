//! arXiv source implementation
//!
//! Fetches recent papers from arXiv API for relevant categories.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

/// Maximum entries to parse from a single arXiv query
const MAX_ENTRIES_PER_QUERY: usize = 200;

// ============================================================================
// arXiv API Types (Atom feed parsed as JSON via simple extraction)
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct ArxivEntry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) authors: Vec<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) published: String,
    pub(crate) link: String,
}

// ============================================================================
// arXiv Source
// ============================================================================

/// arXiv source - fetches recent papers from specified categories
pub struct ArxivSource {
    config: SourceConfig,
    client: reqwest::Client,
    /// Categories to fetch (e.g., "cs.AI", "cs.LG", "cs.CL")
    categories: Vec<String>,
}

impl ArxivSource {
    /// Create a new arXiv source with default CS categories
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 20,
                fetch_interval_secs: 3600, // 1 hour (arXiv updates daily)
                custom: None,
            },
            client: super::shared_client(),
            // Default to software-engineering-relevant categories
            categories: vec![
                "cs.SE".to_string(), // Software Engineering
                "cs.PL".to_string(), // Programming Languages
                "cs.DB".to_string(), // Databases
                "cs.CR".to_string(), // Cryptography and Security
            ],
        }
    }

    /// Create with custom categories
    pub fn with_categories(categories: Vec<String>) -> Self {
        let mut source = Self::new();
        source.categories = categories;
        source
    }

    /// Parse arXiv Atom feed response into entries
    pub(crate) fn parse_atom_feed(&self, xml: &str) -> Vec<ArxivEntry> {
        let mut entries = Vec::new();

        // Simple XML parsing - find <entry> blocks
        for entry_block in xml.split("<entry>").skip(1) {
            if entries.len() >= MAX_ENTRIES_PER_QUERY {
                break;
            }
            let entry_end = entry_block.find("</entry>").unwrap_or(entry_block.len());
            let entry_xml = &entry_block[..entry_end];

            // Extract fields with simple string parsing
            let id = Self::extract_tag(entry_xml, "id").unwrap_or_default();
            let title = Self::extract_tag(entry_xml, "title")
                .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            let summary = Self::extract_tag(entry_xml, "summary")
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            let published = Self::extract_tag(entry_xml, "published").unwrap_or_default();

            // Extract authors
            let mut authors = Vec::new();
            for author_block in entry_xml.split("<author>").skip(1) {
                if let Some(name) = Self::extract_tag(author_block, "name") {
                    authors.push(name);
                }
            }

            // Extract categories
            let mut categories = Vec::new();
            for cat_match in entry_xml.match_indices("category term=\"") {
                let start = cat_match.0 + 15;
                if let Some(end) = entry_xml[start..].find('"') {
                    categories.push(entry_xml[start..start + end].to_string());
                }
            }

            // Extract link (PDF preferred)
            let link = if let Some(pdf_pos) = entry_xml.find("title=\"pdf\"") {
                // Find href before this
                let search_start = entry_xml[..pdf_pos].rfind("href=\"").map(|p| p + 6);
                if let Some(start) = search_start {
                    entry_xml[start..pdf_pos]
                        .find('"')
                        .map(|end| entry_xml[start..start + end].to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                // Fall back to abstract page link
                id.clone()
            };

            if !id.is_empty() && !title.is_empty() {
                entries.push(ArxivEntry {
                    id,
                    title,
                    summary,
                    authors,
                    categories,
                    published,
                    link,
                });
            }
        }

        entries
    }

    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{tag}");
        let close_tag = format!("</{tag}>");

        let start_pos = xml.find(&open_tag)?;
        let content_start = xml[start_pos..].find('>')? + start_pos + 1;
        let end_pos = xml[content_start..].find(&close_tag)? + content_start;

        Some(xml[content_start..end_pos].to_string())
    }

    /// Extract arXiv ID from full URL
    pub(crate) fn extract_arxiv_id(url: &str) -> String {
        // URL format: http://arxiv.org/abs/2401.12345v1
        let id = url.rsplit('/').next().unwrap_or(url);

        // Remove version suffix (e.g., "v1", "v2")
        if let Some(v_pos) = id.rfind('v') {
            // Check if everything after 'v' is digits
            if id[v_pos + 1..].chars().all(|c| c.is_ascii_digit()) && !id[v_pos + 1..].is_empty() {
                return id[..v_pos].to_string();
            }
        }
        id.to_string()
    }
}

impl Default for ArxivSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ArxivSource {
    /// Helper to fetch papers for specified categories
    async fn fetch_for_categories(
        &self,
        categories: &[String],
        max_items: usize,
    ) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        info!(categories = ?categories, "Fetching papers for categories");

        let cat_query = categories
            .iter()
            .map(|c| format!("cat:{c}"))
            .collect::<Vec<_>>()
            .join("+OR+");

        let url = format!(
            "https://export.arxiv.org/api/query?search_query={cat_query}&start=0&max_results={max_items}&sortBy=submittedDate&sortOrder=descending"
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
                "arXiv rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "arXiv forbidden (HTTP 403)".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "arXiv API error: HTTP {}",
                status.as_u16()
            )));
        }

        let xml = response
            .text()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;
        let entries = self.parse_atom_feed(&xml);
        info!(count = entries.len(), "Parsed entries from feed");

        let items: Vec<SourceItem> = entries
            .into_iter()
            .map(|entry| {
                let arxiv_id = Self::extract_arxiv_id(&entry.id);
                SourceItem::new("arxiv", &arxiv_id, &entry.title)
                    .with_url(Some(entry.link.clone()))
                    .with_content(entry.summary)
                    .with_metadata(serde_json::json!({
                        "authors": entry.authors,
                        "categories": entry.categories,
                        "published": entry.published,
                        "arxiv_url": entry.id,
                    }))
            })
            .collect();

        info!(count = items.len(), "Fetched papers");
        Ok(items)
    }
}

#[async_trait]
impl Source for ArxivSource {
    fn source_type(&self) -> &'static str {
        "arxiv"
    }

    fn name(&self) -> &'static str {
        "arXiv"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Research,
            default_content_type: "deep_dive",
            default_multiplier: 1.15,
            label: "arXiv",
            color_hint: "purple",
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        self.fetch_for_categories(&self.categories, self.config.max_items)
            .await
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        let deep_categories = [
            "cs.SE", "cs.PL", "cs.DB", "cs.CR", "cs.DC", "cs.HC", "cs.IR",
        ];

        info!(
            category_count = deep_categories.len(),
            items_per = items_per_category,
            "Deep fetching arXiv"
        );

        let categories: Vec<String> = deep_categories
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        // Multiplier of 10 gives ~1000 papers across 16 categories for deep scan
        self.fetch_for_categories(&categories, items_per_category * 10)
            .await
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // arXiv items already have abstracts as content - no scraping needed
        // The abstract is the primary content for papers
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
    fn test_arxiv_source_defaults() {
        let source = ArxivSource::new();
        assert_eq!(source.source_type(), "arxiv");
        assert_eq!(source.name(), "arXiv");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 20);
        // Default categories should be software-engineering relevant, not ML
        assert!(source.categories.contains(&"cs.SE".to_string()));
        assert!(source.categories.contains(&"cs.PL".to_string()));
        assert!(!source.categories.contains(&"cs.AI".to_string()));
        assert!(!source.categories.contains(&"cs.LG".to_string()));
    }

    #[test]
    fn test_extract_arxiv_id() {
        assert_eq!(
            ArxivSource::extract_arxiv_id("http://arxiv.org/abs/2401.12345v1"),
            "2401.12345"
        );
        assert_eq!(
            ArxivSource::extract_arxiv_id("http://arxiv.org/abs/2401.12345"),
            "2401.12345"
        );
    }

    #[test]
    fn test_parse_atom_sample() {
        let source = ArxivSource::new();
        let sample_xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00001v1</id>
            <title>Test Paper Title</title>
            <summary>This is the abstract of the paper.</summary>
            <author><name>John Doe</name></author>
            <category term="cs.AI"/>
            <published>2024-01-01T00:00:00Z</published>
        </entry>
        </feed>
        "#;

        let entries = source.parse_atom_feed(sample_xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Test Paper Title");
        assert_eq!(entries[0].authors, vec!["John Doe"]);
    }
}
