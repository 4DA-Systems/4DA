// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Multi-path fallback system for source adapters.
//!
//! When a primary source API fails after retries, this module provides
//! alternative endpoints that deliver equivalent content. The cascade
//! tries each fallback in order until one succeeds or all fail.

use crate::sources::{SourceError, SourceItem};
use reqwest::Client;
use tracing::{info, warn};

// ============================================================================
// Fallback types
// ============================================================================

/// An alternative endpoint for a source
#[derive(Debug, Clone)]
pub struct FallbackEndpoint {
    /// Human-readable name (e.g., "Algolia HN Search API")
    pub name: String,
    /// The URL template (may contain {query} or {param} placeholders)
    pub url: String,
    /// Parser function identifier
    pub parser: FallbackParser,
}

/// Which parser to use for the fallback response
#[derive(Debug, Clone)]
pub enum FallbackParser {
    /// Algolia HN Search API (JSON)
    HnAlgolia,
    /// hnrss.org RSS feed (XML)
    HnRss,
    /// Reddit .rss suffix (XML/Atom)
    RedditRss,
    /// GitHub GraphQL API
    GithubGraphql,
    /// GitHub releases.atom feed
    GithubRss,
    /// arXiv RSS feed
    ArxivRss,
    /// Standard RSS/Atom parser
    GenericRss,
}

// ============================================================================
// Fallback endpoint registry
// ============================================================================

/// Get ordered fallback endpoints for a given source type.
///
/// Returns an empty vec for sources without known alternatives.
pub fn get_fallbacks(source_type: &str) -> Vec<FallbackEndpoint> {
    match source_type {
        "hackernews" => vec![
            FallbackEndpoint {
                name: "Algolia HN Search".into(),
                url: "https://hn.algolia.com/api/v1/search?tags=front_page&hitsPerPage=30".into(),
                parser: FallbackParser::HnAlgolia,
            },
            FallbackEndpoint {
                name: "HN RSS (hnrss.org)".into(),
                url: "https://hnrss.org/frontpage?count=30".into(),
                parser: FallbackParser::HnRss,
            },
        ],
        "reddit" => vec![FallbackEndpoint {
            name: "Reddit RSS".into(),
            url: "https://www.reddit.com/r/programming/.rss".into(),
            parser: FallbackParser::RedditRss,
        }],
        "github" => vec![FallbackEndpoint {
            name: "GitHub Trending RSS".into(),
            url: "https://github.com/trending?since=daily".into(),
            parser: FallbackParser::GenericRss,
        }],
        "arxiv" => vec![FallbackEndpoint {
            name: "arXiv RSS".into(),
            url: "http://arxiv.org/rss/cs.AI".into(),
            parser: FallbackParser::ArxivRss,
        }],
        _ => vec![], // No fallbacks for other sources
    }
}

// ============================================================================
// Cascade logic
// ============================================================================

/// Try fallback endpoints in order after primary source failure.
///
/// Returns items from the first successful fallback, or the original
/// primary error if all fallbacks fail (or none are configured).
pub async fn try_fallbacks(
    source_type: &str,
    client: &Client,
    primary_error: &SourceError,
) -> Result<Vec<SourceItem>, SourceError> {
    let fallbacks = get_fallbacks(source_type);
    if fallbacks.is_empty() {
        return Err(primary_error.clone());
    }

    info!(
        target: "4da::fallback",
        source = source_type,
        fallback_count = fallbacks.len(),
        primary_error = %primary_error,
        "Primary source failed, trying {} fallback(s)",
        fallbacks.len()
    );

    for (i, fb) in fallbacks.iter().enumerate() {
        match fetch_fallback(client, fb).await {
            Ok(items) if !items.is_empty() => {
                info!(
                    target: "4da::fallback",
                    source = source_type,
                    fallback = fb.name.as_str(),
                    items = items.len(),
                    "Fallback {}/{} succeeded: {} items",
                    i + 1,
                    fallbacks.len(),
                    items.len()
                );
                return Ok(items);
            }
            Ok(_) => {
                warn!(
                    target: "4da::fallback",
                    source = source_type,
                    fallback = fb.name.as_str(),
                    "Fallback {}/{} returned 0 items, trying next",
                    i + 1,
                    fallbacks.len()
                );
            }
            Err(e) => {
                warn!(
                    target: "4da::fallback",
                    source = source_type,
                    fallback = fb.name.as_str(),
                    error = %e,
                    "Fallback {}/{} failed: {}",
                    i + 1,
                    fallbacks.len(),
                    e
                );
            }
        }
    }

    warn!(
        target: "4da::fallback",
        source = source_type,
        "All {} fallback(s) exhausted for {}",
        fallbacks.len(),
        source_type
    );
    Err(primary_error.clone())
}

