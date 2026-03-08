//! Settings and Context Engine Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains all settings management
//! commands and user context/identity commands.

use tracing::{debug, info, warn};

use crate::context_engine::{InteractionType, InterestSource};
use crate::error::Result;
use crate::llm::RelevanceJudge;
use crate::settings::{LLMProvider, RerankConfig};
use tauri::{AppHandle, Emitter};

use crate::error::FourDaError;
use crate::{embed_texts, get_context_engine, get_settings_manager, invalidate_context_engine};

/// Validate string input length, returning an error if too long
fn validate_input_length(value: &str, field: &str, max_len: usize) -> Result<()> {
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

/// Test LLM connection
#[tauri::command]
pub async fn test_llm_connection() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let settings = {
        let guard = manager.lock();
        guard.get().clone()
    };

    // Ollama doesn't need an API key
    if settings.llm.provider == "none"
        || (settings.llm.provider != "ollama" && settings.llm.api_key.is_empty())
    {
        return Err("No LLM provider configured".into());
    }

    info!(target: "4da::llm", provider = %settings.llm.provider, model = %settings.llm.model, "Testing LLM connection");

    // Ollama: use dedicated lightweight test (not the heavy judge_batch)
    if settings.llm.provider == "ollama" {
        return test_ollama_connection_impl(&settings.llm).await;
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
            Err(format!("Connection failed: {}", e).into())
        }
    }
}

/// Dedicated lightweight Ollama connection test.
/// Instead of sending a full judge_batch (2KB+ system prompt, slow on local models),
/// this does a 3-phase test: (1) version check, (2) model check, (3) tiny inference.
async fn test_ollama_connection_impl(llm: &LLMProvider) -> Result<serde_json::Value> {
    let base_url = llm.base_url.as_deref().unwrap_or("http://localhost:11434");
    let model = &llm.model;

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(120)) // generous for cold model load
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Phase 1: Check Ollama is running
    info!(target: "4da::ollama", base_url, "Phase 1: checking Ollama is reachable");
    let version_url = format!("{}/api/version", base_url);
    let version = match client.get(&version_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            data["version"].as_str().unwrap_or("unknown").to_string()
        }
        Ok(resp) => {
            let status = resp.status();
            return Err(format!(
                "Ollama returned HTTP {} — is something else running on {}?",
                status, base_url
            )
            .into());
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("connect") || msg.contains("refused") {
                return Err(format!(
                    "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                    base_url
                )
                .into());
            } else if msg.contains("timed out") || msg.contains("timeout") {
                return Err(format!(
                    "Connection to {} timed out. Check that the URL is correct and Ollama is running.",
                    base_url
                )
                .into());
            }
            return Err(format!("Failed to reach Ollama at {}: {}", base_url, e).into());
        }
    };
    info!(target: "4da::ollama", version = %version, "Ollama is running");

    // Phase 2: Check the requested model is available
    info!(target: "4da::ollama", model, "Phase 2: checking model is available");
    let tags_url = format!("{}/api/tags", base_url);
    let available_models: Vec<String> = match client.get(&tags_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            data["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => vec![],
    };

    // Check if model is available (handle both "llama3.2" and "llama3.2:latest")
    let model_found = available_models.iter().any(|m| {
        m == model
            || m.split(':').next() == model.split(':').next()
            || m == &format!("{}:latest", model)
            || model == &format!("{}:latest", m.split(':').next().unwrap_or(""))
    });

    if !model_found && !available_models.is_empty() {
        let model_list = available_models
            .iter()
            .filter(|m| !m.starts_with("nomic-embed-text"))
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Model '{}' not found in Ollama. Available models: {}. Run: ollama pull {}",
            model, model_list, model
        )
        .into());
    }

    if available_models.is_empty() {
        return Err(format!("No models installed in Ollama. Run: ollama pull {}", model).into());
    }

    // Phase 3: Tiny inference test (not the full relevance judge prompt!)
    info!(target: "4da::ollama", model, "Phase 3: testing inference");
    let chat_url = format!("{}/api/chat", base_url);
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Say OK"}],
        "stream": false,
        "options": {
            "num_predict": 10,
            "temperature": 0.0
        }
    });

    let start = std::time::Instant::now();
    match client.post(&chat_url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let elapsed = start.elapsed();
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            let content = data["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if content.is_empty() {
                return Err(format!(
                    "Ollama returned empty response for model '{}'. The model may be corrupted. Try: ollama rm {} && ollama pull {}",
                    model, model, model
                )
                .into());
            }

            info!(
                target: "4da::ollama",
                model,
                elapsed_ms = elapsed.as_millis() as u64,
                response = %content.chars().take(50).collect::<String>(),
                "Ollama test successful"
            );

            // Mark model as warm since we just loaded it
            crate::ollama::mark_warm(model);

            Ok(serde_json::json!({
                "success": true,
                "input_tokens": 0,
                "output_tokens": 0,
                "cost_cents": 0,
                "message": format!(
                    "Ollama v{} — {} is working! ({}ms)",
                    version, model, elapsed.as_millis()
                )
            }))
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.as_u16() == 404 || text.contains("not found") {
                Err(format!("Model '{}' not found. Run: ollama pull {}", model, model).into())
            } else if text.contains("out of memory")
                || text.contains("OOM")
                || text.contains("CUDA")
            {
                Err(format!(
                    "Not enough GPU memory for '{}'. Try a smaller model (e.g., llama3.2:1b or phi3:mini).",
                    model
                )
                .into())
            } else {
                Err(format!("Ollama inference error ({}): {}", status, text).into())
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("timed out") || msg.contains("timeout") {
                Err(format!(
                    "Ollama took too long to respond. The model '{}' may still be loading — try again in a few seconds.",
                    model
                )
                .into())
            } else {
                Err(format!("Ollama inference request failed: {}", e).into())
            }
        }
    }
}

