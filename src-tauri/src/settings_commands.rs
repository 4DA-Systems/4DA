// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Settings and Context Engine Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains all settings management
//! commands and user context/identity commands.
//!
//! LLM commands, license/trial/context/STREETS commands, and tests are in
//! sibling modules to stay under file-size limits.

use tracing::info;

use crate::error::Result;
use crate::settings::{LLMProvider, LlmLimitsConfig, RerankConfig};

use crate::get_settings_manager;

/// Validate string input length, returning an error if too long
pub(crate) fn validate_input_length(value: &str, field: &str, max_len: usize) -> Result<()> {
    if value.len() > max_len {
        return Err(format!(
            "{} too long ({} chars, max {})",
            field,
            value.len(),
            max_len
        )
        .into());
    }
    Ok(())
}

// ============================================================================
// Settings Commands
// ============================================================================

/// Get current settings
#[tauri::command]
pub async fn get_settings() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();

    Ok(serde_json::json!({
        "llm": {
            "provider": settings.llm.provider,
            "model": settings.llm.model,
            "has_api_key": !settings.llm.api_key.is_empty(),
            "base_url": settings.llm.base_url
        },
        "rerank": {
            "enabled": settings.rerank.enabled,
            "max_items_per_batch": settings.rerank.max_items_per_batch,
            "min_embedding_score": settings.rerank.min_embedding_score,
            "daily_token_limit": settings.rerank.daily_token_limit,
            "daily_cost_limit_cents": settings.rerank.daily_cost_limit_cents
        },
        "llm_limits": {
            "daily_token_limit": settings.llm_limits.daily_token_limit,
            "daily_cost_limit_cents": settings.llm_limits.daily_cost_limit_cents
        },
        "usage": {
            "tokens_today": guard.get_usage().tokens_today,
            "cost_today_cents": guard.get_usage().cost_today_cents,
            "tokens_total": guard.get_usage().tokens_total,
            "items_reranked": guard.get_usage().items_reranked
        },
        "embedding_threshold": settings.embedding_threshold,
        "onboarding_complete": settings.onboarding_complete,
        "auto_discovery_completed": settings.auto_discovery_completed,
        "license": {
            "tier": settings.license.tier,
            "has_key": !settings.license.license_key.is_empty(),
            "activated_at": settings.license.activated_at,
        }
    }))
}

/// Get current daily LLM token and cost usage vs configured limits.
/// Returns token and cost usage information for the frontend.
#[tauri::command]
pub async fn get_llm_usage() -> Result<serde_json::Value> {
    let (tokens_used, tokens_limit) = crate::get_llm_token_usage();
    let (cost_used, cost_limit) = crate::state::get_llm_cost_usage();
    let token_limit_reached = tokens_limit > 0 && tokens_used >= tokens_limit;
    let cost_limit_reached = cost_limit > 0 && cost_used >= cost_limit;
    Ok(serde_json::json!({
        "used": tokens_used,
        "limit": tokens_limit,
        "limit_reached": token_limit_reached || cost_limit_reached,
        "unlimited": tokens_limit == 0 && cost_limit == 0,
        "cost_used_cents": cost_used,
        "cost_limit_cents": cost_limit,
        "cost_limit_reached": cost_limit_reached,
    }))
}

