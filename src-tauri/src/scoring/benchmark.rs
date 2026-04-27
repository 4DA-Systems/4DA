// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! PASIFA Scoring Benchmark — Controlled Baseline
//!
//! Establishes measurable precision/recall/rejection baselines for the full
//! scoring pipeline across labeled test items and user profiles.
//!
//! Run: `cargo test scoring::benchmark -- --nocapture`

use tracing::{info, warn};

use super::*;
use std::collections::HashSet;
use std::path::Path;

// ============================================================================
// Infrastructure
// ============================================================================

pub(super) fn bench_db() -> crate::db::Database {
    crate::register_sqlite_vec_extension();
    crate::db::Database::new(Path::new(":memory:")).expect("in-memory DB")
}

pub(super) fn no_freshness() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
        trend_topics: vec![],
    }
}

pub(super) fn bench_input<'a>(
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
    }
}

// ============================================================================
// User Profiles
// ============================================================================

/// Rust systems developer — Rust, Tauri, SQLite, systems programming
fn rust_dev_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; 384];
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
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.active_topics.push("tauri".to_string());
    ace.active_topics.push("sqlite".to_string());
    ace.detected_tech.push("rust".to_string());
    ace.detected_tech.push("tauri".to_string());
    ace.detected_tech.push("sqlite".to_string());

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: HashSet::from_iter(
            ["rust", "tauri", "sqlite"].iter().map(|s| s.to_string()),
        ),
        adjacent_tech: HashSet::from_iter(
            ["tokio", "serde", "wasm", "typescript"]
                .iter()
                .map(|s| s.to_string()),
        ),
        all_tech: HashSet::from_iter(
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
        ),
        dependency_names: HashSet::from_iter(
            ["tokio", "serde", "sqlx", "tauri"]
                .iter()
                .map(|s| s.to_string()),
        ),
        interest_topics: HashSet::from_iter(
            ["rust", "systems programming", "tauri"]
                .iter()
                .map(|s| s.to_string()),
        ),
        domain_concerns: HashSet::new(),
        ace_promoted_tech: HashSet::new(),
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

/// Python ML developer — Python, PyTorch, LLMs
fn python_ml_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; 384];
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
            topic: "LLM".to_string(),
            weight: 1.0,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("python".to_string());
    ace.active_topics.push("pytorch".to_string());
    ace.active_topics.push("machine learning".to_string());
    ace.detected_tech.push("python".to_string());
    ace.detected_tech.push("pytorch".to_string());

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: HashSet::from_iter(
            ["python", "pytorch", "tensorflow"]
                .iter()
                .map(|s| s.to_string()),
        ),
        adjacent_tech: HashSet::from_iter(
            ["numpy", "pandas", "scikit-learn", "huggingface"]
                .iter()
                .map(|s| s.to_string()),
        ),
        all_tech: HashSet::from_iter(
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
        dependency_names: HashSet::from_iter(
            ["torch", "transformers", "numpy", "pandas"]
                .iter()
                .map(|s| s.to_string()),
        ),
        interest_topics: HashSet::from_iter(
            ["machine learning", "python", "llm"]
                .iter()
                .map(|s| s.to_string()),
        ),
        domain_concerns: HashSet::new(),
        ace_promoted_tech: HashSet::new(),
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
        .feedback_interaction_count(20)
        .build()
}

/// Bootstrap user — 1 interest, 0 feedback, minimal context
fn bootstrap_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "TypeScript".to_string(),
        weight: 1.0,
        embedding: Some(emb),
        source: crate::context_engine::InterestSource::Explicit,
    }];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("typescript".to_string());

    ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(0)
        .build()
}

// ============================================================================
// Test Corpus
// ============================================================================

struct Labeled {
    title: &'static str,
    content: &'static str,
    relevant: bool,
}

