//! Autonomous Translation Pipeline
//!
//! Uses the existing LLM client to translate UI strings and STREETS content
//! from English into other languages. Translations are saved to
//! `data/translations/{lang}/` and loaded by the i18n module at runtime.

use crate::llm;
use std::collections::HashMap;
use tracing::{info, warn};

/// Maximum number of key-value pairs to send in a single LLM call.
/// Keeps prompts within token limits for all providers.
const BATCH_SIZE: usize = 50;

// ============================================================================
// Core Translation
// ============================================================================

/// Translate a batch of key-value strings from English to the target language.
///
/// Strings are sent in chunks of up to [`BATCH_SIZE`] to avoid token limits.
/// Returns a merged map of all translated key-value pairs.
pub async fn translate_batch(
    strings: &HashMap<String, String>,
    target_lang: &str,
) -> Result<HashMap<String, String>, String> {
    let client = get_llm_client()?;

    let target_name = lang_name(target_lang);
    let entries: Vec<(&String, &String)> = strings.iter().collect();
    let mut all_translated: HashMap<String, String> = HashMap::new();

    for chunk in entries.chunks(BATCH_SIZE) {
        let pairs: Vec<String> = chunk
            .iter()
            .map(|(k, v)| format!("  \"{}\": \"{}\"", k, escape_json_value(v)))
            .collect();
        let json_block = format!("{{\n{}\n}}", pairs.join(",\n"));

        let system = format!(
            "You are a professional translator. Translate the JSON values from English to {}. \
             Keep JSON keys exactly as-is. Preserve {{{{interpolation}}}} variables like {{{{count}}}}, {{{{name}}}}. \
             Return ONLY valid JSON, no markdown fences or explanation.",
            target_name
        );

        let response = client
            .complete(
                &system,
                vec![llm::Message {
                    role: "user".to_string(),
                    content: json_block,
                }],
            )
            .await
            .map_err(|e| format!("Translation failed: {}", e))?;

        // Strip markdown fences if the LLM wraps the response
        let clean = strip_markdown_fences(&response.content);

        let translated: HashMap<String, String> = serde_json::from_str(clean).map_err(|e| {
            let preview_len = response.content.len().min(200);
            format!(
                "Invalid JSON from LLM: {} -- response: {}",
                e,
                &response.content[..preview_len]
            )
        })?;

        // Validate: warn about missing keys
        for (k, _) in chunk {
            if !translated.contains_key(k.as_str()) {
                warn!(target: "4da::i18n", key = %k, lang = target_lang, "Missing key in translation");
            }
        }

        all_translated.extend(translated);
    }

    Ok(all_translated)
}

/// Translate markdown content preserving structure.
#[allow(dead_code)] // Public API for STREETS lesson translation
pub async fn translate_markdown(content: &str, target_lang: &str) -> Result<String, String> {
    let client = get_llm_client()?;

    let system = format!(
        "Translate the following Markdown from English to {}. \
         Preserve all Markdown formatting, headings (## Lesson N: ...), code blocks, and links. \
         Translate the heading text after 'Lesson N:' but keep the '## Lesson N:' prefix structure. \
         Return only the translated Markdown.",
        lang_name(target_lang)
    );

    let response = client
        .complete(
            &system,
            vec![llm::Message {
                role: "user".to_string(),
                content: content.to_string(),
            }],
        )
        .await
        .map_err(|e| format!("Markdown translation failed: {}", e))?;

    Ok(response.content)
}

// ============================================================================
// Key Comparison
// ============================================================================

/// Get untranslated keys for a target language by comparing against English source.
///
/// Returns a map of `"namespace:flat.key" -> "English value"` for every key
/// present in the English locale files but absent in the target translations.
pub fn get_untranslated_keys(target_lang: &str) -> Result<HashMap<String, String>, String> {
    let english_strings = load_english_strings()?;

    // Load existing translations for target language
    let trans_dir = crate::i18n::translations_dir().join(target_lang);
    let mut existing: HashMap<String, String> = HashMap::new();

    if trans_dir.exists() {
        for ns in &["ui", "coach", "streets", "errors"] {
            let path = trans_dir.join(format!("{}.json", ns));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    for (k, v) in map {
                        existing.insert(format!("{}:{}", ns, k), v);
                    }
                }
            }
        }
    }

    // Return keys that are in English but not in target
    let untranslated: HashMap<String, String> = english_strings
        .into_iter()
        .filter(|(k, _)| !existing.contains_key(k))
        .collect();

    Ok(untranslated)
}

/// Load all English source strings from the bundled locale files.
/// Returns `"namespace:flat.key" -> "English value"`.
pub fn load_english_strings() -> Result<HashMap<String, String>, String> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let locales_dir = manifest_dir
        .parent()
        .map(|p| p.join("src").join("locales").join("en"))
        .ok_or_else(|| "Cannot resolve locales directory".to_string())?;

    let mut english_strings: HashMap<String, String> = HashMap::new();

    for ns in &["ui", "coach", "streets", "errors"] {
        let path = locales_dir.join(format!("{}.json", ns));
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                for (k, v) in map {
                    english_strings.insert(format!("{}:{}", ns, k), v);
                }
            }
        }
    }

    Ok(english_strings)
}