// ============================================================================
// Fetch + dispatch
// ============================================================================

/// Fetch a single fallback endpoint and parse its response.
async fn fetch_fallback(
    client: &Client,
    endpoint: &FallbackEndpoint,
) -> Result<Vec<SourceItem>, SourceError> {
    let response = client
        .get(&endpoint.url)
        .header("User-Agent", "4DA/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| SourceError::Network(format!("{}: {}", endpoint.name, e)))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(format!(
            "{} rate limited",
            endpoint.name
        )));
    }
    if !status.is_success() {
        return Err(SourceError::Network(format!(
            "{}: HTTP {}",
            endpoint.name,
            status.as_u16()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| SourceError::Network(format!("{}: body read failed: {}", endpoint.name, e)))?;

    match endpoint.parser {
        FallbackParser::HnAlgolia => parse_hn_algolia(&body),
        FallbackParser::HnRss => parse_rss_generic(&body, "hackernews"),
        FallbackParser::RedditRss => parse_rss_generic(&body, "reddit"),
        FallbackParser::GithubRss => parse_rss_generic(&body, "github"),
        FallbackParser::ArxivRss => parse_rss_generic(&body, "arxiv"),
        FallbackParser::GithubGraphql => Err(SourceError::Other(
            "GraphQL fallback not yet implemented".into(),
        )),
        FallbackParser::GenericRss => parse_rss_generic(&body, "rss"),
    }
}

// ============================================================================
// Algolia HN parser
// ============================================================================

/// Parse Algolia HN Search API response into SourceItems.
fn parse_hn_algolia(body: &str) -> Result<Vec<SourceItem>, SourceError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| SourceError::Parse(format!("Algolia JSON: {e}")))?;

    let hits = json
        .get("hits")
        .and_then(|h| h.as_array())
        .ok_or_else(|| SourceError::Parse("Algolia: missing hits array".into()))?;

    let items: Vec<SourceItem> = hits
        .iter()
        .filter_map(|hit| {
            let title = hit.get("title")?.as_str()?;
            let object_id = hit.get("objectID")?.as_str()?;
            let url = hit.get("url").and_then(|u| u.as_str()).map(String::from);
            let points = hit
                .get("points")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let author = hit
                .get("author")
                .and_then(|a| a.as_str())
                .unwrap_or("unknown");

            let mut item = SourceItem::new("hackernews", object_id, title);
            item.url =
                url.or_else(|| Some(format!("https://news.ycombinator.com/item?id={object_id}")));
            item.metadata = Some(serde_json::json!({
                "score": points,
                "author": author,
                "source": "algolia_fallback",
            }));
            Some(item)
        })
        .collect();

    Ok(items)
}

// ============================================================================
// Generic RSS/Atom parser
// ============================================================================

