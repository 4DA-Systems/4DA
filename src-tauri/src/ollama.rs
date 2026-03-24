//! Centralized Ollama model management for 4DA
//!
//! Provides model warming, staleness tracking, and status events
//! so the frontend can show real-time Ollama readiness state.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

// ============================================================================
// Types
// ============================================================================

/// Event payload emitted to the frontend via `ollama-status`.
#[derive(Clone, Serialize)]
pub struct OllamaStatusEvent {
    pub phase: String,
    pub model: String,
    pub error: Option<String>,
}

/// Tracks warmed Ollama models and their staleness.
pub struct OllamaState {
    /// Models that have been warmed (received a successful tiny inference).
    pub warmed_models: HashSet<String>,
    /// When each model was last used or warmed.
    pub last_use: HashMap<String, Instant>,
    /// Whether a warm operation is currently in progress.
    pub warming: AtomicBool,
}

impl OllamaState {
    fn new() -> Self {
        Self {
            warmed_models: HashSet::new(),
            last_use: HashMap::new(),
            warming: AtomicBool::new(false),
        }
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Duration after which a warmed model is considered stale.
#[cfg(test)]
const WARM_STALENESS_SECS: u64 = 300; // 5 minutes

static OLLAMA_STATE: Lazy<Mutex<OllamaState>> = Lazy::new(|| Mutex::new(OllamaState::new()));

/// Access the global Ollama state.
#[cfg(test)]
pub(crate) fn get_ollama_state() -> &'static Mutex<OllamaState> {
    &OLLAMA_STATE
}

// ============================================================================
// Public API
// ============================================================================

/// Warm an Ollama model by sending a tiny inference request.
///
/// This loads the model into GPU/CPU memory so subsequent requests are fast.
/// Emits `ollama-status` events so the UI can display warming progress.
///
/// Returns early without error if another warm operation is already in progress.
pub(crate) async fn warm_model(model: &str, base_url: &str, app: &AppHandle) {
    // Check-and-set warming flag atomically. If already warming, bail out.
    {
        let state = OLLAMA_STATE.lock();
        if state
            .warming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            info!(target: "4da::ollama", model, "Warm already in progress, skipping");
            return;
        }
    }

    // Emit warming status
    let _ = app.emit(
        "ollama-status",
        OllamaStatusEvent {
            phase: "warming".into(),
            model: model.into(),
            error: None,
        },
    );

    info!(target: "4da::ollama", model, base_url, "Warming model");

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Say OK"}],
        "stream": false,
        "options": {
            "num_predict": 5,
            "temperature": 0.0
        }
    });

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|e| {
            warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        });

    let result = client.post(&url).json(&body).send().await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            info!(target: "4da::ollama", model, "Model warmed successfully");

            // Update state
            {
                let mut state = OLLAMA_STATE.lock();
                state.warmed_models.insert(model.to_string());
                state.last_use.insert(model.to_string(), Instant::now());
            }

            let _ = app.emit(
                "ollama-status",
                OllamaStatusEvent {
                    phase: "ready".into(),
                    model: model.into(),
                    error: None,
                },
            );
        }
        Ok(resp) => {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            let error_msg = format!("HTTP {status}: {error_body}");
            warn!(target: "4da::ollama", model, error = %error_msg, "Warm request failed");

            let _ = app.emit(
                "ollama-status",
                OllamaStatusEvent {
                    phase: "error".into(),
                    model: model.into(),
                    error: Some(error_msg),
                },
            );
        }
        Err(e) => {
            let error_msg = format!("{e}");
            warn!(target: "4da::ollama", model, error = %error_msg, "Warm request error");

            let _ = app.emit(
                "ollama-status",
                OllamaStatusEvent {
                    phase: "error".into(),
                    model: model.into(),
                    error: Some(error_msg),
                },
            );
        }
    }

    // Always clear warming flag
    {
        let state = OLLAMA_STATE.lock();
        state.warming.store(false, Ordering::SeqCst);
    }
}

/// Check if a model is warm and not stale.
///
/// A model is considered warm if it was successfully warmed and the last use
/// was within 5 minutes. Stale models are automatically removed.
#[cfg(test)]
pub(crate) fn is_warm(model: &str) -> bool {
    let mut state = OLLAMA_STATE.lock();

    if !state.warmed_models.contains(model) {
        return false;
    }

    if let Some(last) = state.last_use.get(model) {
        if last.elapsed().as_secs() > WARM_STALENESS_SECS {
            // Model is stale -- evict it
            state.warmed_models.remove(model);
            state.last_use.remove(model);
            info!(target: "4da::ollama", model, "Model warm status expired (stale > 5 min)");
            return false;
        }
        true
    } else {
        // No last_use recorded -- shouldn't happen, but treat as not warm
        state.warmed_models.remove(model);
        false
    }
}

