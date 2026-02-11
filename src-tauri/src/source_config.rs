//! Source configuration commands for RSS, Twitter, YouTube, and GitHub.
//!
//! Extracted from lib.rs to reduce file size. These are pure Tauri command
//! wrappers around the settings manager.

use tracing::{info, warn};

use crate::get_settings_manager;

// ============================================================================
// RSS Feed Commands
// ============================================================================

/// Get configured RSS feed URLs
#[tauri::command]
pub async fn get_rss_feeds() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let feeds = settings_guard.get_rss_feeds();

    Ok(serde_json::json!({
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Add an RSS feed URL
#[tauri::command]
pub async fn add_rss_feed(url: String) -> Result<serde_json::Value, String> {
    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL: must start with http:// or https://".to_string());
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.add_rss_feed(url.clone())?;

    let feeds = settings_guard.get_rss_feeds();

    info!(target: "4da::rss", url = %url, "Added RSS feed");

    Ok(serde_json::json!({
        "success": true,
        "added": url,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Remove an RSS feed URL
#[tauri::command]
pub async fn remove_rss_feed(url: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_rss_feed(&url)?;

    let feeds = settings_guard.get_rss_feeds();

    info!(target: "4da::rss", url = %url, "Removed RSS feed");

    Ok(serde_json::json!({
        "success": true,
        "removed": url,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Set all RSS feed URLs (replacing existing)
#[tauri::command]
pub async fn set_rss_feeds(feeds: Vec<String>) -> Result<serde_json::Value, String> {
    // Validate all URLs
    for url in &feeds {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!(
                "Invalid URL: {} must start with http:// or https://",
                url
            ));
        }
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
pub async fn get_twitter_handles() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let handles = settings_guard.get_twitter_handles();

    Ok(serde_json::json!({
        "handles": handles,
        "count": handles.len()
    }))
}

/// Add a Twitter handle
#[tauri::command]
pub async fn add_twitter_handle(handle: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();

    // Validate handle (remove @ if present)
    let clean_handle = handle.trim_start_matches('@').to_string();

    settings_guard.add_twitter_handle(clean_handle.clone())?;

    let handles = settings_guard.get_twitter_handles();

    info!(target: "4da::twitter", handle = %clean_handle, "Added Twitter handle");

    Ok(serde_json::json!({
        "success": true,
        "added": clean_handle,
        "handles": handles,
        "count": handles.len()
    }))
}

/// Remove a Twitter handle
#[tauri::command]
pub async fn remove_twitter_handle(handle: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_twitter_handle(&handle)?;

    let handles = settings_guard.get_twitter_handles();

    info!(target: "4da::twitter", handle = %handle, "Removed Twitter handle");

    Ok(serde_json::json!({
        "success": true,
        "removed": handle,
        "handles": handles,
        "count": handles.len()
    }))
}

/// Set all Twitter handles (replacing existing)
#[tauri::command]
pub async fn set_twitter_handles(handles: Vec<String>) -> Result<serde_json::Value, String> {
    // Clean all handles (remove @ if present)
    let clean_handles: Vec<String> = handles
        .iter()
        .map(|h| h.trim_start_matches('@').to_string())
        .collect();

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_twitter_handles(clean_handles.clone())?;

    info!(target: "4da::twitter", count = clean_handles.len(), "Set Twitter handles");

    Ok(serde_json::json!({
        "success": true,
        "handles": clean_handles,
        "count": clean_handles.len()
    }))
}

/// Get configured Nitter instance
#[tauri::command]
pub async fn get_nitter_instance() -> Result<String, String> {
    let settings_guard = get_settings_manager().lock();
    Ok(settings_guard.get_nitter_instance())
}

/// Set Nitter instance
#[tauri::command]
pub async fn set_nitter_instance(instance: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_nitter_instance(instance.clone())?;

    info!(target: "4da::twitter", instance = %instance, "Set Nitter instance");

    Ok(serde_json::json!({
        "success": true,
        "instance": instance
    }))
}

// ============================================================================
// X API Key Commands
// ============================================================================

/// Get configured X API Bearer Token
#[tauri::command]
pub async fn get_x_api_key() -> Result<String, String> {
    let settings_guard = get_settings_manager().lock();
    Ok(settings_guard.get_x_api_key())
}

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

/// Set X API Bearer Token
#[tauri::command]
pub async fn set_x_api_key(key: String) -> Result<serde_json::Value, String> {
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get("https://api.x.com/2/users/by/username/twitter")
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
            Err("Invalid X API Bearer Token. Make sure you're using the Bearer Token from your X Developer Portal (not the API Key/Secret). It should start with 'AAAA...'.".to_string())
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
            ))
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
pub async fn get_youtube_channels() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let channels = settings_guard.get_youtube_channels();

    Ok(serde_json::json!({
        "channels": channels,
        "count": channels.len()
    }))
}

/// Add a YouTube channel ID
#[tauri::command]
pub async fn add_youtube_channel(channel_id: String) -> Result<serde_json::Value, String> {
    if channel_id.trim().is_empty() {
        return Err("Channel ID cannot be empty".to_string());
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.add_youtube_channel(channel_id.clone())?;

    let channels = settings_guard.get_youtube_channels();

    info!(target: "4da::youtube", channel_id = %channel_id, "Added YouTube channel");

    Ok(serde_json::json!({
        "success": true,
        "added": channel_id,
        "channels": channels,
        "count": channels.len()
    }))
}

/// Remove a YouTube channel ID
#[tauri::command]
pub async fn remove_youtube_channel(channel_id: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_youtube_channel(&channel_id)?;

    let channels = settings_guard.get_youtube_channels();

    info!(target: "4da::youtube", channel_id = %channel_id, "Removed YouTube channel");

    Ok(serde_json::json!({
        "success": true,
        "removed": channel_id,
        "channels": channels,
        "count": channels.len()
    }))
}

/// Set all YouTube channel IDs (replacing existing)
#[tauri::command]
pub async fn set_youtube_channels(channels: Vec<String>) -> Result<serde_json::Value, String> {
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
pub async fn get_github_languages() -> Result<serde_json::Value, String> {
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
pub async fn set_github_languages(languages: Vec<String>) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_github_languages(languages.clone())?;

    info!(target: "4da::github", count = languages.len(), "Set GitHub languages");

    Ok(serde_json::json!({
        "success": true,
        "languages": languages,
        "count": languages.len()
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
}