/// Parse an RSS 2.0 or Atom feed into SourceItems.
///
/// Handles both `<item>` (RSS) and `<entry>` (Atom) formats.
/// Reuses the same XML tag extraction approach as `sources::rss`.
fn parse_rss_generic(body: &str, source_type: &str) -> Result<Vec<SourceItem>, SourceError> {
    let mut items = Vec::new();

    // Split on <item> or <entry> tags depending on feed format
    let item_blocks: Vec<&str> = if body.contains("<entry") {
        body.split("<entry").skip(1).collect()
    } else {
        body.split("<item").skip(1).collect()
    };

    for block in item_blocks.iter().take(30) {
        let title = extract_tag(block, "title").unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let link = extract_tag(block, "link")
            .or_else(|| extract_attr(block, "link", "href"))
            .unwrap_or_default();
        let description = extract_tag(block, "description")
            .or_else(|| extract_tag(block, "summary"))
            .or_else(|| extract_tag(block, "content"))
            .unwrap_or_default();
        let guid = extract_tag(block, "guid")
            .or_else(|| extract_tag(block, "id"))
            .unwrap_or_else(|| link.clone());
        let pub_date = extract_tag(block, "pubDate")
            .or_else(|| extract_tag(block, "published"))
            .or_else(|| extract_tag(block, "updated"));

        let source_id = if guid.is_empty() {
            format!(
                "{}:{}",
                source_type,
                title.chars().take(50).collect::<String>()
            )
        } else {
            guid
        };

        let mut item = SourceItem::new(source_type, &source_id, &title);
        item.url = if link.is_empty() { None } else { Some(link) };
        item.content = description;

        let mut meta = serde_json::Map::new();
        meta.insert(
            "source".into(),
            serde_json::json!(format!("{}_rss_fallback", source_type)),
        );
        if let Some(date) = pub_date {
            meta.insert("published_at".into(), serde_json::json!(date));
        }
        item.metadata = Some(serde_json::Value::Object(meta));

        items.push(item);
    }

    Ok(items)
}

// ============================================================================
// XML extraction helpers
// ============================================================================

