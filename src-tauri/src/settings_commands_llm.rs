// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM provider commands: test connection, Ollama status, model pulling.

use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{info, warn};

use crate::error::Result;
use crate::llm::RelevanceJudge;
use crate::settings::LLMProvider;
use tauri::{AppHandle, Emitter};

use crate::get_settings_manager;

/// Abort flag for cancelling an in-progress Ollama model pull.
static OLLAMA_PULL_ABORT: AtomicBool = AtomicBool::new(false);

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
        return test_ollama_connection_impl(&settings.llm).await;
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

/// Dedicated lightweight Ollama connection test.
/// Instead of sending a full judge_batch (2KB+ system prompt, slow on local models),
/// this does a 3-phase test: (1) version check, (2) model check, (3) tiny inference.
async fn test_ollama_connection_impl(llm: &LLMProvider) -> Result<serde_json::Value> {
    let base_url = llm.base_url.as_deref().unwrap_or("http://localhost:11434");
    let model = &llm.model;

    // Security: warn if non-localhost Ollama URL uses HTTP (keys/data sent in cleartext)
    if base_url.starts_with("http://") {
        let is_localhost = base_url.contains("://localhost")
            || base_url.contains("://127.0.0.1")
            || base_url.contains("://[::1]");
        if !is_localhost {
            warn!(
                target: "4da::ollama",
                base_url,
                "Remote Ollama URL uses HTTP — data will be sent unencrypted. Use HTTPS for remote connections."
            );
        }
    }

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(120)) // generous for cold model load
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    // Phase 1: Check Ollama is running
    info!(target: "4da::ollama", base_url, "Phase 1: checking Ollama is reachable");
    let version_url = format!("{base_url}/api/version");
    let version = match client.get(&version_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            data["version"].as_str().unwrap_or("unknown").to_string()
        }
        Ok(resp) => {
            let status = resp.status();
            return Err(format!(
                "Ollama returned HTTP {status} — is something else running on {base_url}?"
            )
            .into());
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("connect") || msg.contains("refused") {
                return Err(format!(
                    "Cannot connect to Ollama at {base_url}. Make sure Ollama is running (ollama serve)."
                )
                .into());
            } else if msg.contains("timed out") || msg.contains("timeout") {
                return Err(format!(
                    "Connection to {base_url} timed out. Check that the URL is correct and Ollama is running."
                )
                .into());
            }
            return Err(format!("Failed to reach Ollama at {base_url}: {e}").into());
        }
    };
    info!(target: "4da::ollama", version = %version, "Ollama is running");

    // Phase 2: Check the requested model is available
    info!(target: "4da::ollama", model, "Phase 2: checking model is available");
    let tags_url = format!("{base_url}/api/tags");
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
            || m == &format!("{model}:latest")
            || model == &format!("{}:latest", m.split(':').next().unwrap_or(""))
    });

    if !model_found && !available_models.is_empty() {
        let model_list = available_models
            .iter()
            .filter(|m| {
                let embed_model = crate::reembed::get_embedding_model();
                !m.starts_with("nomic-embed-text") && !m.starts_with(&embed_model)
            })
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Model '{model}' not found in Ollama. Available models: {model_list}. Run: ollama pull {model}"
        )
        .into());
    }

    if available_models.is_empty() {
        return Err(format!("No models installed in Ollama. Run: ollama pull {model}").into());
    }

    // Phase 3: Tiny inference test (not the full relevance judge prompt!)
    info!(target: "4da::ollama", model, "Phase 3: testing inference");
    let chat_url = format!("{base_url}/api/chat");
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
                    "Ollama returned empty response for model '{model}'. The model may be corrupted. Try: ollama rm {model} && ollama pull {model}"
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
                Err(format!("Model '{model}' not found. Run: ollama pull {model}").into())
            } else if text.contains("out of memory")
                || text.contains("OOM")
                || text.contains("CUDA")
            {
                Err(format!(
                    "Not enough GPU memory for '{model}'. Try a smaller model (e.g., gemma3:4b or qwen3:8b)."
                )
                .into())
            } else {
                Err(format!("Ollama inference error ({status}): {text}").into())
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("timed out") || msg.contains("timeout") {
                Err(format!(
                    "Ollama took too long to respond. The model '{model}' may still be loading — try again in a few seconds."
                )
                .into())
            } else {
                Err(format!("Ollama inference request failed: {e}").into())
            }
        }
    }
}

