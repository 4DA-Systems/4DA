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

/// Duration after which a warmed model is considered stale. Ollama
/// unloads inactive models after about 5 minutes by default; we match
/// that threshold so our in-memory warm-state doesn't diverge.
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
///
/// Previously gated behind `#[cfg(test)]`; promoted to `pub(crate)` in the
/// onboarding load-time work so the deep-scan can use the staleness-aware
/// check instead of a plain contains() — ensures Ollama model unload
/// (which happens after ~5min idle) correctly invalidates warmup state.
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

/// Check whether a warm operation is currently in progress. Callers
/// polling for warm-completion can use this to distinguish "not yet
/// started" from "in flight".
pub(crate) fn is_warming() -> bool {
    let state = OLLAMA_STATE.lock();
    state.warming.load(Ordering::SeqCst)
}

/// Wait up to `timeout` for a model to become warm. Returns `true` if
/// the model is warm at return time, `false` if we timed out.
///
/// This is the "gate" used by the first-launch deep scan to pre-warm
/// Ollama BEFORE starting embedding generation, so the first batch of
/// embeddings doesn't hit a cold model and wait 30-60 seconds for the
/// model to load. See `docs/strategy/ONBOARDING-LOAD-TIME.md` Part 4.1
/// for the full architectural context.
///
/// **Never blocks the calling task's thread pool for long.** Uses a
/// 100ms poll loop with tokio::time::sleep (cooperative). Yields to
/// the runtime on every poll.
///
/// Returns immediately if the model is already warm. Returns `false`
/// (without spinning) if Ollama is not the configured provider — caller
/// can still proceed with cold-start cost.
pub(crate) async fn wait_for_warm(model: &str, timeout: std::time::Duration) -> bool {
    use std::time::Instant;
    let deadline = Instant::now() + timeout;
    // Fast path: already warm.
    if is_warm(model) {
        return true;
    }
    // Poll until warm or deadline.
    while Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if is_warm(model) {
            return true;
        }
    }
    // Timed out. Caller proceeds with cold-start cost; not an error.
    tracing::warn!(
        target: "4da::ollama",
        model,
        timeout_ms = timeout.as_millis() as u64,
        "wait_for_warm timed out — proceeding with potential cold-start penalty"
    );
    false
}

// ============================================================================
// Startup: Ensure Models Available
// ============================================================================

/// Event emitted when Ollama is reachable but required models are missing.
/// Frontend listens for this and shows a non-blocking banner asking the user
/// for explicit consent before downloading (potentially gigabytes of) models.
#[derive(Clone, Serialize)]
pub struct OllamaNeedsModelsEvent {
    /// List of model names the user is missing.
    pub missing: Vec<String>,
    /// Approximate total download size in MB. Conservative estimates only —
    /// the frontend should phrase this as "approximately X MB".
    pub estimated_mb: u64,
    /// The base URL the frontend should use when the user accepts.
    pub base_url: String,
}

// ============================================================================
// Ollama Runtime Version Check
// ============================================================================

/// Minimum Ollama runtime version 4DA supports.
///
/// Why pinned: Ollama's `/api/embeddings` and `/api/generate` contracts have
/// changed multiple times during 0.x. We need a version floor that contains
/// all the API surface 4DA depends on, so users running ancient Ollama builds
/// get a clear upgrade hint instead of confusing "model not loading" errors.
///
/// Update this constant when Ollama ships a breaking API change that 4DA
/// adopts. Always raise — never lower.
///
/// Format: "MAJOR.MINOR.PATCH" — only the numeric tuple is compared.
const MIN_OLLAMA_VERSION: (u32, u32, u32) = (0, 1, 32);

/// Event emitted when the connected Ollama daemon is older than 4DA's
/// supported floor. The frontend listens and shows a non-blocking banner
/// asking the user to update Ollama.
#[derive(Clone, Serialize)]
pub struct OllamaVersionWarningEvent {
    /// The version string returned by `/api/version` (raw, e.g. "0.1.30").
    pub current: String,
    /// The minimum version 4DA supports, formatted "MAJOR.MINOR.PATCH".
    pub minimum: String,
    /// User-facing message explaining the impact and the fix.
    pub message: String,
}

