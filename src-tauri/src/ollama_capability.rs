// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Model capability detection for Ollama-backed LLM inference.
//!
//! Determines whether the user's configured model has enough parameters
//! to produce reliable synthesis, analysis explanations, etc.

use tracing::{info, warn};

/// Minimum parameter count (in billions) required for reliable synthesis.
///
/// Models below this threshold produce hallucinated narratives, fake
/// connections between unrelated signals, and confidently wrong project
/// assessments. Observed with Llama 3.2 (3B): every prompt variant
/// produced unusable output. The 7B floor is based on testing with
/// Llama 3.1 8B, Qwen 2.5 7B, and Mistral 7B — all produce usable
/// single-signal summaries with the constrained briefing prompt.
pub(crate) const SYNTHESIS_MIN_PARAMS_B: f64 = 7.0;

/// Minimum parameter count for full narrative synthesis.
/// Below this, the briefing uses a structured extraction prompt instead of
/// asking the model to write analyst-quality prose. The 32B floor is based
/// on testing: Qwen 2.5 32B and Llama 3.1 33B produce usable narratives;
/// 14B models produce correct-but-pedestrian summaries with banned phrases.
pub(crate) const NARRATIVE_MIN_PARAMS_B: f64 = 32.0;

/// Minimum parameter count for high-quality analysis explanations.
/// Below this, LLM "Why this matters" text is verbose, hedgy, and
/// sometimes hallucinates article content. The frontend hides the
/// explanation box when the model is below this tier and shows the
/// pipeline's deterministic explanation instead.
pub(crate) const ANALYSIS_QUALITY_MIN_PARAMS_B: f64 = 14.0;

/// Query Ollama's `/api/show` endpoint for a model's parameter count.
///
/// Returns the parameter count in billions (e.g., 3.2 for llama3.2,
/// 8.0 for llama3.1:8b). Returns `None` if Ollama is unreachable,
/// the model isn't found, or the response doesn't contain param info.
pub(crate) async fn get_model_params_billions(model: &str, base_url: &str) -> Option<f64> {
    let url = format!("{}/api/show", base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let body = serde_json::json!({ "name": model });

    let response = client.post(&url).json(&body).send().await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let data: serde_json::Value = response.json().await.ok()?;

    // Primary: details.parameter_size (e.g., "3.2B", "8B", "70B")
    if let Some(size_str) = data
        .get("details")
        .and_then(|d| d.get("parameter_size"))
        .and_then(|v| v.as_str())
    {
        if let Some(billions) = parse_param_size(size_str) {
            return Some(billions);
        }
    }

    // Fallback: model_info.general.parameter_count (raw integer)
    if let Some(count) = data
        .get("model_info")
        .and_then(|mi| mi.get("general.parameter_count"))
        .and_then(|v| v.as_u64())
    {
        return Some(count as f64 / 1_000_000_000.0);
    }

    None
}

/// Parse Ollama's parameter_size string into billions.
/// Handles: "3.2B", "8B", "70B", "0.5B", "137M", "1.5B"
pub(crate) fn parse_param_size(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(num_str) = s.strip_suffix('B') {
        num_str.trim().parse::<f64>().ok()
    } else if let Some(num_str) = s.strip_suffix('M') {
        num_str.trim().parse::<f64>().ok().map(|m| m / 1000.0)
    } else {
        None
    }
}

/// Check whether the user's configured LLM can reliably synthesize
/// a morning intelligence brief.
///
/// Cloud APIs (Anthropic, OpenAI) always return true — all current
/// cloud models exceed the synthesis capability floor.
///
/// Ollama models must have ≥7B parameters. Below that, free-form
/// generation produces hallucinated narratives and confidently wrong
/// advice. The briefing skips synthesis entirely for small models
/// and shows ranked signals without a narrative — accurate over impressive.
pub(crate) async fn can_synthesize(provider: &crate::settings::LLMProvider) -> bool {
    match provider.provider.as_str() {
        "anthropic" | "openai" => {
            // Cloud models all exceed synthesis floor.
            // Still need a valid API key.
            !provider.api_key.is_empty()
        }
        "openai-compatible" => {
            warn!(
                target: "4da::ollama",
                "openai-compatible provider — synthesis quality is unverified"
            );
            !provider.api_key.is_empty()
        }
        "ollama" => {
            let base_url = provider
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            if provider.model.is_empty() {
                warn!(
                    target: "4da::ollama",
                    "No Ollama model configured — skipping synthesis"
                );
                return false;
            }
            let model = &provider.model;

            match get_model_params_billions(model, base_url).await {
                Some(params) => {
                    let capable = params >= SYNTHESIS_MIN_PARAMS_B;
                    if !capable {
                        info!(
                            target: "4da::ollama",
                            model,
                            params_b = params,
                            min_b = SYNTHESIS_MIN_PARAMS_B,
                            "Model below synthesis threshold — briefing will show ranked signals only"
                        );
                    }
                    capable
                }
                None => {
                    // Can't determine model size — conservative: skip synthesis.
                    // A wrong synthesis is worse than no synthesis.
                    warn!(
                        target: "4da::ollama",
                        model,
                        "Could not determine model parameters — skipping synthesis"
                    );
                    false
                }
            }
        }
        "builtin" => {
            // Built-in sidecar uses verified models from the allowlist.
            // If the sidecar is running, the model has already been vetted.
            crate::llm_engine::sidecar_status() == crate::llm_engine::SidecarStatus::Ready
        }
        _ => false,
    }
}

/// Resolve all available LLM providers for synthesis, in priority order.
/// Returns the configured provider first (if it passes the capability check),
/// then builtin sidecar, then Ollama. The caller should try each in order,
/// falling through on auth/connection failures.
pub(crate) async fn resolve_synthesis_providers(
    configured: &crate::settings::LLMProvider,
) -> Vec<crate::settings::LLMProvider> {
    let mut candidates = Vec::new();

    // 1. Configured provider (if it passes the basic capability check)
    if can_synthesize(configured).await {
        candidates.push(configured.clone());
    }

    // 2. Builtin sidecar (skip if configured is already builtin)
    if configured.provider != "builtin"
        && crate::llm_engine::sidecar_status() == crate::llm_engine::SidecarStatus::Ready
    {
        candidates.push(crate::settings::LLMProvider {
            provider: "builtin".to_string(),
            api_key: String::new(),
            model: String::new(),
            base_url: None,
            openai_api_key: String::new(),
            embedding_model: configured.embedding_model.clone(),
        });
    }

    // 3. Ollama (skip if configured is already ollama)
    if configured.provider != "ollama" {
        let ollama_url = configured
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        if let Some(model) = find_capable_ollama_model(ollama_url).await {
            candidates.push(crate::settings::LLMProvider {
                provider: "ollama".to_string(),
                api_key: String::new(),
                model,
                base_url: Some(ollama_url.to_string()),
                openai_api_key: String::new(),
                embedding_model: configured.embedding_model.clone(),
            });
        }
    }

    if candidates.is_empty() {
        info!(
            target: "4da::ollama",
            configured_provider = %configured.provider,
            "No synthesis-capable provider available — briefing will show signals only"
        );
    }

    candidates
}

/// Probe Ollama for the first model that meets the synthesis parameter floor.
async fn find_capable_ollama_model(base_url: &str) -> Option<String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?
        .get(&url)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct TagsResponse {
        models: Option<Vec<TagModel>>,
    }
    #[derive(serde::Deserialize)]
    struct TagModel {
        name: String,
    }

    let tags: TagsResponse = resp.json().await.ok()?;
    let models = tags.models?;

    for m in &models {
        if let Some(params) = get_model_params_billions(&m.name, base_url).await {
            if params >= SYNTHESIS_MIN_PARAMS_B {
                return Some(m.name.clone());
            }
        }
    }
    None
}

