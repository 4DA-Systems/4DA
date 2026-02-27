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