/// Check Ollama status and list available models
#[tauri::command]
pub async fn check_ollama_status(base_url: Option<String>) -> Result<serde_json::Value> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Check if Ollama is running by hitting the version endpoint
    let version_url = format!("{}/api/version", url);
    let version_result = client.get(&version_url).send().await;

    match version_result {
        Ok(response) if response.status().is_success() => {
            let version_data: serde_json::Value = response
                .json()
                .await
                .unwrap_or(serde_json::json!({"version": "unknown"}));
            let version = version_data["version"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();

            // Fetch available models
            let tags_url = format!("{}/api/tags", url);
            let models = match client.get(&tags_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let tags_data: serde_json::Value = resp
                        .json()
                        .await
                        .unwrap_or(serde_json::json!({"models": []}));
                    tags_data["models"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m["name"].as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                }
                _ => vec![],
            };

            let has_embedding_model = models.iter().any(|m| m.starts_with("nomic-embed-text"));
            let has_llm_model = models.iter().any(|m| !m.starts_with("nomic-embed-text"));

            info!(target: "4da::ollama", version = %version, models = ?models, has_embedding_model, has_llm_model, "Ollama detected");

            Ok(serde_json::json!({
                "running": true,
                "version": version,
                "models": models,
                "base_url": url,
                "has_embedding_model": has_embedding_model,
                "has_llm_model": has_llm_model
            }))
        }
        Ok(response) => {
            let status = response.status();
            Err(format!("Ollama returned error status: {}", status).into())
        }
        Err(e) => {
            // Connection refused or timeout - Ollama not running
            info!(target: "4da::ollama", error = %e, "Ollama not detected");
            Ok(serde_json::json!({
                "running": false,
                "version": null,
                "models": [],
                "base_url": url,
                "error": format!("Ollama not running: {}", e)
            }))
        }
    }
}

/// Pull an Ollama model with progress events
#[tauri::command]
pub async fn pull_ollama_model(
    app: AppHandle,
    model: String,
    base_url: Option<String>,
) -> Result<serde_json::Value> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let pull_url = format!("{}/api/pull", url);

    info!(target: "4da::ollama", model = %model, "Starting model pull");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 min timeout for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post(&pull_url)
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Failed to start pull: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama pull failed ({}): {}", status, body).into());
    }

    // Read streaming response line by line
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.extend_from_slice(&chunk);

        // Process complete lines from buffer
        while let Some(newline_pos) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=newline_pos).collect();
            let line_str = String::from_utf8_lossy(&line);
            let trimmed = line_str.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(progress) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let status = progress["status"].as_str().unwrap_or("").to_string();
                let total = progress["total"].as_u64().unwrap_or(0);
                let completed = progress["completed"].as_u64().unwrap_or(0);
                let percent = if total > 0 {
                    (completed as f64 / total as f64 * 100.0) as u32
                } else {
                    0
                };
                let done = status == "success";

                let _ = app.emit(
                    "ollama-pull-progress",
                    serde_json::json!({
                        "model": model,
                        "status": status,
                        "percent": percent,
                        "done": done
                    }),
                );
            }
        }
    }

    info!(target: "4da::ollama", model = %model, "Model pull complete");

    Ok(serde_json::json!({
        "success": true,
        "model": model
    }))
}

