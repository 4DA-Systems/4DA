// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Model capability detection for Ollama-backed LLM inference.
//!
//! Determines whether the user's configured model has enough parameters
//! to produce reliable synthesis, analysis explanations, etc.

use tracing::{info, warn};


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
/// Briefing synthesis is BYOK-only: Anthropic, OpenAI, or OpenAI-compatible
/// with a valid API key. Local models (builtin sidecar, Ollama) are excluded
/// — testing showed they produce hallucinated narratives, contradictory
/// clusters, and confidently wrong advice regardless of model size.
pub(crate) async fn can_synthesize(provider: &crate::settings::LLMProvider) -> bool {
    match provider.provider.as_str() {
        "anthropic" | "openai" => !provider.api_key.is_empty(),
        "openai-compatible" => {
            warn!(
                target: "4da::ollama",
                "openai-compatible provider — synthesis quality is unverified"
            );
            !provider.api_key.is_empty()
        }
        "ollama" | "builtin" => {
            info!(
                target: "4da::ollama",
                provider = %provider.provider,
                "Local models are not used for briefing synthesis — configure a cloud API key"
            );
            false
        }
        _ => false,
    }
}

/// Resolve cloud LLM providers for briefing synthesis (BYOK only).
/// Only Anthropic, OpenAI, and OpenAI-compatible providers with valid
/// API keys qualify. Local models (builtin, Ollama) are never used.
pub(crate) async fn resolve_synthesis_providers(
    configured: &crate::settings::LLMProvider,
) -> Vec<crate::settings::LLMProvider> {
    let mut candidates = Vec::new();

    if can_synthesize(configured).await {
        candidates.push(configured.clone());
    }

    if candidates.is_empty() {
        info!(
            target: "4da::ollama",
            configured_provider = %configured.provider,
            "No cloud API key configured — briefing synthesis disabled"
        );
    }

    candidates
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

/// Synthesis quality tier — only Cloud is used (BYOK providers only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SynthesisTier {
    Cloud,
}

impl SynthesisTier {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
        }
    }
}

/// All synthesis providers are cloud-tier (BYOK only).
pub(crate) async fn synthesis_tier(_provider: &crate::settings::LLMProvider) -> SynthesisTier {
    SynthesisTier::Cloud
}
