// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM capability tier detection — probes models on first use to determine
//! what intelligence features they can reliably support.
//!
//! Tiers:
//!   Full  — Cloud-class models (Claude, GPT-4o): all features enabled
//!   Good  — Medium local models (70B+, Mixtral): most features, analysis text may vary
//!   Basic — Small local models (<14B): pipeline scoring only, heuristic explanations
//!
//! The probe sends a known prompt and checks:
//!   1. Can the model return valid JSON?
//!   2. Does it follow the scoring rubric?
//!   3. Can it produce grounded reasoning (not generic filler)?

use crate::error::Result;
use crate::llm::{LLMClient, Message};
use crate::settings::LLMProvider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, info, warn};
use ts_rs::TS;

// Cache of probed model tiers, keyed by model identity string
static TIER_CACHE: std::sync::LazyLock<Mutex<HashMap<String, ModelTier>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Capability tier for an LLM model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    /// Cloud-class: all features including adversarial deliberation, LLM reranking,
    /// full analysis text generation, briefing synthesis.
    Full,
    /// Medium capability: scoring works, analysis text enabled but may be less specific,
    /// adversarial deliberation enabled.
    Good,
    /// Small/weak model: pipeline scoring only. LLM reranking disabled.
    /// Heuristic explanations from scoring/explanation.rs shown instead of LLM text.
    /// Adversarial deliberation disabled. Briefing uses template, not LLM synthesis.
    Basic,
}

impl ModelTier {
    pub fn supports_reranking(self) -> bool {
        matches!(self, ModelTier::Full | ModelTier::Good)
    }

    pub fn supports_adversarial(self) -> bool {
        matches!(self, ModelTier::Full | ModelTier::Good)
    }

    pub fn supports_llm_explanations(self) -> bool {
        matches!(self, ModelTier::Full | ModelTier::Good)
    }

    pub fn supports_briefing_synthesis(self) -> bool {
        matches!(self, ModelTier::Full | ModelTier::Good)
    }

