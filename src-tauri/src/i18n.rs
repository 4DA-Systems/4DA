// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Backend i18n -- simple key-based translation for Rust-generated messages.
//!
//! Loads JSON translation files from `data/translations/{lang}/` at runtime.
//! Falls back to English if a key is missing in the target language.

use crate::error::{FourDaError, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// In-memory translation cache: lang -> namespace -> key -> value
static TRANSLATIONS: Lazy<RwLock<HashMap<String, HashMap<String, Value>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// ============================================================================
// Allowlists — the trust boundary for every locale/namespace that reaches a path
// ============================================================================

/// Locale codes 4DA ships translation files for — one entry per directory
/// under `src/locales/`.
///
/// This is the allowlist every IPC command taking a language code validates
/// against. It is deliberately a compile-time constant rather than a directory
/// listing: an allowlist that widens when someone creates a directory is not an
/// allowlist. `locale_list_matches_shipped_files` keeps it honest by asserting
/// it still matches `src/locales/` exactly.
///
/// `ar` ships files but is not offered in the language picker (RTL layout
/// untested — see `SUPPORTED_LANGUAGES` in `src/i18n/index.ts`). It stays here
/// because the files exist and `t()` resolves them; the picker, not this
/// constant, gates user-visible activation.
pub const SUPPORTED_LOCALES: [&str; 13] = [
    "ar", "de", "en", "es", "fr", "hi", "it", "ja", "ko", "pt-BR", "ru", "tr", "zh",
];

/// Translation namespaces — one JSON file per namespace per locale.
///
/// Single source of truth: the English loader, the untranslated-key diff, and
/// the override loader all read exactly these, so a namespace cannot be
/// writable-but-never-read.
pub const TRANSLATION_NAMESPACES: [&str; 3] = ["ui", "errors", "signals"];

/// Maximum length of a translation key. Keys are dotted identifiers
/// (`app.title`, `error.db.unavailable`), never prose.
pub const MAX_TRANSLATION_KEY_LENGTH: usize = 256;

/// Validate a language code against [`SUPPORTED_LOCALES`].
///
/// Used at every IPC boundary that accepts a language code, because those codes
/// are joined into filesystem paths. An allowlist beats a sanitizer here: the
/// valid set is small, closed, and known at compile time.
pub(crate) fn validate_locale(field: &str, lang: &str) -> Result<String> {
    if SUPPORTED_LOCALES.contains(&lang) {
        return Ok(lang.to_string());
    }
    tracing::warn!(
        target: "4da::security",
        field,
        "Rejected unsupported locale code"
    );
    Err(FourDaError::Validation(format!(
        "{field} is not a supported language code"
    )))
}

/// Validate a translation namespace against [`TRANSLATION_NAMESPACES`].
pub(crate) fn validate_namespace(field: &str, namespace: &str) -> Result<String> {
    if TRANSLATION_NAMESPACES.contains(&namespace) {
        return Ok(namespace.to_string());
    }
    tracing::warn!(
        target: "4da::security",
        field,
        "Rejected unknown translation namespace"
    );
    Err(FourDaError::Validation(format!(
        "{field} is not a known translation namespace"
    )))
}

/// Validate a translation key. The key becomes a JSON object key, not a path
/// segment, so the character rules are looser than
/// `ipc_guard::validate_path_component` — but it is still attacker-controlled
/// text that gets persisted and rendered, so it is capped and stripped of
/// control characters (which subsumes NUL).
pub(crate) fn validate_translation_key(field: &str, key: &str) -> Result<String> {
    if key.is_empty() {
        return Err(FourDaError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    if key.len() > MAX_TRANSLATION_KEY_LENGTH {
        return Err(FourDaError::Validation(format!(
            "{field} exceeds maximum length of {MAX_TRANSLATION_KEY_LENGTH} characters"
        )));
    }
    if key.chars().any(char::is_control) {
        tracing::warn!(
            target: "4da::security",
            field,
            "Rejected control character in translation key"
        );
        return Err(FourDaError::Validation(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(key.to_string())
}

/// Get the translations directory path.
pub(crate) fn translations_dir() -> PathBuf {
    let paths = crate::runtime_paths::RuntimePaths::get();

    // Primary: data/translations inside the data directory
    let data_path = paths.data_dir.join("translations");
    if data_path.exists() {
        return data_path;
    }

    // Fallback: resource_dir/data/translations (production bundle)
    let resource_path = paths.resource_dir.join("data").join("translations");
    if resource_path.exists() {
        return resource_path;
    }

    PathBuf::from("data/translations")
}

/// Load translations for a language (if not already cached).
///
/// Searches `data/translations/{lang}/` first (runtime translations),
/// then falls back to `src/locales/{lang}/` (bundled frontend translations)
/// so the Rust `t()` function works in development without pre-generating
/// translation files.
fn ensure_loaded(lang: &str) {
    {
        let cache = TRANSLATIONS.read();
        if cache.contains_key(lang) {
            return;
        }
    }

    // `lang` is joined into three separate directory paths below and every
    // `*.json` found there is parsed and cached for display. It reaches here
    // from user settings, which the frontend can write, so treat it as
    // untrusted: an unsafe component would turn `t()` into a read primitive
    // for arbitrary directories. Not an allowlist check — `t()` is called with
    // fallback and test codes that are safe but unshipped — just a guarantee
    // that the value cannot escape the directory it is joined onto.
    if crate::ipc_guard::validate_path_component("lang", lang).is_err() {
        debug!(target: "4da::i18n", "Refusing to load translations for an unsafe language code");
        return;
    }

    let dir = translations_dir().join(lang);

    // Fallback: frontend locale directory (for development)
    let fallback_dir = {
        let locales_base = crate::runtime_paths::RuntimePaths::get().locales_dir();
        let candidate = locales_base.join(lang);
        if candidate.exists() {
            Some(candidate)
        } else {
            // Also check src/locales (legacy dev layout)
            let src_locales = crate::runtime_paths::RuntimePaths::get()
                .resource_dir
                .join("src")
                .join("locales")
                .join(lang);
            if src_locales.exists() {
                Some(src_locales)
            } else {
                None
            }
        }
    };

    let search_dirs: Vec<&std::path::Path> = [Some(dir.as_path()), fallback_dir.as_deref()]
        .into_iter()
        .flatten()
        .filter(|d| d.exists())
        .collect();

    if search_dirs.is_empty() {
        debug!(target: "4da::i18n", lang, "No translations directory found");
        return;
    }

    let mut namespaces = HashMap::new();

    for search_dir in search_dirs {
        if let Ok(entries) = std::fs::read_dir(search_dir) {
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
                            // Don't overwrite if already loaded from primary dir
                            namespaces.entry(ns).or_insert(value);
                        }
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

/// Look up a key in a JSON value.
///
/// Tries the key as a flat string first (e.g. `"type.securityAlert"`),
/// then falls back to nested traversal (e.g. `{"type": {"securityAlert": ...}}`).
fn lookup_nested<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    // Try flat key first (how i18next stores keys)
    if let Some(v) = value.get(key) {
        return Some(v);
    }
    // Fall back to nested dot-traversal
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() <= 1 {
        return None;
    }
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

    #[test]
    fn test_en_translations_load() {
        // Verify English locale files load and flat dotted keys resolve correctly
        assert_eq!(t("signals:type.securityAlert", "en", &[]), "Security Alert");
        assert_eq!(t("ui:app.title", "en", &[]), "4DA");
    }

    #[test]
    fn test_variable_interpolation() {
        let result = t(
            "signals:action.securityReview",
            "en",
            &[("title", "CVE-2026")],
        );
        assert_eq!(result, "Review security implications: CVE-2026");
    }
}
