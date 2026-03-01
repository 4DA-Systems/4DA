//! Information Channels — types and seed definitions.
//!
//! A channel is a topic 4DA maintains intelligence on — continuously, locally,
//! verified against multiple sources. Not static articles — rendered queries
//! against the local knowledge base.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Enums
// ============================================================================

/// Channel lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ChannelStatus {
    Active,
    Paused,
    Archived,
}

/// Freshness indicator for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ChannelFreshness {
    /// Rendered within the last 24 hours.
    Fresh,
    /// Rendered, but more than 24 hours ago.
    Stale,
    /// Never rendered — no snapshot exists yet.
    NeverRendered,
}

// ============================================================================
// Core Records
// ============================================================================

/// Full channel record as stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Channel {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub topic_query: Vec<String>,
    pub status: ChannelStatus,
    pub source_count: i64,
    pub render_count: i64,
    pub last_rendered_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight summary for list views (cheaper to transfer than full Channel).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChannelSummary {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub source_count: i64,
    pub render_count: i64,
    pub freshness: ChannelFreshness,
    pub last_rendered_at: Option<String>,
}

// ============================================================================
// Renders & Provenance
// ============================================================================

/// A rendered snapshot of channel content — immutable once saved.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChannelRender {
    pub id: i64,
    pub channel_id: i64,
    pub version: i64,
    pub content_markdown: String,
    pub content_hash: String,
    pub source_item_ids: Vec<i64>,
    pub model: Option<String>,
    pub tokens_used: Option<i64>,
    pub latency_ms: Option<i64>,
    pub rendered_at: String,
}

/// Provenance: maps a claim in the render to its source items.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderProvenance {
    pub render_id: i64,
    pub claim_index: i64,
    pub claim_text: String,
    pub source_item_ids: Vec<i64>,
    pub source_titles: Vec<String>,
    pub source_urls: Vec<String>,
}

/// Changelog between two render versions.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChannelChangelog {
    pub channel_id: i64,
    pub from_version: i64,
    pub to_version: i64,
    pub summary: String,
    pub added_lines: Vec<String>,
    pub removed_lines: Vec<String>,
    pub changed_at: String,
}

// ============================================================================
// Source Matching
// ============================================================================

/// A source item matched to a channel via topic relevance.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChannelSourceMatch {
    pub channel_id: i64,
    pub source_item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub match_score: f64,
    pub matched_at: String,
}

// ============================================================================
// Seed Definitions (internal only)
// ============================================================================

/// A seed channel definition used to bootstrap the default channel set.
pub(crate) struct SeedChannel {
    pub slug: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub topics: &'static [&'static str],
}

