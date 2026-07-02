#![allow(clippy::manual_range_contains)]
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
mod ace_context;
mod affinity;
pub(crate) mod aliases;
mod analyzer;
pub(crate) mod authority;
#[cfg(test)]
mod benchmark;
#[cfg(test)]
pub(crate) mod benchmark_calibration;
#[cfg(test)]
pub(crate) mod benchmark_scenarios;
mod calibration;
pub(crate) mod calibration_monitor;
mod composition;
mod context;
mod dedup;
mod dependencies;
mod explanation;
mod gate;
mod keywords;
pub(crate) mod necessity;
mod pipeline;
mod pipeline_signals;
mod pipeline_v2;
#[allow(dead_code, unused_imports)]
pub(crate) mod query_weighting;
pub(crate) mod reexamination;
mod role_inference;
mod semantic;
#[cfg(test)]
mod simulation;
pub(crate) mod stemming;
mod telemetry;
mod temporal_cluster;
pub(crate) mod triage;
mod utils;
pub(crate) mod validation;

// Public API — external callers use crate::scoring::function_name unchanged
pub(crate) use ace_context::{check_ace_exclusions, get_ace_context, ACEContext};
pub(crate) use affinity::{
    compute_affinity_multiplier, compute_anti_penalty, compute_unified_relevance,
};
pub(crate) use analyzer::{run_post_analysis_hooks, score_items_full};
pub(crate) use calibration::{calibrate_score, compute_interest_score};
pub(crate) use calibration_monitor::{
    compute_calibration_snapshot, compute_high_stakes_recall, CalibrationSnapshot, HighStakesRecall,
};
pub(crate) use composition::{enforce_composition_floors, FloorConfig};
pub(crate) use context::{
    build_scoring_context, invalidate_scoring_context_cache, is_low_quality_topic,
};
pub(crate) use dedup::{
    apply_domain_diversity, apply_source_topic_diversity, compute_serendipity_candidates,
    dedup_results, fuzzy_dedup_results, sort_results, topic_dedup_results,
};
pub(crate) use dependencies::{is_ambiguous_dep_name, match_dependencies, VersionDelta};
pub(crate) use explanation::{
    calculate_confidence, compute_temporal_freshness, generate_relevance_explanation,
};
pub(crate) use gate::apply_confirmation_gate;
pub(crate) use pipeline::{ScoringInput, ScoringOptions};
pub(crate) use pipeline_v2::finalize_scores;
pub(crate) use telemetry::ScoringTelemetry;
pub(crate) use temporal_cluster::temporal_cluster_results;
pub(crate) use triage::{triage_item, TriageReason, TriageThresholds};
/// Bump this whenever the scoring pipeline changes to invalidate stale scores.
/// Items scored under an older version will be re-scored on the next analysis run.
///
/// v5 (2026-06-04): propagate this session's scoring changes to the existing
/// backlog — necessity stack-update path (dependency releases surface instead of
/// decaying to noise), curated>synthesized domain detection, ACE topic-noise gate,
/// and the dependency generic-subterm filter. Without this bump, every backlog
/// item stayed stamped v4 = "not stale" and none of the above ever re-applied.
///
/// v6 (2026-06-14): propagate the direct-dep-CVE + clickbait scoring changes
/// (e49e978c cve_dep_match_score, 749ef4a8 direct_dep_floor 0.65, a595db05
/// clickbait hard-ceiling + domain_concerns fidelity) to the existing v5 backlog.
/// These three commits changed scoring LOGIC but shipped without a version bump, so
/// the 60.8k v5 items — including direct-dependency CVEs structurally pinned at the
/// old 0.50 floor (e.g. the live axios CVE-2026-44490) — would never re-score and
/// the fix would stay dark on the real corpus. The stale-drain re-scores the backlog
/// over subsequent cycles; this is the rule-10 dogfood window made real.
///
/// v7 (2026-06-15): semver-compat version awareness in dependency matching
/// (aa3302ee). The content-relevance path matched deps by NAME with only major-only
/// version logic — which collapsed the entire pre-1.0 crate ecosystem (gtk-rs 0.18 vs
/// 0.20 read as "same major" → boosted) and discarded the OlderMajor signal entirely,
/// so framework content rode the dependency boost regardless of version ("just because
/// it's Tauri"). Now uses the semver breaking-axis (minor for 0.x, major for >=1.0) and
/// penalizes content about versions the user has moved past. Drained in one shot via
/// `fourda.exe --engine-drain` rather than the 500/run scheduler trickle.
// v8 (2026-06-18): ubiquitous-framework relevance correction. A dep match on a
// big ubiquitous framework alone (react, vue, node, ...) no longer forces an
// off-domain item to domain_relevance 1.0 — it needs a corroborating on-stack
// topic. Closes the leak where "Show HN: AI CAD tool built with React" scored
// CORE/0.91 purely on a react dep match. See domain_profile::is_ubiquitous_framework.
//
// v9 (2026-07-02): activates the #174 canonical-grounding logic (is_strongly_grounded:
// non-dev + confidence >= 0.40 + !is_ambiguous_package_name, plus the OS-proper-noun
// ambiguity fix for windows/linux/android/macos/unix) on the existing corpus. #174
// merged 2026-06-26 WITHOUT a version bump, so the stale-drain never re-scored the
// v8 items — 65 of 77 live critical signals were phantom-grounded (measured 2026-07-02).
// No scoring-logic change in this commit; the bump makes the drain re-stamp the corpus
// with the merged logic.
pub(crate) const PIPELINE_VERSION: i32 = 9;

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
    /// Taste embedding: user's holistic preference vector (EMBEDDING_DIMS-dim, unit normalized)
    /// Computed from weighted centroid of topic affinity embeddings
    pub taste_embedding: Option<Vec<f32>>,
    /// Topic-aware decay half-lives: topic -> half_life_hours
    pub topic_half_lives: HashMap<String, f32>,
    /// Per-source engagement rates from autophagy analysis: source_type -> rate (0.0-1.0)
    pub source_autopsies: HashMap<String, f32>,
    /// Per-feed engagement rates from autophagy: feed_url -> rate (0.0-1.0)
    pub feed_autopsies: HashMap<String, f32>,
    /// Anti-pattern penalties from autophagy bias detection: source_type -> penalty (-0.15 to +0.20)
    pub anti_pattern_penalties: HashMap<String, f32>,
    /// Dismissal archetype penalties from TitanCA-inspired learning: archetype_id -> penalty (0.0-0.25)
    pub archetype_penalties: HashMap<String, f32>,
    /// Unified sovereign developer profile (assembled once per run)
    pub sovereign_profile: Option<crate::sovereign_developer_profile::SovereignDeveloperProfile>,
    /// Hours since last user interaction per topic (attention gap boost).
    pub topic_attention_gaps: HashMap<String, f32>,
    /// Topics with contradictory signals (both high affinity AND anti-topic).
    /// Content touching these topics gets a necessity boost to help resolve confusion.
    pub contradicted_topics: std::collections::HashSet<String>,
    /// Dominant persona from continuous taste inference (persona_index, weight)
    /// Present when dominant weight exceeds uniform threshold (> 0.2)
    // REMOVE BY 2026-08-10: diagnostic field — wire into score breakdown UI or delete
    #[allow(dead_code)]
    pub dominant_persona: Option<(usize, f32)>,
    /// User's professional role from onboarding (developer, security, devops, data, manager)
    pub user_role: Option<String>,
    /// User's experience level (learning, building, leading, architecting)
    pub experience_level: Option<String>,
}
