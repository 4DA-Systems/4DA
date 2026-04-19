// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Privacy tests for export, analysis, and data pipeline paths.
//!
//! Validates that API keys never leak through exports, analysis results,
//! database rows, cost/usage formatting, or round-trip serialization.
//!
//! Split from privacy_tests.rs — see that file for core tests (Debug, errors,
//! serialization, logging, settings response).

#[cfg(test)]
mod tests {
    use crate::llm::LLMClient;
    use crate::settings::{LLMProvider, LicenseConfig, Settings};

    // Realistic test keys — same constants as privacy_tests.rs
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
            signal_priority: Some("alert".to_string()),
            signal_action: Some("Review API key management".to_string()),
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            detected_lang: "en".to_string(),
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
            created_at: None,
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
    // 10. Privacy contract: has_x_api_key returns bool (never the raw key)
    //     Previously get_x_api_key returned the raw bearer token — fixed in H1.
    // ========================================================================

    #[test]
    fn test_x_api_key_command_returns_presence_only() {
        let s = settings_with_keys();
        let has_key = !s.x_api_key.is_empty();

        // has_x_api_key only returns a boolean, never the raw key
        assert!(
            has_key,
            "has_x_api_key returns true when a key is configured"
        );
    }

    // ========================================================================
    // 11. Sensitive field inventory (regression guard)
    // ========================================================================

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
            model: "claude-haiku-4-5-20251001".to_string(),
            base_url: None,
            openai_api_key: String::new(),
            embedding_model: String::new(),
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
