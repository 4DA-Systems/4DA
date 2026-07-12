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
pub(crate) mod cvss;
mod dedup;
mod dependencies;
mod explanation;
mod explanation_chain;
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
pub(crate) use dependencies::{
    is_ambiguous_dep_name, is_generic_topic_token, match_dependencies, VersionDelta,
    STRONG_GROUNDING_CONFIDENCE,
};
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
//
// v10 (2026-07-02): gate count-inflation fixes — zero-vector KNN context guard,
// word-boundary keyword matching + generic-interest specificity, critical
// fast-path requires canonical strong grounding. Without the bump these logic
// changes stay dark on the v9 corpus (same failure mode as v6/v9, documented
// above).
//
// v11 (2026-07-03): text-match grounding requires NAME CORROBORATION — a dep
// match grounds (Critical gate / strongly_grounded / fast-path) only when the
// item actually names the package: a full-name token occurrence, with word-like
// single-token names additionally needing software context or an adjacent
// version literal (mirrors dep_linker's persistence proof grades). Kills the
// residual 29 phantom criticals measured post-v9-drain, all minted by subterm
// expansions ("anthropic" -> @anthropic-ai/sdk on company news, "router" ->
// react-router-dom on a Zyxel headline, "updater" -> tauri-plugin-updater on an
// AMD story) or alias overlaps ("sqlite3" -> better-sqlite3 on a sqlite-utils
// release). The structured-advisory route (Affected-packages metadata) is an
// independent proof and is unchanged.
//
// NO bump for the 2026-07-04 canonical project-inclusion change. Scoring's
// project-context inputs are: (a) project_dependencies via
// load_dependency_intelligence AND pipeline_v2's direct read, (b)
// detected_tech via ace_context (tech stack -> synthesized interests). Both
// were checked against the live corpus by read-only probe:
// (a) project_dependencies carries no tier-1/2 rows (the June purge cleared
//     .claude paths; the placeholder pollution lives in user_dependencies /
//     dependency_snapshots, which feed OSV alert surfaces, not score math),
// (b) detected_tech has only two rows with scaffolding evidence, both MIXED
//     with real-project evidence — the startup purge only rewrites their
//     evidence strings; the tech NAME set and confidences scoring consumes
//     are unchanged (rows evidenced solely by scaffolding would be deleted,
//     but none exist live).
// With excluded_project_paths also empty, scoring inputs are byte-identical.
// If a user later excludes a project, scores refresh through the normal
// rescan path — no corpus re-stamp required.
//
// v12 (2026-07-04): interest/topic corroboration — fragment matches can no
// longer confirm axes (Wave 7). `topic_overlaps` (any shared >=3-char
// fragment) is replaced by strict `topic_grounds` at every scoring call site
// (ACE confirmation axis, skill-gap boost, affinity/anti-penalty, intent
// boost, keyword ACE boost, dependency topic-overlap): generic tokens
// (COMMON_ENGLISH_WORDS + tech-generic topic tokens) can never ground, and a
// fragment overlap counts only when the shared fragment is specific. Kills
// the class where user dep `tower-http` confirmed item topic `http` and an
// arXiv "HTTP REST API" paper ranked #1 CORE.
// Also in v12 (same wave): commodity ceiling extended to AcademicPaper and
// ShowAndTell. New ContentType::AcademicPaper (arXiv/PwC manifests, neutral
// 1.0 instead of DeepDive 1.15); papers bypass the ceiling ONLY via strong
// dependency grounding or security/version evidence (sophistication and
// community-signal bypasses withheld — academic prose trips both). ShowAndTell
// keeps the standard bypasses so traction-validated self-promo still surfaces.
//
// v13 (2026-07-05): decision-window matching corroborated. compute_match_score
// used raw substring `contains` for all three signals, so a window whose
// dependency was an ambiguous import-scraped name ("http") matched every item
// containing a URL (+0.6 > the 0.10 necessity threshold) — stamping
// "Relevant to open decision" on unrelated items the moment v12's honest
// labels exposed it. Now: dep signal uses package_ambiguity::dep_grounded_match
// (the same matcher window minting uses), topic signal requires a non-generic
// topic on a word boundary, title-keyword signal picks the first non-generic
// significant word and matches on a word boundary.
//
// v14 (2026-07-05): import-scraped builtin modules no longer persisted as user
// dependencies + purge — dep inputs to scoring changed (Wave 8a). The import
// scraper wrote Node builtins (fs, path, http, ...) and Python stdlib modules
// as version-less direct deps; those rows fed dependency grounding and minted
// the phantom "Security: http" decision-window class v13 had to defang at
// match time. This wave kills them at the SOURCE (scanner skip via
// ace::builtin_modules + startup purge of existing rows + stale-window close),
// marks go.mod `// indirect` modules is_direct=0, parses pyproject
// dependencies section-aware instead of whole-file substring, and prunes deps
// of deleted/moved projects after each full scan. Adversarial-review
// hardening (same wave): migration 87 adds a `detected_from` provenance
// column ('manifest' | 'lockfile' | 'import_scrape' | legacy 'unknown') to
// user_dependencies + project_dependencies — the purge deletes import_scrape
// builtins by provenance, keeps manifest-declared builtin-shadow packages
// (npm `buffer` polyfill), and treats provenance-unknown rows with the
// one-shot version-NULL heuristic (js/py/go). The bump re-stamps the corpus
// so items scored against the polluted dep set re-score against the clean
// one (same failure mode as v6/v9: merged-but-dark without a bump).
//
// v15 (2026-07-12): commodity-ceiling coverage for two CORE-band leaks the
// synthesized brief was silently cleaning up (raw feed / attention cards / Signal
// graph showed them, the LLM filtered them): (1) `hiring` job posts scored CORE
// (0.81–0.91) off pure stack-keyword overlap — now capped at 0.28 with no
// crowd/sophistication bypass; (2) off-stack `security_advisory` items (a CVE for
// a package NOT in the user's deps — 9router 0.91, rama 0.78) rode the
// security-pattern exemption to CORE — now capped to the MATCH band (0.44) unless
// strongly grounded in the user's dependency graph. The bump re-stamps the corpus
// so the polluted CORE band re-scores clean.
pub(crate) const PIPELINE_VERSION: i32 = 15;

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
pub(crate) use utils::{has_word_boundary_match, topic_grounds};

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