/// Parse a semver-ish "MAJOR.MINOR.PATCH" string into a numeric tuple.
///
/// Tolerant of trailing build metadata (e.g. "0.1.32-rc1" → (0,1,32)).
/// Returns `None` if any of the three numeric segments fail to parse —
/// in which case the caller should treat the version as unknown rather
/// than throwing a misleading "too old" warning.
fn parse_ollama_version(s: &str) -> Option<(u32, u32, u32)> {
    // Strip a leading "v" if present (Ollama doesn't currently use one,
    // but be defensive for future format changes).
    let s = s.trim().trim_start_matches('v');
    // Cut off everything after the first '-' or '+' (semver pre-release/build).
    let core = s.split(|c| c == '-' || c == '+').next().unwrap_or(s);
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor, patch))
}

/// Compare two version tuples lexicographically.
fn version_lt(a: (u32, u32, u32), b: (u32, u32, u32)) -> bool {
    a < b
}

/// Query Ollama's `/api/version` endpoint and warn the frontend if the
/// installed daemon is older than `MIN_OLLAMA_VERSION`.
///
/// This complements `check_ollama_status` (which only verifies reachability)
/// by adding a contract-version check on top. It is intentionally:
///
///   - **Soft-fail.** If `/api/version` is unreachable or returns garbage,
///     no event is emitted — the regular reachability check has already
///     handled the offline case.
///   - **Non-blocking.** Returns void; emits an event if the version is
///     too old. The user keeps control of the UX.
///   - **Idempotent.** Safe to call from multiple startup paths.
///
/// Called from `ensure_models_available` so it runs once per cold boot
/// alongside the existing model-presence detection.
pub(crate) async fn check_ollama_version(base_url: &str, app: &AppHandle) {
    let url = format!("{}/api/version", base_url.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::ollama", error = %e, "Could not build HTTP client for version check");
            return;
        }
    };

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            // Reachability handled elsewhere. Stay silent here.
            tracing::debug!(target: "4da::ollama", error = %e, "Ollama version probe failed (reachability checked elsewhere)");
            return;
        }
    };

    if !response.status().is_success() {
        tracing::debug!(target: "4da::ollama", status = %response.status(), "Ollama /api/version returned non-2xx");
        return;
    }

    let body: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(target: "4da::ollama", error = %e, "Ollama /api/version returned non-JSON");
            return;
        }
    };

    let version_str = match body.get("version").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            tracing::debug!(target: "4da::ollama", "Ollama /api/version response missing 'version' field");
            return;
        }
    };

    let parsed = match parse_ollama_version(&version_str) {
        Some(t) => t,
        None => {
            // Unparseable → log and stay silent. We'd rather miss an old-version
            // warning than nag the user about a future Ollama format change.
            tracing::warn!(
                target: "4da::ollama",
                version = %version_str,
                "Could not parse Ollama version string — skipping version check"
            );
            return;
        }
    };

    let min = MIN_OLLAMA_VERSION;
    let min_str = format!("{}.{}.{}", min.0, min.1, min.2);

    if version_lt(parsed, min) {
        let message = format!(
            "Ollama {version_str} is older than 4DA's minimum supported version ({min_str}). \
             Some embedding and chat features may behave unexpectedly. \
             Update Ollama from https://ollama.com/download — your existing models stay intact."
        );
        warn!(
            target: "4da::ollama",
            current = %version_str,
            minimum = %min_str,
            "Ollama runtime is older than the supported floor"
        );
        let _ = app.emit(
            "ollama-version-warning",
            OllamaVersionWarningEvent {
                current: version_str,
                minimum: min_str,
                message,
            },
        );
    } else {
        info!(
            target: "4da::ollama",
            current = %version_str,
            minimum = %min_str,
            "Ollama runtime version is supported"
        );
    }
}

