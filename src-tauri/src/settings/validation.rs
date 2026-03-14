//! Real-time API key validation.
//!
//! Provides instant format checks (no network) and lightweight API
//! connection tests. Keys are validated server-side and never returned
//! to the frontend.

use crate::error::Result;
use serde::Serialize;
use tracing::{info, warn};

/// Result of validating an API key.
#[derive(Debug, Clone, Serialize)]
pub struct KeyValidationResult {
    /// Overall validity.
    pub valid: bool,
    /// Key format matches expected pattern.
    pub format_ok: bool,
    /// API connection test succeeded.
    pub connection_ok: bool,
    /// Error message if validation failed.
    pub error: Option<String>,
    /// Models accessible with this key (populated on success).
    pub model_access: Vec<String>,
}

/// Key format validation result.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatResult {
    Valid,
    InvalidFormat(String),
    PossiblyValid,
}

/// Validate key format without network access (instant).
pub fn validate_key_format(provider: &str, key: &str) -> FormatResult {
    if key.is_empty() {
        return FormatResult::InvalidFormat("Key is empty".to_string());
    }

    match provider {
        "anthropic" => {
            if !key.starts_with("sk-ant-") {
                return FormatResult::InvalidFormat(
                    "Anthropic keys start with sk-ant-".to_string(),
                );
            }
            if key.len() < 90 || key.len() > 200 {
                return FormatResult::InvalidFormat(format!(
                    "Expected 90-200 characters, got {}",
                    key.len()
                ));
            }
            FormatResult::Valid
        }
        "openai" => {
            if !key.starts_with("sk-") {
                return FormatResult::InvalidFormat("OpenAI keys start with sk-".to_string());
            }
            if key.len() < 30 || key.len() > 300 {
                return FormatResult::InvalidFormat(format!(
                    "Expected 30-300 characters, got {}",
                    key.len()
                ));
            }
            FormatResult::Valid
        }
        "openai-compatible" => FormatResult::PossiblyValid, // Skip format check — key formats vary
        _ => FormatResult::PossiblyValid,
    }
}

/// Validate an API key with format check and connection test.
///
/// Steps:
/// 1. Format validation (instant, no network)
/// 2. Lightweight API call to verify the key works
///
/// On success, stores the key in the keychain and updates in-memory settings.
pub async fn validate_and_store_key(
    provider: &str,
    key: &str,
    base_url: Option<&str>,
) -> Result<KeyValidationResult> {
    // Step 1: Format check
    let format = validate_key_format(provider, key);
    let format_ok = matches!(format, FormatResult::Valid | FormatResult::PossiblyValid);

    if let FormatResult::InvalidFormat(reason) = &format {
        return Ok(KeyValidationResult {
            valid: false,
            format_ok: false,
            connection_ok: false,
            error: Some(reason.clone()),
            model_access: vec![],
        });
    }

    // Step 2: Connection test with 10s timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let (connection_ok, error, model_access) = match provider {
        "anthropic" => test_anthropic_key(&client, key).await,
        "openai" => test_openai_key(&client, key).await,
        "openai-compatible" => {
            test_openai_compatible_key(&client, key, base_url.unwrap_or("")).await
        }
        _ => (true, None, vec![]), // Unknown provider — skip connection test
    };

    let valid = format_ok && connection_ok;

    if valid {
        // Store in keychain
        let key_name = match provider {
            "anthropic" => "llm_api_key",
            "openai" => {
                // If provider is "openai" for main LLM, store as llm_api_key
                "llm_api_key"
            }
            _ => "llm_api_key",
        };
        let _ = super::keystore::store_secret(key_name, key);

        // Update in-memory settings
        let manager = crate::get_settings_manager();
        let mut guard = manager.lock();
        let settings = guard.get_mut();
        settings.llm.provider = provider.to_string();
        settings.llm.api_key = key.to_string();
        if settings.llm.model.is_empty() {
            settings.llm.model = model_access.first().cloned().unwrap_or_default();
        }
        let _ = guard.save();

        info!(
            target: "4da::validation",
            provider = provider,
            models = ?model_access,
            "API key validated and stored"
        );
    }

    Ok(KeyValidationResult {
        valid,
        format_ok,
        connection_ok,
        error,
        model_access,
    })
}