// ============================================================================
// Persistence
// ============================================================================

/// Save translated strings to the `data/translations/{lang}/` directory.
///
/// Merges with any existing translations for the language. Returns the number
/// of new keys written.
pub fn save_translations(
    translations: &HashMap<String, String>,
    target_lang: &str,
) -> Result<usize, String> {
    let trans_dir = crate::i18n::translations_dir().join(target_lang);
    std::fs::create_dir_all(&trans_dir)
        .map_err(|e| format!("Cannot create translation dir: {}", e))?;

    // Group by namespace
    let mut by_ns: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (k, v) in translations {
        if let Some((ns, key)) = k.split_once(':') {
            by_ns
                .entry(ns.to_string())
                .or_default()
                .insert(key.to_string(), v.clone());
        }
    }

    let mut count = 0;
    for (ns, map) in &by_ns {
        let path = trans_dir.join(format!("{}.json", ns));

        // Merge with existing translations
        let mut existing: HashMap<String, String> = if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        for (k, v) in map {
            existing.insert(k.clone(), v.clone());
            count += 1;
        }

        let json = serde_json::to_string_pretty(&existing)
            .map_err(|e| format!("JSON serialize error: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;
    }

    info!(target: "4da::i18n", lang = target_lang, count, "Saved translations");
    Ok(count)
}

// ============================================================================
// Status
// ============================================================================

/// Translation status for a language.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationStatus {
    pub language: String,
    pub total_keys: usize,
    pub translated_keys: usize,
    pub percentage: f32,
}

// ============================================================================
// Helpers
// ============================================================================

/// Build an LLM client from the user's current settings.
fn get_llm_client() -> Result<llm::LLMClient, String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        return Err("LLM not configured -- set up your API key in Settings".to_string());
    }
    Ok(llm::LLMClient::new(provider))
}

/// Map language code to human-readable name for LLM prompts.
fn lang_name(code: &str) -> &str {
    match code {
        "ar" => "Arabic",
        "de" => "German",
        "es" => "Spanish",
        "fr" => "French",
        "hi" => "Hindi",
        "ja" => "Japanese",
        "ko" => "Korean",
        "nl" => "Dutch",
        "pl" => "Polish",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "zh" => "Simplified Chinese",
        _ => code,
    }
}