/// Update LLM provider settings
#[tauri::command]
pub async fn set_llm_provider(
    provider: String,
    api_key: String,
    model: String,
    base_url: Option<String>,
    openai_api_key: Option<String>,
) -> Result<()> {
    // Input length validation — prevents buffer bloat attacks
    if provider.len() > 50 {
        return Err("Provider name too long".into());
    }
    if api_key.len() > 500 {
        return Err("API key too long".into());
    }
    if model.len() > 200 {
        return Err("Model name too long".into());
    }
    if let Some(ref url) = base_url {
        if url.len() > 500 {
            return Err("Base URL too long".into());
        }
        // Validate base URL scheme and format
        if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Base URL must use http:// or https:// scheme".into());
        }
        // SSRF prevention: block private/internal IPs for non-Ollama providers.
        // Ollama is expected to run on localhost, so skip this check for it.
        if !url.is_empty() && provider != "ollama" {
            crate::url_validation::validate_not_internal(url)?;
        }
    }
    if let Some(ref key) = openai_api_key {
        if key.len() > 500 {
            return Err("OpenAI API key too long".into());
        }
    }

    // Validate provider
    let valid_providers = ["anthropic", "openai", "openai-compatible", "ollama", "none"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!(
            "Invalid provider '{}'. Must be one of: {}",
            provider,
            valid_providers.join(", ")
        )
        .into());
    }

    let manager = get_settings_manager();
    let mut guard = manager.lock();

    // Store keys in platform keychain before creating the provider struct
    if !api_key.is_empty() {
        let _ = crate::settings::keystore::store_secret("llm_api_key", &api_key);
    }
    if let Some(ref oai_key) = openai_api_key {
        if !oai_key.is_empty() {
            let _ = crate::settings::keystore::store_secret("openai_api_key", oai_key);
        }
    }

    // Preserve the existing embedding model setting when updating LLM provider
    let existing_embedding_model = guard.get().llm.embedding_model.clone();
    let llm_provider = LLMProvider {
        provider,
        api_key,
        model,
        base_url,
        openai_api_key: openai_api_key.unwrap_or_default(),
        embedding_model: existing_embedding_model,
    };

    guard.set_llm_provider(llm_provider)?;
    info!(target: "4da::settings", "LLM provider updated");
    Ok(())
}

/// Returns LLM configuration for MCP servers — keys are masked for security.
/// MCP servers should retrieve actual keys through the backend keychain, not IPC.
#[tauri::command]
pub async fn get_llm_key_for_mcp() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();

    // Mask the API key — only show first 4 and last 2 chars
    let masked_key = if settings.llm.api_key.len() > 8 {
        let key = &settings.llm.api_key;
        format!("{}...{}", &key[..4], &key[key.len() - 2..])
    } else if !settings.llm.api_key.is_empty() {
        "****".to_string()
    } else {
        String::new()
    };

    let has_key = !settings.llm.api_key.is_empty();

    Ok(serde_json::json!({
        "provider": settings.llm.provider,
        "api_key_masked": masked_key,
        "has_api_key": has_key,
        "model": settings.llm.model,
        "base_url": settings.llm.base_url,
    }))
}

/// Mark onboarding wizard as complete
#[tauri::command]
pub async fn mark_onboarding_complete() -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    guard.mark_onboarding_complete()?;
    info!(target: "4da::settings", "Onboarding marked complete");
    Ok(())
}

/// Update re-ranking configuration
#[tauri::command]
pub async fn set_rerank_config(
    enabled: bool,
    max_items: usize,
    min_score: f32,
    daily_token_limit: u64,
    daily_cost_limit: u64,
) -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();

    // Preserve reconciler_enabled from the current config — this setting is
    // managed separately (not exposed via this command) and defaults to true
    // for new installs. See `docs/strategy/INTELLIGENCE-MESH.md` §5.2.
    let existing_reconciler = guard.get().rerank.reconciler_enabled;
    let config = RerankConfig {
        enabled,
        max_items_per_batch: max_items.clamp(1, 1000),
        min_embedding_score: min_score.clamp(0.0, 1.0),
        daily_token_limit: daily_token_limit.max(1),
        daily_cost_limit_cents: daily_cost_limit.max(1),
        reconciler_enabled: existing_reconciler,
    };

    guard.set_rerank_config(config)?;
    info!(target: "4da::settings", enabled = enabled, "Re-rank config updated");
    Ok(())
}

/// Update LLM rate-limiting configuration (daily token and cost caps)
#[tauri::command]
pub async fn set_llm_limits(daily_token_limit: u64, daily_cost_limit_cents: u64) -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();

    let config = LlmLimitsConfig {
        // 0 = unlimited (valid), otherwise must be at least 1
        daily_token_limit: if daily_token_limit == 0 {
            0
        } else {
            daily_token_limit.max(1)
        },
        daily_cost_limit_cents: if daily_cost_limit_cents == 0 {
            0
        } else {
            daily_cost_limit_cents.max(1)
        },
    };

    guard.set_llm_limits(config)?;
    info!(
        target: "4da::settings",
        token_limit = daily_token_limit,
        cost_limit_cents = daily_cost_limit_cents,
        "LLM rate limits updated"
    );
    Ok(())
}

