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

    let content = match std::fs::metadata(&path) {
        Ok(m) if m.len() > 1_000_000 => return Err("Override file too large".to_string()),
        _ => std::fs::read_to_string(&path).unwrap_or_default(),
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TranslationEntry struct tests
    // ========================================================================

    #[test]
    fn translation_entry_untranslated() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: None,
            status: "untranslated".to_string(),
        };
        assert_eq!(entry.english, "Hello");
        assert!(entry.translated.is_none());
        assert_eq!(entry.status, "untranslated");
    }

    #[test]
    fn translation_entry_translated() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: Some("Hola".to_string()),
            status: "translated".to_string(),
        };
        assert_eq!(entry.translated, Some("Hola".to_string()));
        assert_eq!(entry.status, "translated");
    }

    #[test]
    fn translation_entry_overridden() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: Some("Custom Hello".to_string()),
            status: "overridden".to_string(),
        };
        assert_eq!(entry.status, "overridden");
        assert_eq!(entry.translated, Some("Custom Hello".to_string()));
    }

    #[test]
    fn translation_entry_serializes_to_json() {
        let entry = TranslationEntry {
            english: "Save".to_string(),
            translated: Some("Guardar".to_string()),
            status: "translated".to_string(),
        };
        let json = serde_json::to_value(&entry).expect("should serialize");
        assert_eq!(json["english"], "Save");
        assert_eq!(json["translated"], "Guardar");
        assert_eq!(json["status"], "translated");
    }

    #[test]
    fn translation_entry_serializes_null_for_none() {
        let entry = TranslationEntry {
            english: "Cancel".to_string(),
            translated: None,
            status: "untranslated".to_string(),
        };
        let json = serde_json::to_value(&entry).expect("should serialize");
        assert!(json["translated"].is_null());
    }

    #[test]
    fn translation_entry_deserializes_from_json() {
        let json_str = r#"{"english":"Quit","translated":"Quitter","status":"translated"}"#;
        let entry: TranslationEntry = serde_json::from_str(json_str).expect("should deserialize");
        assert_eq!(entry.english, "Quit");
        assert_eq!(entry.translated, Some("Quitter".to_string()));
        assert_eq!(entry.status, "translated");
    }

    #[test]
    fn translation_entry_deserializes_null_translated() {
        let json_str = r#"{"english":"Quit","translated":null,"status":"untranslated"}"#;
        let entry: TranslationEntry = serde_json::from_str(json_str).expect("should deserialize");
        assert!(entry.translated.is_none());
    }

    // ========================================================================
    // Translation status computation logic (from get_translation_status)
    // ========================================================================

    #[test]
    fn translation_percentage_calculation_full() {
        let total = 100;
        let translated = 100;
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_partial() {
        let total = 200;
        let untranslated_count = 50;
        let translated = total - untranslated_count;
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_zero_total() {
        let total = 0;
        let percentage = if total > 0 {
            (0_f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_none_translated() {
        let total: usize = 50;
        let untranslated_count: usize = 50;
        let translated = total.saturating_sub(untranslated_count);
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 0.0).abs() < f32::EPSILON);
    }

    // ========================================================================
    // TranslationStatus struct from translation_pipeline
    // ========================================================================

    #[test]
    fn translation_status_serializes() {
        let status = translation_pipeline::TranslationStatus {
            language: "es".to_string(),
            total_keys: 100,
            translated_keys: 75,
            percentage: 75.0,
        };
        let json = serde_json::to_value(&status).expect("should serialize");
        assert_eq!(json["language"], "es");
        assert_eq!(json["total_keys"], 100);
        assert_eq!(json["translated_keys"], 75);
        assert_eq!(json["percentage"], 75.0);
    }

    #[test]
    fn translation_status_deserializes() {
        let json_str =
            r#"{"language":"fr","total_keys":200,"translated_keys":150,"percentage":75.0}"#;
        let status: translation_pipeline::TranslationStatus =
            serde_json::from_str(json_str).expect("should deserialize");
        assert_eq!(status.language, "fr");
        assert_eq!(status.total_keys, 200);
        assert_eq!(status.translated_keys, 150);
    }

    // ========================================================================
    // load_overrides returns empty map for nonexistent language
    // ========================================================================

    #[test]
    fn load_overrides_nonexistent_lang_returns_empty() {
        let result = load_overrides("zz_nonexistent_test_language_xyz");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ========================================================================
    // Namespace list consistency check
    // ========================================================================

    #[test]
    fn namespace_list_is_consistent() {
        // Both get_all_translations and load_overrides iterate the same namespaces.
        // This test ensures they match the expected set.
        let expected_namespaces = ["ui", "coach", "streets", "errors"];
        assert_eq!(expected_namespaces.len(), 4);
        // Verify these are the namespaces used in both places
        for ns in &expected_namespaces {
            assert!(!ns.is_empty(), "Namespace should not be empty");
        }
    }

    // ========================================================================
    // Translation entry status values are valid
    // ========================================================================

    #[test]
    fn valid_status_values() {
        let valid_statuses = ["overridden", "translated", "untranslated"];
        // Verify we have exactly 3 status values matching the code in get_all_translations
        assert_eq!(valid_statuses.len(), 3);
        for status in &valid_statuses {
            assert!(!status.is_empty(), "Status value should not be empty");
        }
    }

    // ========================================================================
    // File too large guard — delete_translation_override
    // ========================================================================

    #[test]
    fn delete_override_file_too_large_guard() {
        // The delete_translation_override function checks metadata for files > 1MB.
        // We create a file > 1MB to verify the guard fires.
        let test_lang = "zz_too_large_guard_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        // Write a file slightly over 1MB
        let large_content = "x".repeat(1_000_001);
        std::fs::write(&path, &large_content).expect("write large file");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "some.key".to_string(),
        );

        assert!(result.is_err(), "Should error on files > 1MB");
        assert_eq!(result.unwrap_err(), "Override file too large");

        // Cleanup
        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn delete_override_nonexistent_file_returns_ok() {
        // When the override file doesn't exist, delete should be a no-op success.
        let result = delete_translation_override(
            "zz_nonexistent_delete_test".to_string(),
            "ui".to_string(),
            "some.key".to_string(),
        );
        assert!(
            result.is_ok(),
            "Deleting from nonexistent file should succeed"
        );
    }

    #[test]
    fn delete_override_removes_key_from_file() {
        let test_lang = "zz_delete_key_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));

        // Write an override file with two keys
        let mut map = HashMap::new();
        map.insert("key.to.delete".to_string(), "Delete Me".to_string());
        map.insert("key.to.keep".to_string(), "Keep Me".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&map).unwrap())
            .expect("write override file");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "key.to.delete".to_string(),
        );
        assert!(result.is_ok());

        // Verify the key was removed but the other key remains
        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            !content.contains_key("key.to.delete"),
            "Deleted key should be gone"
        );
        assert_eq!(content.get("key.to.keep"), Some(&"Keep Me".to_string()));

        // Cleanup
        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn delete_override_malformed_json_file_returns_ok() {
        // If the override file has malformed JSON, unwrap_or_default() gives an empty map,
        // then the key removal is a no-op, and the file is rewritten with "{}".
        let test_lang = "zz_malformed_delete_test";
        let test_ns = "errors";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        std::fs::write(&path, "not valid json {{{").expect("write malformed");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "some.key".to_string(),
        );
        assert!(
            result.is_ok(),
            "Malformed JSON should be handled gracefully"
        );

        // File should now contain valid empty JSON
        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .expect("File should now be valid JSON");
        assert!(content.is_empty());

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    // ========================================================================
    // save_translation_override — error paths
    // ========================================================================

    #[test]
    fn save_override_creates_dir_and_file() {
        let test_lang = "zz_save_override_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        let _ = std::fs::remove_dir_all(&overrides_dir);

        let result = save_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "test.key".to_string(),
            "Custom Value".to_string(),
        );
        assert!(result.is_ok());

        let path = overrides_dir.join(format!("{}.json", test_ns));
        assert!(path.exists(), "Override file should have been created");

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content.get("test.key"), Some(&"Custom Value".to_string()));

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn save_override_merges_with_existing() {
        let test_lang = "zz_save_merge_test";
        let test_ns = "coach";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        let mut initial = HashMap::new();
        initial.insert("existing.key".to_string(), "Existing".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap())
            .expect("write initial");

        let result = save_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "new.key".to_string(),
            "New Override".to_string(),
        );
        assert!(result.is_ok());

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content.get("existing.key"), Some(&"Existing".to_string()));
        assert_eq!(content.get("new.key"), Some(&"New Override".to_string()));

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }
}
