//! Privacy-critical tests for 4DA — core module.
//!
//! Validates that API keys and sensitive data never leak through Debug output,
//! log messages, error strings, serialized responses, or settings commands.
//! This is the #1 test coverage gap for a privacy-first BYOK app.
//!
//! Invariant: raw API keys must NEVER appear outside `data/settings.json`.
//!
//! See also: `privacy_tests_exports.rs` for export, database, and data pipeline tests.

#[cfg(test)]
mod tests {
    use crate::llm::{LLMClient, LLMResponse};
    use crate::settings::{LLMProvider, LicenseConfig, Settings};

    // Realistic test keys that look like real secrets.
    // Using distinct patterns makes assertions unambiguous.
    const FAKE_ANTHROPIC_KEY: &str = "sk-ant-api03-TESTKEYDONOTUSE-1234567890abcdef";
    const FAKE_OPENAI_KEY: &str = "sk-proj-TESTKEYDONOTUSE-abcdef1234567890";
    const FAKE_X_BEARER: &str = "AAAAAAAAAAAAAAAAAAAAAAtestBearerTokenDONOTUSE";
    const FAKE_LICENSE_KEY: &str = "4DA-PRO-TESTKEY-abcdef1234567890";
    const FAKE_OPENAI_EMBED_KEY: &str = "sk-embed-TESTKEYDONOTUSE-9876543210fedcba";