// ============================================================================
// Environment Detection (Phase 2)
// ============================================================================

/// Detect API keys available in environment variables.
///
/// Returns masked previews only — full keys never cross IPC.
#[tauri::command]
pub async fn detect_environment() -> Result<serde_json::Value> {
    let detected = crate::settings::env_detection::detect_api_keys();
    Ok(serde_json::to_value(detected).unwrap_or_default())
}

/// Import an API key from an environment variable into the keychain.
///
/// The full key is read server-side, stored in the keychain, and never
/// returned to the frontend.
#[tauri::command]
pub async fn import_env_key(provider: String) -> Result<String> {
    validate_input_length(&provider, "provider", 20)?;
    crate::settings::env_detection::import_env_key(&provider)
}

// ============================================================================
// Key Validation (Phase 3)
// ============================================================================

/// Validate an API key with format check and connection test.
///
/// The key is consumed server-side — on success it's stored in the keychain.
/// Never returned to the frontend.
#[tauri::command]
pub async fn validate_api_key(
    provider: String,
    key: String,
    base_url: Option<String>,
) -> Result<serde_json::Value> {
    validate_input_length(&provider, "provider", 30)?;
    validate_input_length(&key, "api_key", 500)?;
    let result =
        crate::settings::validation::validate_and_store_key(&provider, &key, base_url.as_deref())
            .await?;
    Ok(serde_json::to_value(result).unwrap_or_default())
}

// ============================================================================
// Privacy Disclosure Commands
// ============================================================================

/// Get the current privacy configuration (content level, disclosure status,
/// crash-reporting opt-in, activity-tracking opt-in).
#[tauri::command]
pub async fn get_privacy_config() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();
    Ok(serde_json::json!({
        "llm_content_level": settings.privacy.llm_content_level,
        "proxy_url": settings.network.proxy_url,
        "cloud_llm_disclosure_accepted": settings.privacy.cloud_llm_disclosure_accepted,
        "crash_reporting_opt_in": settings.privacy.crash_reporting_opt_in,
        "activity_tracking_opt_in": settings.privacy.activity_tracking_opt_in,
    }))
}

/// Update privacy settings (content level, disclosure acceptance,
/// crash-reporting opt-in, and/or activity-tracking opt-in).
#[tauri::command]
pub async fn set_privacy_config(
    llm_content_level: Option<String>,
    cloud_llm_disclosure_accepted: Option<bool>,
    crash_reporting_opt_in: Option<bool>,
    activity_tracking_opt_in: Option<bool>,
) -> Result<()> {
    if let Some(ref level) = llm_content_level {
        validate_input_length(level, "llm_content_level", 20)?;
    }
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    if let Some(level) = llm_content_level {
        if matches!(level.as_str(), "full" | "titles_only") {
            guard.get_mut().privacy.llm_content_level = level;
        } else {
            return Err(crate::error::FourDaError::Validation(
                "llm_content_level must be 'full' or 'titles_only'".into(),
            ));
        }
    }
    if let Some(accepted) = cloud_llm_disclosure_accepted {
        guard.get_mut().privacy.cloud_llm_disclosure_accepted = accepted;
    }
    if let Some(opt_in) = crash_reporting_opt_in {
        guard.get_mut().privacy.crash_reporting_opt_in = opt_in;
    }
    if let Some(opt_in) = activity_tracking_opt_in {
        guard.get_mut().privacy.activity_tracking_opt_in = opt_in;
    }
    guard.save().map_err(|e| {
        crate::error::FourDaError::Config(format!("Failed to save privacy config: {e}"))
    })?;
    info!(target: "4da::settings", "Privacy config updated");
    Ok(())
}

// --- Sibling modules ---

#[path = "settings_commands_llm.rs"]
mod settings_commands_llm;
pub use settings_commands_llm::*;

#[path = "settings_commands_license.rs"]
mod settings_commands_license;
pub use settings_commands_license::*;

#[path = "settings_commands_tests.rs"]
mod settings_commands_tests;
