//! Tauri commands for real-time content translation.
//!
//! Exposes content translation to the frontend for feed items,
//! briefing summaries, and other dynamic content.

use crate::content_translation::{self, CacheStats, TranslationRequest, TranslationResult};
use crate::error::Result;
use crate::i18n;

// ============================================================================
// Tauri Commands
// ============================================================================

/// Translate a single content item to the user's preferred language.
///
/// Returns immediately if translation is cached, otherwise translates via LLM.
/// If the user's language is English or translation is disabled, returns original text.
#[tauri::command]
pub async fn translate_content(
    id: String,
    text: String,
    source_lang: Option<String>,
) -> Result<TranslationResult> {
    let target_lang = i18n::get_user_language();

    let request = TranslationRequest {
        id,
        text,
        source_lang: source_lang.unwrap_or_else(|| "en".to_string()),
    };

    Ok(content_translation::translate_content(&request, &target_lang).await)
}

/// Translate a batch of content items to the user's preferred language.
///
/// Efficient batch operation: checks cache for all items first, then
/// translates only uncached items in a single LLM call.
/// Returns results in the same order as the input.
#[tauri::command]
pub async fn translate_content_batch(
    items: Vec<TranslationRequest>,
) -> Result<Vec<TranslationResult>> {
    let target_lang = i18n::get_user_language();

    if target_lang == "en" {
        return Ok(items
            .iter()
            .map(|r| TranslationResult {
                id: r.id.clone(),
                original: r.text.clone(),
                translated: r.text.clone(),
                from_cache: false,
                provider: "none".to_string(),
            })
            .collect());
    }

    Ok(content_translation::translate_content_batch(&items, &target_lang).await)
}

/// Get the current content translation settings.
///
/// Returns whether translation is enabled, the provider preference,
/// and the target language derived from user locale.
#[tauri::command]
pub fn get_content_translation_settings() -> Result<content_translation::ContentTranslationSettings>
{
    let target_lang = i18n::get_user_language();

    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();

    // Translation is enabled if the user has a non-English language set
    // and has an LLM provider configured
    let has_llm = !settings.llm.api_key.is_empty() || settings.llm.provider == "ollama";
    let enabled = target_lang != "en" && has_llm;

    Ok(content_translation::ContentTranslationSettings {
        enabled,
        provider: if has_llm {
            "auto".to_string()
        } else {
            "disabled".to_string()
        },
        target_lang,
    })
}

/// Get translation cache statistics for the user's language.
#[tauri::command]
pub fn get_translation_cache_stats() -> Result<CacheStats> {
    let target_lang = i18n::get_user_language();
    content_translation::get_cache_stats(&target_lang)
}

/// Purge expired entries from the translation cache.
/// Returns the number of entries purged.
#[tauri::command]
pub fn purge_translation_cache() -> Result<usize> {
    content_translation::purge_expired_cache()
}

/// Get the dedicated translation provider configuration.
#[tauri::command]
pub fn get_translation_config() -> Result<crate::settings::types::TranslationConfig> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    Ok(guard.get().translation.clone())
}

/// Update the dedicated translation provider configuration.
#[tauri::command]
pub fn set_translation_config(config: crate::settings::types::TranslationConfig) -> Result<()> {
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    guard.get_mut().translation = config;
    guard.save()
}
