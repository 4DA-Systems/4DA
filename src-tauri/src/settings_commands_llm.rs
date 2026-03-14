//! LLM provider commands: test connection, Ollama status, model pulling.

use tracing::{info, warn};

use crate::error::Result;
use crate::llm::RelevanceJudge;
use crate::settings::LLMProvider;
use tauri::{AppHandle, Emitter};

use crate::get_settings_manager;

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

    // Check if Ollama is running
    let version_url = format!("{}/api/version", url);
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
    let tags_url = format!("{}/api/tags", url);
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