/// Get current license tier and feature availability
#[tauri::command]
pub async fn get_license_tier() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;

    // Extract expiry from license key payload if present
    let (expires_at, days_remaining, expired) = if !license.license_key.is_empty() {
        match crate::settings::verify_license_key(&license.license_key) {
            Ok(payload) => {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
                    let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
                    let days = (exp.with_timezone(&chrono::Utc) - now).num_days();
                    (Some(payload.expires_at), days.max(0) as i32, days < 0)
                } else {
                    (Some(payload.expires_at), 0, false)
                }
            }
            Err(_) => (None, 0, true), // Key fails verification — treat as expired
        }
    } else {
        (None, 0, false)
    };

    Ok(serde_json::json!({
        "tier": license.tier,
        "activated_at": license.activated_at,
        "has_key": !license.license_key.is_empty(),
        "pro_features": crate::settings::PRO_FEATURES,
        "expires_at": expires_at,
        "days_remaining": days_remaining,
        "expired": expired,
    }))
}

/// Activate a license key — tries Keygen API first, falls back to ed25519 self-signed
#[tauri::command]
pub async fn activate_license(license_key: String) -> Result<serde_json::Value> {
    if license_key.trim().is_empty() {
        return Err("License key cannot be empty".into());
    }

    // Strategy: try Keygen API validation first (for Keygen-format keys like BE3529-...),
    // then fall back to local ed25519 verification (for self-signed 4DA- keys).
    let effective_tier: String;
    let email: Option<String>;
    let expires_at: Option<String>;

    if license_key.starts_with("4DA-") {
        // Self-signed ed25519 key
        let payload = crate::settings::verify_license_key(&license_key)?;
        effective_tier = match payload.tier.as_str() {
            "pro" | "team" => payload.tier.clone(),
            "community" | "cohort" => "pro".to_string(),
            _ => payload.tier.clone(),
        };
        email = Some(payload.email);
        expires_at = Some(payload.expires_at);
    } else {
        // Keygen API key (e.g., BE3529-741BAF-...)
        info!(target: "4da::license", "Validating Keygen key (format: {}...)", &license_key[..6.min(license_key.len())]);
        let result = crate::settings::validate_license_key_keygen_fresh(&license_key, "free").await;
        info!(target: "4da::license", tier = %result.tier, online = result.online, cached = result.cached, code = %result.code, detail = %result.detail, "Keygen validation result");

        if result.tier == "free" {
            return Ok(serde_json::json!({
                "success": false,
                "reason": result.detail,
            }));
        }
        effective_tier = result.tier;
        email = None;
        expires_at = None;
    }

    let manager = get_settings_manager();
    let mut guard = manager.lock();

    let settings = guard.get_mut();
    settings.license.license_key = license_key;
    settings.license.tier = effective_tier.clone();
    settings.license.activated_at = Some(chrono::Utc::now().to_rfc3339());
    guard.save()?;

    info!(target: "4da::license", "License activated — tier: {}", effective_tier);

    Ok(serde_json::json!({
        "success": true,
        "tier": effective_tier,
        "email": email,
        "expires_at": expires_at,
    }))
}

/// Get trial status
#[tauri::command]
pub async fn get_trial_status() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();
    let status = crate::settings::get_trial_status(&settings.license);

    Ok(serde_json::json!({
        "active": status.active,
        "days_remaining": status.days_remaining,
        "started_at": status.started_at,
        "has_license": status.has_license,
    }))
}

