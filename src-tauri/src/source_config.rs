// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source configuration commands for RSS, Twitter, YouTube, and GitHub.
//!
//! Extracted from lib.rs to reduce file size. These are pure Tauri command
//! wrappers around the settings manager.

use tracing::{info, warn};

use crate::error::Result;
use crate::get_settings_manager;
use crate::sources::MAX_SOURCES;

/// Validate string input length
pub(crate) fn validate_input_length(value: &str, field: &str, max_len: usize) -> Result<()> {
    if value.len() > max_len {
        return Err(format!(
            "{} too long ({} chars, max {})",
            field,
            value.len(),
            max_len
        )
        .into());
    }
    Ok(())
}

// ============================================================================
// RSS Feed Commands
// ============================================================================

/// Get configured RSS feed URLs
#[tauri::command]
pub async fn get_rss_feeds() -> Result<serde_json::Value> {
    let settings_guard = get_settings_manager().lock();
    let feeds = settings_guard.get_rss_feeds();

    Ok(serde_json::json!({
        "feeds": feeds,
        "count": feeds.len()
    }))
}
/// Set all RSS feed URLs (replacing existing)
#[tauri::command]
pub async fn set_rss_feeds(feeds: Vec<String>) -> Result<serde_json::Value> {
    if feeds.len() > MAX_SOURCES {
        return Err(format!(
            "Maximum of {} RSS feeds reached. Remove a feed before adding another.",
            MAX_SOURCES
        )
        .into());
    }
    for url in &feeds {
        validate_input_length(url, "Feed URL", 2000)?;
    }
    // Validate all URLs
    for url in &feeds {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!("Invalid URL: {url} must start with http:// or https://").into());
        }
        // Block internal/private network addresses (SSRF prevention)
        crate::url_validation::validate_not_internal(url)?;
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_rss_feeds(feeds.clone())?;

    info!(target: "4da::rss", count = feeds.len(), "Set RSS feeds");

    Ok(serde_json::json!({
        "success": true,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

// ============================================================================
// Twitter Source Commands
// ============================================================================

/// Get configured Twitter handles
#[tauri::command]
pub async fn get_twitter_handles() -> Result<serde_json::Value> {
    let settings_guard = get_settings_manager().lock();
    let handles = settings_guard.get_twitter_handles();

    Ok(serde_json::json!({
        "handles": handles,
        "count": handles.len()
    }))
}
/// Validate a Twitter handle: 1-15 chars, alphanumeric or underscore only.
fn is_valid_twitter_handle(handle: &str) -> bool {
    !handle.is_empty()
        && handle.len() <= 15
        && handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Set all Twitter handles (replacing existing)
#[tauri::command]
pub async fn set_twitter_handles(handles: Vec<String>) -> Result<serde_json::Value> {
    if handles.len() > MAX_SOURCES {
        return Err(format!(
            "Maximum of {} Twitter handles reached. Remove a handle before adding another.",
            MAX_SOURCES
        )
        .into());
    }
    for handle in &handles {
        validate_input_length(handle, "Twitter handle", 50)?;
    }
    // Clean all handles (remove @ if present)
    let clean_handles: Vec<String> = handles
        .iter()
        .map(|h| h.trim_start_matches('@').to_string())
        .collect();

    // Validate handle format: 1-15 chars, alphanumeric or underscore
    for handle in &clean_handles {
        if !is_valid_twitter_handle(handle) {
            return Err(format!(
                "Invalid Twitter handle '{}': must be 1-15 alphanumeric/underscore characters",
                handle
            )
            .into());
        }
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_twitter_handles(clean_handles.clone())?;

    info!(target: "4da::twitter", count = clean_handles.len(), "Set Twitter handles");

    Ok(serde_json::json!({
        "success": true,
        "handles": clean_handles,
        "count": clean_handles.len()
    }))
}
// ============================================================================
// X API Key Commands
// ============================================================================

/// Sanitize an X API Bearer Token (trim, URL-decode, extract from pasted blobs)
fn sanitize_x_api_key(raw: &str) -> String {
    let mut key = raw.trim().to_string();

    // URL-decode if it contains percent-encoded chars
    if key.contains('%') {
        if let Ok(decoded) = urlencoding::decode(&key) {
            key = decoded.into_owned();
        }
    }

    // If the pasted value contains spaces, try to extract the Bearer Token portion.
    // X Bearer Tokens start with "AAAAAAAAAAAAAAAAAAAAAA" (22+ A's).
    if key.contains(' ') {
        if let Some(token_start) = key.find("AAAAAAAAAAAAAAAAAAAAAA") {
            key = key[token_start..].trim().to_string();
            info!(target: "4da::twitter", "Extracted Bearer Token from pasted credentials");
        }
    }

    key
}

/// Check whether an X API Bearer Token is configured (never returns the key itself)
#[tauri::command]
pub async fn has_x_api_key() -> Result<bool> {
    let settings_guard = get_settings_manager().lock();
    Ok(!settings_guard.get_x_api_key().is_empty())
}

/// Set X API Bearer Token
#[tauri::command]
pub async fn set_x_api_key(key: String) -> Result<serde_json::Value> {
    let cleaned = sanitize_x_api_key(&key);

    if cleaned.is_empty() {
        let mut settings_guard = get_settings_manager().lock();
        settings_guard.set_x_api_key(String::new())?;
        return Ok(serde_json::json!({
            "success": true,
            "has_key": false,
            "validated": false
        }));
    }

    // Validate the token by making a test API call
    let client = crate::sources::shared_client();

    let resp = client
        .get("https://api.x.com/2/users/by/username/twitter")
        .timeout(std::time::Duration::from_secs(10))
        .bearer_auth(&cleaned)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            info!(target: "4da::twitter", "X API key validated successfully");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": true
            }))
        }
        Ok(r) if r.status().as_u16() == 401 => {
            warn!(target: "4da::twitter", "X API key validation failed: 401 Unauthorized");
            Err("Invalid X API Bearer Token. Make sure you're using the Bearer Token from your X Developer Portal (not the API Key/Secret). It should start with 'AAAA...'.".into())
        }
        Ok(r) if r.status().as_u16() == 429 => {
            // Rate limited - token format looks valid if we got this far, save it
            info!(target: "4da::twitter", "X API rate limited during validation - saving token anyway");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Token saved. Could not validate right now (X API rate limit). It will be used on the next fetch cycle."
            }))
        }
        Ok(r) if r.status().as_u16() == 403 => {
            // 403 can mean the token works but doesn't have the right access level
            warn!(target: "4da::twitter", status = %r.status(), "X API key may lack permissions");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Token accepted but may lack required permissions. Ensure your X app has 'Read' access."
            }))
        }
        Ok(r) => {
            warn!(target: "4da::twitter", status = %r.status(), "X API key validation returned unexpected status");
            Err(format!(
                "X API returned HTTP {}. Check your Bearer Token.",
                r.status()
            )
            .into())
        }
        Err(e) => {
            warn!(target: "4da::twitter", error = %e, "Could not reach X API for validation");
            // Save anyway - might be a network issue, not a bad token
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Could not validate token (network issue). Saved anyway."
            }))
        }
    }
}