/// Escape double-quotes and backslashes inside a JSON value string.
fn escape_json_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Strip markdown code fences (```json ... ```) if present.
fn strip_markdown_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    trimmed
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_name() {
        assert_eq!(lang_name("es"), "Spanish");
        assert_eq!(lang_name("ja"), "Japanese");
        assert_eq!(lang_name("xx"), "xx");
    }

    #[test]
    fn test_escape_json_value() {
        assert_eq!(escape_json_value(r#"Hello "world""#), r#"Hello \"world\""#);
        assert_eq!(escape_json_value(r"back\slash"), r"back\\slash");
    }

    #[test]
    fn test_strip_markdown_fences() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_markdown_fences(input), "{\"key\": \"value\"}");

        let plain = "{\"key\": \"value\"}";
        assert_eq!(strip_markdown_fences(plain), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_load_english_strings() {
        let result = load_english_strings();
        assert!(result.is_ok());
        let strings = result.unwrap();
        // Should have entries from ui.json, coach.json, streets.json, errors.json
        assert!(strings.contains_key("ui:app.title"));
        assert!(strings.contains_key("streets:streets.title"));
        assert!(strings.contains_key("errors:error.db.unavailable"));
    }

    #[test]
    fn test_get_untranslated_keys_nonexistent_lang() {
        let result = get_untranslated_keys("zz_nonexistent");
        assert!(result.is_ok());
        let untranslated = result.unwrap();
        // All English keys should be untranslated for a nonexistent language
        let english = load_english_strings().unwrap();
        assert_eq!(untranslated.len(), english.len());
    }

    // ========================================================================
    // lang_name — nonexistent / unsupported language codes
    // ========================================================================

    #[test]
    fn test_lang_name_returns_code_for_unknown() {
        // Unsupported language codes should return the code itself
        assert_eq!(lang_name("zz"), "zz");
        assert_eq!(lang_name(""), "");
        assert_eq!(lang_name("elvish"), "elvish");
    }

    #[test]
    fn test_lang_name_covers_all_supported() {
        let supported = [
            ("ar", "Arabic"),
            ("de", "German"),
            ("es", "Spanish"),
            ("fr", "French"),
            ("hi", "Hindi"),
            ("ja", "Japanese"),
            ("ko", "Korean"),
            ("nl", "Dutch"),
            ("pl", "Polish"),
            ("pt", "Portuguese"),
            ("ru", "Russian"),
            ("zh", "Simplified Chinese"),
        ];
        for (code, name) in supported {
            assert_eq!(
                lang_name(code),
                name,
                "lang_name({}) should be {}",
                code,
                name
            );
        }
    }

    // ========================================================================
    // escape_json_value — edge cases
    // ========================================================================

    #[test]
    fn test_escape_json_value_empty_string() {
        assert_eq!(escape_json_value(""), "");
    }

    #[test]
    fn test_escape_json_value_no_special_chars() {
        assert_eq!(escape_json_value("Hello world"), "Hello world");
    }

    #[test]
    fn test_escape_json_value_multiple_special_chars() {
        assert_eq!(
            escape_json_value(r#"She said "hello\" and left"#),
            r#"She said \"hello\\\" and left"#
        );
    }

    // ========================================================================
    // strip_markdown_fences — edge cases
    // ========================================================================

    #[test]
    fn test_strip_markdown_fences_generic_fence() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_markdown_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_strip_markdown_fences_no_fences() {
        let input = "{\"key\": \"value\"}";
        assert_eq!(strip_markdown_fences(input), input);
    }

    #[test]
    fn test_strip_markdown_fences_unmatched_opening() {
        // Only opening fence, no closing — should return trimmed input
        let input = "```json\n{\"key\": \"value\"}";
        // No closing ```, so strip_prefix succeeds but strip_suffix fails -> returns trimmed original
        assert_eq!(strip_markdown_fences(input), input);
    }

    #[test]
    fn test_strip_markdown_fences_whitespace_handling() {
        let input = "  ```json\n{\"a\": 1}\n```  ";
        assert_eq!(strip_markdown_fences(input), "{\"a\": 1}");
    }

    // ========================================================================
    // save_translations — round-trip through filesystem
    // ========================================================================

    #[test]
    fn test_save_translations_and_reload() {
        // Use a unique test language to avoid interfering with real data
        let test_lang = "zz_test_save_roundtrip";
        let trans_dir = crate::i18n::translations_dir().join(test_lang);

        // Clean up from any previous test run
        let _ = std::fs::remove_dir_all(&trans_dir);

        let mut translations = HashMap::new();
        translations.insert("ui:test.key1".to_string(), "Translated One".to_string());
        translations.insert("ui:test.key2".to_string(), "Translated Two".to_string());
        translations.insert(
            "errors:err.test".to_string(),
            "Error Translation".to_string(),
        );

        let count = save_translations(&translations, test_lang).expect("save should succeed");
        assert_eq!(count, 3);

        // Verify files exist
        let ui_path = trans_dir.join("ui.json");
        assert!(ui_path.exists(), "ui.json should have been created");

        let errors_path = trans_dir.join("errors.json");
        assert!(errors_path.exists(), "errors.json should have been created");

        // Verify content
        let ui_content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&ui_path).unwrap()).unwrap();
        assert_eq!(
            ui_content.get("test.key1"),
            Some(&"Translated One".to_string())
        );
        assert_eq!(
            ui_content.get("test.key2"),
            Some(&"Translated Two".to_string())
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&trans_dir);
    }

    #[test]
    fn test_save_translations_merges_with_existing() {
        let test_lang = "zz_test_merge";
        let trans_dir = crate::i18n::translations_dir().join(test_lang);
        let _ = std::fs::remove_dir_all(&trans_dir);
        std::fs::create_dir_all(&trans_dir).expect("create dir");

        // Write initial translation
        let mut initial: HashMap<String, String> = HashMap::new();
        initial.insert("existing.key".to_string(), "Existing Value".to_string());
        std::fs::write(
            trans_dir.join("ui.json"),
            serde_json::to_string_pretty(&initial).unwrap(),
        )
        .expect("write initial");

        // Save new translations
        let mut new_translations = HashMap::new();
        new_translations.insert("ui:new.key".to_string(), "New Value".to_string());
        save_translations(&new_translations, test_lang).expect("save should succeed");

        // Verify merge: both old and new keys should exist
        let merged: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(trans_dir.join("ui.json")).unwrap())
                .unwrap();
        assert_eq!(
            merged.get("existing.key"),
            Some(&"Existing Value".to_string())
        );
        assert_eq!(merged.get("new.key"), Some(&"New Value".to_string()));

        let _ = std::fs::remove_dir_all(&trans_dir);
    }

    #[test]
    fn test_save_translations_ignores_keys_without_namespace() {
        let test_lang = "zz_test_no_ns";
        let trans_dir = crate::i18n::translations_dir().join(test_lang);
        let _ = std::fs::remove_dir_all(&trans_dir);

        let mut translations = HashMap::new();
        translations.insert("no_namespace_key".to_string(), "Value".to_string());

        let count = save_translations(&translations, test_lang).expect("save should succeed");
        assert_eq!(
            count, 0,
            "Keys without namespace:key format should be skipped"
        );

        let _ = std::fs::remove_dir_all(&trans_dir);
    }

    // ========================================================================
    // TranslationStatus struct
    // ========================================================================

    #[test]
    fn test_translation_status_zero_total() {
        let status = TranslationStatus {
            language: "zz".to_string(),
            total_keys: 0,
            translated_keys: 0,
            percentage: 0.0,
        };
        assert_eq!(status.total_keys, 0);
        assert_eq!(status.percentage, 0.0);
    }
}