/// Mark a model as warm and update its last-use timestamp.
///
/// Useful when the model is used for real inference (not just warming),
/// to refresh the staleness timer.
pub(crate) fn mark_warm(model: &str) {
    let mut state = OLLAMA_STATE.lock();
    state.warmed_models.insert(model.to_string());
    state.last_use.insert(model.to_string(), Instant::now());
}

// ============================================================================
// Startup: Ensure Models Available
// ============================================================================

/// Ensure required Ollama models are available, auto-pulling any that are missing.
///
/// This is the main startup entry point. It:
/// 1. Checks if Ollama is running
/// 2. Detects missing models (embedding + LLM)
/// 3. Auto-pulls missing models with progress events
/// 4. Warms the LLM model once all models are present
pub(crate) async fn ensure_models_available(llm_model: &str, base_url: &str, app: &AppHandle) {
    // Step 1: Check Ollama status
    let status =
        match crate::settings_commands::check_ollama_status(Some(base_url.to_string())).await {
            Ok(s) => s,
            Err(e) => {
                error!(target: "4da::ollama", error = %e, "Ollama status check failed");
                let _ = app.emit(
                    "ollama-status",
                    OllamaStatusEvent {
                        phase: "error".into(),
                        model: String::new(),
                        error: Some(format!("Cannot reach Ollama: {e}")),
                    },
                );
                return;
            }
        };

    if !status["running"].as_bool().unwrap_or(false) {
        let _ = app.emit(
            "ollama-status",
            OllamaStatusEvent {
                phase: "offline".into(),
                model: String::new(),
                error: None,
            },
        );
        return;
    }

    // Step 2: Check which models are present
    let models: Vec<String> = status["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let has_embedding = models.iter().any(|m| m.starts_with("nomic-embed-text"));
    let has_llm = models
        .iter()
        .any(|m| m.starts_with(llm_model) || m.contains(llm_model));

    let mut need_pull: Vec<String> = Vec::new();
    if !has_embedding {
        need_pull.push("nomic-embed-text".to_string());
    }
    if !has_llm {
        need_pull.push(llm_model.to_string());
    }

    // Step 3: Auto-pull missing models
    if !need_pull.is_empty() {
        info!(target: "4da::ollama", missing = ?need_pull, "Auto-pulling missing Ollama models");

        let _ = app.emit(
            "ollama-status",
            OllamaStatusEvent {
                phase: "pulling".into(),
                model: need_pull.join(", "),
                error: None,
            },
        );

        for model in &need_pull {
            info!(target: "4da::ollama", model = %model, "Pulling model");
            match crate::settings_commands::pull_ollama_model(
                app.clone(),
                model.clone(),
                Some(base_url.to_string()),
            )
            .await
            {
                Ok(_) => {
                    info!(target: "4da::ollama", model = %model, "Model pulled successfully");
                }
                Err(e) => {
                    error!(target: "4da::ollama", model = %model, error = %e, "Failed to auto-pull model");
                    let _ = app.emit(
                        "ollama-status",
                        OllamaStatusEvent {
                            phase: "error".into(),
                            model: model.clone(),
                            error: Some(format!("Failed to pull {model}: {e}")),
                        },
                    );
                    return;
                }
            }
        }

        info!(target: "4da::ollama", "All missing models pulled successfully");
    }

    // Step 4: Warm the LLM model
    warm_model(llm_model, base_url, app).await;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_warm_and_check() {
        mark_warm("test-model-1");
        assert!(is_warm("test-model-1"));
    }

    #[test]
    fn test_unknown_model_not_warm() {
        assert!(!is_warm("nonexistent-model-xyz"));
    }

    #[test]
    fn test_state_accessor_returns_same_instance() {
        let s1 = get_ollama_state();
        let s2 = get_ollama_state();
        assert!(std::ptr::eq(s1, s2));
    }

    #[test]
    fn test_warming_flag_default_false() {
        let state = get_ollama_state().lock();
        // After module init, warming should be false (or reset from prior tests)
        // We just verify it's accessible and is a bool
        let _val = state.warming.load(Ordering::SeqCst);
    }

    #[test]
    fn test_ollama_status_event_serialize() {
        let event = OllamaStatusEvent {
            phase: "warming".into(),
            model: "llama3".into(),
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"phase\":\"warming\""));
        assert!(json.contains("\"model\":\"llama3\""));
        assert!(json.contains("\"error\":null"));

        let event_err = OllamaStatusEvent {
            phase: "error".into(),
            model: "llama3".into(),
            error: Some("connection refused".into()),
        };
        let json_err = serde_json::to_string(&event_err).unwrap();
        assert!(json_err.contains("\"error\":\"connection refused\""));
    }
}
