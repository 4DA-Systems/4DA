// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Self-updating LLM model registry.
//!
//! Single source of truth for model names, pricing, and capabilities.
//! Three-layer design: bundled defaults → disk cache → in-memory singleton.
//! Refreshes from LiteLLM's community-maintained registry (≤1x/24h).

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

/// Information about a single LLM model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    /// Cost per token for input (USD). None if unknown.
    pub input_cost_per_token: Option<f64>,
    /// Cost per token for output (USD). None if unknown.
    pub output_cost_per_token: Option<f64>,
    /// Maximum input context window.
    pub max_input_tokens: Option<u64>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<u64>,
}

/// The full model registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    /// When the registry was last fetched/updated (Unix timestamp).
    pub fetched_at: u64,
    /// Source of the data: "bundled" or "litellm".
    pub source: String,
    /// Models keyed by model ID.
    pub models: HashMap<String, ModelInfo>,
}

// ============================================================================
// Singleton
// ============================================================================

static REGISTRY: LazyLock<RwLock<ModelRegistry>> = LazyLock::new(|| {
    // Try loading from disk cache first, fall back to bundled
    let registry = load_from_disk().unwrap_or_else(bundled_registry);
    RwLock::new(registry)
});

/// Get a reference to the global registry lock.
pub fn get_registry() -> &'static RwLock<ModelRegistry> {
    &REGISTRY
}

// ============================================================================
// Bundled Defaults
// ============================================================================

/// Returns a hardcoded registry of current models. Always works offline.
pub fn bundled_registry() -> ModelRegistry {
    let mut models = HashMap::new();

    // --- Anthropic ---
    let anthropic_models = [
        (
            "claude-haiku-4-5-20251001",
            "Claude Haiku 4.5",
            0.80,
            4.00,
            200_000,
            8_192,
        ),
        (
            "claude-sonnet-4-20250514",
            "Claude Sonnet 4",
            3.00,
            15.00,
            200_000,
            8_192,
        ),
        (
            "claude-sonnet-4-6",
            "Claude Sonnet 4.6",
            3.00,
            15.00,
            200_000,
            8_192,
        ),
        (
            "claude-opus-4-20250514",
            "Claude Opus 4",
            15.00,
            75.00,
            200_000,
            32_000,
        ),
        (
            "claude-opus-4-6",
            "Claude Opus 4.6",
            15.00,
            75.00,
            200_000,
            32_000,
        ),
    ];
    for (id, name, input_price, output_price, max_in, max_out) in anthropic_models {
        models.insert(
            id.to_string(),
            ModelInfo {
                id: id.to_string(),
                provider: "anthropic".to_string(),
                display_name: name.to_string(),
                input_cost_per_token: Some(input_price / 1_000_000.0),
                output_cost_per_token: Some(output_price / 1_000_000.0),
                max_input_tokens: Some(max_in),
                max_output_tokens: Some(max_out),
            },
        );
    }

    // --- OpenAI ---
    let openai_models = [
        (
            "gpt-4.1-nano",
            "GPT-4.1 Nano",
            0.10,
            0.40,
            1_047_576,
            32_768,
        ),
        (
            "gpt-4.1-mini",
            "GPT-4.1 Mini",
            0.40,
            1.60,
            1_047_576,
            32_768,
        ),
        ("gpt-4.1", "GPT-4.1", 2.00, 8.00, 1_047_576, 32_768),
        ("gpt-4o-mini", "GPT-4o Mini", 0.15, 0.60, 128_000, 16_384),
        ("gpt-4o", "GPT-4o", 2.50, 10.00, 128_000, 16_384),
    ];
    for (id, name, input_price, output_price, max_in, max_out) in openai_models {
        models.insert(
            id.to_string(),
            ModelInfo {
                id: id.to_string(),
                provider: "openai".to_string(),
                display_name: name.to_string(),
                input_cost_per_token: Some(input_price / 1_000_000.0),
                output_cost_per_token: Some(output_price / 1_000_000.0),
                max_input_tokens: Some(max_in),
                max_output_tokens: Some(max_out),
            },
        );
    }

    ModelRegistry {
        fetched_at: 0,
        source: "bundled".to_string(),
        models,
    }
}

// ============================================================================
// Disk Cache
// ============================================================================

/// Path to the on-disk registry cache.
fn cache_path() -> std::path::PathBuf {
    crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join("model_registry.json")
}

/// Load the registry from the disk cache. Returns None if missing or corrupt.
fn load_from_disk() -> Option<ModelRegistry> {
    let path = cache_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let registry: ModelRegistry = serde_json::from_str(&content).ok()?;
    debug!(target: "4da::registry", source = %registry.source, models = registry.models.len(), "Loaded model registry from disk cache");
    Some(registry)
}