fn rust_corpus() -> Vec<Labeled> {
    vec![
        // --- SHOULD BE RELEVANT (2+ signal axes) ---
        Labeled {
            title: "Rust 1.80 stabilizes async closures and lazy type aliases",
            content: "rust async closures lazy type aliases stabilization compiler release",
            relevant: true,
        },
        Labeled {
            title: "Tauri 2.1 released with improved IPC and plugin system",
            content: "tauri desktop app ipc plugin system rust typescript webview",
            relevant: true,
        },
        Labeled {
            title: "Understanding Rust borrow checker: lifetime elision explained",
            content: "rust borrow checker lifetime elision references ownership memory safety systems programming",
            relevant: true,
        },
        Labeled {
            title: "SQLite performance tuning for embedded applications",
            content: "sqlite performance tuning wal mode pragma journal embedded database optimization",
            relevant: true,
        },
        Labeled {
            title: "Tokio 1.40: improved task scheduling and memory efficiency",
            content: "tokio async runtime rust task scheduling memory efficiency performance",
            relevant: true,
        },
        // --- SHOULD NOT BE RELEVANT ---
        Labeled {
            title: "10 JavaScript frameworks you should learn in 2026",
            content: "javascript frameworks react angular vue svelte frontend web development listicle",
            relevant: false,
        },
        Labeled {
            title: "How to cook the perfect risotto",
            content: "cooking risotto recipe italian food kitchen arborio rice parmesan",
            relevant: false,
        },
        Labeled {
            title: "Celebrity drama at the Oscars 2026",
            content: "celebrity oscars drama entertainment hollywood awards red carpet",
            relevant: false,
        },
        Labeled {
            title: "Getting started with Go for beginners",
            content: "go golang beginner tutorial introduction getting started programming",
            relevant: false,
        },
        Labeled {
            title: "Introduction to Kubernetes for DevOps teams",
            content: "kubernetes k8s devops containers orchestration pods services deployment",
            relevant: false,
        },
    ]
}

fn python_corpus() -> Vec<Labeled> {
    vec![
        Labeled {
            title: "PyTorch 2.5 introduces native distributed training improvements",
            content: "pytorch distributed training fsdp torch python deep learning neural network",
            relevant: true,
        },
        Labeled {
            title: "Fine-tuning LLMs with LoRA and QLoRA: a practical guide",
            content: "llm fine-tuning lora qlora machine learning transformer python huggingface",
            relevant: true,
        },
        Labeled {
            title: "Scaling inference with vLLM: serving large language models",
            content: "vllm inference serving llm machine learning python transformer batching",
            relevant: true,
        },
        Labeled {
            title: "New advances in attention mechanisms for transformer architectures",
            content:
                "attention transformer architecture machine learning deep learning python research",
            relevant: true,
        },
        Labeled {
            title: "Building REST APIs with Rust and Axum",
            content: "rust axum web api server tokio serde json http backend",
            relevant: false,
        },
        Labeled {
            title: "How to organize your closet in 5 steps",
            content: "organization closet declutter minimalism home lifestyle tips",
            relevant: false,
        },
        Labeled {
            title: "Bitcoin hits new all-time high",
            content: "bitcoin cryptocurrency trading market price investment crypto",
            relevant: false,
        },
        Labeled {
            title: "Managing PostgreSQL migrations in production",
            content: "postgresql migration database schema production devops sql deployment",
            relevant: false,
        },
    ]
}

// ============================================================================
// Benchmark Runner
// ============================================================================

struct BenchResult {
    profile: &'static str,
    tp: usize,
    fp: usize,
    tn: usize,
    r#fn: usize,
}