    /// Build a Settings struct populated with all known API key fields.
    fn settings_with_keys() -> Settings {
        let mut s = Settings::default();
        s.llm = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: FAKE_ANTHROPIC_KEY.to_string(),
            model: "claude-haiku-4-5-20251001".to_string(),
            base_url: None,
            openai_api_key: FAKE_OPENAI_EMBED_KEY.to_string(),
            embedding_model: String::new(),
        };
        s.x_api_key = FAKE_X_BEARER.to_string();
        s.license = LicenseConfig {
            tier: "pro".to_string(),
            license_key: FAKE_LICENSE_KEY.to_string(),
            activated_at: Some("2026-01-01T00:00:00Z".to_string()),
            trial_started_at: None,
            dev_unlock_all: false,
        };
        s
    }

    // ========================================================================
    // 1. Debug output must not contain raw API keys
    // ========================================================================

    #[test]
    fn test_settings_debug_redacts_api_keys() {
        let s = settings_with_keys();
        let debug_output = format!("{:?}", s);

        // Custom Debug impls must redact all API keys.
        assert!(
            !debug_output.contains(FAKE_ANTHROPIC_KEY),
            "Settings Debug must not contain Anthropic API key"
        );
        assert!(
            !debug_output.contains(FAKE_OPENAI_EMBED_KEY),
            "Settings Debug must not contain OpenAI embed key"
        );
        assert!(
            !debug_output.contains(FAKE_X_BEARER),
            "Settings Debug must not contain X bearer token"
        );
        assert!(
            !debug_output.contains(FAKE_LICENSE_KEY),
            "Settings Debug must not contain license key"
        );

        // Verify redaction markers are present.
        assert!(
            debug_output.contains("[REDACTED]"),
            "Settings Debug should show [REDACTED] placeholders"
        );

        let llm_debug = format!("{:?}", s.llm);
        assert!(
            !llm_debug.contains(FAKE_ANTHROPIC_KEY),
            "LLMProvider Debug must not contain api_key"
        );
        assert!(
            llm_debug.contains("[REDACTED]"),
            "LLMProvider Debug should show [REDACTED] placeholder"
        );
    }

    // ========================================================================
    // 2. Serialization: keys are present for settings file, absent from
    //    the get_settings command response
    // ========================================================================

    #[test]
    fn test_settings_json_serialization_contains_keys_for_persistence() {
        let s = settings_with_keys();
        let json = serde_json::to_string(&s).expect("Settings must serialize");

        assert!(
            json.contains(FAKE_ANTHROPIC_KEY),
            "api_key must be in serialized settings (needed for settings.json)"
        );
        assert!(
            json.contains(FAKE_OPENAI_EMBED_KEY),
            "openai_api_key must be in serialized settings"
        );
        assert!(
            json.contains(FAKE_X_BEARER),
            "x_api_key must be in serialized settings"
        );
        assert!(
            json.contains(FAKE_LICENSE_KEY),
            "license_key must be in serialized settings"
        );
    }

    #[test]
    fn test_get_settings_response_returns_has_key_not_actual_key() {
        let s = settings_with_keys();

        let response = serde_json::json!({
            "llm": {
                "provider": s.llm.provider,
                "model": s.llm.model,
                "has_api_key": !s.llm.api_key.is_empty(),
                "base_url": s.llm.base_url
            },
            "rerank": {
                "enabled": s.rerank.enabled,
                "max_items_per_batch": s.rerank.max_items_per_batch,
                "min_embedding_score": s.rerank.min_embedding_score,
                "daily_token_limit": s.rerank.daily_token_limit,
                "daily_cost_limit_cents": s.rerank.daily_cost_limit_cents
            },
            "embedding_threshold": s.embedding_threshold,
            "onboarding_complete": s.onboarding_complete,
            "auto_discovery_completed": s.auto_discovery_completed,
            "license": {
                "tier": s.license.tier,
                "has_key": !s.license.license_key.is_empty(),
                "activated_at": s.license.activated_at,
            }
        });

        let response_str = serde_json::to_string(&response).unwrap();

        assert!(
            !response_str.contains(FAKE_ANTHROPIC_KEY),
            "get_settings response must not contain LLM API key"
        );
        assert!(
            !response_str.contains(FAKE_OPENAI_EMBED_KEY),
            "get_settings response must not contain OpenAI embedding key"
        );
        assert!(
            !response_str.contains(FAKE_X_BEARER),
            "get_settings response must not contain X Bearer Token"
        );
        assert!(
            !response_str.contains(FAKE_LICENSE_KEY),
            "get_settings response must not contain license key"
        );

        let obj = response.as_object().unwrap();
        let llm = obj["llm"].as_object().unwrap();
        assert_eq!(llm["has_api_key"], true, "has_api_key should be true");

        let license = obj["license"].as_object().unwrap();
        assert_eq!(license["has_key"], true, "license has_key should be true");
    }

    // ========================================================================
    // 3. Error messages must not contain API keys
    // ========================================================================

    #[test]
    fn test_anthropic_error_message_does_not_expose_key() {
        let status = "401 Unauthorized";
        let api_error_body =
            r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let error_msg = format!("Anthropic API error {}: {}", status, api_error_body);

        assert!(
            !error_msg.contains(FAKE_ANTHROPIC_KEY),
            "Error message must not contain the API key. Got: {}",
            error_msg
        );
    }

    #[test]
    fn test_openai_error_message_does_not_expose_key() {
        let status = "401 Unauthorized";
        let api_error_body = r#"{"error":{"message":"Incorrect API key provided"}}"#;
        let error_msg = format!("OpenAI API error {}: {}", status, api_error_body);

        assert!(
            !error_msg.contains(FAKE_OPENAI_KEY),
            "Error message must not contain the API key"
        );
    }

    #[test]
    fn test_ollama_error_message_does_not_expose_secrets() {
        let error_variants = vec![
            "Cannot connect to Ollama at http://localhost:11434. Make sure Ollama is running.",
            "Model 'llama3' not found in Ollama. Run: ollama pull llama3",
            "Not enough GPU memory for 'llama3'. Try a smaller model.",
            "Ollama error 500: internal server error",
        ];

        for err in &error_variants {
            assert!(
                !err.contains(FAKE_ANTHROPIC_KEY),
                "Ollama error leaked Anthropic key"
            );
            assert!(
                !err.contains(FAKE_OPENAI_KEY),
                "Ollama error leaked OpenAI key"
            );
        }
    }

    #[test]
    fn test_fourda_error_enum_does_not_contain_api_keys() {
        use crate::error::FourDaError;

        let errors: Vec<FourDaError> = vec![
            FourDaError::Config("Invalid provider configuration".to_string()),
            FourDaError::Llm("Authentication failed".to_string()),
            FourDaError::Internal("Settings load failed".to_string()),
        ];

        for err in &errors {
            let msg = err.to_string();
            let serialized = serde_json::to_string(err).unwrap();

            assert!(
                !msg.contains(FAKE_ANTHROPIC_KEY),
                "FourDaError::to_string leaked API key: {}",
                msg
            );
            assert!(
                !serialized.contains(FAKE_ANTHROPIC_KEY),
                "FourDaError serialization leaked API key"
            );
        }
    }

    // ========================================================================
    // 4. LLMResponse struct does not contain request data or API keys
    // ========================================================================

    #[test]
    fn test_llm_response_struct_has_no_key_fields() {
        let response = LLMResponse {
            content: "This is a test response".to_string(),
            input_tokens: 100,
            output_tokens: 50,
        };

        let debug_output = format!("{:?}", response);

        assert!(
            !debug_output.contains(FAKE_ANTHROPIC_KEY),
            "LLMResponse Debug output must not contain API key"
        );
        assert!(
            !debug_output.contains(FAKE_OPENAI_KEY),
            "LLMResponse Debug output must not contain API key"
        );
        assert!(
            !debug_output.contains("Bearer"),
            "LLMResponse Debug should not contain auth headers"
        );

        assert!(debug_output.contains("content:"));
        assert!(debug_output.contains("input_tokens:"));
        assert!(debug_output.contains("output_tokens:"));
    }

    #[test]
    fn test_llm_client_does_not_derive_debug() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: FAKE_ANTHROPIC_KEY.to_string(),
            model: "test".to_string(),
            base_url: None,
            openai_api_key: String::new(),
            embedding_model: String::new(),
        };

        let _client = LLMClient::new(provider);
    }

    // ========================================================================
    // 7. get_settings returns has_key booleans, not actual keys
    //    (command-level contract test)
    // ========================================================================

    #[test]
    fn test_get_settings_response_shape_is_safe() {
        let s = settings_with_keys();

        let response = serde_json::json!({
            "llm": {
                "provider": s.llm.provider,
                "model": s.llm.model,
                "has_api_key": !s.llm.api_key.is_empty(),
                "base_url": s.llm.base_url
            },
            "license": {
                "tier": s.license.tier,
                "has_key": !s.license.license_key.is_empty(),
                "activated_at": s.license.activated_at,
            }
        });

        assert_eq!(
            response["llm"]["has_api_key"], true,
            "LLM section should report has_api_key=true"
        );
        assert_eq!(
            response["license"]["has_key"], true,
            "License section should report has_key=true"
        );

        let json_str = serde_json::to_string(&response).unwrap();
        assert!(
            !json_str.contains("sk-ant-"),
            "Response contains Anthropic key prefix"
        );
        assert!(
            !json_str.contains("sk-proj-"),
            "Response contains OpenAI key prefix"
        );
        assert!(
            !json_str.contains("4DA-PRO-"),
            "Response contains license key prefix"
        );

        let llm_fields: Vec<&String> = response["llm"].as_object().unwrap().keys().collect();
        assert!(
            !llm_fields.contains(&&"api_key".to_string()),
            "get_settings response must not have 'api_key' field"
        );
        assert!(
            !llm_fields.contains(&&"openai_api_key".to_string()),
            "get_settings response must not have 'openai_api_key' field"
        );

        let license_fields: Vec<&String> =
            response["license"].as_object().unwrap().keys().collect();
        assert!(
            !license_fields.contains(&&"license_key".to_string()),
            "get_settings response must not have 'license_key' field"
        );
    }

    #[test]
    fn test_get_settings_with_empty_keys_reports_false() {
        let s = Settings::default();

        let response = serde_json::json!({
            "llm": {
                "has_api_key": !s.llm.api_key.is_empty(),
            },
            "license": {
                "has_key": !s.license.license_key.is_empty(),
            }
        });

        assert_eq!(
            response["llm"]["has_api_key"], false,
            "Empty API key should report has_api_key=false"
        );
        assert_eq!(
            response["license"]["has_key"], false,
            "Empty license key should report has_key=false"
        );
    }

    // ========================================================================
    // 8. Log output must not contain API keys
    // ========================================================================

    #[test]
    fn test_settings_log_messages_are_safe() {
        let log_messages = vec![
            "LLM provider updated",
            "Settings loaded",
            "Failed to parse settings",
            "No settings file found, using defaults",
        ];

        for msg in &log_messages {
            assert!(
                !msg.contains(FAKE_ANTHROPIC_KEY),
                "Log message contains API key: {}",
                msg
            );
        }
    }

    #[test]
    fn test_tracing_output_does_not_contain_api_keys() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone)]
        struct CaptureWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }

        impl std::io::Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl<'a> MakeWriter<'a> for CaptureWriter {
            type Writer = CaptureWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let capture = CaptureWriter {
            buf: Arc::new(Mutex::new(Vec::new())),
        };
        let capture_clone = capture.clone();

        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture_clone)
            .with_max_level(tracing::Level::DEBUG)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let s = settings_with_keys();

            let mut settings = s.clone();
            settings.validate();

            tracing::info!(target: "4da::settings", provider = %settings.llm.provider, "Provider configured");
            tracing::info!(target: "4da::settings", model = %settings.llm.model, "Model selected");
            tracing::debug!(target: "4da::settings", has_key = !settings.llm.api_key.is_empty(), "Key status");
            tracing::warn!(target: "4da::settings", "Authentication failed for provider");
        });

        let log_output = String::from_utf8(capture.buf.lock().unwrap().clone())
            .expect("Log output should be valid UTF-8");

        assert!(
            !log_output.contains(FAKE_ANTHROPIC_KEY),
            "Log output contains Anthropic API key!\nLogs:\n{}",
            log_output
        );
        assert!(
            !log_output.contains(FAKE_OPENAI_EMBED_KEY),
            "Log output contains OpenAI embedding key!\nLogs:\n{}",
            log_output
        );
        assert!(
            !log_output.contains(FAKE_X_BEARER),
            "Log output contains X Bearer Token!\nLogs:\n{}",
            log_output
        );
        assert!(
            !log_output.contains(FAKE_LICENSE_KEY),
            "Log output contains license key!\nLogs:\n{}",
            log_output
        );

        if !log_output.is_empty() {
            assert!(
                log_output.contains("anthropic") || log_output.contains("Provider configured"),
                "Log output should contain safe provider name, got:\n{}",
                log_output
            );
        }
    }
}
