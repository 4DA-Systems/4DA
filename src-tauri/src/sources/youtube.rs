//! YouTube source implementation
//!
//! Fetches recent videos from YouTube channels via their public Atom RSS feeds.
//! No API key required - completely free.

use async_trait::async_trait;
use tracing::{debug, info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// YouTube Source
// ============================================================================

/// YouTube source - fetches videos from channels via public Atom RSS feeds
pub struct YouTubeSource {
    config: SourceConfig,
    client: reqwest::Client,
    /// Channel IDs to monitor (YouTube channel IDs like "UCsBjURrPoezykLs9EqgamOA")
    channels: Vec<YouTubeChannel>,
}

/// A YouTube channel to follow
#[derive(Debug, Clone)]
pub struct YouTubeChannel {
    pub name: String,
    pub channel_id: String,
}

impl YouTubeSource {
    /// Create with default tech YouTube channels
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 1800, // 30 minutes
                custom: None,
            },
            client: super::shared_client(),
            channels: default_channels(),
        }
    }

    /// Create with custom channel list (channel_id strings only)
    pub fn with_channels(channel_ids: Vec<String>) -> Self {
        let mut source = Self::new();
        if !channel_ids.is_empty() {
            source.channels = channel_ids
                .into_iter()
                .map(|id| YouTubeChannel {
                    name: id.clone(),
                    channel_id: id,
                })
                .collect();
        }
        source
    }

    /// Create with named channels
    #[allow(dead_code)]
    pub fn with_named_channels(channels: Vec<YouTubeChannel>) -> Self {
        let mut source = Self::new();
        if !channels.is_empty() {
            source.channels = channels;
        }
        source
    }

    /// Fetch the Atom feed for a single channel
    async fn fetch_channel_feed(
        &self,
        channel: &YouTubeChannel,
    ) -> Result<Vec<VideoEntry>, String> {
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel.channel_id
        );

        debug!(channel_name = %channel.name, channel_id = %channel.channel_id, "Fetching YouTube feed");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error for {}: {}", channel.name, e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "YouTube feed error for {}: HTTP {}",
                channel.name,
                resp.status()
            ));
        }

        let xml = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read feed for {}: {}", channel.name, e))?;

        self.parse_atom_feed(&xml, &channel.name)
    }

    /// Parse YouTube Atom feed XML
    fn parse_atom_feed(&self, xml: &str, channel_name: &str) -> Result<Vec<VideoEntry>, String> {
        let mut entries = Vec::new();

        // Extract feed title (channel name) - fallback to provided name
        let feed_title = extract_tag(xml, "title").unwrap_or_else(|| channel_name.to_string());

        // Find <entry> blocks
        for entry_block in xml.split("<entry>").skip(1) {
            let entry_end = entry_block.find("</entry>").unwrap_or(entry_block.len());
            let entry_xml = &entry_block[..entry_end];

            let video_id = extract_tag(entry_xml, "yt:videoId").unwrap_or_default();
            let title = extract_tag(entry_xml, "title").unwrap_or_default();
            let published = extract_tag(entry_xml, "published").unwrap_or_default();
            let updated = extract_tag(entry_xml, "updated").unwrap_or_default();

            // Extract author name (nested under <author>)
            let author = extract_tag(entry_xml, "name").unwrap_or_else(|| feed_title.clone());

            // Extract description from media:group > media:description
            let description = extract_tag(entry_xml, "media:description").unwrap_or_default();

            // Extract view count from media:community > media:statistics
            let views = extract_attr(entry_xml, "media:statistics", "views")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            // Extract star rating
            let star_rating = extract_attr(entry_xml, "media:starRating", "average")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);

            if !video_id.is_empty() && !title.is_empty() {
                entries.push(VideoEntry {
                    video_id,
                    title,
                    author,
                    description,
                    published,
                    updated,
                    views,
                    star_rating,
                });
            }
        }

        Ok(entries)
    }
}

/// Internal video entry from the feed
#[derive(Debug)]
struct VideoEntry {
    video_id: String,
    title: String,
    author: String,
    description: String,
    published: String,
    updated: String,
    views: u64,
    star_rating: f64,
}

/// Extract content between XML tags: <tag>content</tag>
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);

    let start_pos = xml.find(&open_tag)?;
    let content_start = xml[start_pos..].find('>')? + start_pos + 1;
    let end_pos = xml[content_start..].find(&close_tag)? + content_start;

    let content = xml[content_start..end_pos].trim();

    // Handle CDATA
    if content.starts_with("<![CDATA[") && content.ends_with("]]>") {
        Some(content[9..content.len() - 3].to_string())
    } else {
        Some(content.to_string())
    }
}