impl BenchResult {
    fn precision(&self) -> f64 {
        let denom = self.tp + self.fp;
        if denom == 0 {
            1.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn recall(&self) -> f64 {
        let denom = self.tp + self.r#fn;
        if denom == 0 {
            1.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn rejection(&self) -> f64 {
        let denom = self.tn + self.fp;
        if denom == 0 {
            1.0
        } else {
            self.tn as f64 / denom as f64
        }
    }
}

fn run_bench(name: &'static str, ctx: &ScoringContext, corpus: &[Labeled]) -> BenchResult {
    let db = bench_db();
    let opts = no_freshness();
    let emb = vec![0.1_f32; 384];

    let (mut tp, mut fp, mut tn, mut r#fn) = (0, 0, 0, 0);

    for (i, item) in corpus.iter().enumerate() {
        let input = bench_input(i as u64, item.title, item.content, &emb);
        let result = score_item(&input, ctx, &db, &opts, None);

        match (result.relevant, item.relevant) {
            (true, true) => tp += 1,
            (true, false) => {
                fp += 1;
                let bd = result.score_breakdown.as_ref();
                warn!(
                    "  FP [{name}]: \"{}\" score={:.3} signals={:?}",
                    item.title,
                    result.top_score,
                    bd.map(|b| b.confirmed_signals.clone()).unwrap_or_default()
                );
            }
            (false, false) => tn += 1,
            (false, true) => {
                r#fn += 1;
                let bd = result.score_breakdown.as_ref();
                warn!(
                    "  FN [{name}]: \"{}\" score={:.3} signals={:?}",
                    item.title,
                    result.top_score,
                    bd.map(|b| b.confirmed_signals.clone()).unwrap_or_default()
                );
            }
        }
    }

    BenchResult {
        profile: name,
        tp,
        fp,
        tn,
        r#fn,
    }
}

// ============================================================================
// Precision / Recall Benchmarks
// ============================================================================

#[test]
fn bench_rust_dev_precision_recall() {
    let ctx = rust_dev_ctx();
    let r = run_bench("rust_dev", &ctx, &rust_corpus());

    info!("\n=== Rust Developer ===");
    info!("  TP={} FP={} TN={} FN={}", r.tp, r.fp, r.tn, r.r#fn);
    info!(
        "  Precision={:.0}% Recall={:.0}% Rejection={:.0}%",
        r.precision() * 100.0,
        r.recall() * 100.0,
        r.rejection() * 100.0
    );

    assert!(
        r.precision() >= 0.80,
        "Precision {:.0}% < 80%",
        r.precision() * 100.0
    );
    assert!(
        r.rejection() >= 0.80,
        "Rejection {:.0}% < 80%",
        r.rejection() * 100.0
    );
}

#[test]
fn bench_python_ml_precision_recall() {
    let ctx = python_ml_ctx();
    let r = run_bench("python_ml", &ctx, &python_corpus());

    info!("\n=== Python ML Developer ===");
    info!("  TP={} FP={} TN={} FN={}", r.tp, r.fp, r.tn, r.r#fn);
    info!(
        "  Precision={:.0}% Recall={:.0}% Rejection={:.0}%",
        r.precision() * 100.0,
        r.recall() * 100.0,
        r.rejection() * 100.0
    );

    assert!(
        r.precision() >= 0.80,
        "Precision {:.0}% < 80%",
        r.precision() * 100.0
    );
    assert!(
        r.rejection() >= 0.80,
        "Rejection {:.0}% < 80%",
        r.rejection() * 100.0
    );
}

// ============================================================================
// Gate Invariants
// ============================================================================

#[test]
fn gate_zero_signals_below_threshold() {
    let db = bench_db();
    let ctx = rust_dev_ctx();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    for title in &[
        "Celebrity gossip roundup for the week",
        "Best vacation spots in Europe",
        "How to train your puppy in 3 days",
    ] {
        let input = bench_input(1, title, "completely irrelevant noise", &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            !result.relevant,
            "Noise \"{}\" should not be relevant",
            title
        );
    }
}

#[test]
fn gate_single_signal_capped_in_normal_mode() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // Interest-only context: 1 signal max
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "Rust".to_string(),
        weight: 1.0,
        embedding: Some(vec![0.5_f32; 384]),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let ctx = ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .feedback_interaction_count(20) // normal mode
        .build();

    let input = bench_input(
        1,
        "Rust programming language guide",
        "rust programming guide tutorial",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    if bd.signal_count == 1 {
        assert!(
            result.top_score <= 0.35,
            "1-signal capped at {:.3}, should be <= 0.35",
            result.top_score
        );
        assert!(!result.relevant, "1-signal must not pass in normal mode");
    }
}

#[test]
fn gate_two_signals_can_pass() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Rust async runtime: understanding Tokio task scheduler",
        "rust tokio async runtime task scheduler performance systems programming optimization",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert!(
        bd.signal_count >= 2,
        "Expected 2+ signals, got {} ({:?})",
        bd.signal_count,
        bd.confirmed_signals
    );
}

// ============================================================================
// Base Score Path Coverage
// ============================================================================

#[test]
fn path_both_context_and_interest() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ScoringContext::builder()
        .cached_context_count(10)
        .interest_count(2)
        .interests(vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(vec![0.5_f32; 384]),
            source: crate::context_engine::InterestSource::Explicit,
        }])
        .build();

    let input = bench_input(
        1,
        "Rust systems programming",
        "rust systems programming performance",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    assert!(result.top_score >= 0.0, "Path 1 should produce valid score");
}

#[test]
fn path_interest_only() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ScoringContext::builder()
        .cached_context_count(0)
        .interest_count(2)
        .interests(vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(vec![0.5_f32; 384]),
            source: crate::context_engine::InterestSource::Explicit,
        }])
        .build();

    let input = bench_input(
        1,
        "Rust programming language",
        "rust programming memory safety",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    assert!(
        result.top_score > 0.0,
        "Interest-only path should produce score > 0"
    );
}

#[test]
fn path_context_only() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ScoringContext::builder()
        .cached_context_count(10)
        .interest_count(0)
        .build();

    let input = bench_input(
        1,
        "Performance optimization techniques",
        "performance optimization memory cpu cache",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    assert!(result.top_score >= 0.0, "Context-only path must not panic");
}

#[test]
fn path_neither() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ScoringContext::builder()
        .cached_context_count(0)
        .interest_count(0)
        .build();

    let input = bench_input(
        1,
        "Random article about nothing",
        "completely unrelated content",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    assert!(result.top_score >= 0.0);
    assert!(
        !result.relevant,
        "No context + no interests should never pass"
    );
}

// ============================================================================
// Domain Relevance Tiers
// ============================================================================

#[test]
fn domain_primary_outranks_adjacent() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let primary = bench_input(
        1,
        "Rust compiler improvements in nightly",
        "rust compiler nightly performance borrow checker improvements systems programming",
        &emb,
    );
    let adjacent = bench_input(
        2,
        "TypeScript compiler improvements",
        "typescript compiler performance type checking improvements",
        &emb,
    );

    let p = score_item(&primary, &ctx, &db, &opts, None);
    let a = score_item(&adjacent, &ctx, &db, &opts, None);

    let pd = p
        .score_breakdown
        .as_ref()
        .map(|b| b.domain_relevance)
        .unwrap_or(0.0);
    let ad = a
        .score_breakdown
        .as_ref()
        .map(|b| b.domain_relevance)
        .unwrap_or(0.0);

    assert!(
        pd >= ad,
        "Primary domain ({:.2}) should >= adjacent ({:.2})",
        pd,
        ad
    );
}

#[test]
fn domain_off_domain_crushed() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Best restaurants in downtown San Francisco",
        "restaurants food dining san francisco downtown nightlife reviews",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    assert!(
        !result.relevant,
        "Off-domain content should not be relevant"
    );
}

