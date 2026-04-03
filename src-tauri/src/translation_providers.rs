//! Dedicated translation API providers (DeepL, Google Cloud, Azure).
//!
//! These are cheaper, faster alternatives to using the main LLM for
//! content translation. Each provider function accepts (id, text) pairs
//! and returns a map of id -> translated text.

use crate::error::{Result, ResultExt};
use std::collections::HashMap;
use tracing::warn;

// ============================================================================
// DeepL Translation Provider
// ============================================================================

/// Translate via DeepL API (dedicated translation, not LLM).
/// Uses DeepL Free API (api-free.deepl.com) or Pro API (api.deepl.com) based on key format.
pub async fn translate_via_deepl(
    items: &[(&str, &str)], // (id, text) pairs
    target_lang: &str,
    api_key: &str,
) -> Result<HashMap<String, String>> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }

    let base_url = if api_key.ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    };

    // DeepL uses uppercase 2-letter codes, with some exceptions
    let deepl_lang = map_deepl_lang(target_lang);

    let client = reqwest::Client::new();
    let mut form: Vec<(&str, String)> = vec![("target_lang", deepl_lang)];
    for (_id, text) in items {
        form.push(("text", (*text).to_string()));
    }

    let response = client
        .post(base_url)
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .form(&form)
        .send()
        .await
        .context("DeepL API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("DeepL API error {status}: {body}").into());
    }

    let data: serde_json::Value = response.json().await.context("DeepL response parse failed")?;
    let mut results = HashMap::new();

    if let Some(translations) = data.get("translations").and_then(|t| t.as_array()) {
        for (i, translation) in translations.iter().enumerate() {
            if i < items.len() {
                if let Some(text) = translation.get("text").and_then(|t| t.as_str()) {
                    results.insert(items[i].0.to_string(), text.to_string());
                }
            }
        }
    }

    Ok(results)
}

/// Map a BCP 47 language tag to DeepL's expected format.
pub(crate) fn map_deepl_lang(lang: &str) -> String {
    match lang {
        "pt-BR" => "PT-BR".to_string(),
        "zh" => "ZH-HANS".to_string(),
        "zh-TW" => "ZH-HANT".to_string(),
        other => other.to_uppercase(),
    }
}

// ============================================================================
// Azure Translator Provider
// ============================================================================

/// Translate via Azure Cognitive Services Translator.
pub async fn translate_via_azure(
    items: &[(&str, &str)],
    target_lang: &str,
    api_key: &str,
) -> Result<HashMap<String, String>> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }

    let url = format!(
        "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={target_lang}"
    );

    let body: Vec<serde_json::Value> = items
        .iter()
        .map(|(_, text)| serde_json::json!({ "Text": text }))
        .collect();

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Azure Translator API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Azure Translator error {status}: {body_text}").into());
    }

    let data: Vec<serde_json::Value> =
        response.json().await.context("Azure response parse failed")?;
    let mut results = HashMap::new();

    for (i, item) in data.iter().enumerate() {
        if i < items.len() {
            if let Some(translations) = item.get("translations").and_then(|t| t.as_array()) {
                if let Some(first) = translations.first() {
                    if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                        results.insert(items[i].0.to_string(), text.to_string());
                    }
                }
            }
        }
    }

    Ok(results)
}

// ============================================================================
// Google Cloud Translation Provider
// ============================================================================

/// Translate via Google Cloud Translation API v2 (Basic).
pub async fn translate_via_google(
    items: &[(&str, &str)],
    target_lang: &str,
    api_key: &str,
) -> Result<HashMap<String, String>> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }

    let url = format!(
        "https://translation.googleapis.com/language/translate/v2?key={api_key}"
    );

    let texts: Vec<&str> = items.iter().map(|(_, text)| *text).collect();

    let body = serde_json::json!({
        "q": texts,
        "target": target_lang,
        "format": "text"
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Google Translate API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Google Translate error {status}: {body_text}").into());
    }

    let data: serde_json::Value =
        response.json().await.context("Google response parse failed")?;
    let mut results = HashMap::new();

    if let Some(translations) = data
        .get("data")
        .and_then(|d| d.get("translations"))
        .and_then(|t| t.as_array())
    {
        for (i, translation) in translations.iter().enumerate() {
            if i < items.len() {
                if let Some(text) = translation.get("translatedText").and_then(|t| t.as_str()) {
                    results.insert(items[i].0.to_string(), text.to_string());
                }
            }
        }
    }

    Ok(results)
}