/// Check whether the configured LLM produces reliable analysis text.
/// Cloud APIs always qualify. Local models need 14B+ parameters for
/// explanations that a senior developer wouldn't laugh at.
pub(crate) async fn can_explain(provider: &crate::settings::LLMProvider) -> bool {
    match provider.provider.as_str() {
        "anthropic" | "openai" => !provider.api_key.is_empty(),
        "openai-compatible" => {
            warn!(
                target: "4da::ollama",
                "openai-compatible provider — explanation quality is unverified"
            );
            !provider.api_key.is_empty()
        }
        "ollama" => {
            let base_url = provider
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            if provider.model.is_empty() {
                warn!(
                    target: "4da::ollama",
                    "No Ollama model configured — skipping explanation"
                );
                return false;
            }
            let model = &provider.model;
            match get_model_params_billions(model, base_url).await {
                Some(params) => params >= ANALYSIS_QUALITY_MIN_PARAMS_B,
                None => false,
            }
        }
        "builtin" => crate::llm_engine::sidecar_status() == crate::llm_engine::SidecarStatus::Ready,
        _ => false,
    }
}

/// Synthesis quality tier — determines which prompt strategy to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SynthesisTier {
    /// Cloud APIs or large local models (70B+) — full narrative prompt.
    Cloud,
    /// Local models 32B+ — can attempt narrative prose.
    LocalNarrative,
    /// Local models 7B-31B — structured extraction only.
    LocalStructured,
}

impl SynthesisTier {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::LocalNarrative => "local-narrative",
            Self::LocalStructured => "local-structured",
        }
    }
}

/// Determine the synthesis quality tier for a given provider.
pub(crate) async fn synthesis_tier(provider: &crate::settings::LLMProvider) -> SynthesisTier {
    match provider.provider.as_str() {
        "anthropic" | "openai" | "openai-compatible" => SynthesisTier::Cloud,
        "builtin" => SynthesisTier::Cloud,
        "ollama" => {
            let base_url = provider
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            if provider.model.is_empty() {
                return SynthesisTier::LocalStructured;
            }
            match get_model_params_billions(&provider.model, base_url).await {
                Some(params) if params >= NARRATIVE_MIN_PARAMS_B => SynthesisTier::LocalNarrative,
                _ => SynthesisTier::LocalStructured,
            }
        }
        _ => SynthesisTier::LocalStructured,
    }
}
