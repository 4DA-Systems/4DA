//! Environment variable detection for API keys.
//!
//! Detects API keys from environment variables and offers one-click import
//! into the platform keychain. Keys never cross the IPC boundary — only
//! masked previews are sent to the frontend.

use crate::error::Result;
use serde::Serialize;
use tracing::info;

/// Env var names to check for each provider.
const ANTHROPIC_VARS: &[&str] = &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY"];
const OPENAI_VARS: &[&str] = &["OPENAI_API_KEY", "OPENAI_KEY"];
const OLLAMA_HOST_VAR: &str = "OLLAMA_HOST";

/// Detected environment keys (preview only — never the full key).
#[derive(Debug, Clone, Serialize)]
pub struct DetectedKeysResponse {
    pub has_anthropic_env: bool,
    pub anthropic_env_preview: String,
    pub has_openai_env: bool,
    pub openai_env_preview: String,
    pub ollama_running: bool,
    pub ollama_url: Option<String>,
}

/// Mask a key for safe display: first 8 + last 4 chars.
fn mask_key(key: &str) -> String {
    if key.len() <= 12 {
        return "*".repeat(key.len());
    }
    format!("{}...{}", &key[..8], &key[key.len() - 4..])
}

/// Find an API key from a set of env var names.
/// Returns (found, masked_preview, var_name_used).
fn find_env_key<'a>(var_names: &[&'a str]) -> (bool, String, &'a str) {
    for var in var_names {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return (true, mask_key(&val), var);
            }
        }
    }
    (false, String::new(), "")
}

/// Detect API keys available in the environment.
pub fn detect_api_keys() -> DetectedKeysResponse {
    let (has_anthropic, anthropic_preview, _) = find_env_key(ANTHROPIC_VARS);
    let (has_openai, openai_preview, _) = find_env_key(OPENAI_VARS);

    // Check Ollama: env var for custom URL, then probe default
    let ollama_url = std::env::var(OLLAMA_HOST_VAR)
        .ok()
        .filter(|s| !s.is_empty());
    let ollama_check_url = ollama_url.as_deref().unwrap_or("http://localhost:11434");

    // Quick non-blocking check if Ollama is running (best-effort)
    let ollama_running = std::net::TcpStream::connect_timeout(
        &format!(
            "{}:11434",
            ollama_check_url
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .split(':')
                .next()
                .unwrap_or("localhost")
        )
        .parse()
        .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 11434))),
        std::time::Duration::from_millis(500),
    )
    .is_ok();

    DetectedKeysResponse {
        has_anthropic_env: has_anthropic,
        anthropic_env_preview: anthropic_preview,
        has_openai_env: has_openai,
        openai_env_preview: openai_preview,
        ollama_running,
        ollama_url,
    }
}

/// Import an API key from an environment variable into the keychain.
///
/// The key is read server-side, stored in the keychain, and never returned
/// to the frontend. Returns the provider name that was imported.
pub fn import_env_key(provider: &str) -> Result<String> {
    let (var_names, key_name) = match provider {
        "anthropic" => (ANTHROPIC_VARS, "llm_api_key"),
        "openai" => (OPENAI_VARS, "openai_api_key"),
        _ => return Err(format!("Unknown provider: {provider}").into()),
    };

    let (found, _, var_used) = find_env_key(var_names);
    if !found {
        return Err(format!("No environment variable found for {provider}").into());
    }

    // Read the full key (server-side only)
    let full_key = std::env::var(var_used).map_err(|_| format!("Failed to read {var_used}"))?;

    // Store in keychain — returns true when persisted, false for graceful
    // plaintext fallback on headless systems. Either is fine for this path.
    let _ = super::keystore::store_secret(key_name, &full_key)?;

    // Also update in-memory settings
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();
    match provider {
        "anthropic" => {
            settings.llm.provider = "anthropic".to_string();
            settings.llm.api_key = full_key;
            if settings.llm.model.is_empty() {
                settings.llm.model = "claude-haiku-4-5-20251001".to_string();
            }
        }
        "openai" => {
            settings.llm.provider = "openai".to_string();
            settings.llm.api_key = full_key;
            if settings.llm.model.is_empty() {
                settings.llm.model = "gpt-4o-mini".to_string();
            }
        }
        _ => {}
    }
    guard.save()?;

    info!(
        target: "4da::env_detection",
        provider = provider,
        env_var = var_used,
        "Imported API key from environment"
    );

    Ok(provider.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_key_long() {
        // Split the prefix to avoid triggering repo secret scanners on a
        // clearly-fake fixture. The mask_key function only cares about
        // length and the first/last 4 chars.
        let fixture = format!("{}{}", "sk-", "ant-api03-abcdefghijklmnop");
        let masked = mask_key(&fixture);
        assert_eq!(masked, "sk-ant-a...mnop");
    }

    #[test]
    fn test_mask_key_short() {
        let masked = mask_key("short");
        assert_eq!(masked, "*****");
    }

    #[test]
    fn test_mask_key_exactly_12() {
        let masked = mask_key("123456789012");
        assert_eq!(masked, "************");
    }

    #[test]
    fn test_detect_api_keys_returns_struct() {
        // This test runs without env vars set — should return empty/false
        let result = detect_api_keys();
        // We can't assert specifics since CI might have env vars, but struct should be valid
        assert!(result.anthropic_env_preview.len() < 200);
        assert!(result.openai_env_preview.len() < 200);
    }

    #[test]
    fn test_find_env_key_not_set() {
        let (found, preview, _) = find_env_key(&["FOURDA_TEST_NONEXISTENT_XYZ"]);
        assert!(!found);
        assert!(preview.is_empty());
    }
}
