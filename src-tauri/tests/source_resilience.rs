#![allow(clippy::unwrap_used)]
//! Phase 4: Source adapter resilience tests
//!
//! Validates source error types, SourceItem edge cases, SourceConfig defaults,
//! SourceRegistry operations, and the new per-source resilience/rate-budget
//! settings fields (backward-compatible deserialization).

use fourda_lib::sources::{SourceConfig, SourceError, SourceItem, SourceRegistry};

// ============================================================================
// Test 1: SourceError Display variants
// ============================================================================

#[test]
fn test_source_error_display_variants() {
    let network = SourceError::Network("connection timeout".to_string());
    assert!(
        network.to_string().contains("Network error"),
        "Network variant should contain 'Network error'"
    );
    assert!(
        network.to_string().contains("connection timeout"),
        "Network variant should contain the inner message"
    );

    let parse = SourceError::Parse("invalid JSON".to_string());
    assert!(
        parse.to_string().contains("Parse error"),
        "Parse variant should contain 'Parse error'"
    );

    let rate_limited = SourceError::RateLimited("try again later".to_string());
    assert!(
        rate_limited.to_string().contains("try again later"),
        "RateLimited variant should contain the message"
    );

    let disabled = SourceError::Disabled;
    assert_eq!(disabled.to_string(), "Source disabled");

    let other = SourceError::Other("something went wrong".to_string());
    assert!(other.to_string().contains("something went wrong"));

    // SourceError implements std::error::Error
    let err: &dyn std::error::Error = &network;
    assert!(!err.to_string().is_empty());
}

// ============================================================================
// Test 2: SourceItem creation with edge cases
// ============================================================================

#[test]
fn test_source_item_edge_cases() {
    // Empty content (common when scraping fails)
    let empty = SourceItem::new("test", "1", "Title");
    assert!(empty.content.is_empty(), "Default content should be empty");
    assert!(empty.url.is_none(), "Default url should be None");
    assert!(empty.metadata.is_none(), "Default metadata should be None");
    assert_eq!(
        empty.embedding_text(),
        "Title",
        "Embedding text should be title alone when no content"
    );

    // Very long title (truncation is caller's job, SourceItem should store it)
    let long_title = "A".repeat(10_000);
    let long_item = SourceItem::new("test", "2", &long_title);
    assert_eq!(long_item.title.len(), 10_000);

    // Unicode content (CJK, emoji, RTL)
    let unicode = SourceItem::new("test", "3", "Rust - Safety")
        .with_content("Rust ensures memory safety without a GC.".to_string())
        .with_url(Some("https://example.com".to_string()));
    assert_eq!(unicode.title, "Rust - Safety");
    assert!(unicode.url.is_some());

    // Builder chaining with metadata
    let with_meta = SourceItem::new("hackernews", "99", "HN Story")
        .with_url(Some("https://hn.com/99".to_string()))
        .with_content("Discussion about Rust.".to_string())
        .with_metadata(serde_json::json!({"score": 150, "comments": 42}));
    assert!(with_meta.metadata.is_some());
    let meta = with_meta.metadata.unwrap();
    assert_eq!(meta["score"], 150);
    assert_eq!(meta["comments"], 42);

    // Embedding text with content includes both
    let full =
        SourceItem::new("rss", "rss1", "RSS Title").with_content("RSS body text".to_string());
    let embed = full.embedding_text();
    assert!(embed.contains("RSS Title"));
    assert!(embed.contains("RSS body text"));
}

// ============================================================================
// Test 3: SourceConfig default values
// ============================================================================

#[test]
fn test_source_config_defaults() {
    let config = SourceConfig::default();
    assert!(config.enabled, "Sources should be enabled by default");
    assert_eq!(config.max_items, 30, "Default max_items should be 30");
    assert_eq!(
        config.fetch_interval_secs, 300,
        "Default fetch interval should be 300 seconds (5 minutes)"
    );
    assert!(
        config.custom.is_none(),
        "Default custom config should be None"
    );
}

// ============================================================================
// Test 4: SourceRegistry operations
// ============================================================================

/// A minimal mock source for testing registry operations
struct MockSource {
    source_type: &'static str,
    name: &'static str,
    config: SourceConfig,
}

impl MockSource {
    fn new(source_type: &'static str, name: &'static str, enabled: bool) -> Self {
        Self {
            source_type,
            name,
            config: SourceConfig {
                enabled,
                ..SourceConfig::default()
            },
        }
    }
}

