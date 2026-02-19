// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ContextFile {
    pub path: String,
    pub content: String,
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSettings {
    pub configured_dirs: Vec<String>,
    pub active_dirs: Vec<String>,
    pub using_default: bool,
}

/// Generic source item for multi-source support
#[derive(Debug, Clone)]
pub(crate) struct GenericSourceItem {
    pub id: u64,
    pub source_id: String,
    pub source_type: String,
    pub title: String,
    pub url: Option<String>,
    pub content: String,
}

/// Relevance match between an HN item and context
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RelevanceMatch {
    pub source_file: String,
    pub matched_text: String,
    pub similarity: f32,
}

/// Detailed breakdown of score components
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ScoreBreakdown {
    pub context_score: f32,
    pub interest_score: f32,
    #[serde(default)]
    pub keyword_score: f32,
    pub ace_boost: f32,
    pub affinity_mult: f32,
    pub anti_penalty: f32,
    #[serde(default = "default_freshness")]
    pub freshness_mult: f32,
    #[serde(default)]
    pub feedback_boost: f32,
    #[serde(default)]
    pub source_quality_boost: f32,
    pub confidence_by_signal: HashMap<String, f32>,
    /// Number of independent signal axes that confirmed relevance (0-5)
    #[serde(default)]
    pub signal_count: u8,
    /// Names of confirmed signal axes (e.g. ["context", "ace", "dependency"])
    #[serde(default)]
    pub confirmed_signals: Vec<String>,
    /// Multiplier applied by confirmation gate
    #[serde(default = "default_confirmation_mult")]
    pub confirmation_mult: f32,
    /// Dependency match score (0.0-1.0): how strongly content matches user's installed packages
    #[serde(default)]
    pub dep_match_score: f32,
    /// Package names from user's dependency graph that matched this content
    #[serde(default)]
    pub matched_deps: Vec<String>,
    /// Domain relevance (0.15 off-domain to 1.0 primary stack match)
    #[serde(default = "default_domain_relevance")]
    pub domain_relevance: f32,
    /// Content quality multiplier (0.5 clickbait to 1.2 authoritative)
    #[serde(default = "default_quality_mult")]
    pub content_quality_mult: f32,
    /// Novelty multiplier (0.6 introductory to 1.15 release)
    #[serde(default = "default_quality_mult")]
    pub novelty_mult: f32,
    /// Intent boost from recent work topics (0.0 to 0.15)
    #[serde(default)]
    pub intent_boost: f32,
    /// Content type classification (e.g. "security_advisory", "show_and_tell")
    #[serde(default)]
    pub content_type: Option<String>,
    /// Content DNA utility multiplier (0.3 hiring to 1.3 security)
    #[serde(default = "default_quality_mult")]
    pub content_dna_mult: f32,
    /// Competing tech penalty multiplier (0.5 or 1.0)
    #[serde(default = "default_quality_mult")]
    pub competing_mult: f32,
    /// Stack intelligence: pain point and keyword boost (0.0-0.20)
    #[serde(default)]
    pub stack_boost: f32,
    /// Stack intelligence: ecosystem shift multiplier (0.95-1.25, default 1.0)
    #[serde(default = "default_quality_mult")]
    pub ecosystem_shift_mult: f32,
    /// Stack intelligence: competing tech suppression (0.95 or 1.0)
    #[serde(default = "default_quality_mult")]
    pub stack_competing_mult: f32,
    /// LLM relevance score (1-5 scale, None if LLM skipped)
    #[serde(default)]
    pub llm_score: Option<f32>,
    /// LLM's one-sentence explanation
    #[serde(default)]
    pub llm_reason: Option<String>,
}

pub(crate) fn default_freshness() -> f32 {
    1.0
}

pub(crate) fn default_confirmation_mult() -> f32 {
    1.0
}

pub(crate) fn default_domain_relevance() -> f32 {
    1.0
}

pub(crate) fn default_quality_mult() -> f32 {
    1.0
}

