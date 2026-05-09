// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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
        let mut registry = REGISTRY.write();
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
        let mut registry = REGISTRY.write();
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
        let mut registry = REGISTRY.write();
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
        let mut registry = REGISTRY.write();
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