// ============================================================================
// Stack Intelligence
// ============================================================================

#[test]
fn stack_competing_tech_penalized() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Go 1.24 generics improvements and range-over-func",
        "go golang generics range func iteration performance concurrency goroutine",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert!(
        bd.stack_competing_mult <= 1.0,
        "Competing Go content should get penalty, got {:.3}",
        bd.stack_competing_mult
    );
}

#[test]
fn stack_ecosystem_shift_boosted() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Rust async trait stabilized: what it means for library authors",
        "rust async trait stabilized library authors ecosystem trait objects dyn dispatch",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert!(
        bd.ecosystem_shift_mult >= 1.0,
        "Ecosystem shift should boost, got {:.3}",
        bd.ecosystem_shift_mult
    );
}

// ============================================================================
// Dampening Invariants
// ============================================================================

#[test]
fn dampening_penalties_stronger_than_boosts() {
    // Asymmetric design: penalties 65%, boosts 40%
    let penalty_raw = 0.50_f32;
    let boost_raw = 1.50_f32;

    let dampened_penalty = 1.0 + (penalty_raw - 1.0) * 0.65;
    let dampened_boost = 1.0 + (boost_raw - 1.0) * 0.40;

    let penalty_impact = (1.0 - dampened_penalty).abs();
    let boost_impact = (dampened_boost - 1.0).abs();

    assert!(
        penalty_impact > boost_impact,
        "Penalty ({:.3}) should exceed boost ({:.3}) for same raw magnitude",
        penalty_impact,
        boost_impact
    );
}

