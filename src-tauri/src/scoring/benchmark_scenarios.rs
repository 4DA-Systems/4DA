// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! PASIFA Scoring Benchmark — JSON-driven scenario evaluation
//!
//! Evaluates the full scoring pipeline against 62 labeled test scenarios
//! across 5 categories (true_positive, true_negative, security, edge_case, cold_start).
//!
//! Run: `cargo test scoring::benchmark_scenarios -- --nocapture`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use super::benchmark::no_freshness;
use super::types::ScoringInput;
use super::*;

const SCENARIOS_JSON: &str = include_str!("benchmark_scenarios.json");

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Scenario {
    pub id: String,
    pub category: String,
    pub description: String,
    pub item: ScenarioItem,
    pub profile: String,
    pub expected: Expected,
    /// Score with `apply_freshness: true` (temporal evidence ON — freshness
    /// tiers + the stale-published discount). Default false: most scenarios
    /// pin non-temporal semantics and deliberately neutralize time. Scenarios
    /// that exist to exercise temporal evidence (harness_coverage's stale-
    /// published pair) opt in. 2026-08-23 audit, item 22a.
    #[serde(default)]
    pub apply_freshness: bool,
    /// v29 (2026-09-04): the VERDICT (`expected.should_be_relevant`) is a hard
    /// assertion for this scenario, not just the score band. Score bands
    /// pin the shape of the pipeline but cannot pin what a developer sees:
    /// 32 of 87 bands straddled the 0.40 line and six whole bands
    /// contradicted their own label while CI stayed green. A pinned verdict
    /// counts as passed only when `relevant` matches — or the score sits
    /// within `VERDICT_MARGIN` of the relevance line, the measured
    /// cross-machine embedding noise (#527).
    #[serde(default)]
    pub verdict_pinned: bool,
}

/// Tolerance around the relevance threshold inside which a pinned verdict may
/// flip without failing: real-embedding scores move ~0.01–0.02 between
/// machines (the #527 flake), so a verdict decided by a score that close to
/// the line is noise, not a regression.
pub(crate) const VERDICT_MARGIN: f32 = 0.03;