// ============================================================================
// Provider Routing
// ============================================================================

/// Routing result from `try_dedicated_provider`.
pub enum ProviderResult {
    /// Dedicated provider returned translations successfully.
    Ok(HashMap<String, String>),
    /// Dedicated provider failed — caller should fall back to LLM.
    Fallback,
    /// No dedicated provider configured — caller should use LLM.
    UseLlm,
}

/// Try translating via a dedicated provider based on user settings.
///
/// Returns `ProviderResult::Ok(translations)` if a dedicated API succeeded,
/// `ProviderResult::Fallback` if a dedicated API was configured but failed
/// (only in "auto" mode — explicit provider errors are propagated),
/// or `ProviderResult::UseLlm` if no dedicated provider is configured.
pub async fn try_dedicated_provider(
    items: &[(&str, &str)],
    target_lang: &str,
) -> Result<ProviderResult> {
    let manager = crate::get_settings_manager();
    let (provider, api_key) = {
        let guard = manager.lock();
        let tc = &guard.get().translation;
        (tc.provider.clone(), tc.api_key.clone())
    };

    match provider.as_str() {
        "deepl" if !api_key.is_empty() => {
            let result = translate_via_deepl(items, target_lang, &api_key).await?;
            Ok(ProviderResult::Ok(result))
        }
        "azure" if !api_key.is_empty() => {
            let result = translate_via_azure(items, target_lang, &api_key).await?;
            Ok(ProviderResult::Ok(result))
        }
        "google" if !api_key.is_empty() => {
            let result = translate_via_google(items, target_lang, &api_key).await?;
            Ok(ProviderResult::Ok(result))
        }
        "auto" if !api_key.is_empty() => {
            // Try dedicated API first (assume DeepL for auto mode), fall back to LLM
            match translate_via_deepl(items, target_lang, &api_key).await {
                Ok(results) => Ok(ProviderResult::Ok(results)),
                Err(e) => {
                    warn!(
                        target: "4da::i18n",
                        error = %e,
                        "Dedicated translation API failed, falling back to LLM"
                    );
                    Ok(ProviderResult::Fallback)
                }
            }
        }
        _ => {
            // "ollama", "llm", empty API key, or any other value — use LLM path
            Ok(ProviderResult::UseLlm)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepl_lang_mapping_standard() {
        // Standard 2-letter codes get uppercased
        assert_eq!(map_deepl_lang("de"), "DE");
        assert_eq!(map_deepl_lang("fr"), "FR");
        assert_eq!(map_deepl_lang("ja"), "JA");
    }

    #[test]
    fn test_deepl_lang_mapping_special_cases() {
        // Special cases that DeepL requires specific codes for
        assert_eq!(map_deepl_lang("pt-BR"), "PT-BR");
        assert_eq!(map_deepl_lang("zh"), "ZH-HANS");
        assert_eq!(map_deepl_lang("zh-TW"), "ZH-HANT");
    }

    #[test]
    fn test_deepl_url_selection_free_key() {
        // Free API keys end with ":fx"
        let key = "abc123:fx";
        assert!(key.ends_with(":fx"));
    }

    #[test]
    fn test_deepl_url_selection_pro_key() {
        // Pro API keys do NOT end with ":fx"
        let key = "abc123-pro-key";
        assert!(!key.ends_with(":fx"));
    }

    #[test]
    fn test_google_request_body_format() {
        // Verify the JSON structure matches Google's expected format
        let items = vec![("1", "Hello"), ("2", "World")];
        let texts: Vec<&str> = items.iter().map(|(_, text)| *text).collect();
        let body = serde_json::json!({
            "q": texts,
            "target": "de",
            "format": "text"
        });

        assert_eq!(
            body.get("q").and_then(|q| q.as_array()).map(|a| a.len()),
            Some(2)
        );
        assert_eq!(
            body.get("target").and_then(|t| t.as_str()),
            Some("de")
        );
        assert_eq!(
            body.get("format").and_then(|f| f.as_str()),
            Some("text")
        );
    }
}
