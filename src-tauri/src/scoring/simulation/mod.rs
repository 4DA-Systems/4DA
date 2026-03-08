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
    }
}

/// Load pre-computed corpus embeddings for simulation.
/// Bridges reality.rs → mod.rs → domain_embeddings.rs.
pub(super) fn load_corpus_embeddings() -> Vec<Vec<f32>> {
    domain_embeddings::corpus_embeddings()
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
    Excluded,
}

pub(super) struct LabeledItem {
    pub id: u64,
    pub title: &'static str,
    pub content: &'static str,
    pub category: ContentCategory,
    pub expected: [ExpectedOutcome; 9],
    #[allow(dead_code)]
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

#[allow(dead_code)]
pub(super) const PI_RUST: usize = 0;
#[allow(dead_code)]
pub(super) const PI_PYTHON: usize = 1;
#[allow(dead_code)]
pub(super) const PI_TS: usize = 2;
#[allow(dead_code)]
pub(super) const PI_DEVOPS: usize = 3;
#[allow(dead_code)]
pub(super) const PI_MOBILE: usize = 4;
#[allow(dead_code)]
pub(super) const PI_BOOTSTRAP: usize = 5;
#[allow(dead_code)]
pub(super) const PI_POWER: usize = 6;
#[allow(dead_code)]
pub(super) const PI_SWITCHER: usize = 7;
#[allow(dead_code)]
pub(super) const PI_NICHE: usize = 8;