// ============================================================================
// YouTube Source Commands
// ============================================================================

/// Get configured YouTube channel IDs
#[tauri::command]
pub async fn get_youtube_channels() -> Result<serde_json::Value> {
    let settings_guard = get_settings_manager().lock();
    let channels = settings_guard.get_youtube_channels();

    Ok(serde_json::json!({
        "channels": channels,
        "count": channels.len()
    }))
}
/// Set all YouTube channel IDs (replacing existing)
#[tauri::command]
pub async fn set_youtube_channels(channels: Vec<String>) -> Result<serde_json::Value> {
    if channels.len() > MAX_SOURCES {
        return Err(format!(
            "Maximum of {} YouTube channels reached. Remove a channel before adding another.",
            MAX_SOURCES
        )
        .into());
    }
    for channel in &channels {
        validate_input_length(channel, "YouTube channel ID", 100)?;
    }
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_youtube_channels(channels.clone())?;

    info!(target: "4da::youtube", count = channels.len(), "Set YouTube channels");

    Ok(serde_json::json!({
        "success": true,
        "channels": channels,
        "count": channels.len()
    }))
}

// ============================================================================
// GitHub Source Commands
// ============================================================================

/// Get configured GitHub languages (default: rust, typescript, python)
#[tauri::command]
pub async fn get_github_languages() -> Result<serde_json::Value> {
    let settings_guard = get_settings_manager().lock();
    let languages = settings_guard.get_github_languages();

    // Return saved languages, or defaults if none configured
    let result = if languages.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        languages
    };

    Ok(serde_json::json!({
        "languages": result,
        "count": result.len()
    }))
}