/// Check Ollama status and list available models
#[tauri::command]
pub async fn check_ollama_status(base_url: Option<String>) -> Result<serde_json::Value> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = crate::http_client::PROBE_CLIENT.clone();

    // Check if Ollama is running
    let version_url = format!("{url}/api/version");
    let version = match client.get(&version_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            data["version"].as_str().unwrap_or("unknown").to_string()
        }
        Ok(_) => {
            return Ok(serde_json::json!({
                "running": false,
                "error": "Ollama returned unexpected response"
            }));
        }
        Err(e) => {
            return Ok(serde_json::json!({
                "running": false,
                "error": format!("Cannot connect to Ollama: {}", e)
            }));
        }
    };

    // Get available models
    let tags_url = format!("{url}/api/tags");
    let models: Vec<serde_json::Value> = match client.get(&tags_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            data["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|m| {
                            serde_json::json!({
                                "name": m["name"].as_str().unwrap_or(""),
                                "size": m["size"].as_u64().unwrap_or(0),
                                "modified_at": m["modified_at"].as_str().unwrap_or("")
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => vec![],
    };

    Ok(serde_json::json!({
        "running": true,
        "version": version,
        "models": models,
        "url": url
    }))
}

/// Pull a model in Ollama
#[tauri::command]
pub async fn pull_ollama_model(
    app: AppHandle,
    model: String,
    base_url: Option<String>,
) -> Result<serde_json::Value> {
    // Reset the abort flag so a previous cancellation doesn't immediately kill this pull
    OLLAMA_PULL_ABORT.store(false, Ordering::Relaxed);

    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let pull_url = format!("{url}/api/pull");

    info!(target: "4da::ollama", model = %model, "Starting model pull");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 min timeout for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(&pull_url)
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Failed to start pull: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama pull failed ({status}): {body}").into());
    }

    // Read streaming response line by line
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();

    while let Some(chunk) = stream.next().await {
        // Check for cancellation on each chunk
        if OLLAMA_PULL_ABORT.load(Ordering::Relaxed) {
            info!(target: "4da::ollama", model = %model, "Model pull cancelled by user");
            let _ = app.emit(
                "ollama-pull-progress",
                serde_json::json!({
                    "model": model,
                    "status": "cancelled",
                    "percent": 0,
                    "done": true
                }),
            );
            return Err("Model download cancelled".into());
        }

        let chunk = chunk.map_err(|e| format!("Stream error: {e}"))?;
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

/// Cancel an in-progress Ollama model pull
#[tauri::command]
pub async fn cancel_ollama_pull() -> Result<String> {
    OLLAMA_PULL_ABORT.store(true, Ordering::Relaxed);
    info!(target: "4da::ollama", "Ollama pull cancellation requested");
    Ok("Cancellation requested".to_string())
}

/// Check whether the configured LLM can produce accurate briefing synthesis.
///
/// Returns capability info the settings UI uses to show educational guidance
/// about model requirements, hardware-aware model recommendations, and model
/// tier classification.
#[tauri::command]
pub async fn check_synthesis_capability() -> Result<serde_json::Value> {
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    let capable = crate::ollama::can_synthesize(&llm_settings).await;
    let can_explain = crate::ollama::can_explain(&llm_settings).await;

    let (model_name, model_params, provider) = if llm_settings.provider == "ollama" {
        let base_url = llm_settings
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let model = llm_settings.model.clone();
        let params = crate::ollama::get_model_params_billions(&model, base_url).await;
        (model, params, "ollama".to_string())
    } else if llm_settings.provider == "builtin" {
        (llm_settings.model.clone(), None, "builtin".to_string())
    } else {
        (
            llm_settings.model.clone(),
            None,
            llm_settings.provider.clone(),
        )
    };

    let hw = crate::hardware_detect::detect_hardware();
    let tier = crate::hardware_detect::ram_tier(&hw);

    let model_tier = if provider == "ollama" && !model_name.is_empty() {
        Some(crate::model_allowlist::classify_model(&model_name))
    } else {
        None
    };

    let recommended = crate::model_allowlist::recommend_models(hw.ram_total_gb);
    let top_recommendation = recommended.first().map(|e| e.family);

    let guidance = if provider == "builtin" {
        if crate::llm_engine::sidecar_status() == crate::llm_engine::SidecarStatus::Ready {
            "Built-in LLM is running and ready for synthesis.".to_string()
        } else {
            "Built-in LLM sidecar is not running — start it from settings.".to_string()
        }
    } else if capable {
        "Your model supports AI-powered briefing synthesis.".to_string()
    } else if model_name.is_empty() {
        match top_recommendation {
            Some(rec) => format!(
                "No model selected. Based on your hardware ({:.0} GB RAM), we recommend: ollama pull {rec}",
                hw.ram_total_gb
            ),
            None => "No model selected. Configure a cloud API key (Anthropic/OpenAI) — your system RAM is too low for local models.".to_string(),
        }
    } else {
        match top_recommendation {
            Some(rec) => format!(
                "Your model is below the synthesis threshold. Based on your hardware ({:.0} GB RAM), try: ollama pull {rec}",
                hw.ram_total_gb
            ),
            None => "Briefing synthesis requires a 7B+ parameter model or a cloud API key (Anthropic/OpenAI).".to_string(),
        }
    };

    Ok(serde_json::json!({
        "can_synthesize": capable,
        "can_explain": can_explain,
        "provider": provider,
        "model": model_name,
        "params_billions": model_params,
        "min_params_billions": 7.0,
        "unverified": provider == "openai-compatible",
        "model_tier": model_tier,
        "guidance": guidance,
        "hardware": {
            "ram_total_gb": hw.ram_total_gb,
            "ram_available_gb": hw.ram_available_gb,
            "ram_tier": tier,
            "gpu": hw.gpu,
        },
        "recommended_model": top_recommendation,
    }))
}

/// Start the built-in LLM sidecar with the specified GGUF model path.
#[tauri::command]
pub async fn start_builtin_llm(model_path: String) -> Result<serde_json::Value> {
    let path = std::path::PathBuf::from(&model_path);
    let port = crate::llm_engine::start_sidecar(&path).await?;
    Ok(serde_json::json!({
        "status": "ready",
        "port": port,
        "model_path": model_path,
    }))
}

/// Stop the built-in LLM sidecar.
#[tauri::command]
pub fn stop_builtin_llm() -> Result<serde_json::Value> {
    crate::llm_engine::stop_sidecar();
    Ok(serde_json::json!({ "status": "stopped" }))
}

/// Get the current status of the built-in LLM sidecar.
#[tauri::command]
pub fn get_builtin_llm_status() -> Result<serde_json::Value> {
    let status = crate::llm_engine::sidecar_status();
    let port = crate::llm_engine::sidecar_port();
    Ok(serde_json::json!({
        "status": status,
        "port": port,
    }))
}

/// List available models from the built-in catalog with download status.
#[tauri::command]
pub fn list_builtin_models() -> Result<serde_json::Value> {
    let hw = crate::hardware_detect::detect_hardware();
    let catalog: Vec<serde_json::Value> = crate::model_manager::model_catalog()
        .iter()
        .map(|entry| {
            let downloaded = crate::model_manager::is_model_downloaded(entry);
            let path = crate::model_manager::model_path(entry);
            serde_json::json!({
                "id": entry.id,
                "display_name": entry.display_name,
                "family": entry.family,
                "size_bytes": entry.size_bytes,
                "size_gb": (entry.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0,
                "min_ram_gb": entry.min_ram_gb,
                "quantization": entry.quantization,
                "downloaded": downloaded,
                "path": path,
                "fits_ram": entry.min_ram_gb <= hw.ram_total_gb,
            })
        })
        .collect();

    let recommended = crate::model_manager::recommend_model(hw.ram_available_gb);

    Ok(serde_json::json!({
        "models": catalog,
        "recommended_id": recommended.map(|e| e.id),
        "ram_total_gb": hw.ram_total_gb,
        "ram_available_gb": hw.ram_available_gb,
    }))
}

/// Download a model from the built-in catalog.
#[tauri::command]
pub async fn download_builtin_model(
    app_handle: AppHandle,
    model_id: String,
) -> Result<serde_json::Value> {
    let entry = crate::model_manager::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let handle = app_handle.clone();
    let path = crate::model_manager::download_model(entry, move |progress| {
        let _ = handle.emit("model-download-progress", &progress);
    })
    .await?;

    Ok(serde_json::json!({
        "model_id": model_id,
        "path": path,
        "status": "complete",
    }))
}

/// Cancel an in-progress model download.
#[tauri::command]
pub fn cancel_builtin_model_download() -> Result<String> {
    crate::model_manager::cancel_download();
    Ok("Download cancellation requested".into())
}

/// Delete a downloaded model from disk.
#[tauri::command]
pub fn delete_builtin_model(model_id: String) -> Result<serde_json::Value> {
    let entry = crate::model_manager::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;
    crate::model_manager::delete_model(entry)?;
    Ok(serde_json::json!({
        "model_id": model_id,
        "status": "deleted",
    }))
}

/// Run the model evaluation harness against the currently configured LLM.
#[tauri::command]
pub async fn run_model_eval() -> Result<serde_json::Value> {
    let llm_settings = {
        let mut guard = crate::get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    let summary = crate::model_eval::run_eval(&llm_settings.model, &llm_settings.provider)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&summary).map_err(|e| e.to_string().into())
}
