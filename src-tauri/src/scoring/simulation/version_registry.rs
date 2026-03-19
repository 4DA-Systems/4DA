//! Pipeline Version Registry — abstracts V1/V2 pipeline selection.
//!
//! Enables tests to score items through V1, V2, or both without touching
//! the global `USE_V2` const. Both pipelines have identical function
//! signatures, making this a zero-risk abstraction.

use crate::db::Database;
use crate::SourceRelevance;

use super::super::{ScoringContext, ScoringInput, ScoringOptions};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PipelineVersion {
    V1,
    V2,
}

impl PipelineVersion {
    pub fn name(&self) -> &'static str {
        match self {
            Self::V1 => "V1",
            Self::V2 => "V2",
        }
    }
}

/// Side-by-side comparison result from both pipeline versions.
pub(super) struct VersionComparison {
    pub v1: SourceRelevance,
    pub v2: SourceRelevance,
    /// v2.top_score - v1.top_score
    pub score_delta: f32,
    /// v1.relevant != v2.relevant
    pub relevance_changed: bool,
}

// ============================================================================
// Core functions
// ============================================================================

/// Score an item through a specific pipeline version.
pub(super) fn score_with_version(
    version: PipelineVersion,
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    opts: &ScoringOptions,
) -> SourceRelevance {
    match version {
        PipelineVersion::V1 => super::super::pipeline::score_item(input, ctx, db, opts, None),
        PipelineVersion::V2 => super::super::pipeline_v2::score_item(input, ctx, db, opts, None),
    }
}

/// Score an item through both pipelines and return comparison.
pub(super) fn compare_versions(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    opts: &ScoringOptions,
) -> VersionComparison {
    let v1 = score_with_version(PipelineVersion::V1, input, ctx, db, opts);
    let v2 = score_with_version(PipelineVersion::V2, input, ctx, db, opts);
    let score_delta = v2.top_score - v1.top_score;
    let relevance_changed = v1.relevant != v2.relevant;
    VersionComparison {
        v1,
        v2,
        score_delta,
        relevance_changed,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::personas::all_personas;
    use super::super::{sim_db, sim_input, sim_no_freshness};
    use super::*;

    #[test]
    fn version_registry_v1_returns_result() {
        let db = sim_db();
        let opts = sim_no_freshness();
        let personas = all_personas();
        let emb = vec![0.0_f32; 384];
        let input = sim_input(1, "Rust memory safety", "Ownership and borrowing", &emb);
        let result = score_with_version(PipelineVersion::V1, &input, &personas[0], &db, &opts);
        assert!(result.top_score >= 0.0);
    }

    #[test]
    fn version_registry_v2_returns_result() {
        let db = sim_db();
        let opts = sim_no_freshness();
        let personas = all_personas();
        let emb = vec![0.0_f32; 384];
        let input = sim_input(1, "Rust memory safety", "Ownership and borrowing", &emb);
        let result = score_with_version(PipelineVersion::V2, &input, &personas[0], &db, &opts);
        assert!(result.top_score >= 0.0);
    }

    #[test]
    fn version_comparison_produces_delta() {
        let db = sim_db();
        let opts = sim_no_freshness();
        let personas = all_personas();
        let emb = vec![0.0_f32; 384];
        let input = sim_input(1, "Rust memory safety", "Ownership and borrowing", &emb);
        let cmp = compare_versions(&input, &personas[0], &db, &opts);
        // Delta should be finite
        assert!(cmp.score_delta.is_finite(), "score_delta should be finite");
        // Both scores should be non-negative
        assert!(cmp.v1.top_score >= 0.0);
        assert!(cmp.v2.top_score >= 0.0);
    }

    #[test]
    fn version_name_labels() {
        assert_eq!(PipelineVersion::V1.name(), "V1");
        assert_eq!(PipelineVersion::V2.name(), "V2");
    }
}