    pub fn label(self) -> &'static str {
        match self {
            ModelTier::Full => "full",
            ModelTier::Good => "good",
            ModelTier::Basic => "basic",
        }
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Known model tiers for popular models (skip probe for these).
const KNOWN_TIERS: &[(&str, ModelTier)] = &[
    // Anthropic — all Full
    ("claude-opus", ModelTier::Full),
    ("claude-sonnet", ModelTier::Full),
    ("claude-haiku", ModelTier::Full),
    // OpenAI — all Full
    ("gpt-4o", ModelTier::Full),
    ("gpt-4.1", ModelTier::Full),
    ("gpt-4-turbo", ModelTier::Full),
    ("gpt-4o-mini", ModelTier::Full),
    ("gpt-4.1-mini", ModelTier::Full),
    ("gpt-4.1-nano", ModelTier::Good),
    // Large local models — Good
    ("llama3.1:70b", ModelTier::Good),
    ("llama3.3:70b", ModelTier::Good),
    ("llama-3.1-70b", ModelTier::Good),
    ("mixtral", ModelTier::Good),
    ("qwen2.5:72b", ModelTier::Good),
    ("qwen2.5:32b", ModelTier::Good),
    ("deepseek-r1:70b", ModelTier::Good),
    ("command-r-plus", ModelTier::Good),
    ("gemma2:27b", ModelTier::Good),
    ("phi-3-medium", ModelTier::Good),
    ("phi3:14b", ModelTier::Good),
    // Small local models — Basic
    ("llama3.2", ModelTier::Basic), // 3B default
    ("llama3.2:3b", ModelTier::Basic),
    ("llama3.2:1b", ModelTier::Basic),
    ("llama3.1:8b", ModelTier::Basic),
    ("phi3:mini", ModelTier::Basic),
    ("phi3:3.8b", ModelTier::Basic),
    ("gemma2:2b", ModelTier::Basic),
    ("gemma2:9b", ModelTier::Basic),
    ("qwen2.5:7b", ModelTier::Basic),
    ("qwen2.5:3b", ModelTier::Basic),
    ("qwen2.5:1.5b", ModelTier::Basic),
    ("tinyllama", ModelTier::Basic),
    ("phi", ModelTier::Basic),
    ("mistral:7b", ModelTier::Basic),
    ("mistral-7b", ModelTier::Basic),
];

/// Build a model identity string for cache keying.
fn model_identity(settings: &LLMProvider) -> String {
    format!(
        "{}:{}:{}",
        settings.provider,
        settings.model,
        settings.base_url.as_deref().unwrap_or("")
    )
}

/// Look up a known tier by model name prefix matching.
fn lookup_known_tier(model: &str) -> Option<ModelTier> {
    let lower = model.to_lowercase();
    KNOWN_TIERS
        .iter()
        .find(|(prefix, _)| lower.starts_with(&prefix.to_lowercase()))
        .map(|(_, tier)| *tier)
}

/// Get the capability tier for the current LLM model.
///
/// Returns cached result if available, otherwise checks known-model table.
/// Does NOT run a probe automatically — call `probe_model_capability()` for that.
pub(crate) fn get_model_tier(settings: &LLMProvider) -> ModelTier {
    let id = model_identity(settings);

    // Check cache first
    if let Ok(cache) = TIER_CACHE.lock() {
        if let Some(tier) = cache.get(&id) {
            return *tier;
        }
    }

    // Check known models
    if let Some(tier) = lookup_known_tier(&settings.model) {
        if let Ok(mut cache) = TIER_CACHE.lock() {
            cache.insert(id, tier);
        }
        return tier;
    }

    // Provider-based inference for unknown models
    match settings.provider.as_str() {
        "anthropic" => ModelTier::Full,
        "openai" => ModelTier::Full,
        "ollama" => ModelTier::Basic, // Conservative default for unknown Ollama models
        _ => ModelTier::Good,         // OpenAI-compatible could be anything, assume Good
    }
}

/// Stricter than `ModelTier::Full`: is this model genuinely capable of a coherent,
/// valuable morning-brief NARRATION? The brief is the headline surface and runs once a
/// day, so it demands a real reasoning/writing model — not a cheap/small variant.
///
/// Cloud Sonnet/Opus/GPT-4-class (and equivalents) and large local models (Good/Full
/// tier, e.g. 70B) qualify. Haiku, `*-mini`, `*-nano`, and small consumer-hardware local
/// models (7–14B, Basic tier) do NOT — for those the brief falls back to the
/// deterministic, grounded brief (which needs no LLM and cannot hallucinate) rather than
/// faking synthesis with a model too weak to do it well. Tested finding: consumer-hardware
/// local LLMs produce incoherent briefs; the deterministic floor serves them honestly.
pub(crate) fn is_brief_capable(settings: &LLMProvider) -> bool {
    let model = settings.model.to_lowercase();

    // Small/cheap variants can't carry a genuine brief — exclude regardless of provider,
    // even when the name also contains a capable family (e.g. "gpt-4o-mini" → excluded
    // because "mini" matches first).
    const SMALL: &[&str] = &[
        "haiku",
        "mini",
        "nano",
        "flash-lite",
        "-8b",
        "-7b",
        "-3b",
        "-1b",
    ];
    if SMALL.iter().any(|s| model.contains(s)) {
        return false;
    }

    let is_cloud = matches!(
        settings.provider.as_str(),
        "anthropic" | "openai" | "openai-compatible" | "openrouter"
    );
    if is_cloud {
        // Genuine cloud reasoning/writing models (Sonnet-class and equivalents).
        const BRIEF_GRADE: &[&str] = &[
            "sonnet",
            "opus",
            "gpt-4o",
            "gpt-4.1",
            "gpt-4-turbo",
            "gpt-5",
            "o1",
            "o3",
            "mistral-large",
            "command-r-plus",
            "deepseek-chat",
            "deepseek-r1",
            "gemini-1.5-pro",
            "gemini-2.0-pro",
            "gemini-2.5-pro",
            "qwen-max",
        ];
        BRIEF_GRADE.iter().any(|s| model.contains(s))
    } else {
        // Local: only large models clear the bar (Good/Full tier — 70B-class). Consumer
        // 7–14B (Basic, or unknown Ollama defaulting to Basic) fall back to the floor.
        matches!(get_model_tier(settings), ModelTier::Full | ModelTier::Good)
    }
}

/// Why the morning brief will (or won't) be AI-narrated. Drives the Settings/onboarding
/// hint copy so the user understands what they're getting — and how to upgrade it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
#[serde(rename_all = "snake_case")]
pub enum BriefNarrationReason {
    /// No usable LLM configured → the deterministic, grounded brief is served.
    NoLlm,
    /// An LLM is configured but too weak for genuine narration (Haiku / *-mini / *-nano /
    /// consumer-hardware local) → deterministic brief, never faked synthesis.
    ModelTooWeak,
    /// A Sonnet-class+ model is configured → the brief is AI-narrated.
    Capable,
}

/// Whether the configured model can NARRATE the morning brief, and why. Mirrors the exact
/// gate in `digest_commands` (`compute_has_llm` AND `is_brief_capable`) so the Settings hint
/// always tells the truth about what the next brief will actually do.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BriefCapability {
    /// True → the brief is AI-narrated. False → the deterministic grounded floor is served
    /// (always available, works offline, cannot hallucinate).
    pub brief_capable: bool,
    /// Classification for the UI to pick honest copy.
    pub reason: BriefNarrationReason,
    /// The configured provider (e.g. "anthropic", "ollama", "none").
    pub provider: String,
    /// The configured model id (empty when no provider is set).
    pub model: String,
}