pub(crate) const SEED_CHANNELS: &[SeedChannel] = &[
    SeedChannel {
        slug: "local-ai-hardware",
        title: "Hardware for Local AI",
        description: "GPU availability, VRAM benchmarks, quantization advances, and hardware acceleration for local inference.",
        topics: &[
            "gpu", "nvidia", "amd", "apple silicon", "vram", "quantization",
            "gguf", "local inference", "hardware acceleration", "npu", "cuda",
            "rocm", "metal",
        ],
    },
    SeedChannel {
        slug: "local-llm-landscape",
        title: "Local LLM Landscape",
        description: "Open-weight models, inference engines, fine-tuning techniques, and the local AI ecosystem.",
        topics: &[
            "ollama", "llama", "llm", "gguf", "mistral", "llama.cpp", "vllm",
            "mlx", "fine-tuning", "lora", "open source model", "embedding model",
            "whisper", "inference engine",
        ],
    },
    SeedChannel {
        slug: "developer-tools-shifting",
        title: "Developer Tools Shifting",
        description: "IDE evolution, AI coding assistants, build systems, and the changing developer toolchain.",
        topics: &[
            "developer tools", "cli", "ide", "vscode", "neovim", "build system",
            "ai coding", "copilot", "cursor", "toolchain", "dx", "bun", "deno",
            "turbopack",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_status_serde_roundtrip() {
        let statuses = vec![
            ChannelStatus::Active,
            ChannelStatus::Paused,
            ChannelStatus::Archived,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let back: ChannelStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn channel_status_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&ChannelStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&ChannelStatus::Paused).unwrap(),
            "\"paused\""
        );
        assert_eq!(
            serde_json::to_string(&ChannelStatus::Archived).unwrap(),
            "\"archived\""
        );
    }

    #[test]
    fn channel_freshness_serde_roundtrip() {
        let json = serde_json::to_string(&ChannelFreshness::Fresh).unwrap();
        let back: ChannelFreshness = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    #[test]
    fn channel_summary_serializes_correctly() {
        let summary = ChannelSummary {
            id: 1,
            slug: "test-channel".to_string(),
            title: "Test".to_string(),
            description: "A test channel".to_string(),
            source_count: 42,
            render_count: 3,
            freshness: ChannelFreshness::Fresh,
            last_rendered_at: Some("2026-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["slug"], "test-channel");
        assert_eq!(json["source_count"], 42);
        assert_eq!(json["freshness"], "fresh");
    }

    #[test]
    fn channel_render_serializes_correctly() {
        let render = ChannelRender {
            id: 1,
            channel_id: 1,
            version: 3,
            content_markdown: "# Test".to_string(),
            content_hash: "abc123".to_string(),
            source_item_ids: vec![10, 20, 30],
            model: Some("gpt-4".to_string()),
            tokens_used: Some(500),
            latency_ms: Some(1200),
            rendered_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&render).unwrap();
        assert_eq!(json["version"], 3);
        assert_eq!(json["source_item_ids"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn render_provenance_serializes_correctly() {
        let prov = RenderProvenance {
            render_id: 1,
            claim_index: 0,
            claim_text: "Rust is fast".to_string(),
            source_item_ids: vec![5],
            source_titles: vec!["Why Rust".to_string()],
            source_urls: vec!["https://example.com".to_string()],
        };
        let json = serde_json::to_value(&prov).unwrap();
        assert_eq!(json["claim_text"], "Rust is fast");
        assert_eq!(json["source_titles"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn channel_changelog_serializes_correctly() {
        let log = ChannelChangelog {
            channel_id: 1,
            from_version: 2,
            to_version: 3,
            summary: "Added new section".to_string(),
            added_lines: vec!["new line".to_string()],
            removed_lines: vec![],
            changed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&log).unwrap();
        assert_eq!(json["from_version"], 2);
        assert_eq!(json["to_version"], 3);
        assert_eq!(json["removed_lines"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn seed_channels_not_empty() {
        assert!(!SEED_CHANNELS.is_empty(), "seed channels must not be empty");
    }

    #[test]
    fn seed_channels_have_unique_slugs() {
        let slugs: std::collections::HashSet<&str> = SEED_CHANNELS.iter().map(|c| c.slug).collect();
        assert_eq!(
            slugs.len(),
            SEED_CHANNELS.len(),
            "duplicate slugs in SEED_CHANNELS"
        );
    }

    #[test]
    fn seed_channels_have_nonempty_fields() {
        for ch in SEED_CHANNELS {
            assert!(!ch.slug.is_empty(), "seed channel has empty slug");
            assert!(
                !ch.title.is_empty(),
                "seed channel '{}' has empty title",
                ch.slug
            );
            assert!(
                !ch.description.is_empty(),
                "seed channel '{}' has empty description",
                ch.slug
            );
            assert!(
                !ch.topics.is_empty(),
                "seed channel '{}' has no topics",
                ch.slug
            );
        }
    }

    #[test]
    fn seed_channels_topics_are_nonempty_strings() {
        for ch in SEED_CHANNELS {
            for topic in ch.topics {
                assert!(
                    !topic.is_empty(),
                    "empty topic in seed channel '{}'",
                    ch.slug
                );
            }
        }
    }

    #[test]
    fn channel_source_match_serializes_correctly() {
        let m = ChannelSourceMatch {
            channel_id: 1,
            source_item_id: 42,
            title: "Test Item".to_string(),
            url: Some("https://example.com".to_string()),
            source_type: "hackernews".to_string(),
            match_score: 0.85,
            matched_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["match_score"], 0.85);
        assert_eq!(json["source_type"], "hackernews");
    }
}
