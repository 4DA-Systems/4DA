//! Source fetching functions extracted from lib.rs
//!
//! Contains: fetch_all_sources, fetch_all_sources_deep, fill_cache_background,
//! process_source_items, and settings loader helpers.

mod fetcher;
mod processor;

// Re-export everything at the module level so `crate::source_fetching::X` paths still work
pub(crate) use fetcher::{fetch_all_sources, fetch_all_sources_deep};
pub(crate) use processor::fill_cache_background;

use crate::get_settings_manager;

// ============================================================================
// Settings loader helpers
// ============================================================================

/// Load RSS feed URLs from settings
pub(crate) fn load_rss_feeds_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let feeds = settings.get_rss_feeds();
    drop(settings);
    feeds
}

/// Load Twitter handles and X API key from settings
pub(crate) fn load_twitter_settings() -> (Vec<String>, String) {
    let settings = get_settings_manager().lock();
    let handles = settings.get_twitter_handles();
    let api_key = settings.get_x_api_key();
    drop(settings);
    (handles, api_key)
}

/// Load YouTube channel IDs from settings
pub(crate) fn load_youtube_channels_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let channels = settings.get_youtube_channels();
    drop(settings);
    channels
}

/// Load GitHub languages from settings (defaults if empty)
pub(crate) fn load_github_languages_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let langs = settings.get_github_languages();
    drop(settings);
    if langs.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        langs
    }
}