#[async_trait::async_trait]
impl fourda_lib::sources::Source for MockSource {
    fn source_type(&self) -> &'static str {
        self.source_type
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn config(&self) -> &SourceConfig {
        &self.config
    }
    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }
    async fn fetch_items(&self) -> fourda_lib::sources::SourceResult<Vec<SourceItem>> {
        Ok(vec![])
    }
    async fn scrape_content(
        &self,
        _item: &SourceItem,
    ) -> fourda_lib::sources::SourceResult<String> {
        Ok(String::new())
    }
}

#[test]
fn test_source_registry_operations() {
    let mut registry = SourceRegistry::new();
    assert_eq!(registry.count(), 0, "New registry should be empty");

    // Register sources
    registry.register(Box::new(MockSource::new("hackernews", "Hacker News", true)));
    registry.register(Box::new(MockSource::new("reddit", "Reddit", false)));
    registry.register(Box::new(MockSource::new("github", "GitHub", true)));

    assert_eq!(registry.count(), 3, "Should have 3 registered sources");

    // Lookup by type
    let hn = registry.get_source("hackernews");
    assert!(hn.is_some(), "Should find hackernews");
    assert_eq!(hn.unwrap().name(), "Hacker News");

    let missing = registry.get_source("nonexistent");
    assert!(missing.is_none(), "Should not find nonexistent source");

    // Enabled filtering
    let enabled = registry.enabled_sources();
    assert_eq!(
        enabled.len(),
        2,
        "Should have 2 enabled sources (HN and GitHub)"
    );
    let enabled_types: Vec<&str> = enabled.iter().map(|s| s.source_type()).collect();
    assert!(enabled_types.contains(&"hackernews"));
    assert!(enabled_types.contains(&"github"));
    assert!(!enabled_types.contains(&"reddit"), "Reddit is disabled");

    // All sources
    let all = registry.sources();
    assert_eq!(all.len(), 3);
}

// ============================================================================
// Test 5: Settings deserialization is forward- and backward-compatible
// ============================================================================

/// A user's on-disk `settings.json` must survive both directions of drift:
/// keys the current build has never heard of (written by a newer/older build,
/// or left behind when a dead config block is deleted) must be IGNORED, and
/// keys that are absent must fall back to defaults. `Settings` carries a
/// container-level `#[serde(default)]` and no `deny_unknown_fields`, which is
/// exactly what makes that true — this test pins it so a future `serde`
/// attribute change cannot silently start rejecting real users' settings.
#[test]
fn test_settings_ignores_unknown_keys_and_defaults_missing_ones() {
    use fourda_lib::settings::Settings;

    // Sparse JSON: only a handful of keys present, plus retired keys that no
    // longer exist on the struct (source_resilience / rate_budgets / predictive
    // / audio_briefing / health_radar / attention / nitter_instance were removed
    // once it was confirmed nothing read them).
    let legacy_json = r#"{
        "llm": {"provider":"none","api_key":"","model":"","base_url":null,"openai_api_key":""},
        "rerank": {"enabled":true,"max_items_per_batch":48,"min_embedding_score":0.2,"daily_token_limit":500000,"daily_cost_limit_cents":100},
        "context_dirs": [],
        "embedding_threshold": 0.5,
        "nitter_instance": "https://nitter.example",
        "predictive": {"enabled": true, "prefetch_window_minutes": 30},
        "audio_briefing": {"enabled": false, "tts_model": "auto", "max_duration_seconds": 180},
        "health_radar": {"enabled": true, "check_interval_hours": 24},
        "attention": {"enabled": true},
        "source_resilience": {"reddit": {"max_failures": 5, "cooldown_seconds": 600}},
        "rate_budgets": {"reddit": {"requests_per_minute": 10}},
        "some_key_from_a_future_build": {"nested": [1, 2, 3]}
    }"#;
    let deserialized: Settings = serde_json::from_str(legacy_json)
        .expect("retired and unknown settings keys must be ignored, never rejected");

    // Present keys are honoured...
    assert_eq!(deserialized.embedding_threshold, 0.5);
    assert!(deserialized.rerank.enabled);
    // ...and absent keys fall back to their defaults rather than failing.
    assert_eq!(deserialized.license.tier, "free");
    assert!(!deserialized.onboarding_complete);
}