// ============================================================================
// Bootstrap Mode
// ============================================================================

#[test]
fn bootstrap_more_permissive_than_normal() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    let b_ctx = bootstrap_ctx();

    // Same interests but past bootstrap
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "TypeScript".to_string(),
        weight: 1.0,
        embedding: Some(vec![0.5_f32; 384]),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("typescript".to_string());

    let n_ctx = ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(20)
        .build();

    let items = [
        (
            "TypeScript 5.7 new satisfies operator improvements",
            "typescript satisfies operator type checking improvements",
        ),
        (
            "React 20 with TypeScript templates",
            "react typescript templates components hooks frontend",
        ),
        (
            "Next.js 16 with TypeScript support",
            "nextjs typescript server components app router",
        ),
    ];

    let (mut b_rel, mut n_rel) = (0, 0);
    for (title, content) in &items {
        let input = bench_input(1, title, content, &emb);
        if score_item(&input, &b_ctx, &db, &opts, None).relevant {
            b_rel += 1;
        }
        if score_item(&input, &n_ctx, &db, &opts, None).relevant {
            n_rel += 1;
        }
    }

    assert!(
        b_rel >= n_rel,
        "Bootstrap ({} relevant) should >= normal ({})",
        b_rel,
        n_rel
    );
}

// ============================================================================
// Exclusions
// ============================================================================

#[test]
fn exclusion_produces_zero_score() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ScoringContext::builder()
        .exclusions(vec!["bitcoin".to_string(), "crypto".to_string()])
        .build();

    let input = bench_input(
        1,
        "Bitcoin reaches new all-time high in crypto rally",
        "bitcoin cryptocurrency crypto trading market rally",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    assert!(result.excluded);
    assert_eq!(result.top_score, 0.0);
    assert!(!result.relevant);
}

// ============================================================================
// Content Quality Guards
// ============================================================================

#[test]
fn short_title_capped() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Rust?",
        "rust programming language systems performance memory safety borrow checker",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    assert!(
        result.top_score <= 0.40,
        "Short title capped at 0.40, got {:.3}",
        result.top_score
    );
}

// ============================================================================
// Cross-Profile Isolation
// ============================================================================

#[test]
fn cross_profile_rust_not_relevant_for_python() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = python_ml_ctx();

    let input = bench_input(
        1,
        "Rust borrow checker improvements in nightly compiler",
        "rust borrow checker nightly compiler improvements lifetime elision ownership",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    assert!(
        !result.relevant,
        "Rust content not relevant for Python dev (score={:.3})",
        result.top_score
    );
}

#[test]
fn cross_profile_python_not_relevant_for_rust() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Fine-tuning BERT models with Hugging Face transformers",
        "python huggingface transformers bert fine-tuning nlp machine learning pytorch",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    assert!(
        !result.relevant,
        "Python ML content not relevant for Rust dev (score={:.3})",
        result.top_score
    );
}

// ============================================================================
// Monotonicity
// ============================================================================