/// Set GitHub languages to monitor
#[tauri::command]
pub async fn set_github_languages(languages: Vec<String>) -> Result<serde_json::Value> {
    if languages.len() > 50 {
        return Err("Too many languages (max 50)".into());
    }
    for lang in &languages {
        validate_input_length(lang, "Language", 50)?;
    }
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_github_languages(languages.clone())?;

    info!(target: "4da::github", count = languages.len(), "Set GitHub languages");

    Ok(serde_json::json!({
        "success": true,
        "languages": languages,
        "count": languages.len()
    }))
}

// ============================================================================
// Default Feed List Commands
// ============================================================================

/// Get the built-in default RSS feed URLs
#[tauri::command]
pub async fn get_default_rss_feeds() -> Result<serde_json::Value> {
    let feeds = crate::source_fetching::load_default_rss_feeds();
    Ok(serde_json::json!({ "feeds": feeds }))
}

/// Get the built-in default YouTube channel IDs
#[tauri::command]
pub async fn get_default_youtube_channels() -> Result<serde_json::Value> {
    let channels = crate::source_fetching::load_default_youtube_channels();
    Ok(serde_json::json!({ "channels": channels }))
}

/// Get the built-in default Twitter handles
#[tauri::command]
pub async fn get_default_twitter_handles() -> Result<serde_json::Value> {
    let handles = crate::source_fetching::load_default_twitter_handles();
    Ok(serde_json::json!({ "handles": handles }))
}

// ============================================================================
// Disabled Default Commands
// ============================================================================

/// Get the list of default RSS feeds that the user has disabled
#[tauri::command]
pub async fn get_disabled_default_rss_feeds() -> Result<serde_json::Value> {
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_rss_feeds();
    Ok(serde_json::json!({ "disabled": disabled }))
}

/// Set which default RSS feeds are disabled
#[tauri::command]
pub async fn set_disabled_default_rss_feeds(feeds: Vec<String>) -> Result<serde_json::Value> {
    let mut settings = get_settings_manager().lock();
    settings.set_disabled_default_rss_feeds(feeds)?;
    Ok(serde_json::json!({ "success": true }))
}

/// Get the list of default YouTube channels that the user has disabled
#[tauri::command]
pub async fn get_disabled_default_youtube_channels() -> Result<serde_json::Value> {
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_youtube_channels();
    Ok(serde_json::json!({ "disabled": disabled }))
}

/// Set which default YouTube channels are disabled
#[tauri::command]
pub async fn set_disabled_default_youtube_channels(
    channels: Vec<String>,
) -> Result<serde_json::Value> {
    let mut settings = get_settings_manager().lock();
    settings.set_disabled_default_youtube_channels(channels)?;
    Ok(serde_json::json!({ "success": true }))
}

/// Get the list of default Twitter handles that the user has disabled
#[tauri::command]
pub async fn get_disabled_default_twitter_handles() -> Result<serde_json::Value> {
    let settings = get_settings_manager().lock();
    let disabled = settings.get_disabled_default_twitter_handles();
    Ok(serde_json::json!({ "disabled": disabled }))
}

