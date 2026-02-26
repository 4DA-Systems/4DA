//! Tauri commands for the autonomous translation pipeline.
//!
//! Exposes `get_translation_status`, `trigger_translation`, and
//! user-override CRUD to the frontend.

use crate::translation_pipeline;
use std::collections::HashMap;

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get translation completion status for a target language.
///
/// Compares the English locale source files against the existing translations
/// in `data/translations/{lang}/` and returns a percentage-complete report.
#[tauri::command]
pub fn get_translation_status(
    lang: String,
) -> Result<translation_pipeline::TranslationStatus, String> {
    let english = translation_pipeline::load_english_strings()?;
    let total = english.len();

    let untranslated = translation_pipeline::get_untranslated_keys(&lang)?;
    let translated = total.saturating_sub(untranslated.len());

    Ok(translation_pipeline::TranslationStatus {
        language: lang,
        total_keys: total,
        translated_keys: translated,
        percentage: if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        },
    })
}

/// Trigger LLM-powered translation of missing strings for a target language.
///
/// 1. Identifies all untranslated keys (English source minus existing target).
/// 2. Sends them to the configured LLM in batches of ~50 keys.
/// 3. Saves results to `data/translations/{lang}/`.
/// 4. Clears the i18n cache so translations take effect immediately.
///
/// Returns a human-readable summary string.
#[tauri::command]
pub async fn trigger_translation(lang: String) -> Result<String, String> {
    let untranslated = translation_pipeline::get_untranslated_keys(&lang)?;
    if untranslated.is_empty() {
        return Ok(format!("{} is fully translated", lang));
    }

    let translated = translation_pipeline::translate_batch(&untranslated, &lang).await?;
    let count = translation_pipeline::save_translations(&translated, &lang)?;

    // Clear i18n cache so new translations take effect
    crate::i18n::clear_cache();

    Ok(format!("Translated {} strings to {}", count, lang))
}

// ============================================================================
// Translation Override Commands
// ============================================================================

/// Get all translation entries for a language, merged with override status.
///
/// Returns a map of `"namespace:key"` to `{ english, translated, status }` where
/// status is one of: `"overridden"`, `"translated"`, `"untranslated"`.
#[tauri::command]
pub fn get_all_translations(lang: String) -> Result<HashMap<String, TranslationEntry>, String> {
    let english = translation_pipeline::load_english_strings()?;
    let overrides = load_overrides(&lang)?;

    // Load auto-translated strings
    let trans_dir = crate::i18n::translations_dir().join(&lang);
    let mut auto_translated: HashMap<String, String> = HashMap::new();
    if trans_dir.exists() {
        for ns in &["ui", "coach", "streets", "errors"] {
            let path = trans_dir.join(format!("{}.json", ns));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    for (k, v) in map {
                        auto_translated.insert(format!("{}:{}", ns, k), v);
                    }
                }
            }
        }
    }

    let mut result: HashMap<String, TranslationEntry> = HashMap::new();

    for (key, en_value) in &english {
        let override_value = overrides.get(key);
        let auto_value = auto_translated.get(key);

        let (translated, status) = if let Some(ov) = override_value {
            (Some(ov.clone()), "overridden".to_string())
        } else if let Some(av) = auto_value {
            (Some(av.clone()), "translated".to_string())
        } else {
            (None, "untranslated".to_string())
        };

        result.insert(
            key.clone(),
            TranslationEntry {
                english: en_value.clone(),
                translated,
                status,
            },
        );
    }

    Ok(result)
}

/// Save a single user override for a translation key.
///
/// Persists to `data/translations/overrides/{lang}/{namespace}.json`.
#[tauri::command]
pub fn save_translation_override(
    lang: String,
    namespace: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let overrides_dir = crate::i18n::translations_dir()
        .join("overrides")
        .join(&lang);
    std::fs::create_dir_all(&overrides_dir)
        .map_err(|e| format!("Cannot create overrides dir: {}", e))?;

    let path = overrides_dir.join(format!("{}.json", namespace));

    let mut existing: HashMap<String, String> = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    existing.insert(key.clone(), value);

    let json = serde_json::to_string_pretty(&existing)
        .map_err(|e| format!("JSON serialize error: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;

    // Clear cache so the override takes effect immediately
    crate::i18n::clear_cache();

    tracing::info!(target: "4da::i18n", lang = %lang, ns = %namespace, key = %key, "Translation override saved");
    Ok(())
}

/// Get all user overrides for a language.
///
/// Returns a flat map of `"namespace:key"` to override value.
#[tauri::command]
pub fn get_translation_overrides(lang: String) -> Result<HashMap<String, String>, String> {
    load_overrides(&lang)
}

/// Delete a single user override.
#[tauri::command]
pub fn delete_translation_override(
    lang: String,
    namespace: String,
    key: String,
) -> Result<(), String> {
    let overrides_dir = crate::i18n::translations_dir()
        .join("overrides")
        .join(&lang);
    let path = overrides_dir.join(format!("{}.json", namespace));

    if !path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut map: HashMap<String, String> = serde_json::from_str(&content).unwrap_or_default();
    map.remove(&key);

    let json =
        serde_json::to_string_pretty(&map).map_err(|e| format!("JSON serialize error: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;

    crate::i18n::clear_cache();

    tracing::info!(target: "4da::i18n", lang = %lang, ns = %namespace, key = %key, "Translation override deleted");
    Ok(())
}

// ============================================================================
// Types
// ============================================================================

/// A single translation entry with status metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationEntry {
    pub english: String,
    pub translated: Option<String>,
    pub status: String,
}

// ============================================================================
// Helpers
// ============================================================================

/// Load all override files for a language into a single flat map.
fn load_overrides(lang: &str) -> Result<HashMap<String, String>, String> {
    let overrides_dir = crate::i18n::translations_dir().join("overrides").join(lang);
    let mut overrides: HashMap<String, String> = HashMap::new();

    if !overrides_dir.exists() {
        return Ok(overrides);
    }

    for ns in &["ui", "coach", "streets", "errors"] {
        let path = overrides_dir.join(format!("{}.json", ns));
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                for (k, v) in map {
                    overrides.insert(format!("{}:{}", ns, k), v);
                }
            }
        }
    }

    Ok(overrides)
}
