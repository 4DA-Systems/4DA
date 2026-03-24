//! Backend i18n -- simple key-based translation for Rust-generated messages.
//!
//! Loads JSON translation files from `data/translations/{lang}/` at runtime.
//! Falls back to English if a key is missing in the target language.

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// In-memory translation cache: lang -> namespace -> key -> value
static TRANSLATIONS: Lazy<RwLock<HashMap<String, HashMap<String, Value>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Get the translations directory path.
pub(crate) fn translations_dir() -> PathBuf {
    // Development: data/translations relative to project root
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest_dir.parent() {
        let dev_path = root.join("data").join("translations");
        if dev_path.exists() {
            return dev_path;
        }
    }

    // Production: relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let rel = exe_dir.join("data").join("translations");
            if rel.exists() {
                return rel;
            }
        }
    }

    PathBuf::from("data/translations")
}

/// Load translations for a language (if not already cached).
#[allow(dead_code)] // Reason: i18n system built but not yet called from Rust backend
fn ensure_loaded(lang: &str) {
    {
        let cache = TRANSLATIONS.read();
        if cache.contains_key(lang) {
            return;
        }
    }

    let dir = translations_dir().join(lang);
    if !dir.exists() {
        debug!(target: "4da::i18n", lang, "No translations directory found");
        return;
    }

    let mut namespaces = HashMap::new();

    // Load all JSON files in the language directory
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str::<Value>(&content) {
                        let ns = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("ui")
                            .to_string();
                        namespaces.insert(ns, value);
                    }
                }
            }
        }
    }

    if !namespaces.is_empty() {
        debug!(target: "4da::i18n", lang, namespaces = namespaces.len(), "Loaded translations");
        let mut cache = TRANSLATIONS.write();
        cache.insert(lang.to_string(), namespaces);
    }
}

/// Translate a key for the given language.
///
/// Key format: `"namespace:dotted.key"` or just `"dotted.key"` (defaults to "ui" namespace).
/// Falls back to English if the key is not found in the target language.
/// Returns the key itself if no translation exists at all.
///
/// ## Variables
/// Pass a slice of `(name, value)` pairs for interpolation.
/// Uses `{{name}}` placeholder syntax matching i18next frontend.
#[allow(dead_code)] // Reason: i18n system built but not yet called from Rust backend
pub fn t(key: &str, lang: &str, vars: &[(&str, &str)]) -> String {
    // Parse namespace from key
    let (namespace, lookup_key) = if let Some(colon_pos) = key.find(':') {
        (&key[..colon_pos], &key[colon_pos + 1..])
    } else {
        ("ui", key)
    };

    // Try target language first, then English fallback
    for try_lang in &[lang, "en"] {
        ensure_loaded(try_lang);
        let cache = TRANSLATIONS.read();
        if let Some(namespaces) = cache.get(*try_lang) {
            if let Some(ns_data) = namespaces.get(namespace) {
                if let Some(value) = lookup_nested(ns_data, lookup_key) {
                    if let Some(text) = value.as_str() {
                        let mut result = text.to_string();
                        for (name, val) in vars {
                            result = result.replace(&format!("{{{{{name}}}}}"), val);
                        }
                        return result;
                    }
                }
            }
        }
    }

    // No translation found -- return the key
    key.to_string()
}

/// Look up a dotted key path in a JSON value.
#[allow(dead_code)] // Reason: helper for t(), which is not yet called from Rust backend
fn lookup_nested<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}

/// Get the current user language from settings.
pub fn get_user_language() -> String {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let lang = &guard.get().locale.language;
    if lang.is_empty() {
        "en".to_string()
    } else {
        lang.clone()
    }
}

/// Clear the translation cache (used when new translations are generated).
pub fn clear_cache() {
    let mut cache = TRANSLATIONS.write();
    cache.clear();
    debug!(target: "4da::i18n", "Translation cache cleared");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_nested() {
        let value = serde_json::json!({
            "app": {
                "title": "4DA",
                "tagline": "All signal. No feed."
            }
        });
        assert_eq!(
            lookup_nested(&value, "app.title").and_then(|v| v.as_str()),
            Some("4DA")
        );
        assert_eq!(
            lookup_nested(&value, "app.tagline").and_then(|v| v.as_str()),
            Some("All signal. No feed.")
        );
        assert!(lookup_nested(&value, "app.missing").is_none());
    }

    #[test]
    fn test_t_returns_key_when_no_translation() {
        // With no translation files loaded, t() should return the key itself
        let result = t("some.missing.key", "xx", &[]);
        assert_eq!(result, "some.missing.key");
    }
}