/// Save the registry to disk.
fn save_to_disk(registry: &ModelRegistry) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(registry) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!(target: "4da::registry", error = %e, "Failed to write model registry cache");
            }
        }
        Err(e) => {
            warn!(target: "4da::registry", error = %e, "Failed to serialize model registry");
        }
    }
}

// ============================================================================
// Lookup Functions
// ============================================================================

/// Look up a model by ID. Uses fuzzy matching: exact > starts-with > contains.
/// When multiple models match in fuzzy tiers, returns the shortest key (most specific).
pub fn get_model_info(model_id: &str) -> Option<ModelInfo> {
    let registry = REGISTRY.read();
    let id_lower = model_id.to_lowercase();

    // Exact match
    if let Some(info) = registry.models.get(model_id) {
        return Some(info.clone());
    }

    // Case-insensitive exact
    for (key, info) in &registry.models {
        if key.to_lowercase() == id_lower {
            return Some(info.clone());
        }
    }

    // Starts-with (e.g., "claude-sonnet-4" matches "claude-sonnet-4-20250514")
    // Pick shortest key to avoid non-deterministic HashMap ordering
    let mut best: Option<(&str, &ModelInfo)> = None;
    for (key, info) in &registry.models {
        if key.to_lowercase().starts_with(&id_lower) && best.is_none_or(|b| key.len() < b.0.len()) {
            best = Some((key.as_str(), info));
        }
    }
    if let Some((_, info)) = best {
        return Some(info.clone());
    }

    // Contains (e.g., "sonnet" matches "claude-sonnet-4-20250514")
    // Pick shortest key for determinism
    let mut best: Option<(&str, &ModelInfo)> = None;
    for (key, info) in &registry.models {
        if key.to_lowercase().contains(&id_lower) && best.is_none_or(|b| key.len() < b.0.len()) {
            best = Some((key.as_str(), info));
        }
    }
    if let Some((_, info)) = best {
        return Some(info.clone());
    }

    None
}

/// Get curated model list for a provider (for UI dropdowns).
pub fn get_provider_models(provider: &str) -> Vec<String> {
    let registry = REGISTRY.read();

    let mut models: Vec<String> = registry
        .models
        .values()
        .filter(|m| m.provider == provider)
        .map(|m| m.id.clone())
        .collect();

    models.sort();
    models
}

