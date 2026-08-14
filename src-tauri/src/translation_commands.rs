// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for the autonomous translation pipeline.
//!
//! Exposes `get_translation_status`, `trigger_translation`, and
//! user-override CRUD to the frontend.

use crate::error::{Result, ResultExt};
use crate::i18n::{validate_locale, validate_namespace, validate_translation_key};
use crate::translation_pipeline;
use std::collections::HashMap;

/// Refuse to read an override file larger than this. Override files hold a
/// handful of short UI strings; anything at this size is corrupt or hostile,
/// and parsing it would only serve to burn memory.
const MAX_OVERRIDE_FILE_BYTES: u64 = 1_000_000;

/// Cap on a single override value. Overrides are UI strings, not documents.
const MAX_OVERRIDE_VALUE_LENGTH: usize = 4_096;

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get translation completion status for a target language.
///
/// Compares the English locale source files against the existing translations
/// in `data/translations/{lang}/` and returns a percentage-complete report.
#[tauri::command]
pub fn get_translation_status(lang: String) -> Result<translation_pipeline::TranslationStatus> {
    let lang = validate_locale("lang", &lang)?;
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
pub async fn trigger_translation(lang: String) -> Result<String> {
    let lang = validate_locale("lang", &lang)?;
    let untranslated = translation_pipeline::get_untranslated_keys(&lang)?;
    if untranslated.is_empty() {
        return Ok(format!("{lang} is fully translated"));
    }

    let translated = translation_pipeline::translate_batch(&untranslated, &lang).await?;
    let count = translation_pipeline::save_translations(&translated, &lang)?;

    // Clear i18n cache so new translations take effect
    crate::i18n::clear_cache();

    Ok(format!("Translated {count} strings to {lang}"))
}

// ============================================================================
// Translation Override Commands
// ============================================================================

/// Get all translation entries for a language, merged with override status.
///
/// Returns a map of `"namespace:key"` to `{ english, translated, status }` where
/// status is one of: `"overridden"`, `"translated"`, `"untranslated"`.
#[tauri::command]
pub fn get_all_translations(lang: String) -> Result<HashMap<String, TranslationEntry>> {
    let lang = validate_locale("lang", &lang)?;
    let english = translation_pipeline::load_english_strings()?;
    let overrides = load_overrides(&lang)?;

    // Load auto-translated strings
    let trans_dir = crate::i18n::translations_dir().join(&lang);
    let mut auto_translated: HashMap<String, String> = HashMap::new();
    if trans_dir.exists() {
        for ns in &crate::i18n::TRANSLATION_NAMESPACES {
            let path = trans_dir.join(format!("{ns}.json"));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    for (k, v) in map {
                        auto_translated.insert(format!("{ns}:{k}"), v);
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
///
/// # Security
///
/// `lang` and `namespace` are joined into the destination path, so both are
/// allowlisted before they touch the filesystem. Without that, `namespace`
/// alone was an arbitrary-file-write primitive: `Path::join` with an absolute
/// component discards the prefix entirely, so traversal was not even needed,
/// and `create_dir_all` would build whatever directory chain was named. Both
/// the file's key and its value are attacker-chosen, which made the write
/// fully controlled content at a fully controlled location.
#[tauri::command]
pub fn save_translation_override(
    lang: String,
    namespace: String,
    key: String,
    value: String,
) -> Result<()> {
    let lang = validate_locale("lang", &lang)?;
    let namespace = validate_namespace("namespace", &namespace)?;
    let key = validate_translation_key("key", &key)?;
    let value = crate::ipc_guard::validate_length("value", &value, MAX_OVERRIDE_VALUE_LENGTH)?;
    crate::ipc_guard::validate_no_null_bytes("value", &value)?;

    let overrides_dir = crate::i18n::translations_dir()
        .join("overrides")
        .join(&lang);
    std::fs::create_dir_all(&overrides_dir).context("Cannot create overrides dir")?;

    let path = overrides_dir.join(format!("{namespace}.json"));

    let mut existing: HashMap<String, String> = if path.exists() {
        read_override_map(&path)?
    } else {
        HashMap::new()
    };

    existing.insert(key.clone(), value);

    let json = serde_json::to_string_pretty(&existing).context("JSON serialize error")?;
    std::fs::write(&path, json).context("Write error")?;

    // Clear cache so the override takes effect immediately
    crate::i18n::clear_cache();

    tracing::info!(target: "4da::i18n", lang = %lang, ns = %namespace, key = %key, "Translation override saved");
    Ok(())
}

/// Get all user overrides for a language.
///
/// Returns a flat map of `"namespace:key"` to override value.
#[tauri::command]
pub fn get_translation_overrides(lang: String) -> Result<HashMap<String, String>> {
    let lang = validate_locale("lang", &lang)?;
    load_overrides(&lang)
}

/// Delete a single user override.
///
/// # Security
///
/// Same path-injection surface as [`save_translation_override`], plus a
/// destructive twist: this function rewrites the target file with the parsed
/// map. When the parse was `unwrap_or_default()`, *any* readable file that was
/// not a JSON string-map parsed as an empty map and was then overwritten with
/// `{}` — turning an unvalidated path into an arbitrary-file-truncation
/// primitive. `read_override_map` now distinguishes "parsed to an empty map"
/// from "did not parse", and this function refuses to write in the latter case.
#[tauri::command]
pub fn delete_translation_override(lang: String, namespace: String, key: String) -> Result<()> {
    let lang = validate_locale("lang", &lang)?;
    let namespace = validate_namespace("namespace", &namespace)?;
    let key = validate_translation_key("key", &key)?;

    let overrides_dir = crate::i18n::translations_dir()
        .join("overrides")
        .join(&lang);
    let path = overrides_dir.join(format!("{namespace}.json"));

    if !path.exists() {
        return Ok(());
    }

    let mut map = read_override_map(&path)?;
    map.remove(&key);

    let json = serde_json::to_string_pretty(&map).context("JSON serialize error")?;
    std::fs::write(&path, json).context("Write error")?;

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

/// Read an override file into a map, refusing to guess when it does not parse.
///
/// Both callers write the returned map straight back to `path`, so the
/// distinction between "this file is an empty map" and "this file is not a map
/// at all" is the difference between a no-op and destroying its contents.
/// `unwrap_or_default()` collapses those two cases; this does not.
///
/// Whitespace-only content is treated as an empty map rather than an error:
/// there is nothing there to destroy, and failing would strand a user whose
/// override file was truncated by an earlier crash.
pub(crate) fn read_override_map(path: &std::path::Path) -> Result<HashMap<String, String>> {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_OVERRIDE_FILE_BYTES {
            return Err("Override file too large".into());
        }
    }

    let content = std::fs::read_to_string(path).context("Cannot read override file")?;
    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }

    serde_json::from_str(&content).map_err(|e| {
        tracing::warn!(
            target: "4da::i18n",
            path = %path.display(),
            error = %e,
            "Refusing to rewrite an override file that is not a JSON string map"
        );
        crate::error::FourDaError::Validation(format!(
            "{} is not a valid translation override file; \
             move or delete it before editing overrides for this language",
            path.display()
        ))
    })
}

/// Load all override files for a language into a single flat map.
///
/// Callers reaching this from IPC have already allowlisted `lang`; the
/// component check here is the second layer, so a future internal caller
/// cannot reintroduce the escape.
pub(crate) fn load_overrides(lang: &str) -> Result<HashMap<String, String>> {
    crate::ipc_guard::validate_path_component("lang", lang)?;
    let overrides_dir = crate::i18n::translations_dir().join("overrides").join(lang);
    let mut overrides: HashMap<String, String> = HashMap::new();

    if !overrides_dir.exists() {
        return Ok(overrides);
    }

    for ns in &crate::i18n::TRANSLATION_NAMESPACES {
        let path = overrides_dir.join(format!("{ns}.json"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                for (k, v) in map {
                    overrides.insert(format!("{ns}:{k}"), v);
                }
            }
        }
    }

    Ok(overrides)
}