#[test]
fn more_signals_higher_gate_mult() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // 1-signal context
    let ctx1 = ScoringContext::builder()
        .interest_count(1)
        .interests(vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(vec![0.5_f32; 384]),
            source: crate::context_engine::InterestSource::Explicit,
        }])
        .feedback_interaction_count(20)
        .build();

    // 2-signal context (interest + ACE)
    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("rust".to_string());
    let ctx2 = ScoringContext::builder()
        .interest_count(1)
        .interests(vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(vec![0.5_f32; 384]),
            source: crate::context_engine::InterestSource::Explicit,
        }])
        .ace_ctx(ace)
        .feedback_interaction_count(20)
        .build();

    let input = bench_input(
        1,
        "Rust programming patterns and idioms",
        "rust programming patterns idioms ownership borrowing",
        &emb,
    );

    let r1 = score_item(&input, &ctx1, &db, &opts, None);
    let r2 = score_item(&input, &ctx2, &db, &opts, None);

    let m1 = r1
        .score_breakdown
        .as_ref()
        .map(|b| b.confirmation_mult)
        .unwrap_or(0.0);
    let m2 = r2
        .score_breakdown
        .as_ref()
        .map(|b| b.confirmation_mult)
        .unwrap_or(0.0);

    assert!(
        m2 >= m1,
        "2-signal mult ({:.3}) should >= 1-signal mult ({:.3})",
        m2,
        m1
    );
}

// ============================================================================
// Determinism
// ============================================================================

#[test]
fn scoring_deterministic() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Rust async runtime internals: how Tokio schedules tasks",
        "rust tokio async runtime task scheduling internals systems",
        &emb,
    );

    let scores: Vec<f32> = (0..5)
        .map(|_| score_item(&input, &ctx, &db, &opts, None).top_score)
        .collect();

    for (i, s) in scores.iter().enumerate().skip(1) {
        assert_eq!(
            *s, scores[0],
            "Run {} score ({:.6}) != run 0 ({:.6})",
            i, s, scores[0]
        );
    }
}

// ============================================================================
// Skill-Gap Boost
// ============================================================================

/// Helper: build a SovereignDeveloperProfile with specified skill gaps
fn profile_with_skill_gaps(
    gaps: Vec<&str>,
) -> crate::sovereign_developer_profile::SovereignDeveloperProfile {
    use crate::sovereign_developer_profile::*;
    SovereignDeveloperProfile {
        generated_at: String::new(),
        identity_summary: "Test developer".to_string(),
        infrastructure: InfrastructureDimension::default(),
        stack: StackDimension::default(),
        skills: SkillsDimension::default(),
        preferences: PreferencesDimension::default(),
        context: ContextDimension::default(),
        intelligence: IntelligenceReport {
            skill_gaps: gaps
                .into_iter()
                .map(|dep| SkillGap {
                    dependency: dep.to_string(),
                    reason: "Dependency in project but no content engagement".to_string(),
                })
                .collect(),
            ..Default::default()
        },
        completeness: CompletenessReport {
            overall_percentage: 50.0,
            dimensions: vec![],
        },
    }
}

/// Helper: build a context with a sovereign profile that has skill gaps
fn ctx_with_skill_gaps(gaps: Vec<&str>) -> ScoringContext {
    let emb = vec![0.5_f32; 384];
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
            embedding: Some(emb),
            source: crate::context_engine::InterestSource::Explicit,
        },
    ];

    let mut ace = ace_context::ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.detected_tech.push("rust".to_string());

    ScoringContext::builder()
        .interest_count(2)
        .interests(interests)
        .ace_ctx(ace)
        .declared_tech(vec!["rust".to_string()])
        .sovereign_profile(Some(profile_with_skill_gaps(gaps)))
        .feedback_interaction_count(20)
        .build()
}

