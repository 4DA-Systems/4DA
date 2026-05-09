// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Curated feed library and feed validation commands.
//!
//! Split from `source_config.rs` to stay under the 700-line file-size limit.
//! Re-exported via `pub use` in the parent module so all paths remain unchanged.

use tracing::info;

use crate::error::Result;
use crate::get_settings_manager;

use super::validate_input_length;

// ============================================================================
// Curated Feed Library Commands
// ============================================================================

/// Get the full curated feed catalog with metadata for the Source Browser UI.
/// Returns all feeds grouped by domain, with enabled/disabled status.
#[tauri::command]
pub async fn get_curated_feeds() -> Result<serde_json::Value> {
    let registry = crate::curated_feeds::get_curated_registry();
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_rss_feeds();
    drop(settings);

    let feeds: Vec<serde_json::Value> = registry
        .all_feeds()
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "url": f.url,
                "homepage": f.homepage,
                "description": f.description,
                "domains": f.domains,
                "content_type": f.content_type,
                "tier": f.tier,
                "editorial_model": f.editorial_model,
                "expected_frequency_days": f.expected_frequency_days,
                "color_hint": f.color_hint,
                "enabled": !disabled.contains(&f.url),
            })
        })
        .collect();

    let domains = registry.all_domains();

    Ok(serde_json::json!({
        "feeds": feeds,
        "total": feeds.len(),
        "domains": domains,
    }))
}

/// Get curated feeds filtered by domain, with enabled/disabled status.
#[tauri::command]
pub async fn get_curated_feeds_by_domain(domain: String) -> Result<serde_json::Value> {
    let registry = crate::curated_feeds::get_curated_registry();
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_rss_feeds();
    drop(settings);

    let feeds: Vec<serde_json::Value> = registry
        .feeds_for_domain(&domain)
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "url": f.url,
                "homepage": f.homepage,
                "description": f.description,
                "domains": f.domains,
                "content_type": f.content_type,
                "tier": f.tier,
                "editorial_model": f.editorial_model,
                "expected_frequency_days": f.expected_frequency_days,
                "color_hint": f.color_hint,
                "enabled": !disabled.contains(&f.url),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "feeds": feeds,
        "count": feeds.len(),
        "domain": domain,
    }))
}

/// Get curated feeds suggested for the user based on ACE-detected stack.
/// Matches feed domains against detected languages and project types.
#[tauri::command]
pub async fn get_suggested_curated_feeds() -> Result<serde_json::Value> {
    let registry = crate::curated_feeds::get_curated_registry();
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_rss_feeds();
    drop(settings);

    // Map ACE-detected languages to feed domain tags
    let detected_languages = crate::state::get_ace_detected_languages();
    let mut relevant_domains: Vec<&str> = Vec::new();

    for lang in &detected_languages {
        match lang.to_lowercase().as_str() {
            "rust" => relevant_domains.push("rust"),
            "typescript" | "javascript" => {
                relevant_domains.push("typescript");
                relevant_domains.push("javascript");
                relevant_domains.push("web-platform");
            }
            "python" => relevant_domains.push("python"),
            "go" => relevant_domains.push("go"),
            _ => {}
        }
    }

    // Security and open-source are always relevant
    relevant_domains.push("security");
    relevant_domains.push("open-source");
    relevant_domains.dedup();

    let suggested: Vec<serde_json::Value> = registry
        .all_feeds()
        .iter()
        .filter(|f| {
            f.domains
                .iter()
                .any(|d| relevant_domains.contains(&d.as_str()))
        })
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "url": f.url,
                "homepage": f.homepage,
                "description": f.description,
                "domains": f.domains,
                "content_type": f.content_type,
                "tier": f.tier,
                "editorial_model": f.editorial_model,
                "color_hint": f.color_hint,
                "enabled": !disabled.contains(&f.url),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "feeds": suggested,
        "count": suggested.len(),
        "matched_domains": relevant_domains,
        "detected_languages": detected_languages,
    }))
}

/// Toggle a curated feed on or off by its URL.
/// Disabling adds the URL to disabled_default_rss_feeds.
/// Enabling removes it from that list.
#[tauri::command]
pub async fn toggle_curated_feed(url: String, enabled: bool) -> Result<serde_json::Value> {
    // Verify it's actually a curated feed
    if !crate::curated_feeds::is_curated_feed(&url) {
        return Err("URL is not a curated feed".into());
    }

    let mut settings = get_settings_manager().lock();
    let mut disabled = settings.get_disabled_default_rss_feeds();

    if enabled {
        disabled.retain(|f| f != &url);
    } else if !disabled.contains(&url) {
        disabled.push(url.clone());
    }

    settings.set_disabled_default_rss_feeds(disabled)?;
    info!(target: "4da::curated", url = %url, enabled = enabled, "Toggled curated feed");

    Ok(serde_json::json!({
        "success": true,
        "url": url,
        "enabled": enabled,
    }))
}

// ============================================================================
// Feed Validation Commands
// ============================================================================