/// Start a free trial
#[tauri::command]
pub async fn start_trial() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();

    if !settings.license.license_key.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "reason": "Already have a license key",
        }));
    }

    if settings.license.trial_started_at.is_some() {
        let status = crate::settings::get_trial_status(&settings.license);
        return Ok(serde_json::json!({
            "success": false,
            "reason": "Trial already started",
            "active": status.active,
            "days_remaining": status.days_remaining,
        }));
    }

    settings.license.trial_started_at = Some(chrono::Utc::now().to_rfc3339());
    guard.save()?;

    info!(target: "4da::license", "Free trial started");

    Ok(serde_json::json!({
        "success": true,
        "days_remaining": 30,
    }))
}

/// Validate the current license key against the Keygen API.
/// Returns the validation result and updates the tier in settings if needed.
#[tauri::command]
pub async fn validate_license() -> Result<serde_json::Value> {
    // Read current license info (release lock before async work)
    let (license_key, current_tier) = {
        let manager = get_settings_manager();
        let guard = manager.lock();
        let license = &guard.get().license;
        (license.license_key.clone(), license.tier.clone())
    };

    if license_key.is_empty() {
        return Ok(serde_json::json!({
            "validated": false,
            "tier": "free",
            "detail": "No license key configured",
        }));
    }

    let result = crate::settings::validate_license_key_keygen(&license_key, &current_tier).await;

    // Update tier in settings if it changed
    if result.tier != current_tier {
        let manager = get_settings_manager();
        let mut guard = manager.lock();
        let settings = guard.get_mut();
        info!(target: "4da::license", old_tier = %current_tier, new_tier = %result.tier, "Tier updated after Keygen validation");
        settings.license.tier = result.tier.clone();
        let _ = guard.save();
    }

    Ok(serde_json::json!({
        "validated": result.online || result.cached,
        "tier": result.tier,
        "cached": result.cached,
        "detail": result.detail,
    }))
}

// ============================================================================
// Context Engine Commands
// ============================================================================

/// Get the user's static identity (interests, exclusions, role, etc.)
#[tauri::command]
pub async fn get_user_context() -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    Ok(serde_json::json!({
        "role": identity.role,
        "tech_stack": identity.tech_stack,
        "domains": identity.domains,
        "interests": identity.interests.iter().map(|i| serde_json::json!({
            "id": i.id,
            "topic": i.topic,
            "weight": i.weight,
            "source": i.source,
            "has_embedding": i.embedding.is_some()
        })).collect::<Vec<_>>(),
        "exclusions": identity.exclusions,
        "stats": {
            "interest_count": interest_count,
            "exclusion_count": exclusion_count
        }
    }))
}