/// Estimate cost in cents. Returns None for unknown models/providers.
pub fn estimate_cost(
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Option<u64> {
    // Ollama is always free
    if provider == "ollama" {
        return Some(0);
    }

    // openai-compatible has unknown pricing
    if provider == "openai-compatible" {
        return None;
    }

    let info = get_model_info(model)?;

    // Validate provider matches to prevent cross-provider pricing
    if info.provider != provider {
        return None;
    }

    let input_cost_per_token = info.input_cost_per_token?;
    let output_cost_per_token = info.output_cost_per_token?;

    // Convert USD to cents, clamp to non-negative
    let input_cost = input_tokens as f64 * input_cost_per_token * 100.0;
    let output_cost = output_tokens as f64 * output_cost_per_token * 100.0;

    Some((input_cost + output_cost).max(0.0).round() as u64)
}

/// Estimate cost in cents, returning 0 for unknown (backward compat).
pub fn estimate_cost_or_zero(
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> u64 {
    estimate_cost(provider, model, input_tokens, output_tokens).unwrap_or(0)
}

/// Get the full registry snapshot (for frontend consumption).
pub fn get_registry_snapshot() -> ModelRegistry {
    REGISTRY.read().clone()
}

// ============================================================================
// Refresh from LiteLLM
// ============================================================================

/// LiteLLM raw entry shape.
#[derive(Debug, Deserialize)]
struct LiteLLMEntry {
    #[serde(default)]
    litellm_provider: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    input_cost_per_token: Option<f64>,
    #[serde(default)]
    output_cost_per_token: Option<f64>,
    #[serde(default)]
    max_input_tokens: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    max_tokens: Option<u64>,
}

/// Providers we care about from LiteLLM.
const SUPPORTED_LITELLM_PROVIDERS: &[&str] = &["anthropic", "openai"];

/// Prefixes that indicate wrapped/hosted models we should skip.
const SKIP_PREFIXES: &[&str] = &[
    "azure/",
    "bedrock/",
    "vertex_ai/",
    "vertex_ai_beta/",
    "sagemaker/",
    "anyscale/",
    "fireworks_ai/",
];

/// Refresh the registry from LiteLLM's GitHub-hosted JSON.
/// Fire-and-forget — errors are logged, never propagated to UI.
pub async fn refresh_registry() -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Check if we've refreshed in the last 24 hours
    {
        let registry = REGISTRY.read();
        if registry.fetched_at > 0 && now - registry.fetched_at < 86_400 {
            debug!(target: "4da::registry", age_hours = (now - registry.fetched_at) / 3600, "Registry is fresh, skipping refresh");
            return Ok(());
        }
    }

    info!(target: "4da::registry", "Refreshing model registry from LiteLLM");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client for registry refresh")?;

    let url = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch LiteLLM model data")?;

    if !response.status().is_success() {
        return Err(format!("LiteLLM fetch failed with status {}", response.status()).into());
    }

    // Read body with size cap to prevent memory exhaustion from malformed upstream
    let bytes = response
        .bytes()
        .await
        .context("Failed to read LiteLLM response body")?;

    const MAX_REGISTRY_SIZE: usize = 10 * 1024 * 1024; // 10 MB
    if bytes.len() > MAX_REGISTRY_SIZE {
        return Err(format!(
            "LiteLLM registry too large ({} bytes, max {})",
            bytes.len(),
            MAX_REGISTRY_SIZE
        )
        .into());
    }

    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_slice(&bytes).context("Failed to parse LiteLLM JSON")?;

    let mut models = HashMap::new();

    // Start with bundled defaults (preserved if not in LiteLLM)
    let bundled = bundled_registry();
    for (k, v) in &bundled.models {
        models.insert(k.clone(), v.clone());
    }

    // Parse LiteLLM entries
    for (raw_id, value) in &raw {
        // Skip the "sample_spec" key that LiteLLM includes as documentation
        if raw_id == "sample_spec" {
            continue;
        }

        // Skip wrapped/hosted models
        if SKIP_PREFIXES.iter().any(|p| raw_id.starts_with(p)) {
            continue;
        }

        let entry: LiteLLMEntry = match serde_json::from_value(value.clone()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Filter: only chat models from supported providers
        let provider = match &entry.litellm_provider {
            Some(p) if SUPPORTED_LITELLM_PROVIDERS.contains(&p.as_str()) => p.clone(),
            _ => continue,
        };
        match &entry.mode {
            Some(m) if m == "chat" => {}
            _ => continue,
        }

        // Strip provider prefix (e.g., "openai/gpt-4.1" → "gpt-4.1")
        let model_id = raw_id
            .strip_prefix(&format!("{provider}/"))
            .unwrap_or(raw_id)
            .to_string();

        // Skip if it still has a slash (nested prefix we didn't handle)
        if model_id.contains('/') {
            continue;
        }

        let max_out = entry.max_output_tokens.or(entry.max_tokens);

        let display_name = model_id
            .replace('-', " ")
            .replace("claude ", "Claude ")
            .replace("gpt ", "GPT ");

        models.insert(
            model_id.clone(),
            ModelInfo {
                id: model_id,
                provider,
                display_name,
                input_cost_per_token: entry.input_cost_per_token,
                output_cost_per_token: entry.output_cost_per_token,
                max_input_tokens: entry.max_input_tokens,
                max_output_tokens: max_out,
            },
        );
    }

    let new_registry = ModelRegistry {
        fetched_at: now,
        source: "litellm".to_string(),
        models,
    };

    let model_count = new_registry.models.len();

    // Update in-memory singleton
    {
        let mut registry = REGISTRY.write();
        *registry = new_registry.clone();
    }

    // Persist to disk
    save_to_disk(&new_registry);

    info!(target: "4da::registry", models = model_count, "Model registry refreshed successfully");
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the full model registry (grouped by provider) for the frontend.
#[tauri::command]
pub async fn get_model_registry() -> Result<serde_json::Value> {
    let registry = get_registry_snapshot();

    // Group models by provider
    let mut by_provider: HashMap<String, Vec<&ModelInfo>> = HashMap::new();
    for model in registry.models.values() {
        by_provider
            .entry(model.provider.clone())
            .or_default()
            .push(model);
    }

    // Sort each provider's models by ID
    for models in by_provider.values_mut() {
        models.sort_by(|a, b| a.id.cmp(&b.id));
    }

    Ok(serde_json::json!({
        "fetched_at": registry.fetched_at,
        "source": registry.source,
        "model_count": registry.models.len(),
        "providers": by_provider,
    }))
}

/// Manually trigger a registry refresh.
#[tauri::command]
pub async fn refresh_model_registry() -> Result<serde_json::Value> {
    // Force refresh by temporarily setting fetched_at to 0
    {
        let mut registry = REGISTRY.write();
        registry.fetched_at = 0;
    }

    refresh_registry().await?;

    let registry = get_registry_snapshot();
    Ok(serde_json::json!({
        "success": true,
        "model_count": registry.models.len(),
        "source": registry.source,
    }))
}

#[cfg(test)]
#[path = "model_registry_tests.rs"]
mod tests;
