//! Settings module for 4DA
//!
//! Manages user configuration including API keys (BYOK), preferences,
//! and usage limits. Settings are stored in the app data directory.

mod discovery;
pub mod env_detection;
mod helpers;
pub mod keystore;
mod license;
#[cfg(test)]
mod license_tests;
mod manager;
pub mod types;
pub mod validation;

pub use discovery::*;
pub use helpers::*;
pub use license::*;
pub use manager::*;
pub use types::*;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert!(settings.rerank.enabled);
        assert_eq!(settings.embedding_threshold, 0.50);
        assert_eq!(settings.rerank.max_items_per_batch, 48);
    }

    #[test]
    fn test_llm_provider_default() {
        let provider = LLMProvider::default();
        assert_eq!(provider.provider, "none");
        assert!(provider.api_key.is_empty());
    }

    // ========================================================================
    // SettingsManager -- missing settings file returns defaults
    // ========================================================================

    #[test]
    fn test_settings_manager_missing_file_returns_defaults() {
        let tmp = std::env::temp_dir().join("4da_test_missing_settings");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let manager = SettingsManager::new(&tmp);
        let settings = manager.get();

        // Should have all defaults since no settings.json exists
        assert_eq!(settings.llm.provider, "none");
        assert!(settings.llm.api_key.is_empty());
        assert!(settings.rerank.enabled);
        assert_eq!(settings.embedding_threshold, 0.50);
        assert!(!settings.onboarding_complete);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ========================================================================
    // SettingsManager -- malformed JSON settings file
    // ========================================================================

    #[test]
    fn test_settings_manager_malformed_json_returns_defaults() {
        let tmp = std::env::temp_dir().join("4da_test_malformed_settings");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Write invalid JSON
        let settings_path = tmp.join("settings.json");
        std::fs::write(&settings_path, "{ this is not valid json !!!")
            .expect("write malformed settings");

        let manager = SettingsManager::new(&tmp);
        let settings = manager.get();

        // Should fall back to defaults when JSON is invalid
        assert_eq!(settings.llm.provider, "none");
        assert!(settings.rerank.enabled);
        assert_eq!(settings.embedding_threshold, 0.50);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_settings_manager_empty_file_returns_defaults() {
        let tmp = std::env::temp_dir().join("4da_test_empty_settings");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Write empty file
        let settings_path = tmp.join("settings.json");
        std::fs::write(&settings_path, "").expect("write empty settings");

        let manager = SettingsManager::new(&tmp);
        let settings = manager.get();

        // Should fall back to defaults
        assert_eq!(settings.llm.provider, "none");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_settings_manager_partial_json_uses_defaults_for_missing() {
        let tmp = std::env::temp_dir().join("4da_test_partial_settings");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Write partial valid JSON (missing most fields)
        let settings_path = tmp.join("settings.json");
        std::fs::write(
            &settings_path,
            r#"{
                "llm": {
                    "provider": "anthropic",
                    "api_key": "sk-test-key",
                    "model": "claude-haiku-4-5-20251001",
                    "base_url": null,
                    "openai_api_key": ""
                },
                "rerank": {
                    "enabled": true,
                    "max_items_per_batch": 48,
                    "min_embedding_score": 0.20,
                    "daily_token_limit": 500000,
                    "daily_cost_limit_cents": 100
                },
                "context_dirs": [],
                "embedding_threshold": 0.50
            }"#,
        )
        .expect("write partial settings");

        let manager = SettingsManager::new(&tmp);
        let settings = manager.get();

        // Explicit fields should be loaded
        assert_eq!(settings.llm.provider, "anthropic");
        assert_eq!(settings.llm.api_key, "sk-test-key");
        // Default fields should still work
        assert!(!settings.onboarding_complete);
        assert_eq!(settings.license.tier, "free");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ========================================================================
    // SettingsManager -- malformed usage.json
    // ========================================================================

    #[test]
    fn test_settings_manager_malformed_usage_returns_defaults() {
        let tmp = std::env::temp_dir().join("4da_test_malformed_usage");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Write valid settings.json
        let settings_json =
            serde_json::from_str::<Settings>("{\"llm\":{\"provider\":\"none\",\"api_key\":\"\",\"model\":\"\",\"base_url\":null,\"openai_api_key\":\"\"},\"rerank\":{\"enabled\":true,\"max_items_per_batch\":48,\"min_embedding_score\":0.2,\"daily_token_limit\":500000,\"daily_cost_limit_cents\":100},\"context_dirs\":[],\"embedding_threshold\":0.5}")
                .ok();
        let settings_json = serde_json::to_string_pretty(&settings_json.unwrap_or_default())
            .expect("serialize default settings");
        std::fs::write(tmp.join("settings.json"), settings_json).expect("write settings");

        // Write malformed usage.json
        std::fs::write(tmp.join("usage.json"), "NOT JSON {{{").expect("write malformed usage");

        let manager = SettingsManager::new(&tmp);
        let usage = manager.get_usage();

        // Should fall back to default usage
        assert_eq!(usage.tokens_today, 0);
        assert_eq!(usage.tokens_total, 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ========================================================================
    // Settings validation -- clamping invalid values
    // ========================================================================

    #[test]
    fn test_validate_clamps_invalid_embedding_threshold() {
        let mut settings = Settings::default();
        settings.embedding_threshold = 2.5; // out of range (max 1.0)
        settings.validate();
        assert!((settings.embedding_threshold - 1.0).abs() < f32::EPSILON);

        settings.embedding_threshold = -0.5; // negative
        settings.validate();
        assert!((settings.embedding_threshold - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_validate_clamps_zero_batch_size() {
        let mut settings = Settings::default();
        settings.rerank.max_items_per_batch = 0;
        settings.validate();
        assert_eq!(settings.rerank.max_items_per_batch, 1);
    }

    #[test]
    fn test_validate_clamps_zero_monitoring_interval() {
        let mut settings = Settings::default();
        settings.monitoring.interval_minutes = 0;
        settings.validate();
        assert_eq!(settings.monitoring.interval_minutes, 1);
    }

    #[test]
    fn test_validate_removes_empty_context_dirs() {
        let mut settings = Settings::default();
        settings.context_dirs = vec![
            "valid/dir".to_string(),
            "".to_string(),
            "   ".to_string(),
            "another/valid".to_string(),
        ];
        settings.validate();
        assert_eq!(settings.context_dirs.len(), 2);
        assert_eq!(settings.context_dirs[0], "valid/dir");
        assert_eq!(settings.context_dirs[1], "another/valid");
    }

    #[test]
    fn test_validate_clamps_serendipity_budget_over_100() {
        let mut settings = Settings::default();
        settings.serendipity.budget_percent = 150;
        settings.validate();
        assert_eq!(settings.serendipity.budget_percent, 100);
    }

    #[test]
    fn test_validate_clamps_min_embedding_score() {
        let mut settings = Settings::default();
        settings.rerank.min_embedding_score = 5.0;
        settings.validate();
        assert!((settings.rerank.min_embedding_score - 1.0).abs() < f32::EPSILON);

        settings.rerank.min_embedding_score = -1.0;
        settings.validate();
        assert!((settings.rerank.min_embedding_score - 0.0).abs() < f32::EPSILON);
    }

    // ========================================================================
    // SettingsManager -- is_rerank_enabled logic
    // ========================================================================

    #[test]
    fn test_is_rerank_enabled_no_provider() {
        let tmp = std::env::temp_dir().join("4da_test_rerank_none");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let manager = SettingsManager::new(&tmp);
        // Default provider is "none", so rerank should be disabled
        assert!(!manager.is_rerank_enabled());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_is_rerank_enabled_ollama_no_key() {
        let tmp = std::env::temp_dir().join("4da_test_rerank_ollama");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let mut settings = Settings::default();
        settings.llm.provider = "ollama".to_string();
        settings.rerank.enabled = true;
        let json = serde_json::to_string_pretty(&settings).expect("serialize");
        std::fs::write(tmp.join("settings.json"), json).expect("write");

        let manager = SettingsManager::new(&tmp);
        // Ollama doesn't need an API key, so rerank should work
        assert!(manager.is_rerank_enabled());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_is_rerank_enabled_anthropic_no_key() {
        let tmp = std::env::temp_dir().join("4da_test_rerank_anthro_nokey");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let mut settings = Settings::default();
        settings.llm.provider = "anthropic".to_string();
        settings.llm.api_key = String::new();
        settings.rerank.enabled = true;
        let json = serde_json::to_string_pretty(&settings).expect("serialize");
        std::fs::write(tmp.join("settings.json"), json).expect("write");

        let manager = SettingsManager::new(&tmp);
        // Anthropic with no API key should disable rerank
        assert!(!manager.is_rerank_enabled());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ========================================================================
    // SettingsManager -- save and reload round-trip
    // ========================================================================

    #[test]
    fn test_settings_save_and_reload() {
        let tmp = std::env::temp_dir().join("4da_test_save_reload");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Create and modify settings
        let mut manager = SettingsManager::new(&tmp);
        manager
            .set_llm_provider(LLMProvider {
                provider: "anthropic".to_string(),
                api_key: "test-key-123".to_string(),
                model: "claude-haiku-4-5-20251001".to_string(),
                base_url: None,
                openai_api_key: String::new(),
                embedding_model: String::new(),
            })
            .expect("save provider");

        // Reload from disk
        let manager2 = SettingsManager::new(&tmp);
        let settings = manager2.get();
        assert_eq!(settings.llm.provider, "anthropic");
        assert_eq!(settings.llm.api_key, "test-key-123");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ========================================================================
    // Locale detection helpers
    // ========================================================================

    #[test]
    fn test_country_to_currency() {
        assert_eq!(helpers::country_to_currency("US"), "USD");
        assert_eq!(helpers::country_to_currency("GB"), "GBP");
        assert_eq!(helpers::country_to_currency("DE"), "EUR");
        assert_eq!(helpers::country_to_currency("JP"), "JPY");
        assert_eq!(helpers::country_to_currency("AU"), "AUD");
        // Unknown country defaults to USD
        assert_eq!(helpers::country_to_currency("ZZ"), "USD");
    }

    #[test]
    fn test_locale_config_default() {
        let locale = LocaleConfig::default();
        assert_eq!(locale.country, "US");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.currency, "USD");
    }

    // ========================================================================
    // Settings serialization round-trip
    // ========================================================================

    #[test]
    fn test_settings_serialization_roundtrip() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: Settings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.llm.provider, settings.llm.provider);
        assert_eq!(
            deserialized.embedding_threshold,
            settings.embedding_threshold
        );
        assert_eq!(deserialized.rerank.enabled, settings.rerank.enabled);
    }

    #[test]
    fn test_llm_provider_serialization_roundtrip() {
        let provider = LLMProvider {
            provider: "openai".to_string(),
            api_key: "sk-test".to_string(),
            model: "gpt-4o".to_string(),
            base_url: Some("https://custom.openai.com".to_string()),
            openai_api_key: "sk-embed".to_string(),
            embedding_model: "nomic-embed-text-v2-moe".to_string(),
        };
        let json = serde_json::to_string(&provider).expect("serialize");
        let deserialized: LLMProvider = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.provider, "openai");
        assert_eq!(
            deserialized.base_url,
            Some("https://custom.openai.com".to_string())
        );
    }

    // ========================================================================
    // Legacy tier migration: "pro" -> "signal"
    // ========================================================================

    #[test]
    fn test_legacy_pro_tier_migrated_to_signal() {
        let tmp = std::env::temp_dir().join("4da_test_pro_to_signal");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Write settings with legacy "pro" tier
        let mut settings = Settings::default();
        settings.license.tier = "pro".to_string();
        let json = serde_json::to_string_pretty(&settings).expect("serialize");
        std::fs::write(tmp.join("settings.json"), json).expect("write settings");

        // Load -- migration should fire
        let manager = SettingsManager::new(&tmp);
        assert_eq!(manager.get().license.tier, "signal");

        // Verify persisted to disk
        let on_disk: Settings = serde_json::from_str(
            &std::fs::read_to_string(tmp.join("settings.json")).expect("read"),
        )
        .expect("parse");
        assert_eq!(on_disk.license.tier, "signal");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_signal_tier_not_modified() {
        let tmp = std::env::temp_dir().join("4da_test_signal_unchanged");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let mut settings = Settings::default();
        settings.license.tier = "signal".to_string();
        let json = serde_json::to_string_pretty(&settings).expect("serialize");
        std::fs::write(tmp.join("settings.json"), json).expect("write settings");

        let manager = SettingsManager::new(&tmp);
        assert_eq!(manager.get().license.tier, "signal");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_free_tier_not_modified() {
        let tmp = std::env::temp_dir().join("4da_test_free_unchanged");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        let mut settings = Settings::default();
        settings.license.tier = "free".to_string();
        let json = serde_json::to_string_pretty(&settings).expect("serialize");
        std::fs::write(tmp.join("settings.json"), json).expect("write settings");

        let manager = SettingsManager::new(&tmp);
        assert_eq!(manager.get().license.tier, "free");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
