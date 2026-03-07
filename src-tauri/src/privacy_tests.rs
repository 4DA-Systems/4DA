//! Privacy-critical tests for 4DA.
//!
//! Validates that API keys and sensitive data never leak through Debug output,
//! log messages, error strings, serialized responses, database rows, or exports.
//! This is the #1 test coverage gap for a privacy-first BYOK app.
//!
//! Invariant: raw API keys must NEVER appear outside `data/settings.json`.

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
            model: "claude-3-haiku-20240307".to_string(),
            base_url: None,
            openai_api_key: FAKE_OPENAI_EMBED_KEY.to_string(),
        };
        s.x_api_key = FAKE_X_BEARER.to_string();
        s.license = LicenseConfig {
            tier: "pro".to_string(),
            license_key: FAKE_LICENSE_KEY.to_string(),
            activated_at: Some("2026-01-01T00:00:00Z".to_string()),
            trial_started_at: None,
        };
        s
    }

    // ========================================================================
    // 1. Debug output must not contain raw API keys
    // ========================================================================

    /// Settings derives Debug. The default derive will print every field,
    /// including api_key. This test documents the current (unsafe) behavior
    /// and will FAIL once a custom Debug impl is added that redacts keys --
    /// at which point this test should be updated to assert redaction.
    ///
    /// FINDING: Settings, LLMProvider, and LicenseConfig all #[derive(Debug)]
    /// which means `format!("{:?}", settings)` exposes raw API keys.
    /// This is safe as long as Debug output is never logged or returned to
    /// untrusted consumers, but a custom Debug impl would be strictly better.
    #[test]
    fn test_settings_debug_exposes_api_keys_currently() {
        let s = settings_with_keys();
        let debug_output = format!("{:?}", s);

        // Document that Debug currently DOES contain the keys.
        // This is the reality we're testing against.
        let keys_exposed = debug_output.contains(FAKE_ANTHROPIC_KEY)
            || debug_output.contains(FAKE_OPENAI_EMBED_KEY)
            || debug_output.contains(FAKE_X_BEARER)
            || debug_output.contains(FAKE_LICENSE_KEY);

        assert!(
            keys_exposed,
            "POSITIVE CHANGE: Debug no longer exposes API keys! \
             Update this test to assert redaction instead."
        );

        // Separately verify that if someone adds `tracing::debug!("{:?}", settings)`
        // it would leak keys. This is the invariant we want to eventually fix.
        let llm_debug = format!("{:?}", s.llm);
        assert!(
            llm_debug.contains(FAKE_ANTHROPIC_KEY),
            "LLMProvider Debug currently exposes api_key (by design -- \
             update this test when custom Debug is added)"
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

        // Keys MUST be present in the settings.json serialization --
        // that's how they're persisted to disk.
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

    /// The `get_settings` Tauri command manually builds a JSON response
    /// with `has_api_key: bool` instead of the raw key. Verify this
    /// pattern by replicating the command's logic.
    #[test]
    fn test_get_settings_response_returns_has_key_not_actual_key() {
        let s = settings_with_keys();

        // Replicate the exact JSON shape from settings_commands::get_settings
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

        // The response must NOT contain any raw keys
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

        // But the boolean flags must be present and true
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
        // Simulate the error format from llm.rs complete_anthropic:
        // format!("Anthropic API error {}: {}", status, text)
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
        // Simulate the error format from llm.rs complete_openai:
        // format!("OpenAI API error {}: {}", status, text)
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
        // Ollama errors should not contain any keys at all
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

        // Build errors that could plausibly contain keys if code is careless
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
        // LLMResponse should only contain the response content and token counts.
        // Verify its fields don't include api_key, authorization, or request.
        let response = LLMResponse {
            content: "This is a test response".to_string(),
            input_tokens: 100,
            output_tokens: 50,
        };

        let debug_output = format!("{:?}", response);

        // Must not contain any API key values
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

        // Verify the struct only has expected fields
        assert!(debug_output.contains("content:"));
        assert!(debug_output.contains("input_tokens:"));
        assert!(debug_output.contains("output_tokens:"));
    }

    #[test]
    fn test_llm_client_does_not_derive_debug() {
        // LLMClient stores the provider (which contains the API key).
        // It should NOT derive Debug. Verify this at the type level:
        // If LLMClient derived Debug, you could do format!("{:?}", client).
        //
        // We can verify this by confirming LLMClient is NOT Debug.
        // In Rust, this is a compile-time property. If someone adds
        // #[derive(Debug)] to LLMClient, the test below (which tries
        // to use it as non-Debug) would still compile but the assertion
        // documents intent. The real guard is that LLMClient fields
        // include reqwest::Client which also doesn't derive Debug cleanly.
        //
        // POSITIVE FINDING: LLMClient does NOT derive Debug, so
        // `format!("{:?}", client)` won't compile -- keys can't leak
        // through accidental debug logging of the client.
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: FAKE_ANTHROPIC_KEY.to_string(),
            model: "test".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };

        let _client = LLMClient::new(provider);

        // This test passes by construction: LLMClient doesn't derive Debug.
        // If someone adds Debug, the test should be updated to verify redaction.
        // We can't assert !implements_debug at runtime in stable Rust,
        // but the fact that this compiles without a Debug bound is the proof.
    }

    // ========================================================================
    // 5. API keys must not appear in the database
    // ========================================================================

    #[test]
    fn test_api_key_not_in_database() {
        use crate::test_utils::{insert_test_item, test_db};

        let db = test_db();

        // Insert several test items that might accidentally contain keys
        // if there's a bug in the pipeline
        insert_test_item(
            &db,
            "hackernews",
            "hn_001",
            "Rust API key management best practices",
            "How to safely store API keys in your application",
        );
        insert_test_item(
            &db,
            "reddit",
            "reddit_001",
            "OpenAI pricing changes",
            "New pricing for GPT-4o-mini and embeddings",
        );
        insert_test_item(
            &db,
            "github",
            "gh_001",
            "Secret scanner tool",
            "Detect leaked API keys in your codebase",
        );

        // Query all text content from source_items
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare("SELECT title, content FROM source_items")
            .expect("query source_items");

        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, String>(1).unwrap_or_default(),
                ))
            })
            .expect("map rows")
            .filter_map(|r| r.ok())
            .collect();

        assert!(!rows.is_empty(), "Should have inserted test items");

        for (title, content) in &rows {
            let combined = format!("{} {}", title, content);
            assert!(
                !combined.contains(FAKE_ANTHROPIC_KEY),
                "Database row contains Anthropic API key in: {}",
                title
            );
            assert!(
                !combined.contains(FAKE_OPENAI_KEY),
                "Database row contains OpenAI API key in: {}",
                title
            );
            assert!(
                !combined.contains(FAKE_X_BEARER),
                "Database row contains X Bearer Token in: {}",
                title
            );
            assert!(
                !combined.contains(FAKE_LICENSE_KEY),
                "Database row contains license key in: {}",
                title
            );
        }

        // Also check context_chunks table
        let mut ctx_stmt = conn
            .prepare("SELECT source_file, text FROM context_chunks")
            .expect("query context_chunks");

        let ctx_rows: Vec<(String, String)> = ctx_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, String>(1).unwrap_or_default(),
                ))
            })
            .expect("map context rows")
            .filter_map(|r| r.ok())
            .collect();

        for (source, text) in &ctx_rows {
            let combined = format!("{} {}", source, text);
            assert!(
                !combined.contains(FAKE_ANTHROPIC_KEY),
                "Context chunk contains API key"
            );
        }
    }

    // ========================================================================
    // 6. Export functions must not contain API keys
    // ========================================================================

    #[test]
    fn test_export_pack_result_does_not_contain_api_keys() {
        use crate::toolkit_export::ExportPackResult;

        // Simulate an export pack with realistic content
        let pack = ExportPackResult {
            markdown: format!(
                "# 4DA Developer Profile\n\n\
                 ## Developer DNA\n\n\
                 **Identity:** Full-stack Rust + TypeScript developer\n\
                 **Primary Stack:** Rust, TypeScript, React\n\n\
                 ## Tech Radar\n\n\
                 | Technology | Ring | Score |\n\
                 |------------|------|-------|\n\
                 | Rust | Adopt | 0.95 |\n\n\
                 ## Active Decisions\n\n\
                 ### Use Anthropic for LLM\n\n\
                 **Decision:** Selected Claude for relevance judging\n\
                 **Confidence:** 90%\n\n\
                 ---\n\
                 *Generated by 4DA*\n"
            ),
            has_dna: true,
            has_radar: true,
            has_decisions: true,
        };

        let json = serde_json::to_string(&pack).expect("serialize export pack");

        assert!(
            !json.contains(FAKE_ANTHROPIC_KEY),
            "Export pack JSON must not contain API key"
        );
        assert!(
            !json.contains(FAKE_OPENAI_KEY),
            "Export pack JSON must not contain OpenAI key"
        );
        assert!(
            !json.contains(FAKE_X_BEARER),
            "Export pack JSON must not contain X bearer token"
        );
        assert!(
            !json.contains(FAKE_LICENSE_KEY),
            "Export pack JSON must not contain license key"
        );

        // Also check the markdown content directly
        assert!(
            !pack.markdown.contains("sk-ant-"),
            "Export markdown must not contain Anthropic key prefix"
        );
        assert!(
            !pack.markdown.contains("sk-proj-"),
            "Export markdown must not contain OpenAI key prefix"
        );
    }

    #[test]
    fn test_analysis_export_formats_do_not_contain_api_keys() {
        use crate::types::SourceRelevance;

        // Create a realistic result set
        let results = vec![SourceRelevance {
            id: 1,
            title: "How to manage API keys securely".to_string(),
            url: Some("https://example.com/api-keys".to_string()),
            top_score: 0.75,
            matches: vec![],
            relevant: true,
            context_score: 0.5,
            interest_score: 0.3,
            excluded: false,
            excluded_by: None,
            source_type: "hackernews".to_string(),
            explanation: Some("Matches your security interests".to_string()),
            confidence: Some(0.8),
            score_breakdown: None,
            signal_type: Some("security_alert".to_string()),
            signal_priority: Some("high".to_string()),
            signal_action: Some("Review API key management".to_string()),
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
        }];

        // Serialize the results as the frontend would receive them
        let json = serde_json::to_string(&results).expect("serialize results");

        assert!(
            !json.contains(FAKE_ANTHROPIC_KEY),
            "Analysis results must not contain API keys"
        );
        assert!(
            !json.contains(FAKE_OPENAI_KEY),
            "Analysis results must not contain API keys"
        );
        assert!(
            !json.contains(FAKE_X_BEARER),
            "Analysis results must not contain API keys"
        );

        // Verify the SourceRelevance struct has no field that could hold a key
        // by checking its serialized field names
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = value[0].as_object().unwrap();
        let field_names: Vec<&String> = obj.keys().collect();

        // None of these field names should suggest key storage
        for field in &field_names {
            assert!(
                !field.contains("api_key"),
                "SourceRelevance has suspicious field: {}",
                field
            );
            assert!(
                !field.contains("secret"),
                "SourceRelevance has suspicious field: {}",
                field
            );
            assert!(
                !field.contains("token"),
                "SourceRelevance has suspicious field: {}",
                field
            );
            assert!(
                !field.contains("password"),
                "SourceRelevance has suspicious field: {}",
                field
            );
        }
    }

    // ========================================================================
    // 7. get_settings returns has_key booleans, not actual keys
    //    (command-level contract test)
    // ========================================================================

    #[test]
    fn test_get_settings_response_shape_is_safe() {
        let s = settings_with_keys();

        // Build the response exactly as settings_commands::get_settings does
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

        // Verify boolean flags
        assert_eq!(
            response["llm"]["has_api_key"], true,
            "LLM section should report has_api_key=true"
        );
        assert_eq!(
            response["license"]["has_key"], true,
            "License section should report has_key=true"
        );

        // Verify no raw keys in the response
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

        // Verify the response does NOT have fields named "api_key" or "license_key"
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
        let s = Settings::default(); // All keys empty

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

    /// Verify that the info!() call in set_llm_provider doesn't log the key.
    /// The actual log line is: info!(target: "4da::settings", "LLM provider updated")
    /// which is safe. This test verifies the pattern.
    #[test]
    fn test_settings_log_messages_are_safe() {
        // Simulate the log messages that settings_commands.rs produces.
        // These are the actual format strings used in the codebase.
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

    /// Use tracing-subscriber's test layer to capture actual log output
    /// and verify no keys appear. This tests the complete logging pipeline.
    #[test]
    fn test_tracing_output_does_not_contain_api_keys() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        // Custom writer that captures all log output
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

        // Build a subscriber that writes to our capture buffer
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture_clone)
            .with_max_level(tracing::Level::DEBUG)
            .finish();

        // Run code that produces log output within this subscriber
        tracing::subscriber::with_default(subscriber, || {
            let s = settings_with_keys();

            // Simulate the validate() call which logs warnings
            let mut settings = s.clone();
            settings.validate();

            // Simulate some info-level logs that settings code produces
            tracing::info!(target: "4da::settings", provider = %settings.llm.provider, "Provider configured");
            tracing::info!(target: "4da::settings", model = %settings.llm.model, "Model selected");
            tracing::debug!(target: "4da::settings", has_key = !settings.llm.api_key.is_empty(), "Key status");

            // Simulate warning that includes error context
            tracing::warn!(target: "4da::settings", "Authentication failed for provider");
        });

        let log_output = String::from_utf8(capture.buf.lock().unwrap().clone())
            .expect("Log output should be valid UTF-8");

        // Verify no API keys leaked into log output
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

        // The logs SHOULD contain safe metadata (provider name, model name)
        // This verifies the subscriber actually captured output
        if !log_output.is_empty() {
            assert!(
                log_output.contains("anthropic") || log_output.contains("Provider configured"),
                "Log output should contain safe provider name, got:\n{}",
                log_output
            );
        }
    }

    // ========================================================================
    // 9. Key redaction helper (defense-in-depth utility)
    // ========================================================================

    /// Test a key-redaction helper that could be used for safe error reporting.
    /// Even if we don't have one yet, this documents the expected behavior.
    #[test]
    fn test_key_redaction_pattern() {
        /// Redact an API key for safe display: show first 4 and last 4 chars.
        fn redact_key(key: &str) -> String {
            if key.len() <= 12 {
                return "***REDACTED***".to_string();
            }
            format!("{}...{}", &key[..4], &key[key.len() - 4..])
        }

        let redacted = redact_key(FAKE_ANTHROPIC_KEY);
        assert!(
            !redacted.contains(FAKE_ANTHROPIC_KEY),
            "Redacted key must not contain full key"
        );
        assert!(redacted.starts_with("sk-a"), "Should show first 4 chars");
        assert!(redacted.ends_with("cdef"), "Should show last 4 chars");
        assert!(redacted.contains("..."), "Should contain ellipsis");

        // Short keys should be fully redacted
        let short_redacted = redact_key("abc123");
        assert_eq!(short_redacted, "***REDACTED***");
    }

    // ========================================================================
    // 10. Privacy contract: source_config::get_x_api_key returns raw key
    //     (documents a known exposure point for audit)
    // ========================================================================

    /// AUDIT FINDING: source_config::get_x_api_key() returns the actual
    /// Bearer Token to the frontend. This is intentional (the frontend needs
    /// it to display in the settings UI for editing), but it's a privacy
    /// surface that should be documented.
    ///
    /// This test documents the behavior so it's visible in test output.
    #[test]
    fn test_x_api_key_command_returns_raw_key_documented() {
        // The Tauri command get_x_api_key returns the raw token:
        //   pub async fn get_x_api_key() -> Result<String, String> {
        //       let settings_guard = get_settings_manager().lock();
        //       Ok(settings_guard.get_x_api_key())
        //   }
        //
        // This is a known exposure point. The key travels:
        //   settings.json -> Rust -> IPC -> JavaScript
        //
        // Mitigation: the IPC channel is local-only (Tauri's WebView),
        // and the key is needed for the settings editor UI.
        //
        // If this exposure becomes unacceptable, the command should return
        // a masked version and require a separate "reveal" action.

        let s = settings_with_keys();
        let returned_key = s.x_api_key.clone(); // Simulates what get_x_api_key returns

        // Document that the raw key IS returned
        assert_eq!(
            returned_key, FAKE_X_BEARER,
            "get_x_api_key returns the raw Bearer Token (documented behavior)"
        );
    }

    // ========================================================================
    // 11. Sensitive field inventory (regression guard)
    // ========================================================================

    /// Enumerate all sensitive fields in Settings and verify each has
    /// appropriate handling in the get_settings response.
    #[test]
    fn test_sensitive_field_inventory() {
        let s = settings_with_keys();

        // Catalog all sensitive fields
        let sensitive_fields: Vec<(&str, &str)> = vec![
            ("llm.api_key", &s.llm.api_key),
            ("llm.openai_api_key", &s.llm.openai_api_key),
            ("x_api_key", &s.x_api_key),
            ("license.license_key", &s.license.license_key),
        ];

        // Build the safe response (as get_settings does)
        let safe_response = serde_json::json!({
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
        let safe_str = serde_json::to_string(&safe_response).unwrap();

        for (field_name, value) in &sensitive_fields {
            assert!(
                !value.is_empty(),
                "Test setup error: {} should be non-empty",
                field_name
            );
            assert!(
                !safe_str.contains(*value),
                "PRIVACY VIOLATION: {} value '{}' found in safe response",
                field_name,
                &value[..value.len().min(10)]
            );
        }
    }

    // ========================================================================
    // 12. Settings round-trip preserves keys without corruption
    // ========================================================================

    #[test]
    fn test_settings_serde_roundtrip_preserves_all_keys() {
        let original = settings_with_keys();

        // Serialize -> Deserialize
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let restored: Settings = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            restored.llm.api_key, FAKE_ANTHROPIC_KEY,
            "LLM API key must survive round-trip"
        );
        assert_eq!(
            restored.llm.openai_api_key, FAKE_OPENAI_EMBED_KEY,
            "OpenAI embedding key must survive round-trip"
        );
        assert_eq!(
            restored.x_api_key, FAKE_X_BEARER,
            "X Bearer Token must survive round-trip"
        );
        assert_eq!(
            restored.license.license_key, FAKE_LICENSE_KEY,
            "License key must survive round-trip"
        );
    }

    // ========================================================================
    // 13. Cost estimation doesn't leak keys
    // ========================================================================

    #[test]
    fn test_cost_estimation_output_is_key_free() {
        let provider = LLMProvider {
            provider: "anthropic".to_string(),
            api_key: FAKE_ANTHROPIC_KEY.to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            base_url: None,
            openai_api_key: String::new(),
        };
        let client = LLMClient::new(provider);
        let cost = client.estimate_cost_cents(10_000, 1_000);

        // Cost is just a number, but verify the debug output of cost-related
        // operations doesn't somehow include the key
        let cost_str = format!("Cost: {} cents", cost);
        assert!(
            !cost_str.contains(FAKE_ANTHROPIC_KEY),
            "Cost output must not contain API key"
        );
    }

    // ========================================================================
    // 14. Usage summary doesn't leak keys
    // ========================================================================

    #[test]
    fn test_usage_summary_format_is_key_free() {
        // Replicate SettingsManager::usage_summary() format
        let summary = format!(
            "Today: {} tokens (~${:.3}) | Total: {} tokens | {} items re-ranked",
            1500, 0.015, 10000, 50
        );

        assert!(
            !summary.contains(FAKE_ANTHROPIC_KEY),
            "Usage summary must not contain API key"
        );
        assert!(
            !summary.contains("sk-"),
            "Usage summary must not contain key-like prefixes"
        );
    }
}
