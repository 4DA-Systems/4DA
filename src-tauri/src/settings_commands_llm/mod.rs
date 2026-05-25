// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM provider commands: test connection, Ollama status, model pulling,
//! built-in sidecar management, and model evaluation.
//!
//! Submodules hold the implementation logic; this file owns the
//! `#[tauri::command]` wrappers so the Tauri macro-generated items
//! live at the correct module depth for `pub use *` re-export.

mod builtin;
mod eval;
mod ollama;

use std::sync::atomic::AtomicBool;

use tracing::{info, warn};

use crate::error::Result;
use crate::llm::RelevanceJudge;

use crate::get_settings_manager;

/// Abort flag for cancelling an in-progress Ollama model pull.
pub(crate) static OLLAMA_PULL_ABORT: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Connection & discovery commands
// ============================================================================

/// Test LLM connection
#[tauri::command]
pub async fn test_llm_connection() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let settings = {
        let mut guard = manager.lock();
        guard.ensure_keys_hydrated();
        guard.get().clone()
    };

    if settings.llm.provider == "none"
        || (!matches!(settings.llm.provider.as_str(), "ollama" | "builtin")
            && settings.llm.api_key.is_empty())
    {
        return Err("No LLM provider configured".into());
    }

    info!(target: "4da::llm", provider = %settings.llm.provider, model = %settings.llm.model, "Testing LLM connection");

    // Ollama: use dedicated lightweight test (not the heavy judge_batch)
    if settings.llm.provider == "ollama" {
        return ollama::test_ollama_connection_impl(&settings.llm).await;
    }

    // Builtin: check sidecar status then use the standard cloud-style test path
    if settings.llm.provider == "builtin" {
        if crate::llm_engine::sidecar_status() != crate::llm_engine::SidecarStatus::Ready {
            return Err("Built-in LLM sidecar is not running — start it from settings".into());
        }
    }

    // Cloud providers: use a lightweight direct LLM call
    let judge = RelevanceJudge::new(settings.llm.clone());
    let test_items = vec![(
        "test".to_string(),
        "Test Item".to_string(),
        "This is a test.".to_string(),
    )];

    match judge
        .judge_batch("User is testing the connection.", test_items)
        .await
    {
        Ok((_, input_tokens, output_tokens)) => {
            let cost = judge.estimate_cost_cents(input_tokens, output_tokens);
            info!(target: "4da::llm", input_tokens, output_tokens, cost_cents = cost, "LLM test successful");

            Ok(serde_json::json!({
                "success": true,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cost_cents": cost,
                "message": format!("Connection successful! Test used {} tokens.", input_tokens + output_tokens)
            }))
        }
        Err(e) => {
            warn!(target: "4da::llm", error = %e, "LLM test failed");
            Err(format!("Connection failed: {e}").into())
        }
    }
}