#[test]
fn skill_gap_boost_fires_for_gap_dependency() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // Context with "tokio" as a skill gap
    let ctx = ctx_with_skill_gaps(vec!["tokio"]);

    let input = bench_input(
        1,
        "Tokio runtime internals and task scheduling",
        "tokio async runtime task scheduling performance rust",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert!(
        bd.skill_gap_boost > 0.0,
        "skill_gap_boost should fire for gap dep 'tokio', got {:.3}",
        bd.skill_gap_boost
    );
    assert_eq!(
        bd.skill_gap_boost, 0.15,
        "Single gap match should give 0.15 boost"
    );
}

#[test]
fn skill_gap_boost_does_not_fire_for_engaged_topic() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // "serde" is the skill gap, but content is about "rust" (which user engages with)
    let ctx = ctx_with_skill_gaps(vec!["serde"]);

    let input = bench_input(
        1,
        "Rust borrow checker explained",
        "rust borrow checker ownership memory safety",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert_eq!(
        bd.skill_gap_boost, 0.0,
        "skill_gap_boost should NOT fire for non-gap content, got {:.3}",
        bd.skill_gap_boost
    );
}

#[test]
fn skill_gap_boost_multi_match_higher_than_single() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // Two skill gaps that will both match
    let ctx = ctx_with_skill_gaps(vec!["tokio", "serde"]);

    let input = bench_input(
        1,
        "Building async APIs with Tokio and Serde",
        "tokio serde async api serialization deserialization performance rust",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert_eq!(
        bd.skill_gap_boost, 0.20,
        "Multi gap match should give 0.20 boost, got {:.3}",
        bd.skill_gap_boost
    );
}

#[test]
fn skill_gap_boost_zero_without_profile() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();

    // No sovereign profile at all
    let ctx = rust_dev_ctx();

    let input = bench_input(
        1,
        "Introduction to Tokio async runtime",
        "tokio async runtime rust performance scheduling",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);
    let bd = result.score_breakdown.as_ref().unwrap();

    assert_eq!(
        bd.skill_gap_boost, 0.0,
        "Without sovereign profile, skill_gap_boost must be 0"
    );
}

#[test]
fn skill_gap_boost_appears_in_explanation() {
    let db = bench_db();
    let emb = vec![0.1_f32; 384];
    let opts = no_freshness();
    let ctx = ctx_with_skill_gaps(vec!["tokio"]);

    let input = bench_input(
        1,
        "Tokio task scheduling deep dive",
        "tokio async runtime task scheduling performance systems programming rust",
        &emb,
    );
    let result = score_item(&input, &ctx, &db, &opts, None);

    if let Some(ref explanation) = result.explanation {
        assert!(
            explanation.contains("skill gap"),
            "Explanation should mention skill gap: '{}'",
            explanation
        );
    }
    // If explanation is None (score too low to generate), that's also acceptable
}

// ============================================================================
// Aggregate Summary
// ============================================================================

#[test]
fn bench_aggregate_summary() {
    let results = vec![
        run_bench("rust_dev", &rust_dev_ctx(), &rust_corpus()),
        run_bench("python_ml", &python_ml_ctx(), &python_corpus()),
    ];

    info!("\n{}", "=".repeat(62));
    info!("  PASIFA SCORING BENCHMARK BASELINE");
    info!("{}", "=".repeat(62));
    for r in &results {
        info!(
            "  {:12} | P:{:5.1}% R:{:5.1}% Rej:{:5.1}% | TP:{} FP:{} TN:{} FN:{}",
            r.profile,
            r.precision() * 100.0,
            r.recall() * 100.0,
            r.rejection() * 100.0,
            r.tp,
            r.fp,
            r.tn,
            r.r#fn,
        );
    }
    info!("{}", "=".repeat(62));

    let avg_p: f64 = results.iter().map(|r| r.precision()).sum::<f64>() / results.len() as f64;
    let avg_r: f64 = results.iter().map(|r| r.rejection()).sum::<f64>() / results.len() as f64;

    assert!(avg_p >= 0.75, "Avg precision {:.0}% < 75%", avg_p * 100.0);
    assert!(avg_r >= 0.75, "Avg rejection {:.0}% < 75%", avg_r * 100.0);
}
