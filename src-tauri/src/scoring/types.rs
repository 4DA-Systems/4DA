// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared scoring input/option types.
//!
//! These were declared in the (now deleted) V1 `pipeline` module. They are the
//! call contract for `scoring::score_item` and are consumed by the V2 pipeline,
//! the analysis paths, and every benchmark/simulation harness — so they live in
//! a neutral module rather than inside one pipeline implementation.

/// Input data for scoring a single item
pub(crate) struct ScoringInput<'a> {
    pub id: u64,
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub content: &'a str,
    pub source_type: &'a str,
    pub embedding: &'a [f32],
    pub created_at: Option<&'a chrono::DateTime<chrono::Utc>>,
    pub detected_lang: &'a str,
    /// Structured tags from source metadata (e.g., SO tags, Dev.to tags, arXiv categories).
    /// Enables source-fair topic extraction by providing structured signal
    /// alongside text-based extraction.
    pub source_tags: &'a [String],
    /// Raw JSON metadata from source (contains community signals like score, points).
    /// Used for community quality signal extraction in Phase 5.
    pub tags_json: Option<&'a str>,
    /// Per-feed provenance (RSS feed URL, YouTube channel ID, etc.).
    /// Used by curated feed registry to override tier and content type.
    pub feed_origin: Option<&'a str>,
    /// The adapter's stable per-item identifier (`source_items.source_id`).
    /// For registry sources this structurally names the SUBJECT package
    /// (`crate-serde`, `react@19.2.5`) — the only trustworthy grounding
    /// evidence for a registry release item. `None` only on paths that
    /// don't originate from a stored source item (ad-hoc scoring, tests).
    pub source_id: Option<&'a str>,
}

/// Options controlling which scoring stages are applied
pub(crate) struct ScoringOptions {
    pub apply_freshness: bool,
    pub apply_signals: bool,
    pub trend_topics: Vec<String>,
}
