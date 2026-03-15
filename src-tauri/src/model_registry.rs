//! Self-updating LLM model registry.
//!
//! Single source of truth for model names, pricing, and capabilities.
//! Three-layer design: bundled defaults → disk cache → in-memory singleton.
//! Refreshes from LiteLLM's community-maintained registry (≤1x/24h).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
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
    let mut base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base.push("model_registry.json");
    base
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
    let registry = REGISTRY.read().ok()?;
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
        if key.to_lowercase().starts_with(&id_lower) {
            if best.is_none() || key.len() < best.unwrap().0.len() {
                best = Some((key.as_str(), info));
            }
        }
    }
    if let Some((_, info)) = best {
        return Some(info.clone());
    }

    // Contains (e.g., "sonnet" matches "claude-sonnet-4-20250514")
    // Pick shortest key for determinism
    let mut best: Option<(&str, &ModelInfo)> = None;
    for (key, info) in &registry.models {
        if key.to_lowercase().contains(&id_lower) {
            if best.is_none() || key.len() < best.unwrap().0.len() {
                best = Some((key.as_str(), info));
            }
        }
    }
    if let Some((_, info)) = best {
        return Some(info.clone());
    }

    None
}

/// Get curated model list for a provider (for UI dropdowns).
pub fn get_provider_models(provider: &str) -> Vec<String> {
    let registry = match REGISTRY.read() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

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
    REGISTRY
        .read()
        .map(|r| r.clone())
        .unwrap_or_else(|_| bundled_registry())
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
        let registry = REGISTRY
            .read()
            .map_err(|e| format!("Registry lock poisoned: {e}"))?;
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
            .strip_prefix(&format!("{}/", provider))
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
        let mut registry = REGISTRY
            .write()
            .map_err(|e| format!("Registry lock poisoned: {e}"))?;
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
        if let Ok(mut registry) = REGISTRY.write() {
            registry.fetched_at = 0;
        }
    }

    refresh_registry().await?;

    let registry = get_registry_snapshot();
    Ok(serde_json::json!({
        "success": true,
        "model_count": registry.models.len(),
        "source": registry.source,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_registry_has_current_models() {
        let registry = bundled_registry();
        assert!(registry.models.contains_key("claude-haiku-4-5-20251001"));
        assert!(registry.models.contains_key("claude-sonnet-4-6"));
        assert!(registry.models.contains_key("claude-opus-4-6"));
        assert!(registry.models.contains_key("gpt-4o-mini"));
        assert!(registry.models.contains_key("gpt-4o"));
        assert!(registry.models.contains_key("gpt-4.1"));
        assert!(registry.models.contains_key("gpt-4.1-mini"));
        assert!(registry.models.contains_key("gpt-4.1-nano"));
    }

    #[test]
    fn test_fuzzy_model_lookup() {
        // Initialize registry with bundled data
        {
            let mut registry = REGISTRY.write().unwrap();
            *registry = bundled_registry();
        }

        // Exact match
        let info = get_model_info("gpt-4o").unwrap();
        assert_eq!(info.id, "gpt-4o");

        // Contains match: "sonnet" should find a Claude Sonnet model
        let info = get_model_info("sonnet").unwrap();
        assert!(info.id.contains("sonnet"));

        // Contains match: "haiku" should find Haiku
        let info = get_model_info("haiku").unwrap();
        assert!(info.id.contains("haiku"));
    }

    #[test]
    fn test_estimate_cost_matches_expectations() {
        // Initialize
        {
            let mut registry = REGISTRY.write().unwrap();
            *registry = bundled_registry();
        }

        // Haiku: $0.80/1M input, $4.00/1M output
        // 10k input = $0.008, 1k output = $0.004 = $0.012 = ~1.2 cents
        let cost = estimate_cost("anthropic", "claude-haiku-4-5-20251001", 10_000, 1_000);
        assert!(cost.is_some());
        let cents = cost.unwrap();
        assert!(cents < 2, "Haiku cost should be < 2 cents, got {cents}");

        // GPT-4o-mini: $0.15/1M input, $0.60/1M output
        // 10k input + 1k output should be < 1 cent
        let cost = estimate_cost("openai", "gpt-4o-mini", 10_000, 1_000);
        assert!(cost.is_some());
        assert!(cost.unwrap() < 1);
    }

    #[test]
    fn test_unknown_model_returns_none() {
        {
            let mut registry = REGISTRY.write().unwrap();
            *registry = bundled_registry();
        }

        let cost = estimate_cost("anthropic", "claude-nonexistent-9000", 10_000, 1_000);
        assert!(cost.is_none(), "Unknown model should return None, not zero");
    }

    #[test]
    fn test_openai_compatible_returns_none() {
        let cost = estimate_cost("openai-compatible", "some-model", 10_000, 1_000);
        assert!(cost.is_none(), "openai-compatible should return None");
    }

    #[test]
    fn test_ollama_is_free() {
        let cost = estimate_cost("ollama", "llama3.2", 100_000, 10_000);
        assert_eq!(cost, Some(0));
    }

    #[test]
    fn test_get_provider_models() {
        {
            let mut registry = REGISTRY.write().unwrap();
            *registry = bundled_registry();
        }

        let anthropic = get_provider_models("anthropic");
        assert!(!anthropic.is_empty());
        assert!(anthropic.iter().any(|m| m.contains("haiku")));
        assert!(anthropic.iter().any(|m| m.contains("sonnet")));

        let openai = get_provider_models("openai");
        assert!(!openai.is_empty());
        assert!(openai.iter().any(|m| m.contains("gpt-4o")));
    }

    #[test]
    fn test_disk_cache_roundtrip() {
        let registry = bundled_registry();
        let json = serde_json::to_string_pretty(&registry).unwrap();
        let loaded: ModelRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.models.len(), registry.models.len());
        assert_eq!(loaded.source, "bundled");
    }

    #[test]
    fn test_litellm_parse_filters_correctly() {
        // Simulate LiteLLM entries
        let raw_json = serde_json::json!({
            "claude-sonnet-4-20250514": {
                "litellm_provider": "anthropic",
                "mode": "chat",
                "input_cost_per_token": 0.000003,
                "output_cost_per_token": 0.000015,
                "max_input_tokens": 200000,
                "max_output_tokens": 8192
            },
            "text-embedding-ada-002": {
                "litellm_provider": "openai",
                "mode": "embedding",
                "input_cost_per_token": 0.0000001
            },
            "azure/gpt-4o": {
                "litellm_provider": "openai",
                "mode": "chat",
                "input_cost_per_token": 0.0000025
            },
            "bedrock/claude-3-sonnet": {
                "litellm_provider": "anthropic",
                "mode": "chat"
            },
            "sample_spec": {
                "max_tokens": 100
            }
        });

        let raw: HashMap<String, serde_json::Value> = serde_json::from_value(raw_json).unwrap();
        let mut accepted = Vec::new();

        for (raw_id, value) in &raw {
            if raw_id == "sample_spec" {
                continue;
            }
            if SKIP_PREFIXES.iter().any(|p| raw_id.starts_with(p)) {
                continue;
            }

            let entry: LiteLLMEntry = match serde_json::from_value(value.clone()) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let provider = match &entry.litellm_provider {
                Some(p) if SUPPORTED_LITELLM_PROVIDERS.contains(&p.as_str()) => p.clone(),
                _ => continue,
            };
            match &entry.mode {
                Some(m) if m == "chat" => {}
                _ => continue,
            }

            let model_id = raw_id
                .strip_prefix(&format!("{}/", provider))
                .unwrap_or(raw_id)
                .to_string();
            if model_id.contains('/') {
                continue;
            }

            accepted.push(model_id);
        }

        // Only claude-sonnet-4-20250514 should pass all filters
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0], "claude-sonnet-4-20250514");
    }
}