/// Test an Anthropic API key by making a minimal messages request.
async fn test_anthropic_key(
    client: &reqwest::Client,
    key: &str,
) -> (bool, Option<String>, Vec<String>) {
    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "hi"}]
    });

    match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                (
                    true,
                    None,
                    vec![
                        "claude-haiku-4-5-20251001".to_string(),
                        "claude-sonnet-4-20250514".to_string(),
                        "claude-opus-4-20250514".to_string(),
                    ],
                )
            } else if status.as_u16() == 401 {
                (false, Some("Invalid API key".to_string()), vec![])
            } else {
                // Non-auth errors (rate limit, etc.) — key is probably valid
                warn!(
                    target: "4da::validation",
                    status = status.as_u16(),
                    "Anthropic API returned non-200 but key may be valid"
                );
                (
                    true,
                    Some(format!("API returned status {} — key may be valid", status)),
                    vec!["claude-haiku-4-5-20251001".to_string()],
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                (false, Some("Connection timed out".to_string()), vec![])
            } else {
                (false, Some(format!("Connection failed: {}", e)), vec![])
            }
        }
    }
}

/// Test an OpenAI API key by listing available models.
async fn test_openai_key(
    client: &reqwest::Client,
    key: &str,
) -> (bool, Option<String>, Vec<String>) {
    match client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {key}"))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                // Parse model list
                let mut models = vec![];
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                        for model in data.iter().take(20) {
                            if let Some(id) = model.get("id").and_then(|i| i.as_str()) {
                                // Only include GPT models
                                if id.starts_with("gpt-") {
                                    models.push(id.to_string());
                                }
                            }
                        }
                    }
                }
                if models.is_empty() {
                    models.push("gpt-4o-mini".to_string());
                }
                (true, None, models)
            } else if status.as_u16() == 401 {
                (false, Some("Invalid API key".to_string()), vec![])
            } else {
                (
                    true,
                    Some(format!("API returned status {}", status)),
                    vec!["gpt-4o-mini".to_string()],
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                (false, Some("Connection timed out".to_string()), vec![])
            } else {
                (false, Some(format!("Connection failed: {}", e)), vec![])
            }
        }
    }
}

/// Test an OpenAI-compatible API key by making a minimal chat completion request.
/// We skip format validation since key formats vary across providers.
async fn test_openai_compatible_key(
    client: &reqwest::Client,
    key: &str,
    base_url: &str,
) -> (bool, Option<String>, Vec<String>) {
    if base_url.is_empty() {
        return (
            false,
            Some("Base URL is required for OpenAI-compatible providers".to_string()),
            vec![],
        );
    }

    let base = base_url.trim_end_matches('/');
    let url = if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{}/chat/completions", base)
    };

    let body = serde_json::json!({
        "model": "test",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "hi"}]
    });

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                // Connection works and key is valid
                (true, None, vec![])
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                (false, Some("Invalid API key".to_string()), vec![])
            } else {
                // Non-auth errors (model not found, rate limit, etc.) — key is probably valid
                warn!(
                    target: "4da::validation",
                    status = status.as_u16(),
                    "OpenAI-compatible API returned non-200 but key may be valid"
                );
                (
                    true,
                    Some(format!(
                        "API returned status {} — key appears valid",
                        status
                    )),
                    vec![],
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                (false, Some("Connection timed out".to_string()), vec![])
            } else {
                (false, Some(format!("Endpoint unreachable: {}", e)), vec![])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_format_valid() {
        // Generate a key that's exactly 100 chars starting with sk-ant-
        let key = format!("sk-ant-{}", "a".repeat(93));
        assert_eq!(validate_key_format("anthropic", &key), FormatResult::Valid);
    }

    #[test]
    fn test_anthropic_format_wrong_prefix() {
        let result = validate_key_format("anthropic", "sk-proj-abc123");
        assert!(matches!(result, FormatResult::InvalidFormat(_)));
    }

    #[test]
    fn test_anthropic_format_too_short() {
        let result = validate_key_format("anthropic", "sk-ant-short");
        assert!(matches!(result, FormatResult::InvalidFormat(_)));
    }

    #[test]
    fn test_openai_format_valid() {
        let key = format!("sk-{}", "b".repeat(45));
        assert_eq!(validate_key_format("openai", &key), FormatResult::Valid);
    }

    #[test]
    fn test_openai_format_project_key() {
        let key = format!("sk-proj-{}", "c".repeat(40));
        assert_eq!(validate_key_format("openai", &key), FormatResult::Valid);
    }

    #[test]
    fn test_openai_format_wrong_prefix() {
        let result = validate_key_format("openai", "pk-abc123456789012345678901234567890");
        assert!(matches!(result, FormatResult::InvalidFormat(_)));
    }

    #[test]
    fn test_empty_key() {
        let result = validate_key_format("anthropic", "");
        assert!(matches!(result, FormatResult::InvalidFormat(_)));
    }

    #[test]
    fn test_unknown_provider() {
        let result = validate_key_format("custom", "some-key-value");
        assert_eq!(result, FormatResult::PossiblyValid);
    }
}