/// Extract text content between XML tags.
///
/// Handles CDATA sections. Returns `None` if the tag is not found or empty.
fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    let content_start = block[start..].find('>')? + start + 1;
    let end = block[content_start..].find(&close)? + content_start;
    let text = &block[content_start..end];
    // Strip CDATA if present
    let text = text.strip_prefix("<![CDATA[").unwrap_or(text);
    let text = text.strip_suffix("]]>").unwrap_or(text);
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Extract an attribute value from a self-closing tag (e.g., `<link href="..."/>`).
fn extract_attr(block: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let tag_end = block[start..].find('>')? + start;
    let tag_block = &block[start..tag_end];
    let attr_pattern = format!("{attr}=\"");
    let attr_start = tag_block.find(&attr_pattern)? + attr_pattern.len();
    let attr_end = tag_block[attr_start..].find('"')? + attr_start;
    Some(tag_block[attr_start..attr_end].to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fallback registry tests ---

    #[test]
    fn test_fallbacks_exist_for_key_sources() {
        assert!(!get_fallbacks("hackernews").is_empty());
        assert!(!get_fallbacks("reddit").is_empty());
        assert!(!get_fallbacks("github").is_empty());
        assert!(!get_fallbacks("arxiv").is_empty());
    }

    #[test]
    fn test_no_fallbacks_for_unknown() {
        assert!(get_fallbacks("unknown_source").is_empty());
    }

    // --- Algolia HN parser tests ---

    #[test]
    fn test_parse_hn_algolia() {
        let json = r#"{"hits":[{"title":"Test Post","objectID":"123","url":"https://example.com","points":42,"author":"test"}]}"#;
        let items = parse_hn_algolia(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test Post");
        assert_eq!(items[0].source_type, "hackernews");
        assert_eq!(items[0].url, Some("https://example.com".into()));
    }

    #[test]
    fn test_parse_hn_algolia_no_url() {
        let json = r#"{"hits":[{"title":"Ask HN: Something","objectID":"456","points":10,"author":"dev"}]}"#;
        let items = parse_hn_algolia(json).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].url.as_ref().unwrap().contains("456"));
    }

    #[test]
    fn test_parse_hn_algolia_empty_hits() {
        let json = r#"{"hits":[]}"#;
        let items = parse_hn_algolia(json).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_hn_algolia_missing_hits() {
        let json = r#"{"results":[]}"#;
        let result = parse_hn_algolia(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_hn_algolia_invalid_json() {
        let result = parse_hn_algolia("not json at all");
        assert!(result.is_err());
    }

    // --- RSS/Atom parser tests ---

    #[test]
    fn test_parse_rss_generic_rss2() {
        let xml = r#"<rss><channel><item><title>Test Article</title><link>https://example.com/article</link><description>Some content</description><guid>guid-123</guid><pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate></item></channel></rss>"#;
        let items = parse_rss_generic(xml, "reddit").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test Article");
        assert_eq!(items[0].source_type, "reddit");
    }

    #[test]
    fn test_parse_rss_generic_atom() {
        let xml = r#"<feed><entry><title>Atom Entry</title><link href="https://example.com/entry"/><id>atom-1</id><published>2024-01-01T00:00:00Z</published><summary>Summary text</summary></entry></feed>"#;
        let items = parse_rss_generic(xml, "arxiv").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Atom Entry");
        assert_eq!(items[0].url, Some("https://example.com/entry".into()));
    }

    #[test]
    fn test_parse_rss_generic_empty() {
        let xml = "<rss><channel></channel></rss>";
        let items = parse_rss_generic(xml, "test").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_rss_generic_multiple_items() {
        let xml = r#"<rss><channel>
            <item><title>First</title><link>https://a.com</link><guid>1</guid></item>
            <item><title>Second</title><link>https://b.com</link><guid>2</guid></item>
            <item><title>Third</title><link>https://c.com</link><guid>3</guid></item>
        </channel></rss>"#;
        let items = parse_rss_generic(xml, "hackernews").unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "First");
        assert_eq!(items[2].title, "Third");
    }

    #[test]
    fn test_parse_rss_generic_skips_empty_titles() {
        let xml = r#"<rss><channel>
            <item><title></title><link>https://a.com</link></item>
            <item><title>Real Item</title><link>https://b.com</link></item>
        </channel></rss>"#;
        let items = parse_rss_generic(xml, "test").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Real Item");
    }

    #[test]
    fn test_parse_rss_generic_metadata_contains_source() {
        let xml = r#"<rss><channel><item><title>Test</title><link>https://a.com</link><pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate></item></channel></rss>"#;
        let items = parse_rss_generic(xml, "reddit").unwrap();
        assert_eq!(items.len(), 1);
        let meta = items[0].metadata.as_ref().unwrap();
        assert_eq!(meta.get("source").unwrap(), "reddit_rss_fallback");
        assert!(meta.get("published_at").is_some());
    }

    // --- XML extraction helper tests ---

    #[test]
    fn test_extract_tag() {
        assert_eq!(
            extract_tag("<title>Hello</title>", "title"),
            Some("Hello".into())
        );
        assert_eq!(
            extract_tag("<title><![CDATA[Hello]]></title>", "title"),
            Some("Hello".into())
        );
        assert_eq!(extract_tag("<title></title>", "title"), None);
    }

    #[test]
    fn test_extract_tag_with_attributes() {
        assert_eq!(
            extract_tag(r#"<title type="text">Hello</title>"#, "title"),
            Some("Hello".into())
        );
    }

    #[test]
    fn test_extract_tag_not_found() {
        assert_eq!(extract_tag("<other>Hello</other>", "title"), None);
    }

    #[test]
    fn test_extract_attr() {
        assert_eq!(
            extract_attr(r#"<link href="https://example.com"/>"#, "link", "href"),
            Some("https://example.com".into())
        );
    }

    #[test]
    fn test_extract_attr_not_found() {
        assert_eq!(
            extract_attr(r#"<link rel="alternate"/>"#, "link", "href"),
            None
        );
    }

    #[test]
    fn test_extract_attr_no_tag() {
        assert_eq!(extract_attr("<other/>", "link", "href"), None);
    }

    // --- try_fallbacks async tests ---

    #[tokio::test]
    async fn test_try_fallbacks_no_fallbacks_returns_primary_error() {
        let client = Client::new();
        let err = SourceError::Network("primary failed".into());
        let result = try_fallbacks("unknown_source", &client, &err).await;
        assert!(result.is_err());
    }
}