/// Set which default Twitter handles are disabled
#[tauri::command]
pub async fn set_disabled_default_twitter_handles(
    handles: Vec<String>,
) -> Result<serde_json::Value> {
    let mut settings = get_settings_manager().lock();
    settings.set_disabled_default_twitter_handles(handles)?;
    Ok(serde_json::json!({ "success": true }))
}

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

    /// Validate an RSS feed URL
    fn is_valid_rss_url(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    /// Clean a Twitter handle (remove @ prefix)
    fn clean_twitter_handle(handle: &str) -> String {
        handle.trim_start_matches('@').to_string()
    }

    #[test]
    fn test_sanitize_x_api_key_trim() {
        assert_eq!(sanitize_x_api_key("  abc123  "), "abc123");
    }

    #[test]
    fn test_sanitize_x_api_key_url_decode() {
        assert_eq!(sanitize_x_api_key("hello%20world"), "hello world");
    }

    #[test]
    fn test_sanitize_x_api_key_extract_bearer() {
        let pasted = "Bearer AAAAAAAAAAAAAAAAAAAAAA%2FsomeToken";
        let result = sanitize_x_api_key(pasted);
        // After URL-decode: "Bearer AAAAAAAAAAAAAAAAAAAAAA/someToken"
        // Extract from "AAAA..." portion
        assert!(result.starts_with("AAAAAAAAAAAAAAAAAAAAAA"));
    }

    #[test]
    fn test_sanitize_x_api_key_empty() {
        assert_eq!(sanitize_x_api_key(""), "");
        assert_eq!(sanitize_x_api_key("   "), "");
    }

    #[test]
    fn test_sanitize_x_api_key_passthrough() {
        let token = "AAAAAAAAAAAAAAAAAAAAAAAAtoken123";
        assert_eq!(sanitize_x_api_key(token), token);
    }

    #[test]
    fn test_is_valid_rss_url() {
        assert!(is_valid_rss_url("https://example.com/feed"));
        assert!(is_valid_rss_url("http://example.com/rss"));
        assert!(!is_valid_rss_url("ftp://example.com"));
        assert!(!is_valid_rss_url("example.com/feed"));
        assert!(!is_valid_rss_url(""));
    }

    #[test]
    fn test_clean_twitter_handle() {
        assert_eq!(clean_twitter_handle("@elonmusk"), "elonmusk");
        assert_eq!(clean_twitter_handle("elonmusk"), "elonmusk");
        assert_eq!(clean_twitter_handle("@@double"), "double");
    }

    #[test]
    fn test_ssrf_internal_urls_blocked() {
        use crate::url_validation::validate_not_internal;
        // Internal addresses should be rejected
        assert!(validate_not_internal("http://127.0.0.1/feed").is_err());
        assert!(validate_not_internal("http://localhost/feed").is_err());
        assert!(validate_not_internal("http://10.0.0.1/feed").is_err());
        assert!(validate_not_internal("http://192.168.1.1/feed").is_err());
        assert!(validate_not_internal("http://169.254.169.254/latest/meta-data/").is_err());
        // Public addresses should pass
        assert!(validate_not_internal("https://example.com/feed.xml").is_ok());
    }

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

    #[test]
    fn test_twitter_handle_format_validation() {
        // Valid handles
        assert!(is_valid_twitter_handle("rustlang"));
        assert!(is_valid_twitter_handle("_test_"));
        assert!(is_valid_twitter_handle("a"));
        assert!(is_valid_twitter_handle("A1_b2_C3"));

        // Invalid handles
        assert!(!is_valid_twitter_handle(""));
        assert!(!is_valid_twitter_handle(
            "this_handle_is_too_long_for_twitter"
        ));
        assert!(!is_valid_twitter_handle("has spaces"));
        assert!(!is_valid_twitter_handle("has.dots"));
        assert!(!is_valid_twitter_handle("has-dashes"));
        assert!(!is_valid_twitter_handle("has@symbol"));
    }
}