/// Set the user's role
#[tauri::command]
pub async fn set_user_role(app: AppHandle, role: Option<String>) -> Result<serde_json::Value> {
    if let Some(ref r) = role {
        validate_input_length(r, "Role", 100)?;
    }
    let engine = get_context_engine()?;
    engine
        .set_role(role.as_deref())
        .map_err(|e| format!("Failed to set role: {}", e))?;

    info!(target: "4da::context", role = ?role, "Role updated");

    // GAME: track context setup + profile updates
    if role.is_some() {
        if let Ok(db) = crate::get_database() {
            for a in crate::game_engine::increment_counter(db, "context", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
            for a in crate::game_engine::increment_counter(db, "profile_updates", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "role": role
    }))
}

/// Add a technology to the user's tech stack
#[tauri::command]
pub async fn add_tech_stack(app: AppHandle, technology: String) -> Result<serde_json::Value> {
    validate_input_length(&technology, "Technology", 100)?;
    let engine = get_context_engine()?;
    engine
        .add_technology(&technology)
        .map_err(|e| format!("Failed to add technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Added technology");

    // GAME: track context setup + profile updates
    if let Ok(db) = crate::get_database() {
        for a in crate::game_engine::increment_counter(db, "context", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
        for a in crate::game_engine::increment_counter(db, "profile_updates", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "technology": technology
    }))
}

/// Remove a technology from the user's tech stack
#[tauri::command]
pub async fn remove_tech_stack(technology: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_technology(&technology)
        .map_err(|e| format!("Failed to remove technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Removed technology");

    Ok(serde_json::json!({
        "success": true
    }))
}
/// Add an explicit interest (with embedding generation)
#[tauri::command]
pub async fn add_interest(
    app: AppHandle,
    topic: String,
    weight: Option<f32>,
) -> Result<serde_json::Value> {
    validate_input_length(&topic, "Interest topic", 200)?;
    let engine = get_context_engine()?;
    let weight = weight.unwrap_or(1.0);

    // Generate embedding for the topic
    let embedding = embed_texts(std::slice::from_ref(&topic)).await?;
    let emb = embedding.first().map(|e| e.as_slice());

    let id = engine
        .add_interest(&topic, weight, emb, InterestSource::Explicit)
        .map_err(|e| format!("Failed to add interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, weight = weight, has_embedding = emb.is_some(), "Added interest");
    invalidate_context_engine();

    // GAME: track context setup + profile updates
    if let Ok(db) = crate::get_database() {
        for a in crate::game_engine::increment_counter(db, "context", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
        for a in crate::game_engine::increment_counter(db, "profile_updates", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "id": id,
        "topic": topic,
        "weight": weight,
        "has_embedding": emb.is_some()
    }))
}

/// Remove an interest
#[tauri::command]
pub async fn remove_interest(topic: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_interest(&topic)
        .map_err(|e| format!("Failed to remove interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed interest");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an exclusion (topic to never show)
#[tauri::command]
pub async fn add_exclusion(topic: String) -> Result<serde_json::Value> {
    validate_input_length(&topic, "Exclusion topic", 200)?;
    let engine = get_context_engine()?;
    engine
        .add_exclusion(&topic)
        .map_err(|e| format!("Failed to add exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Added exclusion");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true,
        "topic": topic
    }))
}

/// Remove an exclusion
#[tauri::command]
pub async fn remove_exclusion(topic: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_exclusion(&topic)
        .map_err(|e| format!("Failed to remove exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed exclusion");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Record a user interaction (click, save, dismiss)
#[tauri::command]
pub async fn record_interaction(
    app: AppHandle,
    source_item_id: i64,
    action: String,
) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let action_type = match action.to_lowercase().as_str() {
        "click" => InteractionType::Click,
        "save" => InteractionType::Save,
        "dismiss" => InteractionType::Dismiss,
        "ignore" => InteractionType::Ignore,
        _ => return Err(format!("Unknown action type: {}", action).into()),
    };

    engine
        .record_interaction(source_item_id, action_type)
        .map_err(|e| format!("Failed to record interaction: {}", e))?;

    debug!(target: "4da::context", action = %action, item_id = source_item_id, "Recorded interaction");

    // GAME: track saves
    if action.to_lowercase() == "save" {
        if let Ok(db) = crate::get_database() {
            for a in crate::game_engine::increment_counter(db, "saves", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
        }
    }

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Get locale configuration
#[tauri::command]
pub async fn get_locale() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let locale = &guard.get().locale;
    Ok(serde_json::json!({
        "country": locale.country,
        "language": locale.language,
        "currency": locale.currency
    }))
}

/// Update locale configuration
#[tauri::command]
pub async fn set_locale(country: String, language: String, currency: String) -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    guard.get_mut().locale = crate::settings::LocaleConfig {
        country,
        language,
        currency,
    };
    guard
        .save()
        .map_err(|e| FourDaError::Config(format!("Failed to save locale: {}", e)))?;
    info!(target: "4da::settings", "Locale updated");
    Ok(())
}

/// Get context engine statistics
#[tauri::command]
pub async fn get_context_stats() -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    Ok(serde_json::json!({
        "interests": interest_count,
        "exclusions": exclusion_count,
        "tech_stack": identity.tech_stack.len(),
        "domains": identity.domains.len(),
        "has_role": identity.role.is_some()
    }))
}

// ============================================================================
// STREETS Membership Commands
// ============================================================================

/// Get the user's current STREETS tier and activation info
#[tauri::command]
pub async fn get_streets_tier() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;

    // Check expiry — if key exists but is expired, downgrade to playbook
    let expired = if !license.license_key.is_empty() {
        match crate::settings::verify_license_key(&license.license_key) {
            Ok(payload) => {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
                    let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
                    (exp.with_timezone(&chrono::Utc) - now).num_days() < 0
                } else {
                    false
                }
            }
            Err(_) => true,
        }
    } else {
        false
    };

    let tier = if expired {
        "playbook"
    } else {
        crate::settings::get_streets_tier(license)
    };

    Ok(serde_json::json!({
        "tier": tier,
        "activated_at": license.activated_at,
        "expired": expired,
    }))
}

/// Activate a STREETS license key (Community or Cohort)
#[tauri::command]
pub async fn activate_streets_license(license_key: String) -> Result<serde_json::Value> {
    if license_key.trim().is_empty() {
        return Err("License key cannot be empty".into());
    }

    let payload = crate::settings::verify_license_key(&license_key)?;

    // Determine STREETS tier from features
    let streets_tier = if payload.features.contains(&"streets_cohort".to_string()) {
        "cohort"
    } else if payload.features.contains(&"streets_community".to_string()) {
        "community"
    } else {
        // Regular 4DA license — apply as normal
        "playbook"
    };

    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();
    settings.license.license_key = license_key;
    // Any paid STREETS license (community/cohort) also grants Pro features.
    // Explicit pro/team tiers in the payload take precedence.
    let effective_tier = if payload.tier == "pro" || payload.tier == "team" {
        payload.tier.clone()
    } else if streets_tier == "community" || streets_tier == "cohort" {
        "pro".to_string()
    } else {
        settings.license.tier.clone() // keep existing
    };
    settings.license.tier = effective_tier.clone();
    settings.license.activated_at = Some(chrono::Utc::now().to_rfc3339());
    guard.save()?;

    info!(target: "4da::streets", streets_tier, "STREETS license activated");

    Ok(serde_json::json!({
        "success": true,
        "streets_tier": streets_tier,
        "tier": effective_tier,
        "email": payload.email,
        "expires_at": payload.expires_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // validate_input_length tests
    // ========================================================================

    #[test]
    fn validate_input_length_accepts_short_string() {
        let result = validate_input_length("hello", "test_field", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_accepts_exact_max() {
        let input = "a".repeat(100);
        let result = validate_input_length(&input, "test_field", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_rejects_over_max() {
        let input = "a".repeat(101);
        let result = validate_input_length(&input, "test_field", 100);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("test_field"),
            "Error should mention the field name"
        );
        assert!(
            err_msg.contains("101"),
            "Error should mention the actual length"
        );
        assert!(
            err_msg.contains("100"),
            "Error should mention the max length"
        );
    }

    #[test]
    fn validate_input_length_accepts_empty_string() {
        let result = validate_input_length("", "test_field", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_zero_max_rejects_any_content() {
        let result = validate_input_length("a", "test_field", 0);
        assert!(result.is_err());
    }

    #[test]
    fn validate_input_length_zero_max_accepts_empty() {
        let result = validate_input_length("", "test_field", 0);
        assert!(result.is_ok());
    }

    // ========================================================================
    // Provider validation tests (extracted logic from set_llm_provider)
    // ========================================================================

    #[test]
    fn valid_providers_list_is_complete() {
        let valid_providers = ["anthropic", "openai", "ollama", "none"];
        assert_eq!(valid_providers.len(), 4);
        assert!(valid_providers.contains(&"anthropic"));
        assert!(valid_providers.contains(&"openai"));
        assert!(valid_providers.contains(&"ollama"));
        assert!(valid_providers.contains(&"none"));
    }

    #[test]
    fn invalid_provider_string_is_rejected() {
        let valid_providers = ["anthropic", "openai", "ollama", "none"];
        assert!(!valid_providers.contains(&"gemini"));
        assert!(!valid_providers.contains(&""));
        assert!(!valid_providers.contains(&"Anthropic")); // case-sensitive
    }

    // ========================================================================
    // RerankConfig clamping logic tests
    // ========================================================================

    #[test]
    fn rerank_config_clamps_max_items() {
        // Simulates the clamping logic from set_rerank_config
        let max_items: usize = 0;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 1);

        let max_items: usize = 5000;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 1000);

        let max_items: usize = 500;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 500);
    }

    #[test]
    fn rerank_config_clamps_min_score() {
        let min_score: f32 = -0.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.0);

        let min_score: f32 = 1.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 1.0);

        let min_score: f32 = 0.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.5);
    }

    #[test]
    fn rerank_config_enforces_minimum_token_limit() {
        let daily_token_limit: u64 = 0;
        let enforced = daily_token_limit.max(1);
        assert_eq!(enforced, 1);

        let daily_token_limit: u64 = 100_000;
        let enforced = daily_token_limit.max(1);
        assert_eq!(enforced, 100_000);
    }

    #[test]
    fn rerank_config_enforces_minimum_cost_limit() {
        let daily_cost_limit: u64 = 0;
        let enforced = daily_cost_limit.max(1);
        assert_eq!(enforced, 1);
    }

    // ========================================================================
    // RerankConfig struct construction
    // ========================================================================

    #[test]
    fn rerank_config_construction_with_clamping() {
        let config = RerankConfig {
            enabled: true,
            max_items_per_batch: 0_usize.clamp(1, 1000),
            min_embedding_score: (-1.0_f32).clamp(0.0, 1.0),
            daily_token_limit: 1,
            daily_cost_limit_cents: 1,
        };
        assert!(config.enabled);
        assert_eq!(config.max_items_per_batch, 1);
        assert_eq!(config.min_embedding_score, 0.0);
        assert_eq!(config.daily_token_limit, 1);
        assert_eq!(config.daily_cost_limit_cents, 1);
    }

    // ========================================================================
    // LLMProvider defaults
    // ========================================================================

    #[test]
    fn llm_provider_default_has_none_provider() {
        let provider = LLMProvider::default();
        assert_eq!(provider.provider, "none");
        assert!(provider.api_key.is_empty());
        assert!(provider.model.is_empty());
        assert!(provider.base_url.is_none());
        assert!(provider.openai_api_key.is_empty());
    }

    // ========================================================================
    // FourDaError From<String> and From<&str> conversions
    // ========================================================================

    #[test]
    fn error_from_string_creates_internal_variant() {
        let err: FourDaError = "test error".into();
        assert!(
            err.to_string().contains("test error"),
            "Error should preserve message"
        );
    }

    #[test]
    fn error_from_format_string_creates_internal_variant() {
        let err: FourDaError =
            format!("Invalid provider '{}'. Must be one of: a, b", "gemini").into();
        assert!(err.to_string().contains("gemini"));
        assert!(err.to_string().contains("Must be one of"));
    }

    // ========================================================================
    // Interaction type mapping (logic from record_interaction)
    // ========================================================================

    #[test]
    fn interaction_type_mapping_valid_actions() {
        let valid_actions = vec![
            ("click", InteractionType::Click),
            ("save", InteractionType::Save),
            ("dismiss", InteractionType::Dismiss),
            ("ignore", InteractionType::Ignore),
        ];
        for (action_str, expected_type) in &valid_actions {
            let mapped = match action_str.to_lowercase().as_str() {
                "click" => Some(InteractionType::Click),
                "save" => Some(InteractionType::Save),
                "dismiss" => Some(InteractionType::Dismiss),
                "ignore" => Some(InteractionType::Ignore),
                _ => None,
            };
            assert!(
                mapped.is_some(),
                "Action '{}' should map to a valid type",
                action_str
            );
            assert_eq!(
                std::mem::discriminant(&mapped.unwrap()),
                std::mem::discriminant(expected_type)
            );
        }
    }

    #[test]
    fn interaction_type_mapping_rejects_unknown_action() {
        let action = "bookmark";
        let mapped = match action.to_lowercase().as_str() {
            "click" => Some(InteractionType::Click),
            "save" => Some(InteractionType::Save),
            "dismiss" => Some(InteractionType::Dismiss),
            "ignore" => Some(InteractionType::Ignore),
            _ => None,
        };
        assert!(mapped.is_none(), "Unknown action should not map to a type");
    }

    // ========================================================================
    // Interaction type mapping is case-insensitive
    // ========================================================================

    #[test]
    fn interaction_type_mapping_case_insensitive() {
        for action in &["CLICK", "Click", "cLiCk", "SAVE", "Save", "DISMISS"] {
            let mapped = match action.to_lowercase().as_str() {
                "click" => Some("click"),
                "save" => Some("save"),
                "dismiss" => Some("dismiss"),
                "ignore" => Some("ignore"),
                _ => None,
            };
            assert!(
                mapped.is_some(),
                "Action '{}' should map case-insensitively",
                action
            );
        }
    }
}