/// Compute brief-narration capability for a provider config. Separate from the Tauri
/// command so it is unit-testable without a settings manager. The verdict is IDENTICAL to
/// the one `digest_commands` uses to choose the brief path — keep them in lockstep.
pub(crate) fn compute_brief_capability(settings: &LLMProvider) -> BriefCapability {
    let has_llm = crate::content_personalization::context::compute_has_llm(
        &settings.provider,
        &settings.api_key,
    );
    let brief_capable = has_llm && is_brief_capable(settings);
    let reason = if !has_llm {
        BriefNarrationReason::NoLlm
    } else if !brief_capable {
        BriefNarrationReason::ModelTooWeak
    } else {
        BriefNarrationReason::Capable
    };
    BriefCapability {
        brief_capable,
        reason,
        provider: settings.provider.clone(),
        model: settings.model.clone(),
    }
}

/// Probe the model's capability by sending a test prompt.
///
/// This is expensive (1 API call) so should only be called:
/// - On first use of a new model
/// - When user explicitly requests a re-probe
/// - NOT on every analysis run
pub(crate) async fn probe_model_capability(settings: &LLMProvider) -> Result<ModelTier> {
    let id = model_identity(settings);

    // Skip probe for known models
    if let Some(tier) = lookup_known_tier(&settings.model) {
        if let Ok(mut cache) = TIER_CACHE.lock() {
            cache.insert(id, tier);
        }
        return Ok(tier);
    }

    info!(
        provider = &settings.provider,
        model = &settings.model,
        "Probing LLM capability tier"
    );

    let client = LLMClient::new(settings.clone());
    let system =
        "You are a JSON-only assistant. Respond with ONLY valid JSON, no markdown, no explanation.";

    let probe_prompt = concat!(
        "Rate this article for a Rust/React developer. Return ONLY JSON:\n",
        r#"[{"id": "1", "score": 4, "reason": "one sentence explaining relevance"}]"#,
        "\n\n",
        "Article: \"CVE-2026-1234: Critical memory safety vulnerability discovered in tokio 1.38. ",
        "The vulnerability allows use-after-free via crafted async task cancellation. ",
        "All users of tokio < 1.38.1 should upgrade immediately.\"\n",
    );

    let messages = vec![Message {
        role: "user".to_string(),
        content: probe_prompt.to_string(),
    }];

    let response = match client.complete_for_translation(system, messages).await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Capability probe failed, defaulting to Basic");
            let tier = ModelTier::Basic;
            if let Ok(mut cache) = TIER_CACHE.lock() {
                cache.insert(id, tier);
            }
            return Ok(tier);
        }
    };

    let tier = evaluate_probe_response(&response.content);

    info!(
        provider = &settings.provider,
        model = &settings.model,
        tier = tier.label(),
        "LLM capability tier determined"
    );

    if let Ok(mut cache) = TIER_CACHE.lock() {
        cache.insert(id, tier);
    }

    Ok(tier)
}

