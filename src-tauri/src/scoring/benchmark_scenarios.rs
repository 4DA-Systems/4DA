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

use super::benchmark::{bench_db, no_freshness};
use super::pipeline::ScoringInput;
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
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioItem {
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub tags_json: Option<String>,
    pub created_hours_ago: Option<u64>,
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
        let info = super::dependencies::DepInfo {
            package_name: (*name).to_string(),
            version: None,
            is_dev: false,
            is_direct: true,
            search_terms: super::dependencies::extract_search_terms(name),
            ecosystem: (*ecosystem).to_string(),
        };
        for term in &info.search_terms {
            ace.dependency_names.insert(term.clone());
        }
        ace.dependency_names.insert((*name).to_string());
        ace.dependency_info.insert((*name).to_string(), info);
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
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut total = 0;
    let mut passed = 0;
    let mut relevance_correct = 0;
    let mut failures = Vec::new();
    let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();

    for scenario in &scenarios {
        total += 1;
        let ctx = profile_ctx(&scenario.profile);

        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let tags_json_ref = scenario.item.tags_json.as_deref();

        let input = ScoringInput {
            id: total as u64,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding: &zero_emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &tags,
            tags_json: tags_json_ref,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, db, &opts, None);

        let actual_relevant = result.relevant;
        let actual_score = result.top_score;
        let bd = result.score_breakdown.as_ref();
        let signal_count = bd.map(|b| b.signal_count).unwrap_or(0);
        let confirmed_signals = bd.map(|b| b.confirmed_signals.clone()).unwrap_or_default();

        let relevance_ok = actual_relevant == scenario.expected.should_be_relevant;
        let score_in_range = actual_score >= scenario.expected.score_min
            && actual_score <= scenario.expected.score_max;

        if relevance_ok {
            relevance_correct += 1;
        }

        let cat_entry = by_category
            .entry(scenario.category.clone())
            .or_insert((0, 0));
        cat_entry.0 += 1;

        if score_in_range {
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

// ============================================================================
// Tests
// ============================================================================

const VALID_PROFILES: &[&str] = &[
    "rust_developer",
    "fullstack_js",
    "python_data_scientist",
    "minimal",
];

#[test]
fn scenarios_parse_correctly() {
    let scenarios = load_scenarios();
    assert_eq!(
        scenarios.len(),
        78,
        "Expected 78 scenarios, got {}",
        scenarios.len()
    );
    for s in &scenarios {
        assert!(!s.id.is_empty(), "Scenario has empty id");
        assert!(
            !s.category.is_empty(),
            "Scenario {} has empty category",
            s.id
        );
    }
}

#[test]
fn scenarios_have_valid_profiles() {
    let scenarios = load_scenarios();
    for s in &scenarios {
        assert!(
            VALID_PROFILES.contains(&s.profile.as_str()),
            "Scenario {} uses unknown profile '{}'",
            s.id,
            s.profile,
        );
    }
}

#[test]
fn scenarios_have_valid_score_ranges() {
    let scenarios = load_scenarios();
    for s in &scenarios {
        assert!(
            s.expected.score_min < s.expected.score_max,
            "Scenario {} has score_min ({}) >= score_max ({})",
            s.id,
            s.expected.score_min,
            s.expected.score_max,
        );
        assert!(
            s.expected.score_min >= 0.0 && s.expected.score_min <= 1.0,
            "Scenario {} has score_min {} outside [0,1]",
            s.id,
            s.expected.score_min,
        );
        assert!(
            s.expected.score_max >= 0.0 && s.expected.score_max <= 1.0,
            "Scenario {} has score_max {} outside [0,1]",
            s.id,
            s.expected.score_max,
        );
    }
}

#[test]
#[ignore = "re-baseline after Arctic-M real embeddings replace synthetic test vectors"]
fn benchmark_scoring_accuracy() {
    let db = bench_db();
    let report = run_benchmark(&db);

    assert!(
        report.accuracy >= 0.75,
        "Overall accuracy {:.1}% < 75% threshold ({} of {} passed)",
        report.accuracy * 100.0,
        report.passed,
        report.total,
    );
}

#[test]
fn cold_start_scores_have_spread() {
    let scenarios = load_scenarios();
    let cold_start: Vec<&Scenario> = scenarios
        .iter()
        .filter(|s| s.category == "cold_start")
        .collect();

    assert!(
        cold_start.len() >= 5,
        "Need at least 5 cold_start scenarios, got {}",
        cold_start.len()
    );

    let db = bench_db();
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut scores = Vec::new();
    for scenario in &cold_start {
        let ctx = profile_ctx(&scenario.profile);
        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let tags_json_ref = scenario.item.tags_json.as_deref();

        let input = ScoringInput {
            id: 1,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding: &zero_emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &tags,
            tags_json: tags_json_ref,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, &db, &opts, None);
        scores.push(result.top_score);
    }

    // Verify non-uniformity: not all scores are identical
    let min = scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let spread = max - min;

    assert!(
        spread > 0.01,
        "Cold start scores are uniform (spread={:.4}), expected variation. Scores: {:?}",
        spread,
        scores,
    );
}

/// L1b regression: a CVE confirmed against the user's DIRECT dependency must
/// score clearly relevant (>= the direct-dep fast-path floor), NOT sit at the
/// bare 0.50 generic floor — the flagship preemption case. Hermetic: zero
/// embedding (the floor is additive, so it fires without embedding signal);
/// deps injected the way production's ACE records them.
#[test]
fn direct_dep_cve_clears_the_direct_dep_floor() {
    fn mk_dep(name: &str) -> super::dependencies::DepInfo {
        super::dependencies::DepInfo {
            package_name: name.to_string(),
            version: None,
            is_dev: false,
            is_direct: true,
            search_terms: super::dependencies::extract_search_terms(name),
            ecosystem: "rust".to_string(),
        }
    }

    let db = bench_db();
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut ctx = rust_developer_ctx();
    ctx.ace_ctx
        .dependency_info
        .insert("hyper".to_string(), mk_dep("hyper"));
    ctx.ace_ctx.dependency_names.insert("hyper".to_string());

    // A pure-dep-signal CVE (no topic/interest overlap, zero embedding) — exactly
    // the case that floored at 0.50 before L1b.
    let tags = vec!["security".to_string(), "cve".to_string()];
    let input = ScoringInput {
        id: 1,
        title: "CVE-2026-5678: hyper HTTP/2 CONTINUATION frame flood",
        url: Some("https://example.com"),
        content: "A vulnerability in the hyper crate's HTTP/2 handling allows a CONTINUATION frame flood.",
        source_type: "cve",
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &tags,
        tags_json: None,
        feed_origin: None,
    };
    let direct = score_item(&input, &ctx, &db, &opts, None).top_score;

    // An unrelated CVE (package NOT a dependency) must still score low — the
    // floor only lifts CONFIRMED direct-dep matches.
    let unrelated_input = ScoringInput {
        id: 2,
        title: "CVE-2026-88888: php-curl remote code execution in cURL wrapper",
        url: Some("https://example.com"),
        content: "A PHP cURL wrapper vulnerability allows remote code execution.",
        source_type: "cve",
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &tags,
        tags_json: None,
        feed_origin: None,
    };
    let unrelated = score_item(&unrelated_input, &ctx, &db, &opts, None).top_score;

    assert!(
        direct >= 0.60,
        "direct-dep CVE must clear the direct-dep floor (~0.65), got {direct:.3}"
    );
    assert!(
        unrelated < 0.30,
        "unrelated CVE (php-curl, not a dep) must stay low, got {unrelated:.3}"
    );
}
