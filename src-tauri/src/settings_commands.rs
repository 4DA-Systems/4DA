//! Settings and Context Engine Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains all settings management
//! commands and user context/identity commands.

use tracing::{debug, info, warn};

use crate::context_engine::{InteractionType, InterestSource};
use crate::llm::RelevanceJudge;
use crate::settings::{LLMProvider, RerankConfig};
use crate::{embed_texts, get_context_engine, get_settings_manager};

// ============================================================================
// Settings Commands
// ============================================================================

/// Get current settings
#[tauri::command]
pub async fn get_settings() -> Result<serde_json::Value, String> {
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
            "tokens_today": settings.usage.tokens_today,
            "cost_today_cents": settings.usage.cost_today_cents,
            "tokens_total": settings.usage.tokens_total,
            "items_reranked": settings.usage.items_reranked
        },
        "embedding_threshold": settings.embedding_threshold
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
) -> Result<(), String> {
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
pub async fn mark_onboarding_complete() -> Result<(), String> {
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
) -> Result<(), String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();

    let config = RerankConfig {
        enabled,
        max_items_per_batch: max_items,
        min_embedding_score: min_score,
        daily_token_limit,
        daily_cost_limit_cents: daily_cost_limit,
    };

    guard.set_rerank_config(config)?;
    info!(target: "4da::settings", enabled = enabled, "Re-rank config updated");
    Ok(())
}

/// Test LLM connection
#[tauri::command]
pub async fn test_llm_connection() -> Result<serde_json::Value, String> {
    let manager = get_settings_manager();
    let settings = {
        let guard = manager.lock();
        guard.get().clone()
    };

    // Ollama doesn't need an API key
    if settings.llm.provider == "none"
        || (settings.llm.provider != "ollama" && settings.llm.api_key.is_empty())
    {
        return Err("No LLM provider configured".to_string());
    }

    info!(target: "4da::llm", provider = %settings.llm.provider, "Testing LLM connection");

    let judge = RelevanceJudge::new(settings.llm.clone());

    // Simple test - ask for a short response
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
            info!(target: "4da::llm", input_tokens = input_tokens, output_tokens = output_tokens, cost_cents = cost, "LLM test successful");

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
            Err(format!("Connection failed: {}", e))
        }
    }
}

/// Check Ollama status and list available models
#[tauri::command]
pub async fn check_ollama_status(base_url: Option<String>) -> Result<serde_json::Value, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
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

            info!(target: "4da::ollama", version = %version, models = ?models, "Ollama detected");

            Ok(serde_json::json!({
                "running": true,
                "version": version,
                "models": models,
                "base_url": url
            }))
        }
        Ok(response) => {
            let status = response.status();
            Err(format!("Ollama returned error status: {}", status))
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

/// Get usage statistics
#[tauri::command]
pub async fn get_usage_stats() -> Result<serde_json::Value, String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let within_limits = guard.within_daily_limits();
    let summary = guard.usage_summary();
    let settings = guard.get();

    Ok(serde_json::json!({
        "tokens_today": settings.usage.tokens_today,
        "cost_today_cents": settings.usage.cost_today_cents,
        "tokens_total": settings.usage.tokens_total,
        "items_reranked": settings.usage.items_reranked,
        "daily_token_limit": settings.rerank.daily_token_limit,
        "daily_cost_limit_cents": settings.rerank.daily_cost_limit_cents,
        "within_limits": within_limits,
        "summary": summary
    }))
}

// ============================================================================
// Context Engine Commands
// ============================================================================

/// Get the user's static identity (interests, exclusions, role, etc.)
#[tauri::command]
pub async fn get_user_context() -> Result<serde_json::Value, String> {
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
pub async fn set_user_role(role: Option<String>) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .set_role(role.as_deref())
        .map_err(|e| format!("Failed to set role: {}", e))?;

    info!(target: "4da::context", role = ?role, "Role updated");

    Ok(serde_json::json!({
        "success": true,
        "role": role
    }))
}

/// Add a technology to the user's tech stack
#[tauri::command]
pub async fn add_tech_stack(technology: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_technology(&technology)
        .map_err(|e| format!("Failed to add technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Added technology");

    Ok(serde_json::json!({
        "success": true,
        "technology": technology
    }))
}

/// Remove a technology from the user's tech stack
#[tauri::command]
pub async fn remove_tech_stack(technology: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_technology(&technology)
        .map_err(|e| format!("Failed to remove technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Removed technology");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add a domain of interest
#[tauri::command]
pub async fn add_domain(domain: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_domain(&domain)
        .map_err(|e| format!("Failed to add domain: {}", e))?;

    debug!(target: "4da::context", domain = %domain, "Added domain");

    Ok(serde_json::json!({
        "success": true,
        "domain": domain
    }))
}

/// Remove a domain of interest
#[tauri::command]
pub async fn remove_domain(domain: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_domain(&domain)
        .map_err(|e| format!("Failed to remove domain: {}", e))?;

    debug!(target: "4da::context", domain = %domain, "Removed domain");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an explicit interest (with embedding generation)
#[tauri::command]
pub async fn add_interest(topic: String, weight: Option<f32>) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    let weight = weight.unwrap_or(1.0);

    // Generate embedding for the topic
    let embedding = embed_texts(std::slice::from_ref(&topic)).await?;
    let emb = embedding.first().map(|e| e.as_slice());

    let id = engine
        .add_interest(&topic, weight, emb, InterestSource::Explicit)
        .map_err(|e| format!("Failed to add interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, weight = weight, has_embedding = emb.is_some(), "Added interest");

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
pub async fn remove_interest(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_interest(&topic)
        .map_err(|e| format!("Failed to remove interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed interest");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an exclusion (topic to never show)
#[tauri::command]
pub async fn add_exclusion(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_exclusion(&topic)
        .map_err(|e| format!("Failed to add exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Added exclusion");

    Ok(serde_json::json!({
        "success": true,
        "topic": topic
    }))
}

/// Remove an exclusion
#[tauri::command]
pub async fn remove_exclusion(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_exclusion(&topic)
        .map_err(|e| format!("Failed to remove exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed exclusion");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Record a user interaction (click, save, dismiss)
#[tauri::command]
pub async fn record_interaction(
    source_item_id: i64,
    action: String,
) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;

    let action_type = match action.to_lowercase().as_str() {
        "click" => InteractionType::Click,
        "save" => InteractionType::Save,
        "dismiss" => InteractionType::Dismiss,
        "ignore" => InteractionType::Ignore,
        _ => return Err(format!("Unknown action type: {}", action)),
    };

    engine
        .record_interaction(source_item_id, action_type)
        .map_err(|e| format!("Failed to record interaction: {}", e))?;

    debug!(target: "4da::context", action = %action, item_id = source_item_id, "Recorded interaction");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Get context engine statistics
#[tauri::command]
pub async fn get_context_stats() -> Result<serde_json::Value, String> {
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
