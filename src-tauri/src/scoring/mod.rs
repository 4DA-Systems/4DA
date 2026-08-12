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
pub(crate) mod epochs;
mod explanation;
mod explanation_chain;
mod gate;
mod keywords;
pub(crate) mod necessity;
mod pipeline_signals;
mod pipeline_v2;
pub(crate) mod query_weighting;
pub(crate) mod reexamination;
#[cfg(test)]
mod registry_grounding_tests;
mod role_inference;
mod semantic;
#[cfg(test)]
mod simulation;
pub(crate) mod stemming;
mod telemetry;
mod temporal_cluster;
pub(crate) mod triage;
mod types;
mod utils;
#[cfg(test)]
pub(crate) mod validation;

// Public API — external callers use crate::scoring::function_name unchanged
pub(crate) use ace_context::{get_ace_context, ACEContext};
pub(crate) use affinity::compute_affinity_multiplier;
pub(crate) use analyzer::{run_post_analysis_hooks, score_items_full};
pub(crate) use calibration::calibrate_score;
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
    is_ambiguous_dep_name, is_generic_topic_token, match_dependencies, STRONG_GROUNDING_CONFIDENCE,
};
pub(crate) use explanation::{calculate_confidence, compute_temporal_freshness};
pub(crate) use pipeline_v2::finalize_scores;
pub(crate) use telemetry::ScoringTelemetry;
pub(crate) use temporal_cluster::temporal_cluster_results;
pub(crate) use triage::{triage_item, TriageReason, TriageThresholds};
pub(crate) use types::{ScoringInput, ScoringOptions};
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
//
// v17 (2026-07-23): signal-feed precision — registry-release grounding +
// federated-social community caps + stack-confidence floor. Live evidence:
// ~81% of relevant items were crates_io releases, with non-dependency
// look-alike crates (forge-plugin-sdk-rust 0.947, axum-connect-rpc 0.926,
// serde_v8 0.700 — all dep_links=0) out-scoring the user's real dependency
// releases (axum 0.888, tokio 0.873), and mastodon/lemmy promo/social noise
// (0.61–0.91) riding the neutral community-signal arm. Changes: (1)
// ungrounded registry releases (subject NOT a user dependency) lose the
// ReleaseNotes boost and take registry_release_grounding.ungrounded_penalty
// (0.35) — the registry signal is releases of YOUR dependencies; (2)
// mastodon/lemmy/bluesky/twitter join community-signal scoring (no-metadata
// default 0.25) and the UGC low-community cap list; (3) auto-detected stack
// profiles below confidence 0.5 (stale java_enterprise 0.18 etc.) no longer
// compose into scoring. The bump re-stamps the corpus so the crates-flooded
// relevant band re-scores clean.
//
// v18 (2026-07-26): look-alike registry releases become a CATEGORICAL
// non-verdict. v17 capped them at 0.35, below the 0.40 relevance threshold —
// but the ceiling is applied inside `apply_final_adjustments`, while
// `normalize_score_offset` (+0.02) and the topic-attention-gap boost (+0.05)
// run AFTER it. Live evidence (post-v17 drained corpus, 2026-07-26): 350
// capped items piled at exactly 0.37 (ceiling+offset) and 84 at exactly 0.42
// (ceiling+offset+full boost) — the 0.42 band cleared the threshold, and 26 of
// those were already `feed_relevant = 1` (vvva_js, wasm4pm, polint, deppilot,
// serde_v8 …) against a 436-item feed. Change: the `relevant` verdict is now
// gated on `!ungrounded_registry_release`, making it score-independent so no
// future post-ceiling boost can reopen the hole. Zero recall cost — the
// critical security fast-path requires `grounding.strong`, which
// `ungrounded_registry_release` negates, so the two are mutually exclusive by
// construction. This bump IS registered in `scoring::epochs::SCOPED_EPOCHS`:
// only registry-source items can change verdict, so the rest of the corpus is
// promoted untouched instead of re-scored.
//
// v19 (2026-08-11): BEHAVIORAL-LEARNING DEMOTION (AD-029) + signalchain
// hardening + score-side cap re-assertion. NOT scope-registered — full drain.
// (a) Behavioral signals lose scoring authority: engagement multiplier
//     reduced to the item-side community term; learned gate axis never
//     confirms; topic-attention-gap boost DELETED (the v18 incident
//     mechanism); affinity mult / anti-topic penalty / feedback boosts /
//     taste embedding / persona boosts / stability-facet injections /
//     autophagy corrections (calibration deltas, topic half-lives,
//     source+feed autopsies, anti-patterns, archetypes) all neutralized at
//     the context loader; ACE anti-topic auto-exclusions removed
//     (user-authored exclusions remain); synthetic affinity seeding
//     removed; bootstrap branches made unconditional (2× dep weight,
//     min_signals=1 — both were the permanent live behavior anyway).
//     Evidence: 2026-07-13 doom loop (own stack at −1.0 affinity from
//     scroll noise), 2026-08-11 degenerate calibration curve (honest 1/5
//     judgments remapped to 5/5, +0.15/item every cycle), three
//     incompatible capture scales, and a loop that never had enough clean
//     labels to measure a lift.
// (b) Signalchain hardening (was dark, unbumped, on codex/signalchain):
//     phantom dep-match kill for non-security items, ungrounded registry
//     context-axis dampening (×0.3), domain-gated trend boost.
// (c) `ScoreBreakdown::score_ceiling` + re-assertion in `finalize_scores`:
//     categorical caps now survive every post-pipeline writer
//     (cross-encoder, dedup boost, source-tier normalize, LLM reconciler).
// (d) Serendipity: budget-true injection (no forced ≥1/cycle floor) and
//     14-day verdict expiry (was immune forever; measured 17.6% of the
//     curated feed vs 8% configured).
// (e) Threshold auto-tuners frozen (two conflicting tuners + a kv
//     resurrection path); threshold is the fixed default.
pub(crate) const PIPELINE_VERSION: i32 = 19;