/// Validate an RSS/Atom feed URL by fetching it and checking structure
#[tauri::command]
pub async fn validate_rss_feed(url: String) -> Result<serde_json::Value> {
    validate_input_length(&url, "Feed URL", 2000)?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }
    crate::url_validation::validate_not_internal(&url)?;

    let client = crate::sources::shared_client();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .header("User-Agent", "Mozilla/5.0 (compatible; 4DA/1.0)")
        .send()
        .await
        .map_err(|e| format!("Could not reach URL: {}", e))?;

    if !resp.status().is_success() {
        return Ok(serde_json::json!({
            "valid": false,
            "reason": "http_error",
            "status": resp.status().as_u16(),
            "message": format!("URL returned HTTP {}", resp.status())
        }));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Could not read response: {}", e))?;

    let is_xml = content_type.contains("xml")
        || content_type.contains("rss")
        || content_type.contains("atom");
    let is_html = content_type.contains("html")
        || body.trim_start().starts_with("<!DOCTYPE")
        || body.trim_start().starts_with("<html");
    let has_rss_tags = body.contains("<rss") || body.contains("<item>");
    let has_atom_tags = body.contains("<feed") || body.contains("<entry>");

    if is_xml || has_rss_tags || has_atom_tags {
        let rss_source = crate::sources::rss::RssSource::new();
        let entries = rss_source.parse_feed(&body, &url);
        let feed_title = crate::sources::rss::RssSource::extract_tag(&body, "title")
            .unwrap_or_else(|| url.clone());

        if entries.is_empty() {
            return Ok(serde_json::json!({
                "valid": false,
                "reason": "empty_feed",
                "message": "Feed parsed but contains no items"
            }));
        }

        return Ok(serde_json::json!({
            "valid": true,
            "feed_title": feed_title,
            "item_count": entries.len(),
            "format": if has_atom_tags && !has_rss_tags { "atom" } else { "rss" },
            "sample_title": entries.first().map(|e| &e.title)
        }));
    }

    if is_html {
        let discovered = discover_feed_links(&body, &url);
        return Ok(serde_json::json!({
            "valid": false,
            "reason": "html_not_feed",
            "message": "This URL is an HTML page, not an RSS feed",
            "discovered_feeds": discovered
        }));
    }

    Ok(serde_json::json!({
        "valid": false,
        "reason": "unknown_format",
        "message": "Could not detect RSS or Atom feed at this URL"
    }))
}

/// Discover RSS/Atom feed links from an HTML page
fn discover_feed_links(html: &str, base_url: &str) -> Vec<String> {
    let mut feeds = Vec::new();
    let base = url::Url::parse(base_url).ok();

    // Look for <link rel="alternate" type="application/rss+xml" href="...">
    // and <link rel="alternate" type="application/atom+xml" href="...">
    for segment in html.split("<link") {
        let lower = segment.to_lowercase();
        if !lower.contains("alternate") {
            continue;
        }
        if !lower.contains("rss+xml") && !lower.contains("atom+xml") {
            continue;
        }

        // Extract href
        if let Some(href_start) = segment.find("href=") {
            let after_href = &segment[href_start + 5..];
            let quote = if after_href.starts_with('"') {
                '"'
            } else if after_href.starts_with('\'') {
                '\''
            } else {
                continue;
            };
            let inner = &after_href[1..];
            if let Some(end) = inner.find(quote) {
                let href = &inner[..end];
                // Resolve relative URL
                let resolved = if href.starts_with("http://") || href.starts_with("https://") {
                    href.to_string()
                } else if let Some(ref base) = base {
                    base.join(href)
                        .map_or_else(|_| href.to_string(), |u| u.to_string())
                } else {
                    href.to_string()
                };
                if !feeds.contains(&resolved) {
                    feeds.push(resolved);
                }
            }
        }

        if feeds.len() >= 5 {
            break;
        }
    }

    feeds
}

/// Validate a YouTube channel ID by fetching its Atom feed
#[tauri::command]
pub async fn validate_youtube_channel(channel_id: String) -> Result<serde_json::Value> {
    validate_input_length(&channel_id, "Channel ID", 100)?;

    let url = format!(
        "https://www.youtube.com/feeds/videos.xml?channel_id={}",
        channel_id
    );

    let client = crate::sources::shared_client();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Could not reach YouTube: {}", e))?;

    if resp.status() == 404 {
        return Ok(serde_json::json!({
            "valid": false,
            "reason": "not_found",
            "message": "No YouTube channel found with this ID"
        }));
    }

    if !resp.status().is_success() {
        return Ok(serde_json::json!({
            "valid": false,
            "reason": "http_error",
            "message": format!("YouTube returned HTTP {}", resp.status())
        }));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Could not read response: {}", e))?;

    let channel_name =
        crate::sources::youtube::extract_tag(&body, "title").unwrap_or_else(|| channel_id.clone());

    let entry_count = body.matches("<entry>").count();

    Ok(serde_json::json!({
        "valid": true,
        "channel_name": channel_name,
        "video_count": entry_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_feed_links_rss() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/feed.xml" />
        </head></html>"#;
        let feeds = discover_feed_links(html, "https://example.com/blog");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0], "https://example.com/feed.xml");
    }

    #[test]
    fn test_discover_feed_links_atom() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/atom+xml" href="https://example.com/atom.xml" />
        </head></html>"#;
        let feeds = discover_feed_links(html, "https://example.com");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0], "https://example.com/atom.xml");
    }

    #[test]
    fn test_discover_feed_links_no_feeds() {
        let html = r#"<html><head><title>No feeds</title></head></html>"#;
        let feeds = discover_feed_links(html, "https://example.com");
        assert!(feeds.is_empty());
    }

    #[test]
    fn test_discover_feed_links_deduplicates() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/feed.xml" />
            <link rel="alternate" type="application/rss+xml" href="/feed.xml" />
        </head></html>"#;
        let feeds = discover_feed_links(html, "https://example.com");
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn test_discover_feed_links_max_five() {
        let mut html = String::from("<html><head>");
        for i in 0..10 {
            html.push_str(&format!(
                r#"<link rel="alternate" type="application/rss+xml" href="/feed{}.xml" />"#,
                i
            ));
        }
        html.push_str("</head></html>");
        let feeds = discover_feed_links(&html, "https://example.com");
        assert!(feeds.len() <= 5);
    }
}
