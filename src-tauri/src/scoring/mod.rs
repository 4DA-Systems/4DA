#![allow(clippy::manual_range_contains)]
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
mod ace_context;
mod affinity;
mod analyzer;
pub(crate) mod authority;
#[cfg(test)]
mod benchmark;
mod calibration;
mod composition;
mod context;
mod dedup;
mod dependencies;
mod explanation;
mod gate;
mod keywords;
pub(crate) mod necessity;
mod pipeline;
mod pipeline_v2;
mod semantic;
#[cfg(test)]
mod simulation;
mod temporal_cluster;
mod utils;
#[allow(dead_code, unused_imports)]
pub(crate) mod validation;

// Public API — external callers use crate::scoring::function_name unchanged
pub(crate) use ace_context::{check_ace_exclusions, get_ace_context, ACEContext};
pub(crate) use affinity::{
    compute_affinity_multiplier, compute_anti_penalty, compute_unified_relevance,
};
pub(crate) use analyzer::{run_background_analysis, run_post_analysis_hooks, score_items_full};
pub(crate) use calibration::{calibrate_score, compute_interest_score};
pub(crate) use composition::{enforce_composition_floors, FloorConfig};
pub(crate) use context::build_scoring_context;
pub(crate) use dedup::{
    compute_serendipity_candidates, dedup_results, fuzzy_dedup_results, sort_results,
    topic_dedup_results,
};
pub(crate) use dependencies::{match_dependencies, VersionDelta};
pub(crate) use explanation::{
    calculate_confidence, compute_temporal_freshness, generate_relevance_explanation,
};
pub(crate) use gate::apply_confirmation_gate;
pub(crate) use pipeline::{ScoringInput, ScoringOptions};
pub(crate) use temporal_cluster::temporal_cluster_results;
// Runtime dispatch: V2 pipeline with 8-phase architecture, fallback to V1
const USE_V2: bool = true;
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &crate::db::Database,
    options: &ScoringOptions,
    classifier: Option<&crate::signals::SignalClassifier>,
) -> crate::SourceRelevance {
    if USE_V2 {
        pipeline_v2::score_item(input, ctx, db, options, classifier)
    } else {
        pipeline::score_item(input, ctx, db, options, classifier)
    }
}
pub(crate) use semantic::{
    compute_semantic_ace_boost, compute_taste_embedding, get_topic_embeddings,
};
pub(crate) use utils::{has_word_boundary_match, topic_overlaps};

use std::collections::HashMap;

use crate::context_engine;
use fourda_macros::ScoringBuilder;

/// Pre-loaded context for scoring (computed once per analysis run)
#[derive(ScoringBuilder, Clone)]
pub(crate) struct ScoringContext {
    pub cached_context_count: i64,
    pub interest_count: usize,
    pub interests: Vec<context_engine::Interest>,
    pub exclusions: Vec<String>,
    pub ace_ctx: ACEContext,
    pub topic_embeddings: HashMap<String, Vec<f32>>,
    /// Feedback-derived topic boosts: topic -> net_score (-1.0 to 1.0)
    pub feedback_boosts: HashMap<String, f64>,
    /// Source quality scores from learned preferences: source_type -> score (-1.0 to 1.0)
    pub source_quality: HashMap<String, f32>,
    /// User's explicitly declared tech stack (3-5 items from onboarding).
    /// Used for signal action text and priority escalation — much smaller than detected_tech.
    pub declared_tech: Vec<String>,
    /// Domain profile: graduated technology identity for domain relevance scoring
    pub domain_profile: crate::domain_profile::DomainProfile,
    /// Recent work topics from git activity (last 2h) for intent-aware scoring
    pub work_topics: Vec<String>,
    /// Total feedback interactions — used to detect bootstrap mode for new users
    pub feedback_interaction_count: i64,
    /// Composed stack profile for stack-aware scoring (inactive when no stacks selected)
    pub composed_stack: crate::stacks::ComposedStack,
    /// Open decision windows for boost injection
    pub open_windows: Vec<crate::decision_advantage::DecisionWindow>,
    /// Autophagy calibration deltas: topic -> delta (scoring correction)
    pub calibration_deltas: HashMap<String, f32>,
    /// Taste embedding: user's holistic preference vector (384-dim, unit normalized)
    /// Computed from weighted centroid of topic affinity embeddings
    pub taste_embedding: Option<Vec<f32>>,
    /// Topic-aware decay half-lives: topic -> half_life_hours
    pub topic_half_lives: HashMap<String, f32>,
    /// Per-source engagement rates from autophagy analysis: source_type -> rate (0.0-1.0)
    pub source_autopsies: HashMap<String, f32>,
    /// Anti-pattern penalties from autophagy bias detection: source_type -> penalty (-0.15 to +0.20)
    pub anti_pattern_penalties: HashMap<String, f32>,
    /// Unified sovereign developer profile (assembled once per run)
    pub sovereign_profile: Option<crate::sovereign_developer_profile::SovereignDeveloperProfile>,
    /// Topics with contradictory signals (both high affinity AND anti-topic).
    /// Content touching these topics gets a necessity boost to help resolve confusion.
    pub contradicted_topics: std::collections::HashSet<String>,
    /// Dominant persona from continuous taste inference (persona_index, weight)
    /// Present when dominant weight exceeds uniform threshold (> 0.2)
    // Diagnostic: populated for scoring introspection
    #[allow(dead_code)] // Reason: diagnostic field for scoring introspection
    pub dominant_persona: Option<(usize, f32)>,
    /// User's professional role from onboarding (developer, security, devops, data, manager)
    pub user_role: Option<String>,
    /// User's experience level (learning, building, leading, architecting)
    pub experience_level: Option<String>,
}