/// Score a single item through the PASIFA V2 pipeline.
///
/// The V1 pipeline was deleted 2026-08-12: dispatch had been pinned to V2 by a
/// hardcoded `const USE_V2: bool = true` with no cfg/env/test override, so the
/// V1 arm had been structurally unreachable while still being co-maintained.
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &crate::db::Database,
    options: &ScoringOptions,
    classifier: Option<&crate::signals::SignalClassifier>,
) -> crate::SourceRelevance {
    pipeline_v2::score_item(input, ctx, db, options, classifier)
}
pub(crate) use semantic::{compute_semantic_ace_boost, get_topic_embeddings};
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
    ///
    /// Loaded permanently empty and read by nothing since AD-029 demoted the
    /// behavioural scoring signals (V2 pins `source_quality_boost` to 0.0). The
    /// field is retained deliberately as AD-029 scaffolding — it still carries
    /// the simulation's enrichment knob — and must NOT be deleted until that
    /// decision is revisited against AD-029's re-enable criteria.
    #[allow(dead_code)] // REMOVE BY 2026-11-12
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
    /// Anti-pattern penalties from autophagy bias detection: source_type -> penalty (-0.15 to +0.20)
    pub anti_pattern_penalties: HashMap<String, f32>,
    /// Dismissal archetype penalties from TitanCA-inspired learning: archetype_id -> penalty (0.0-0.25)
    pub archetype_penalties: HashMap<String, f32>,
    /// Unified sovereign developer profile (assembled once per run)
    pub sovereign_profile: Option<crate::sovereign_developer_profile::SovereignDeveloperProfile>,
    /// Topics with contradictory signals (both high affinity AND anti-topic).
    /// Content touching these topics gets a necessity boost to help resolve confusion.
    pub contradicted_topics: std::collections::HashSet<String>,
    // dominant_persona removed at its 2026-08-10 deadline (v19/AD-029: the
    // persona posterior no longer feeds scoring; the diagnostic field was
    // never wired into the breakdown UI).
    /// User's professional role from onboarding (developer, security, devops, data, manager)
    pub user_role: Option<String>,
    /// User's experience level (learning, building, leading, architecting)
    pub experience_level: Option<String>,
}