/// List available models from a provider (dynamic discovery)
#[tauri::command]
pub async fn list_provider_models(
    provider: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value> {
    let client = crate::http_client::PROBE_CLIENT.clone();

    match provider.as_str() {
        "ollama" => {
            // Ollama uses /api/tags
            let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
            let resp = client
                .get(format!("{url}/api/tags"))
                .send()
                .await
                .map_err(|e| format!("Cannot reach Ollama: {e}"))?;
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            let models: Vec<String> = data["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(std::string::ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Ok(serde_json::json!({ "models": models }))
        }
        "openai" | "openai-compatible" => {
            // OpenAI-compatible /v1/models endpoint
            let url = base_url.unwrap_or_else(|| "https://api.openai.com".to_string());
            let key = api_key.unwrap_or_default();
            let models_url = format!("{}/models", url.trim_end_matches('/'));
            let resp = client
                .get(&models_url)
                .header("Authorization", format!("Bearer {key}"))
                .send()
                .await
                .map_err(|e| format!("Cannot reach API: {e}"))?;
            if !resp.status().is_success() {
                return Ok(
                    serde_json::json!({ "models": [], "error": format!("HTTP {}", resp.status()) }),
                );
            }
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            let mut models: Vec<String> = data["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(std::string::ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();
            models.sort();
            // Cap at 50 models to prevent UI overflow
            models.truncate(50);
            Ok(serde_json::json!({ "models": models }))
        }
        "anthropic" => {
            // Use the model registry for Anthropic (no listing API)
            let models = crate::model_registry::get_provider_models("anthropic");
            if models.is_empty() {
                // Fallback if registry isn't initialized
                Ok(serde_json::json!({
                    "models": ["claude-haiku-4-5-20251001", "claude-sonnet-4-6", "claude-opus-4-6"]
                }))
            } else {
                Ok(serde_json::json!({ "models": models }))
            }
        }
        "builtin" => {
            // Query the sidecar's /v1/models endpoint if running
            if let Some(base) = crate::llm_engine::sidecar_base_url() {
                let models_url = format!("{base}/models");
                match client.get(&models_url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let data: serde_json::Value = resp.json().await.unwrap_or_default();
                        let models: Vec<String> = data["data"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|m| {
                                        m["id"].as_str().map(std::string::ToString::to_string)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        Ok(serde_json::json!({ "models": models }))
                    }
                    _ => Ok(serde_json::json!({ "models": [] })),
                }
            } else {
                Ok(serde_json::json!({ "models": [], "error": "Sidecar not running" }))
            }
        }
        _ => Ok(serde_json::json!({ "models": [] })),
    }
}

/// Detect running local LLM servers by probing common ports
#[tauri::command]
pub async fn detect_local_servers() -> Result<serde_json::Value> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let servers = [
        ("Ollama", "http://localhost:11434", "/api/version"),
        ("LM Studio", "http://localhost:1234", "/v1/models"),
        ("llama.cpp", "http://localhost:8080", "/v1/models"),
        ("Jan", "http://localhost:1337", "/v1/models"),
    ];

    let mut detected = Vec::new();

    for (name, base_url, probe_path) in servers {
        let probe_url = format!("{base_url}{probe_path}");
        match client.get(&probe_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let data: serde_json::Value = resp.json().await.unwrap_or_default();
                let model_count = if name == "Ollama" {
                    data["models"].as_array().map_or(0, std::vec::Vec::len)
                } else {
                    data["data"].as_array().map_or(0, std::vec::Vec::len)
                };
                detected.push(serde_json::json!({
                    "name": name,
                    "base_url": base_url,
                    "model_count": model_count,
                    "running": true
                }));
                info!(target: "4da::llm", server = name, models = model_count, "Detected local LLM server");
            }
            _ => {} // Not running — skip silently
        }
    }

    Ok(serde_json::json!({ "servers": detected }))
}

// ============================================================================
// Ollama commands (delegated to ollama submodule)
// ============================================================================

/// Check Ollama status and list available models
#[tauri::command]
pub async fn check_ollama_status(base_url: Option<String>) -> Result<serde_json::Value> {
    ollama::check_ollama_status_impl(base_url).await
}

/// Pull a model in Ollama
#[tauri::command]
pub async fn pull_ollama_model(
    app: tauri::AppHandle,
    model: String,
    base_url: Option<String>,
) -> Result<serde_json::Value> {
    ollama::pull_ollama_model_impl(app, model, base_url).await
}

/// Cancel an in-progress Ollama model pull
#[tauri::command]
pub async fn cancel_ollama_pull() -> Result<String> {
    ollama::cancel_ollama_pull_impl()
}

// ============================================================================
// Built-in LLM commands (delegated to builtin submodule)
// ============================================================================

/// Start the built-in LLM sidecar by model ID (resolved from catalog).
#[tauri::command]
pub async fn start_builtin_llm(model_id: String) -> Result<serde_json::Value> {
    builtin::start_builtin_llm_impl(model_id).await
}

/// Stop the built-in LLM sidecar.
#[tauri::command]
pub fn stop_builtin_llm() -> Result<serde_json::Value> {
    builtin::stop_builtin_llm_impl()
}

/// Get the current status of the built-in LLM sidecar.
#[tauri::command]
pub fn get_builtin_llm_status() -> Result<serde_json::Value> {
    builtin::get_builtin_llm_status_impl()
}

/// List available models from the built-in catalog with download status.
#[tauri::command]
pub fn list_builtin_models() -> Result<serde_json::Value> {
    builtin::list_builtin_models_impl()
}

/// Download a model from the built-in catalog.
#[tauri::command]
pub async fn download_builtin_model(
    app_handle: tauri::AppHandle,
    model_id: String,
) -> Result<serde_json::Value> {
    builtin::download_builtin_model_impl(app_handle, model_id).await
}

/// Cancel an in-progress model download.
#[tauri::command]
pub fn cancel_builtin_model_download() -> Result<String> {
    builtin::cancel_builtin_model_download_impl()
}

/// Delete a downloaded model from disk.
#[tauri::command]
pub fn delete_builtin_model(model_id: String) -> Result<serde_json::Value> {
    builtin::delete_builtin_model_impl(model_id)
}

// ============================================================================
// Eval commands (delegated to eval submodule)
// ============================================================================

/// Check whether the configured LLM can produce accurate briefing synthesis.
///
/// Returns capability info the settings UI uses to show educational guidance
/// about model requirements, hardware-aware model recommendations, and model
/// tier classification.
#[tauri::command]
pub async fn check_synthesis_capability() -> Result<serde_json::Value> {
    eval::check_synthesis_capability_impl().await
}

/// Run the model evaluation harness against the currently configured LLM.
#[tauri::command]
pub async fn run_model_eval() -> Result<serde_json::Value> {
    eval::run_model_eval_impl().await
}