/// Extract an attribute value from a self-closing tag: <tag attr="value" />
fn extract_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let tag_start = format!("<{}", tag);
    let start_pos = xml.find(&tag_start)?;
    let tag_end = xml[start_pos..].find('>')? + start_pos;
    let tag_content = &xml[start_pos..tag_end];

    let attr_prefix = format!("{}=\"", attr);
    let attr_start = tag_content.find(&attr_prefix)?;
    let value_start = attr_start + attr_prefix.len();
    let value_end = tag_content[value_start..].find('"')? + value_start;

    Some(tag_content[value_start..value_end].to_string())
}

/// Default tech YouTube channels
fn default_channels() -> Vec<YouTubeChannel> {
    vec![
        YouTubeChannel {
            name: "Fireship".into(),
            channel_id: "UCsBjURrPoezykLs9EqgamOA".into(),
        },
        YouTubeChannel {
            name: "ThePrimeagen".into(),
            channel_id: "UCUyeluBRhGPCW4rPe_UvBZQ".into(),
        },
        YouTubeChannel {
            name: "Theo".into(),
            channel_id: "UCbRP3c757lWg9M-U7TyEkXA".into(),
        },
        YouTubeChannel {
            name: "Traversy Media".into(),
            channel_id: "UC29ju8bIPH5as8OGnQzwJyA".into(),
        },
        YouTubeChannel {
            name: "Web Dev Simplified".into(),
            channel_id: "UCFbNIlppjAuEX4znoulh0Cw".into(),
        },
        YouTubeChannel {
            name: "Computerphile".into(),
            channel_id: "UC9-y-6csu5WGm29I7JiwpnA".into(),
        },
        YouTubeChannel {
            name: "3Blue1Brown".into(),
            channel_id: "UCYO_jab_esuFRV4b17AJtAw".into(),
        },
        YouTubeChannel {
            name: "NetworkChuck".into(),
            channel_id: "UC9x0AN7BWHpCDHSm9NiJFJQ".into(),
        },
        YouTubeChannel {
            name: "TechLinked".into(),
            channel_id: "UCeeFfhMcJa1kjtfZAGskOCA".into(),
        },
        YouTubeChannel {
            name: "Coding Train".into(),
            channel_id: "UCvjgXvBlISQQnCVnCBdLMQw".into(),
        },
        YouTubeChannel {
            name: "ArjanCodes".into(),
            channel_id: "UCVhQ2NnY5Rskt6UjCUkJ_DA".into(),
        },
        YouTubeChannel {
            name: "DevOps Toolkit".into(),
            channel_id: "UCfz8x0lVzJpb_dgWm9kPVrw".into(),
        },
        YouTubeChannel {
            name: "Lex Fridman".into(),
            channel_id: "UCSHZKyawb77ixDdsGog4iWA".into(),
        },
        YouTubeChannel {
            name: "Two Minute Papers".into(),
            channel_id: "UCbfYPyITQ-7l4upoX8nvctg".into(),
        },
        YouTubeChannel {
            name: "sentdex".into(),
            channel_id: "UCfzlCWGWYyIQ0aLC5w48gBQ".into(),
        },
        YouTubeChannel {
            name: "LiveOverflow".into(),
            channel_id: "UClcE-kVhqyiHCcjYwcpfj9w".into(),
        },
    ]
}

