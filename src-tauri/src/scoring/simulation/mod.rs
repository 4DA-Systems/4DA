// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Simulation Infrastructure — Controlled scoring validation
//!
//! 5 simulation systems + 3 validation tiers:
//!   System 1 (lifecycle)       — multi-session convergence
//!   System 2 (reality)         — content reality testing per persona
//!   System 3 (first_run)       — first-60-seconds / bootstrap validation
//!   System 4 (differential)    — parameter regression detection
//!   System 5 (golden_snapshot) — canonical item score baselines
//!
//!   Tier 2 (tier2_semantic)    — embedding/semantic scoring validation
//!   Tier 3 (tier3_rerank)      — post-scoring reranking validation
//!   Dashboard (quality)        — aggregate quality reporting

pub(super) mod ace_validation;
pub(super) mod corpus;
pub(super) mod differential;
pub(super) mod domain_embeddings;
pub(super) mod enriched_lifecycle;
pub(super) mod enrichment;
pub(super) mod feedback_sim;
#[cfg(test)]
pub(super) mod feedback_sim_tests;
pub(super) mod first_run;
// Recall-investigation fixture generator + I/O (test infrastructure, off by
// default). Only compiled when a simulation-fixture feature is enabled.
#[cfg(feature = "generate-sim-fixtures")]
pub(super) mod fixtures_gen;
#[cfg(any(feature = "calibrated-sim", feature = "generate-sim-fixtures"))]
pub(super) mod fixtures_io;
pub(super) mod golden_snapshot;
pub(super) mod lifecycle;
pub(super) mod live_reality_check;
pub(super) mod metrics;
pub(super) mod persona_data;
pub(super) mod personas;
pub(super) mod quality_dashboard;
pub(super) mod reality;
pub(super) mod tier2_semantic;
pub(super) mod tier3_rerank;
pub(super) mod version_comparison;
pub(super) mod version_registry;

use super::{ScoringInput, ScoringOptions};
use std::path::Path;

// ============================================================================
// Shared infrastructure (mirrors benchmark.rs helpers, self-contained)
// ============================================================================

pub(super) fn sim_db() -> crate::db::Database {
    crate::register_sqlite_vec_extension();
    crate::db::Database::new(Path::new(":memory:")).expect("in-memory DB")
}

pub(super) fn sim_no_freshness() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
        trend_topics: vec![],
    }
}

/// Load pre-computed corpus embeddings for simulation.
/// Bridges reality.rs → mod.rs → domain_embeddings.rs.
///
/// DEFAULT (no `calibrated-sim`): SYNTHETIC block-signature embeddings — the
/// regression baseline golden_snapshot.rs + reality.rs are calibrated against.
#[cfg(not(feature = "calibrated-sim"))]
pub(super) fn load_corpus_embeddings() -> Vec<Vec<f32>> {
    domain_embeddings::corpus_embeddings()
}

/// `calibrated-sim`: REAL fastembed corpus embeddings loaded from the committed
/// `fixtures/corpus_embeddings.bin`. Indexed by `id - 1` (matching the synthetic
/// layout the consumers expect); any id gap is left as a zero vector. Fails
/// loudly if the fixture is missing or malformed — there is no silent fallback,
/// because a missing fixture would otherwise masquerade as a recall collapse.
#[cfg(feature = "calibrated-sim")]
pub(super) fn load_corpus_embeddings() -> Vec<Vec<f32>> {
    const BYTES: &[u8] = include_bytes!("fixtures/corpus_embeddings.bin");
    let records = fixtures_io::deserialize_u32_keyed(BYTES)
        .expect("calibrated-sim: corpus_embeddings.bin is missing or malformed — regenerate via `cargo test --features generate-sim-fixtures ... -- --ignored`");
    let max_id = records.iter().map(|(id, _)| *id).max().unwrap_or(0) as usize;
    let mut out = vec![vec![0.0_f32; crate::EMBEDDING_DIMS]; max_id];
    for (id, v) in records {
        if id >= 1 {
            out[(id - 1) as usize] = v;
        }
    }
    out
}

/// `calibrated-sim`: REAL fastembed topic/interest embeddings keyed by string
/// (exact + lowercase), loaded from `fixtures/topic_embeddings.bin`. Feeds the
/// production semantic-ACE-boost path via `ScoringContext.topic_embeddings`.
#[cfg(feature = "calibrated-sim")]
pub(super) fn load_topic_embeddings() -> std::collections::HashMap<String, Vec<f32>> {
    const BYTES: &[u8] = include_bytes!("fixtures/topic_embeddings.bin");
    let records = fixtures_io::deserialize_str_keyed(BYTES)
        .expect("calibrated-sim: topic_embeddings.bin is missing or malformed — regenerate via `cargo test --features generate-sim-fixtures ... -- --ignored`");
    records.into_iter().collect()
}

pub(super) fn sim_input<'a>(
    id: u64,
    title: &'a str,
    content: &'a str,
    embedding: &'a [f32],
) -> ScoringInput<'a> {
    ScoringInput {
        id,
        title,
        url: Some("https://example.com"),
        content,
        source_type: "hackernews",
        embedding,
        created_at: None,
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
    }
}

// ============================================================================
// Shared Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContentCategory {
    DirectMatch,
    AdjacentMatch,
    CrossDomainNoise,
    Borderline,
    CareerNoise,
    SecurityAdvisory,
    IntroductoryNoise,
    ShowHNNoise,
    MetaNoise,
    BusinessNoise,
    ReleaseNotes,
    HNDiscussion,
    DistantlyRelevant,
    ReverseEngineering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpectedOutcome {
    StrongRelevant,
    WeakRelevant,
    MildBorderline,
    NotRelevant,
    #[allow(dead_code)] // REMOVE BY 2026-11-26 — test outcome variant
    Excluded,
}

pub(super) struct LabeledItem {
    pub id: u64,
    pub title: &'static str,
    pub content: &'static str,
    pub category: ContentCategory,
    pub expected: [ExpectedOutcome; 9],
    #[allow(dead_code)] // REMOVE BY 2026-11-26 — test fixture metadata
    pub source_type: &'static str,
}

pub(super) const PERSONA_NAMES: [&str; 9] = [
    "rust_systems",
    "python_ml",
    "fullstack_ts",
    "devops_sre",
    "mobile_dev",
    "bootstrap",
    "power_user",
    "context_switcher",
    "niche_specialist",
];

pub(super) const PI_RUST: usize = 0;
pub(super) const PI_PYTHON: usize = 1;
pub(super) const PI_TS: usize = 2;
pub(super) const PI_DEVOPS: usize = 3;
#[allow(dead_code)] // REMOVE BY 2026-11-26 — persona index constants for future simulation expansion
pub(super) const PI_MOBILE: usize = 4;
#[allow(dead_code)] // REMOVE BY 2026-11-26 — persona index constants for future simulation expansion
pub(super) const PI_BOOTSTRAP: usize = 5;
#[allow(dead_code)] // REMOVE BY 2026-11-26 — persona index constants for future simulation expansion
pub(super) const PI_POWER: usize = 6;
#[allow(dead_code)] // REMOVE BY 2026-11-26 — persona index constants for future simulation expansion
pub(super) const PI_SWITCHER: usize = 7;
pub(super) const PI_NICHE: usize = 8;
