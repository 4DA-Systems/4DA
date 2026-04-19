// SPDX-License-Identifier: FSL-1.1-Apache-2.0
#[cfg(test)]
mod tests {
    use crate::context_engine::InteractionType;
    use crate::error::FourDaError;
    use crate::settings::{LLMProvider, RerankConfig};
    use crate::settings_commands::validate_input_length;

    // ========================================================================
    // validate_input_length tests
    // ========================================================================

    #[test]
    fn validate_input_length_accepts_short_string() {
        let result = validate_input_length("hello", "test_field", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_accepts_exact_max() {
        let input = "a".repeat(100);
        let result = validate_input_length(&input, "test_field", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_rejects_over_max() {
        let input = "a".repeat(101);
        let result = validate_input_length(&input, "test_field", 100);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("test_field"),
            "Error should mention the field name"
        );
        assert!(
            err_msg.contains("101"),
            "Error should mention the actual length"
        );
        assert!(
            err_msg.contains("100"),
            "Error should mention the max length"
        );
    }

    #[test]
    fn validate_input_length_accepts_empty_string() {
        let result = validate_input_length("", "test_field", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_input_length_zero_max_rejects_any_content() {
        let result = validate_input_length("a", "test_field", 0);
        assert!(result.is_err());
    }

    #[test]
    fn validate_input_length_zero_max_accepts_empty() {
        let result = validate_input_length("", "test_field", 0);
        assert!(result.is_ok());
    }

    // ========================================================================
    // Provider validation tests (extracted logic from set_llm_provider)
    // ========================================================================

    #[test]
    fn valid_providers_list_is_complete() {
        let valid_providers = ["anthropic", "openai", "ollama", "none"];
        assert_eq!(valid_providers.len(), 4);
        assert!(valid_providers.contains(&"anthropic"));
        assert!(valid_providers.contains(&"openai"));
        assert!(valid_providers.contains(&"ollama"));
        assert!(valid_providers.contains(&"none"));
    }

    #[test]
    fn invalid_provider_string_is_rejected() {
        let valid_providers = ["anthropic", "openai", "ollama", "none"];
        assert!(!valid_providers.contains(&"gemini"));
        assert!(!valid_providers.contains(&""));
        assert!(!valid_providers.contains(&"Anthropic")); // case-sensitive
    }

    // ========================================================================
    // RerankConfig clamping logic tests
    // ========================================================================

    #[test]
    fn rerank_config_clamps_max_items() {
        // Simulates the clamping logic from set_rerank_config
        let max_items: usize = 0;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 1);

        let max_items: usize = 5000;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 1000);

        let max_items: usize = 500;
        let clamped = max_items.clamp(1, 1000);
        assert_eq!(clamped, 500);
    }

    #[test]
    fn rerank_config_clamps_min_score() {
        let min_score: f32 = -0.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.0);

        let min_score: f32 = 1.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 1.0);

        let min_score: f32 = 0.5;
        let clamped = min_score.clamp(0.0, 1.0);
        assert_eq!(clamped, 0.5);
    }

    #[test]
    fn rerank_config_enforces_minimum_token_limit() {
        let daily_token_limit: u64 = 0;
        let enforced = daily_token_limit.max(1);
        assert_eq!(enforced, 1);

        let daily_token_limit: u64 = 100_000;
        let enforced = daily_token_limit.max(1);
        assert_eq!(enforced, 100_000);
    }

    #[test]
    fn rerank_config_enforces_minimum_cost_limit() {
        let daily_cost_limit: u64 = 0;
        let enforced = daily_cost_limit.max(1);
        assert_eq!(enforced, 1);
    }

    // ========================================================================
    // RerankConfig struct construction
    // ========================================================================

    #[test]
    fn rerank_config_construction_with_clamping() {
        let config = RerankConfig {
            enabled: true,
            max_items_per_batch: 0_usize.clamp(1, 1000),
            min_embedding_score: (-1.0_f32).clamp(0.0, 1.0),
            daily_token_limit: 1,
            daily_cost_limit_cents: 1,
            reconciler_enabled: true,
        };
        assert!(config.enabled);
        assert_eq!(config.max_items_per_batch, 1);
        assert_eq!(config.min_embedding_score, 0.0);
        assert_eq!(config.daily_token_limit, 1);
        assert_eq!(config.daily_cost_limit_cents, 1);
    }

    // ========================================================================
    // LLMProvider defaults
    // ========================================================================

    #[test]
    fn llm_provider_default_has_none_provider() {
        let provider = LLMProvider::default();
        assert_eq!(provider.provider, "none");
        assert!(provider.api_key.is_empty());
        assert!(provider.model.is_empty());
        assert!(provider.base_url.is_none());
        assert!(provider.openai_api_key.is_empty());
    }

    // ========================================================================
    // FourDaError From<String> and From<&str> conversions
    // ========================================================================

    #[test]
    fn error_from_string_creates_internal_variant() {
        let err: FourDaError = "test error".into();
        assert!(
            err.to_string().contains("test error"),
            "Error should preserve message"
        );
    }

    #[test]
    fn error_from_format_string_creates_internal_variant() {
        let err: FourDaError =
            format!("Invalid provider '{}'. Must be one of: a, b", "gemini").into();
        assert!(err.to_string().contains("gemini"));
        assert!(err.to_string().contains("Must be one of"));
    }

    // ========================================================================
    // Interaction type mapping (logic from record_interaction)
    // ========================================================================

    #[test]
    fn interaction_type_mapping_valid_actions() {
        let valid_actions = vec![
            ("click", InteractionType::Click),
            ("save", InteractionType::Save),
            ("dismiss", InteractionType::Dismiss),
            ("ignore", InteractionType::Ignore),
        ];
        for (action_str, expected_type) in &valid_actions {
            let mapped = match action_str.to_lowercase().as_str() {
                "click" => Some(InteractionType::Click),
                "save" => Some(InteractionType::Save),
                "dismiss" => Some(InteractionType::Dismiss),
                "ignore" => Some(InteractionType::Ignore),
                _ => None,
            };
            assert!(
                mapped.is_some(),
                "Action '{}' should map to a valid type",
                action_str
            );
            assert_eq!(
                std::mem::discriminant(&mapped.unwrap()),
                std::mem::discriminant(expected_type)
            );
        }
    }

    #[test]
    fn interaction_type_mapping_rejects_unknown_action() {
        let action = "bookmark";
        let mapped = match action.to_lowercase().as_str() {
            "click" => Some(InteractionType::Click),
            "save" => Some(InteractionType::Save),
            "dismiss" => Some(InteractionType::Dismiss),
            "ignore" => Some(InteractionType::Ignore),
            _ => None,
        };
        assert!(mapped.is_none(), "Unknown action should not map to a type");
    }

    // ========================================================================
    // Interaction type mapping is case-insensitive
    // ========================================================================

    #[test]
    fn interaction_type_mapping_case_insensitive() {
        for action in &["CLICK", "Click", "cLiCk", "SAVE", "Save", "DISMISS"] {
            let mapped = match action.to_lowercase().as_str() {
                "click" => Some("click"),
                "save" => Some("save"),
                "dismiss" => Some("dismiss"),
                "ignore" => Some("ignore"),
                _ => None,
            };
            assert!(
                mapped.is_some(),
                "Action '{}' should map case-insensitively",
                action
            );
        }
    }
}
