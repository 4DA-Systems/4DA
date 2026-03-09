//! Settings and Context Engine Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains all settings management
//! commands and user context/identity commands.
//!
//! LLM commands, license/trial/context/STREETS commands, and tests are in
//! sibling modules to stay under file-size limits.

use tracing::info;

use crate::error::Result;
use crate::settings::{LLMProvider, RerankConfig};

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

/// Get current daily LLM token usage vs configured limit.
/// Returns { used, limit, limit_reached, resets_at } for the frontend.
#[tauri::command]
pub async fn get_llm_usage() -> Result<serde_json::Value> {
    let (used, limit) = crate::get_llm_token_usage();
    Ok(serde_json::json!({
        "used": used,
        "limit": limit,
        "limit_reached": limit > 0 && used >= limit,
        "unlimited": limit == 0,
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
    // Validate provider
    let valid_providers = ["anthropic", "openai", "ollama", "none"];
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

    let llm_provider = LLMProvider {
        provider,
        api_key,
        model,
        base_url,
        openai_api_key: openai_api_key.unwrap_or_default(),
    };

    guard.set_llm_provider(llm_provider)?;
    info!(target: "4da::settings", "LLM provider updated");
    Ok(())
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

    let config = RerankConfig {
        enabled,
        max_items_per_batch: max_items.clamp(1, 1000),
        min_embedding_score: min_score.clamp(0.0, 1.0),
        daily_token_limit: daily_token_limit.max(1),
        daily_cost_limit_cents: daily_cost_limit.max(1),
    };

    guard.set_rerank_config(config)?;
    info!(target: "4da::settings", enabled = enabled, "Re-rank config updated");
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