impl Default for YouTubeSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for YouTubeSource {
    fn source_type(&self) -> &'static str {
        "youtube"
    }

    fn name(&self) -> &'static str {
        "YouTube"
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

        if self.channels.is_empty() {
            info!("No YouTube channels configured");
            return Ok(Vec::new());
        }

        info!(
            channel_count = self.channels.len(),
            "Fetching YouTube feeds"
        );

        let mut all_items = Vec::new();

        for channel in &self.channels {
            match self.fetch_channel_feed(channel).await {
                Ok(entries) => {
                    info!(channel = %channel.name, count = entries.len(), "Fetched YouTube videos");
                    for entry in entries {
                        let url = format!("https://www.youtube.com/watch?v={}", entry.video_id);

                        // Truncate long descriptions
                        let desc = if entry.description.len() > 2000 {
                            format!(
                                "{}...",
                                &entry.description[..entry
                                    .description
                                    .char_indices()
                                    .nth(1997)
                                    .map(|(i, _)| i)
                                    .unwrap_or(entry.description.len())]
                            )
                        } else {
                            entry.description
                        };

                        let metadata = serde_json::json!({
                            "author": entry.author,
                            "published": entry.published,
                            "updated": entry.updated,
                            "views": entry.views,
                            "star_rating": entry.star_rating,
                            "video_id": entry.video_id,
                        });

                        let item = SourceItem::new("youtube", &entry.video_id, &entry.title)
                            .with_url(Some(url))
                            .with_content(format!("{}\n\n{}", entry.title, desc))
                            .with_metadata(metadata);

                        all_items.push(item);
                    }
                }
                Err(e) => {
                    warn!(channel = %channel.name, error = %e, "Failed to fetch YouTube feed");
                }
            }
        }

        // Limit total items
        all_items.truncate(self.config.max_items);

        info!(count = all_items.len(), "Fetched YouTube items");
        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // YouTube feed already includes description, no extra scraping needed
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
    fn test_youtube_source_creation() {
        let source = YouTubeSource::new();
        assert_eq!(source.source_type(), "youtube");
        assert_eq!(source.name(), "YouTube");
        assert_eq!(source.channels.len(), 16); // Default channels
        assert!(source.config().enabled);
    }

    #[test]
    fn test_with_channels() {
        let source = YouTubeSource::with_channels(vec!["UCsBjURrPoezykLs9EqgamOA".to_string()]);
        assert_eq!(source.channels.len(), 1);
        assert_eq!(source.channels[0].channel_id, "UCsBjURrPoezykLs9EqgamOA");
    }

    #[test]
    fn test_empty_channels_uses_defaults() {
        let source = YouTubeSource::with_channels(vec![]);
        assert_eq!(source.channels.len(), 16); // Falls back to defaults
    }

    #[test]
    fn test_parse_youtube_atom_feed() {
        let source = YouTubeSource::new();
        let xml = r#"
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/">
  <title>Fireship</title>
  <entry>
    <yt:videoId>abc123</yt:videoId>
    <title>Rust in 100 Seconds</title>
    <name>Fireship</name>
    <published>2026-02-01T18:00:00+00:00</published>
    <updated>2026-02-01T18:00:00+00:00</updated>
    <media:description>Learn Rust in 100 seconds</media:description>
    <media:statistics views="500000"/>
    <media:starRating count="25000" average="4.85"/>
  </entry>
  <entry>
    <yt:videoId>def456</yt:videoId>
    <title>10 Coding Trends for 2026</title>
    <name>Fireship</name>
    <published>2026-01-15T18:00:00+00:00</published>
    <updated>2026-01-15T18:00:00+00:00</updated>
    <media:description>Top coding trends</media:description>
    <media:statistics views="1000000"/>
    <media:starRating count="50000" average="4.90"/>
  </entry>
</feed>
        "#;

        let entries = source.parse_atom_feed(xml, "Fireship").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].video_id, "abc123");
        assert_eq!(entries[0].title, "Rust in 100 Seconds");
        assert_eq!(entries[0].views, 500000);
        assert_eq!(entries[1].video_id, "def456");
        assert_eq!(entries[1].views, 1000000);
    }

    #[test]
    fn test_extract_tag() {
        assert_eq!(
            extract_tag("<title>Test</title>", "title"),
            Some("Test".to_string())
        );
        assert_eq!(
            extract_tag("<yt:videoId>abc</yt:videoId>", "yt:videoId"),
            Some("abc".to_string())
        );
        assert_eq!(extract_tag("no tags here", "title"), None);
    }

    #[test]
    fn test_extract_attr() {
        let xml = r#"<media:statistics views="12345"/>"#;
        assert_eq!(
            extract_attr(xml, "media:statistics", "views"),
            Some("12345".to_string())
        );
    }

    #[test]
    fn test_default_channels() {
        let channels = default_channels();
        assert!(channels.len() >= 5);
        // Fireship should be first
        assert_eq!(channels[0].name, "Fireship");
        assert_eq!(channels[0].channel_id, "UCsBjURrPoezykLs9EqgamOA");
    }

    #[test]
    fn test_fetch_interval() {
        let source = YouTubeSource::new();
        assert_eq!(source.config.fetch_interval_secs, 1800); // 30 minutes
    }
}