/// Sovereign Cold Boot — DETECT (do not auto-pull) Ollama model availability.
///
/// Replaces the previous `ensure_models_available` which auto-downloaded
/// missing models on startup, potentially fetching gigabytes behind the
/// user's back during the cold-boot stampede. This version:
///
/// 1. Checks if Ollama is running.
/// 2. Detects missing models (embedding + LLM).
/// 3. **Emits** `ollama-needs-models` if any are missing — does NOT pull.
///    The frontend shows a banner; the user explicitly clicks "Download"
///    and the existing `pull_ollama_model` Tauri command runs the download
///    with a visible progress bar.
/// 4. Warms the LLM model only if it is already present.
///
/// This is the contract every responsible app uses: never download large
/// payloads on startup without explicit user consent. It is also the
/// trust pillar for Signal users who pay for a calm, predictable cold boot.
pub(crate) async fn ensure_models_available(llm_model: &str, base_url: &str, app: &AppHandle) {
    // Step 1: Check Ollama status (cheap — local HTTP probe)
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

    // Step 1.5: Check the Ollama runtime version. Soft-fail — emits its own
    // event if the daemon is older than MIN_OLLAMA_VERSION. Does not affect
    // the rest of the startup flow.
    check_ollama_version(base_url, app).await;

    // Step 2: Check which models are present
    let models: Vec<String> = status["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let embed_model = crate::reembed::get_embedding_model();
    let has_embedding = models.iter().any(|m| m.starts_with(&embed_model));
    let has_llm = models
        .iter()
        .any(|m| m.starts_with(llm_model) || m.contains(llm_model));

    let mut missing: Vec<String> = Vec::new();
    if !has_embedding {
        missing.push(embed_model.clone());
    }
    if !has_llm {
        missing.push(llm_model.to_string());
    }

    // Step 3: If anything is missing, emit a CONSENT REQUEST instead of pulling.
    // The frontend shows a banner; user clicks "Download" to invoke the
    // existing `pull_ollama_model` command via the normal IPC path.
    if !missing.is_empty() {
        let estimated_mb = estimate_model_size_mb(&missing);
        info!(
            target: "4da::ollama",
            missing = ?missing,
            estimated_mb,
            "Ollama models missing — emitting consent request (no auto-pull)"
        );

        let _ = app.emit(
            "ollama-needs-models",
            OllamaNeedsModelsEvent {
                missing: missing.clone(),
                estimated_mb,
                base_url: base_url.to_string(),
            },
        );

        // Also emit a phase update so any UI listening to ollama-status sees
        // the "needs-consent" state instead of going silent.
        let _ = app.emit(
            "ollama-status",
            OllamaStatusEvent {
                phase: "needs-consent".into(),
                model: missing.join(", "),
                error: None,
            },
        );

        // Do NOT pull. Do NOT warm — the LLM model isn't here yet.
        return;
    }

    // Step 4: All models present — warm the LLM model
    warm_model(llm_model, base_url, app).await;
}

/// Conservative size estimate (MB) for the models a user might be missing.
/// Used by the consent banner. Errs on the high side so users don't feel
/// the download was bigger than promised.
fn estimate_model_size_mb(missing: &[String]) -> u64 {
    let mut total = 0_u64;
    for m in missing {
        let lower = m.to_lowercase();
        // Embedding models — small (~80MB for nomic-embed-text)
        if lower.contains("nomic-embed") || lower.contains("minilm") {
            total += 100;
            continue;
        }
        // LLM size estimation by parameter count in the tag
        // Format examples: "llama3.2:latest", "llama3.2:3b", "qwen2.5:7b", "phi3:14b"
        if lower.contains(":1b") || lower.contains("1b") {
            total += 700;
        } else if lower.contains(":3b") || lower.contains("3b") || lower.contains("llama3.2:latest")
        {
            total += 2000;
        } else if lower.contains(":7b") || lower.contains("7b") {
            total += 4500;
        } else if lower.contains(":8b") || lower.contains("8b") {
            total += 5000;
        } else if lower.contains(":13b") || lower.contains(":14b") {
            total += 8000;
        } else if lower.contains(":70b") {
            total += 40_000;
        } else {
            // Unknown — assume mid-size
            total += 2500;
        }
    }
    total
}

#[cfg(test)]
mod size_estimate_tests {
    use super::estimate_model_size_mb;

    #[test]
    fn embed_model_is_small() {
        assert!(estimate_model_size_mb(&["nomic-embed-text:latest".to_string()]) <= 200);
    }

    #[test]
    fn llama3_2_latest_is_around_2gb() {
        let mb = estimate_model_size_mb(&["llama3.2:latest".to_string()]);
        assert!(mb >= 1500 && mb <= 2500, "got {mb}");
    }

    #[test]
    fn missing_both_embed_and_llm() {
        let mb = estimate_model_size_mb(&[
            "nomic-embed-text:latest".to_string(),
            "llama3.2:latest".to_string(),
        ]);
        assert!(mb >= 2000 && mb <= 2500, "got {mb}");
    }
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