/// Evaluate the probe response to determine capability tier.
fn evaluate_probe_response(response: &str) -> ModelTier {
    let mut score = 0u8;

    // Test 1: Can it produce valid JSON?
    let trimmed = response.trim();
    let json_str = if trimmed.starts_with("```") {
        // Strip markdown code fences
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(json_str);
    match parsed {
        Ok(val) => {
            score += 1; // Valid JSON

            // Test 2: Does it follow the expected structure?
            if let Some(arr) = val.as_array() {
                if let Some(first) = arr.first() {
                    if first.get("id").is_some() && first.get("score").is_some() {
                        score += 1; // Correct structure
                    }

                    // Test 3: Is the score reasonable? (tokio CVE should be 4 or 5)
                    if let Some(s) = first.get("score").and_then(|v| v.as_f64()) {
                        if (4.0..=5.0).contains(&s) {
                            score += 1; // Correct scoring judgment
                        }
                    }

                    // Test 4: Is the reasoning grounded? (mentions specific details)
                    if let Some(reason) = first
                        .get("reason")
                        .or_else(|| first.get("reasoning"))
                        .and_then(|v| v.as_str())
                    {
                        let lower = reason.to_lowercase();
                        let has_specifics = lower.contains("tokio")
                            || lower.contains("memory")
                            || lower.contains("cve")
                            || lower.contains("async")
                            || lower.contains("upgrade")
                            || lower.contains("vulnerability")
                            || lower.contains("security");
                        if has_specifics && reason.len() >= 20 {
                            score += 1; // Grounded reasoning
                        }
                    }
                }
            }
        }
        Err(_) => {
            debug!(
                response_preview = &response[..response.floor_char_boundary(100)],
                "Probe response was not valid JSON"
            );
        }
    }

    match score {
        4 => ModelTier::Full,
        2..=3 => ModelTier::Good,
        _ => ModelTier::Basic,
    }
}

/// Tauri command to get the current model's capability tier.
#[tauri::command]
pub async fn get_llm_capability_tier() -> Result<serde_json::Value> {
    let settings = {
        let manager = crate::get_settings_manager();
        let guard = manager.lock();
        guard.get().llm.clone()
    };

    let tier = get_model_tier(&settings);

    Ok(serde_json::json!({
        "tier": tier.label(),
        "supports_reranking": tier.supports_reranking(),
        "supports_adversarial": tier.supports_adversarial(),
        "supports_llm_explanations": tier.supports_llm_explanations(),
        "supports_briefing_synthesis": tier.supports_briefing_synthesis(),
        "provider": settings.provider,
        "model": settings.model,
    }))
}

/// Tauri command to run a capability probe on the current model.
#[tauri::command]
pub async fn probe_llm_capability() -> Result<serde_json::Value> {
    let settings = {
        let manager = crate::get_settings_manager();
        let guard = manager.lock();
        guard.get().llm.clone()
    };

    let tier = probe_model_capability(&settings).await?;

    Ok(serde_json::json!({
        "tier": tier.label(),
        "supports_reranking": tier.supports_reranking(),
        "supports_adversarial": tier.supports_adversarial(),
        "supports_llm_explanations": tier.supports_llm_explanations(),
        "supports_briefing_synthesis": tier.supports_briefing_synthesis(),
        "probed": true,
    }))
}

/// Tauri command: will the configured model narrate the morning brief, and if not, why?
///
/// Read-only. Surfaced in Settings/onboarding so the user knows whether they'll get an
/// AI-narrated brief or the deterministic grounded floor — and what to change to upgrade.
/// Hydrates keys first so the `has_llm` check sees the real API key, matching exactly what
/// the brief itself evaluates.
#[tauri::command]
pub fn get_brief_capability() -> Result<BriefCapability> {
    let settings = {
        let manager = crate::get_settings_manager();
        let mut guard = manager.lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };
    Ok(compute_brief_capability(&settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_tier_claude() {
        assert_eq!(
            lookup_known_tier("claude-haiku-4-5-20251001"),
            Some(ModelTier::Full)
        );
        assert_eq!(
            lookup_known_tier("claude-sonnet-4-6"),
            Some(ModelTier::Full)
        );
    }

    #[test]
    fn test_known_tier_gpt() {
        assert_eq!(lookup_known_tier("gpt-4o-mini"), Some(ModelTier::Full));
        assert_eq!(lookup_known_tier("gpt-4o"), Some(ModelTier::Full));
    }

    #[test]
    fn test_known_tier_ollama_large() {
        assert_eq!(lookup_known_tier("llama3.1:70b"), Some(ModelTier::Good));
        assert_eq!(lookup_known_tier("mixtral"), Some(ModelTier::Good));
    }

    #[test]
    fn test_known_tier_ollama_small() {
        assert_eq!(lookup_known_tier("llama3.2"), Some(ModelTier::Basic));
        assert_eq!(lookup_known_tier("llama3.2:3b"), Some(ModelTier::Basic));
    }

    #[test]
    fn test_known_tier_unknown() {
        assert_eq!(lookup_known_tier("some-random-model"), None);
    }

    #[test]
    fn test_probe_response_perfect() {
        let response = r#"[{"id": "1", "score": 5, "reason": "Critical tokio memory safety CVE requires immediate upgrade for all Rust async projects"}]"#;
        assert_eq!(evaluate_probe_response(response), ModelTier::Full);
    }

    #[test]
    fn test_probe_response_with_markdown() {
        let response = "```json\n[{\"id\": \"1\", \"score\": 4, \"reason\": \"Security vulnerability in tokio affects async Rust\"}]\n```";
        assert_eq!(evaluate_probe_response(response), ModelTier::Full);
    }

    #[test]
    fn test_probe_response_wrong_score() {
        let response = r#"[{"id": "1", "score": 2, "reason": "Generic security update"}]"#;
        // Valid JSON, correct structure, wrong score, non-grounded reason
        assert_eq!(evaluate_probe_response(response), ModelTier::Good);
    }

    #[test]
    fn test_probe_response_invalid_json() {
        let response =
            "I'd rate this article 5/5 because it's about a critical security vulnerability.";
        assert_eq!(evaluate_probe_response(response), ModelTier::Basic);
    }

    #[test]
    fn test_probe_response_empty() {
        assert_eq!(evaluate_probe_response(""), ModelTier::Basic);
    }

    #[test]
    fn test_model_tier_feature_gating() {
        assert!(ModelTier::Full.supports_reranking());
        assert!(ModelTier::Full.supports_adversarial());
        assert!(ModelTier::Full.supports_llm_explanations());
        assert!(ModelTier::Good.supports_reranking());
        assert!(!ModelTier::Basic.supports_reranking());
        assert!(!ModelTier::Basic.supports_adversarial());
        assert!(!ModelTier::Basic.supports_llm_explanations());
    }

    // ── Brief-narration capability (the Settings hint source of truth) ──
    // Must stay in lockstep with the digest_commands gate: has_llm (compute_has_llm)
    // AND is_brief_capable. These cases pin the reason classification the UI renders.

    // LLMProvider impls Drop (zeroizes the key), so functional struct update
    // (`..Default::default()`) is illegal — mutate a default instead (clippy would
    // otherwise suggest the FRU form that won't compile here).
    #[allow(clippy::field_reassign_with_default)]
    fn provider(p: &str, key: &str, model: &str) -> LLMProvider {
        let mut s = LLMProvider::default();
        s.provider = p.to_string();
        s.api_key = key.to_string();
        s.model = model.to_string();
        s
    }

    #[test]
    fn brief_capability_no_provider_is_no_llm() {
        let cap = compute_brief_capability(&provider("none", "", ""));
        assert!(!cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::NoLlm);
    }

    #[test]
    fn brief_capability_cloud_without_key_is_no_llm() {
        // compute_has_llm requires a key for cloud providers, so a keyless Anthropic
        // config has no usable LLM at all — not "model too weak".
        let cap = compute_brief_capability(&provider("anthropic", "", "claude-sonnet-4-6"));
        assert!(!cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::NoLlm);
    }

    #[test]
    fn brief_capability_sonnet_is_capable() {
        let cap =
            compute_brief_capability(&provider("anthropic", "sk-ant-real", "claude-sonnet-4-6"));
        assert!(cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::Capable);
        assert_eq!(cap.provider, "anthropic");
        assert_eq!(cap.model, "claude-sonnet-4-6");
    }

    #[test]
    fn brief_capability_haiku_is_too_weak() {
        // Has a usable LLM, but Haiku is below the brief bar → deterministic floor.
        let cap =
            compute_brief_capability(&provider("anthropic", "sk-ant-real", "claude-haiku-4-5"));
        assert!(!cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::ModelTooWeak);
    }

    #[test]
    fn brief_capability_small_local_is_too_weak() {
        // Ollama is keyless (has_llm true) but a 3B model is Basic tier → too weak.
        let cap = compute_brief_capability(&provider("ollama", "", "llama3.2:3b"));
        assert!(!cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::ModelTooWeak);
    }

    #[test]
    fn brief_capability_large_local_is_capable() {
        let cap = compute_brief_capability(&provider("ollama", "", "llama3.1:70b"));
        assert!(cap.brief_capable);
        assert_eq!(cap.reason, BriefNarrationReason::Capable);
    }
}