/// Full relevance result for a source item (HN, arXiv, Reddit, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SourceRelevance {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub top_score: f32,
    pub matches: Vec<RelevanceMatch>,
    pub relevant: bool,
    /// Score from context files (what you're working on)
    #[serde(default)]
    pub context_score: f32,
    /// Score from explicit interests (what you care about)
    #[serde(default)]
    pub interest_score: f32,
    /// Whether this item was filtered by an exclusion
    #[serde(default)]
    pub excluded: bool,
    /// The exclusion that blocked this item (if any)
    #[serde(default)]
    pub excluded_by: Option<String>,
    /// Source type (hackernews, arxiv, reddit)
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// Human-readable explanation of why this item was surfaced
    #[serde(default)]
    pub explanation: Option<String>,
    /// Overall confidence score (0.0-1.0)
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Detailed score breakdown for debugging
    #[serde(default)]
    pub score_breakdown: Option<ScoreBreakdown>,
    /// Signal classification type (security_alert, breaking_change, etc.)
    #[serde(default)]
    pub signal_type: Option<String>,
    /// Signal priority level (critical, high, medium, low)
    #[serde(default)]
    pub signal_priority: Option<String>,
    /// Suggested action based on signal classification
    #[serde(default)]
    pub signal_action: Option<String>,
    /// Keywords that triggered the classification
    #[serde(default)]
    pub signal_triggers: Option<Vec<String>>,
    /// Signal time horizon (tactical = act now, strategic = plan ahead)
    #[serde(default)]
    pub signal_horizon: Option<String>,
    /// How many similar items were grouped under this representative (topic dedup)
    #[serde(default)]
    pub similar_count: u32,
    /// Titles of grouped similar items
    #[serde(default)]
    pub similar_titles: Vec<String>,
    /// Whether this item was injected by the serendipity engine (anti-bubble)
    #[serde(default)]
    pub serendipity: bool,
}

pub(crate) fn default_source_type() -> String {
    "hackernews".to_string()
}

/// Status update for the UI (sent via events)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AnalysisStatus {
    pub stage: String,
    pub progress: f32,
    pub message: String,
    pub items_processed: usize,
    pub items_total: usize,
}

/// Background analysis state
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AnalysisState {
    pub running: bool,
    pub completed: bool,
    pub error: Option<String>,
    pub results: Option<Vec<SourceRelevance>>,
    /// When analysis started (unix timestamp seconds)
    #[serde(default)]
    pub started_at: Option<i64>,
    /// When analysis last completed successfully (ISO string for DB query compat)
    #[serde(default)]
    pub last_completed_at: Option<String>,
}

/// Maximum analysis duration in seconds before auto-timeout
pub(crate) const ANALYSIS_TIMEOUT_SECS: i64 = 300;

/// LLM judgment attached to a relevance result
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct LLMJudgment {
    pub relevant: bool,
    pub confidence: f32,
    pub reasoning: String,
    pub key_connections: Vec<String>,
}

/// Enhanced relevance result with optional LLM judgment
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EnhancedRelevance {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub embedding_score: f32,
    pub matches: Vec<RelevanceMatch>,
    pub embedding_relevant: bool,
    /// LLM re-ranking judgment (if enabled)
    pub llm_judgment: Option<LLMJudgment>,
    /// Final relevance after both stages
    pub final_relevant: bool,
}

#[cfg(test)]
mod binding_tests {
    use super::*;

    #[test]
    fn export_bindings() {
        // ts-rs auto-exports when the test runs
        // Just reference the types to ensure they compile
        let _ = std::any::type_name::<ContextFile>();
        let _ = std::any::type_name::<RelevanceMatch>();
        let _ = std::any::type_name::<ScoreBreakdown>();
        let _ = std::any::type_name::<SourceRelevance>();
        let _ = std::any::type_name::<AnalysisStatus>();
        let _ = std::any::type_name::<AnalysisState>();
        let _ = std::any::type_name::<LLMJudgment>();
        let _ = std::any::type_name::<EnhancedRelevance>();
    }
}