/// Did a scenario pass? Score in band, AND (for pinned scenarios) the verdict
/// matches or the score is inside the noise margin of the relevance line.
/// ONE rule for every runner (synthetic, calibrated, diagnostic).
pub(crate) fn scenario_passed(
    scenario: &Scenario,
    actual_score: f32,
    actual_relevant: bool,
) -> bool {
    let score_in_range =
        actual_score >= scenario.expected.score_min && actual_score <= scenario.expected.score_max;
    if !scenario.verdict_pinned {
        return score_in_range;
    }
    let verdict_ok = actual_relevant == scenario.expected.should_be_relevant
        || (actual_score - crate::get_relevance_threshold()).abs() < VERDICT_MARGIN;
    score_in_range && verdict_ok
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioItem {
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub tags_json: Option<String>,
    /// Item age, wired into `ScoringInput.created_at` as (now − N hours).
    /// Every scenario declares one; until the 2026-08-23 audit (item 22a,
    /// §4.4) both runners silently ignored it and scored everything at age 0 —
    /// so the UGC community caps, the voted-source <6h grace, and the stale-
    /// published discount were never exercised by the benchmark at all.
    pub created_hours_ago: Option<u64>,
    /// The adapter's stable per-item id (`source_items.source_id`). For
    /// registry sources this names the released SUBJECT package
    /// (`crate-tokio`, `vitest@3.0.0`) — the only grounding evidence the v18
    /// registry route trusts. `None` (the default for non-registry scenarios)
    /// keeps the corroborated-text fallback route, exactly like production's
    /// ad-hoc scoring paths.
    pub source_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Expected {
    pub score_min: f32,
    pub score_max: f32,
    pub should_be_relevant: bool,
    pub required_signals: Vec<String>,
    pub notes: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BenchmarkReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    /// Score-range accuracy: % of scenarios with actual score in [score_min, score_max]
    pub accuracy: f32,
    /// Relevance accuracy: % of scenarios with correct relevance prediction (tracked separately)
    pub relevance_accuracy: f32,
    pub by_category: HashMap<String, CategoryResult>,
    pub failures: Vec<BenchmarkFailure>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct CategoryResult {
    pub total: usize,
    pub passed: usize,
    pub accuracy: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BenchmarkFailure {
    pub scenario_id: String,
    pub category: String,
    pub expected_relevant: bool,
    pub actual_relevant: bool,
    pub actual_score: f32,
    pub signal_count: u8,
    pub confirmed_signals: Vec<String>,
    pub notes: String,
}

// ============================================================================
// Scenario Loading
// ============================================================================

pub(crate) fn load_scenarios() -> Vec<Scenario> {
    serde_json::from_str(SCENARIOS_JSON).expect("benchmark_scenarios.json must be valid JSON")
}

/// The scenario's `created_at` timestamp: (now − created_hours_ago). Shared by
/// every runner (synthetic, calibrated, diagnostic dump) so no harness can
/// quietly regress to age-0 scoring again (2026-08-23 audit, item 22a).
pub(crate) fn scenario_created_at(scenario: &Scenario) -> Option<chrono::DateTime<chrono::Utc>> {
    scenario
        .item
        .created_hours_ago
        .map(|h| chrono::Utc::now() - chrono::Duration::hours(h as i64))
}

/// Scoring options for one scenario: freshness-neutral by default, temporal
/// evidence ON for scenarios that opted in via `apply_freshness`.
pub(crate) fn scenario_options(scenario: &Scenario) -> ScoringOptions {
    if scenario.apply_freshness {
        ScoringOptions {
            apply_freshness: true,
            apply_signals: false,
            trend_topics: vec![],
        }
    } else {
        no_freshness()
    }
}

// ============================================================================
// Profile Contexts
// ============================================================================

pub(crate) fn profile_ctx(name: &str) -> ScoringContext {
    match name {
        "rust_developer" => rust_developer_ctx(),
        "fullstack_js" => fullstack_js_ctx(),
        "python_data_scientist" => python_data_scientist_ctx(),
        "minimal" => minimal_ctx(),
        _ => panic!("Unknown benchmark profile: {name}"),
    }
}

/// Install realistic ACE dependency intelligence on a benchmark profile.
///
/// Production populates `ace_ctx.dependency_info` (and `dependency_names`) from
/// `load_dependency_intelligence()`. The benchmark profiles set only
/// `domain_profile.dependency_names`, but `match_dependencies` reads
/// `ace_ctx.dependency_info` — so without this the dependency signal never fires
/// and every CVE / registry-update / dependency scenario scores near-zero on weak
/// signals alone. Mirroring deps into the field the matcher uses makes the
/// benchmark faithful to production; the scoring algorithm is unchanged.
fn install_bench_deps(ace: &mut ace_context::ACEContext, deps: &[(&str, &str)]) {
    for (name, ecosystem) in deps {
        install_bench_dep(ace, name, ecosystem, true);
    }
}

/// Lockfile-only (transitive) deps for a benchmark profile. Production's
/// `load_dependency_intelligence` yields these from lockfiles with
/// `is_direct: false` — a serde user's Cargo.lock always contains
/// `serde_derive`, which is exactly the family-rule production case
/// (2026-08-23 audit, item 15).
fn install_bench_transitive_deps(ace: &mut ace_context::ACEContext, deps: &[(&str, &str)]) {
    for (name, ecosystem) in deps {
        install_bench_dep(ace, name, ecosystem, false);
    }
}

fn install_bench_dep(ace: &mut ace_context::ACEContext, name: &str, ecosystem: &str, direct: bool) {
    let info = super::dependencies::DepInfo {
        package_name: name.to_string(),
        version: None,
        is_dev: false,
        is_direct: direct,
        search_terms: super::dependencies::extract_search_terms(name),
        ecosystem: ecosystem.to_string(),
        project_paths: Vec::new(),
        project_relevance: 1.0,
    };
    for term in &info.search_terms {
        ace.dependency_names.insert(term.clone());
    }
    ace.dependency_names.insert(name.to_string());
    ace.dependency_info.insert(name.to_string(), info);
}

/// Installed version for a benchmark dep — the lockfile evidence the security
/// version verdict (`is_version_affected`, v29) reads. Only a dep a pinned
/// scenario's verdict depends on carries one; everything else stays `None`,
/// as a manifest-only dependency does in production.
fn set_bench_dep_version(ace: &mut ace_context::ACEContext, name: &str, version: &str) {
    if let Some(info) = ace.dependency_info.get_mut(name) {
        info.version = Some(version.to_string());
    }
}

fn rust_developer_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; crate::EMBEDDING_DIMS];
    let interests = vec![
        crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(2),
            topic: "systems programming".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(3),
            topic: "Tauri".to_string(),
            weight: 1.0,
            embedding: Some(emb),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics
        .extend(["rust", "tauri", "sqlite"].iter().map(|s| s.to_string()));
    ace.detected_tech
        .extend(["rust", "tauri", "sqlite"].iter().map(|s| s.to_string()));
    install_bench_deps(
        &mut ace,
        &[
            ("tokio", "rust"),
            ("serde", "rust"),
            ("sqlx", "rust"),
            ("tauri", "rust"),
            ("hyper", "rust"),
            ("reqwest", "rust"),
        ],
    );
    // tokio's installed version: below vf_cve_direct_dep_affected's 1.53.2 fix
    // (affected) and past vf_cve_grounded_not_affected's 1.38.1 fix (not
    // affected) — the two pinned version verdicts on one dependency.
    set_bench_dep_version(&mut ace, "tokio", "1.47.1");
    // Lockfile-only family children of the direct deps above, exactly as a
    // real serde/tokio user's Cargo.lock carries them (family rule, item 15).
    install_bench_transitive_deps(
        &mut ace,
        &[("serde_derive", "rust"), ("tokio-util", "rust")],
    );

    let primary_stack = std::collections::HashSet::from_iter(
        ["rust", "tauri", "sqlite"].iter().map(|s| s.to_string()),
    );
    let all_tech = std::collections::HashSet::from_iter(
        [
            "rust",
            "tauri",
            "sqlite",
            "tokio",
            "serde",
            "wasm",
            "typescript",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    // Infer domain concerns the way production does — a Tauri desktop developer
    // cares about packaging / installer / auto-update / code-signing. The
    // benchmark previously left this empty, so domain-concern items (e.g.
    // cross-platform packaging) scored near-zero despite being on-domain.
    let domain_concerns = crate::domain_profile::infer_domain_concerns(&primary_stack, &all_tech);
    let domain = crate::domain_profile::DomainProfile {
        primary_stack,
        adjacent_tech: std::collections::HashSet::from_iter(
            ["tokio", "serde", "wasm", "typescript"]
                .iter()
                .map(|s| s.to_string()),
        ),
        all_tech,
        dependency_names: std::collections::HashSet::from_iter(
            ["tokio", "serde", "sqlx", "tauri", "hyper"]
                .iter()
                .map(|s| s.to_string()),
        ),
        interest_topics: std::collections::HashSet::from_iter(
            ["rust", "systems programming", "tauri"]
                .iter()
                .map(|s| s.to_string()),
        ),
        domain_concerns,
        ace_promoted_tech: std::collections::HashSet::new(),
    };

    let stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);

    ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "rust".to_string(),
            "tauri".to_string(),
            "sqlite".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(20)
        .build()
}

fn fullstack_js_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; crate::EMBEDDING_DIMS];
    let interests = vec![
        crate::context_engine::Interest {
            id: Some(1),
            topic: "TypeScript".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(2),
            topic: "React".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(3),
            topic: "Node.js".to_string(),
            weight: 1.0,
            embedding: Some(emb),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.extend(
        ["typescript", "react", "nodejs"]
            .iter()
            .map(|s| s.to_string()),
    );
    ace.detected_tech
        .extend(["typescript", "react"].iter().map(|s| s.to_string()));
    install_bench_deps(
        &mut ace,
        &[
            ("react", "javascript"),
            ("next", "javascript"),
            ("express", "javascript"),
            ("prisma", "javascript"),
        ],
    );

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: std::collections::HashSet::from_iter(
            ["typescript", "react", "nodejs"]
                .iter()
                .map(|s| s.to_string()),
        ),
        adjacent_tech: std::collections::HashSet::from_iter(
            ["next", "express", "prisma", "tailwind"]
                .iter()
                .map(|s| s.to_string()),
        ),
        all_tech: std::collections::HashSet::from_iter(
            [
                "typescript",
                "react",
                "nodejs",
                "next",
                "express",
                "prisma",
                "tailwind",
            ]
            .iter()
            .map(|s| s.to_string()),
        ),
        dependency_names: std::collections::HashSet::from_iter(
            ["react", "next", "express", "prisma"]
                .iter()
                .map(|s| s.to_string()),
        ),
        interest_topics: std::collections::HashSet::from_iter(
            ["typescript", "react", "node.js"]
                .iter()
                .map(|s| s.to_string()),
        ),
        domain_concerns: std::collections::HashSet::new(),
        ace_promoted_tech: std::collections::HashSet::new(),
    };

    let stack = crate::stacks::compose_profiles(&["fullstack_js".to_string()]);

    ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "typescript".to_string(),
            "react".to_string(),
            "nodejs".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(15)
        .build()
}

fn python_data_scientist_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; crate::EMBEDDING_DIMS];
    let interests = vec![
        crate::context_engine::Interest {
            id: Some(1),
            topic: "Machine Learning".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(2),
            topic: "Python".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
        crate::context_engine::Interest {
            id: Some(3),
            topic: "Data Science".to_string(),
            weight: 1.0,
            embedding: Some(emb),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics
        .extend(["python", "pytorch", "ml"].iter().map(|s| s.to_string()));
    ace.detected_tech
        .extend(["python", "pytorch"].iter().map(|s| s.to_string()));
    install_bench_deps(
        &mut ace,
        &[
            ("pytorch", "python"),
            ("torch", "python"),
            ("transformers", "python"),
            ("numpy", "python"),
            ("pandas", "python"),
        ],
    );

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: std::collections::HashSet::from_iter(
            ["python", "pytorch", "tensorflow"]
                .iter()
                .map(|s| s.to_string()),
        ),
        adjacent_tech: std::collections::HashSet::from_iter(
            ["numpy", "pandas", "scikit-learn", "huggingface"]
                .iter()
                .map(|s| s.to_string()),
        ),
        all_tech: std::collections::HashSet::from_iter(
            [
                "python",
                "pytorch",
                "tensorflow",
                "numpy",
                "pandas",
                "scikit-learn",
                "huggingface",
            ]
            .iter()
            .map(|s| s.to_string()),
        ),
        dependency_names: std::collections::HashSet::from_iter(
            ["torch", "transformers", "numpy", "pandas"]
                .iter()
                .map(|s| s.to_string()),
        ),
        interest_topics: std::collections::HashSet::from_iter(
            ["machine learning", "python", "data science"]
                .iter()
                .map(|s| s.to_string()),
        ),
        domain_concerns: std::collections::HashSet::new(),
        ace_promoted_tech: std::collections::HashSet::new(),
    };

    let stack = crate::stacks::compose_profiles(&["python_ml".to_string()]);

    ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "python".to_string(),
            "pytorch".to_string(),
            "tensorflow".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(10)
        .build()
}

fn minimal_ctx() -> ScoringContext {
    ScoringContext::builder()
        .interest_count(0)
        .feedback_interaction_count(0)
        .build()
}

// ============================================================================
// Benchmark Runner
// ============================================================================

pub(crate) fn run_benchmark(db: &crate::db::Database) -> BenchmarkReport {
    let scenarios = load_scenarios();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut total = 0;
    let mut passed = 0;
    let mut relevance_correct = 0;
    let mut failures = Vec::new();
    let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();

    for scenario in &scenarios {
        total += 1;
        let ctx = profile_ctx(&scenario.profile);
        let opts = scenario_options(scenario);

        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let tags_json_ref = scenario.item.tags_json.as_deref();
        let created_at = scenario_created_at(scenario);

        let input = ScoringInput {
            id: total as u64,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding: &zero_emb,
            created_at: created_at.as_ref(),
            detected_lang: "en",
            source_tags: &tags,
            tags_json: tags_json_ref,
            feed_origin: None,
            source_id: scenario.item.source_id.as_deref(),
        };

        let result = score_item(&input, &ctx, db, &opts, None);

        let actual_relevant = result.relevant;
        let actual_score = result.top_score;
        let bd = result.score_breakdown.as_ref();
        let signal_count = bd.map(|b| b.signal_count).unwrap_or(0);
        let confirmed_signals = bd.map(|b| b.confirmed_signals.clone()).unwrap_or_default();

        let relevance_ok = actual_relevant == scenario.expected.should_be_relevant;
        let scenario_ok = scenario_passed(scenario, actual_score, actual_relevant);

        if relevance_ok {
            relevance_correct += 1;
        }

        let cat_entry = by_category
            .entry(scenario.category.clone())
            .or_insert((0, 0));
        cat_entry.0 += 1;

        if scenario_ok {
            passed += 1;
            cat_entry.1 += 1;
        } else {
            warn!(
                "  FAIL [{}] \"{}\" — score={:.3} relevant={} expected_relevant={} range=[{:.2},{:.2}] signals={:?}",
                scenario.id,
                scenario.item.title,
                actual_score,
                actual_relevant,
                scenario.expected.should_be_relevant,
                scenario.expected.score_min,
                scenario.expected.score_max,
                confirmed_signals,
            );
            failures.push(BenchmarkFailure {
                scenario_id: scenario.id.clone(),
                category: scenario.category.clone(),
                expected_relevant: scenario.expected.should_be_relevant,
                actual_relevant,
                actual_score,
                signal_count,
                confirmed_signals,
                notes: scenario.expected.notes.clone(),
            });
        }
    }

    let accuracy = if total > 0 {
        passed as f32 / total as f32
    } else {
        0.0
    };
    let relevance_accuracy = if total > 0 {
        relevance_correct as f32 / total as f32
    } else {
        0.0
    };

    let by_category = by_category
        .into_iter()
        .map(|(cat, (cat_total, cat_passed))| {
            let cat_accuracy = if cat_total > 0 {
                cat_passed as f32 / cat_total as f32
            } else {
                0.0
            };
            (
                cat,
                CategoryResult {
                    total: cat_total,
                    passed: cat_passed,
                    accuracy: cat_accuracy,
                },
            )
        })
        .collect();

    let failed = total - passed;

    info!("\n{}", "=".repeat(72));
    info!("  PASIFA SCENARIO BENCHMARK — {} scenarios", total);
    info!("{}", "=".repeat(72));
    info!(
        "  Score-range: {}/{} passed ({:.1}%)",
        passed,
        total,
        accuracy * 100.0
    );
    info!(
        "  Relevance:   {}/{} correct ({:.1}%)",
        relevance_correct,
        total,
        relevance_accuracy * 100.0
    );
    info!("{}", "-".repeat(72));

    let report = BenchmarkReport {
        total,
        passed,
        failed,
        accuracy,
        relevance_accuracy,
        by_category,
        failures,
    };

    for (cat, result) in &report.by_category {
        info!(
            "  {:16} {}/{} ({:.0}%)",
            cat,
            result.passed,
            result.total,
            result.accuracy * 100.0
        );
    }

    if !report.failures.is_empty() {
        info!("{}", "-".repeat(72));
        info!("  Failures:");
        for f in &report.failures {
            info!(
                "    [{}] {} score={:.3} relevant={} expected={}",
                f.category, f.scenario_id, f.actual_score, f.actual_relevant, f.expected_relevant
            );
        }
    }
    info!("{}", "=".repeat(72));

    report
}

#[cfg(test)]
#[path = "benchmark_scenarios_tests.rs"]
mod tests;
