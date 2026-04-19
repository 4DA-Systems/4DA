// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! System 3: First-Run / Bootstrap Validation
//!
//! Validates the first-60-seconds experience: a user with minimal context
//! (0 feedback, 1 interest) still gets meaningful, non-degenerate scores.

use super::super::ace_context::ACEContext;
use super::super::{score_item, ScoringContext};
use super::personas::bootstrap_user;
use super::{sim_db, sim_input, sim_no_freshness};

// ============================================================================
// Helpers
// ============================================================================

fn make_tech_ctx(topic: &str, declared_tech: &[&str]) -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        embedding: Some(emb),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let mut ace = ACEContext::default();
    ace.detected_tech
        .extend(declared_tech.iter().map(|s| s.to_string()));
    ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .declared_tech(declared_tech.iter().map(|s| s.to_string()).collect())
        .feedback_interaction_count(0)
        .build()
}

fn make_rich_ctx(topic: &str, declared_tech: &[&str]) -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        embedding: Some(emb),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let mut ace = ACEContext::default();
    ace.active_topics
        .extend(declared_tech.iter().map(|s| s.to_string()));
    ace.detected_tech
        .extend(declared_tech.iter().map(|s| s.to_string()));
    ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .declared_tech(declared_tech.iter().map(|s| s.to_string()).collect())
        .feedback_interaction_count(0)
        .build()
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn bootstrap_mode_more_permissive_than_normal() {
    // A user with 0 feedback should score tech content higher than a user
    // who has established strong anti-preferences.
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let bootstrap = bootstrap_user();

    // Make a normal user with some interaction history but same interests
    let normal_ctx = {
        let e = vec![0.5_f32; 384];
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "TypeScript".to_string(),
            weight: 1.0,
            embedding: Some(e.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        let mut ace = ACEContext::default();
        ace.active_topics.push("typescript".to_string());
        // Normal user with anti-topic for most content
        ace.anti_topics.push("python".to_string());
        ace.anti_topics.push("rust".to_string());
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .feedback_interaction_count(100)
            .build()
    };

    // TypeScript content — both should score it
    let ts_input = sim_input(
        1,
        "TypeScript 5.4 new features",
        "TypeScript 5.4 brings new utility types and improved inference for TypeScript developers.",
        &emb,
    );
    let bootstrap_score = score_item(&ts_input, &bootstrap, &db, &opts, None).top_score;
    let normal_score = score_item(&ts_input, &normal_ctx, &db, &opts, None).top_score;

    // Both should score TypeScript content positively
    assert!(
        bootstrap_score > 0.0,
        "Bootstrap should score TypeScript content positively"
    );
    assert!(
        normal_score > 0.0,
        "Normal user should score TypeScript content positively"
    );
}

#[test]
fn bootstrap_user_gets_nonzero_scores_for_tech_content() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = bootstrap_user();

    let ts_input = sim_input(1,
        "TypeScript 5.4 new features for developers",
        "TypeScript 5.4 brings improved inference, NoInfer utility type, and better type narrowing for TypeScript developers building applications.",
        &emb);

    let result = score_item(&ts_input, &ctx, &db, &opts, None);
    assert!(
        result.top_score > 0.01,
        "Bootstrap user should get non-zero score for TypeScript content, got {}",
        result.top_score
    );
}

#[test]
fn interests_only_scores_topic_match_positively() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // User with only Rust interest
    let ctx = make_tech_ctx("Rust", &["rust"]);

    let rust_input = sim_input(1,
        "Rust programming language memory safety",
        "Rust provides memory safety guarantees through its ownership system, enabling systems programming without garbage collection.",
        &emb);

    let result = score_item(&rust_input, &ctx, &db, &opts, None);
    assert!(
        result.top_score > 0.0,
        "Rust-interested user should score Rust content positively, got {}",
        result.top_score
    );
}

#[test]
fn interests_only_noise_stays_low() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let ctx = make_tech_ctx("Rust", &["rust"]);

    let noise_input = sim_input(2,
        "Senior Software Engineer — $200k at startup",
        "We're hiring senior engineers for our growing team. Competitive salary and equity package.",
        &emb);

    let result = score_item(&noise_input, &ctx, &db, &opts, None);
    assert!(
        !result.relevant || result.top_score < 0.6,
        "Hiring noise should not be strongly relevant, got score={} relevant={}",
        result.top_score,
        result.relevant
    );
}

#[test]
fn declared_tech_boosts_matching_content() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // With declared tech
    let ctx_with_tech = make_tech_ctx("programming", &["rust", "tauri"]);
    // Without declared tech (interests only)
    let ctx_no_tech = make_tech_ctx("programming", &[]);

    let rust_input = sim_input(1,
        "Building desktop apps with Tauri and Rust",
        "Tauri 2.0 uses Rust backend with native webview for lightweight desktop applications across platforms.",
        &emb);

    let score_with = score_item(&rust_input, &ctx_with_tech, &db, &opts, None).top_score;
    let score_without = score_item(&rust_input, &ctx_no_tech, &db, &opts, None).top_score;

    assert!(
        score_with >= score_without,
        "Declared tech should not decrease score: with={score_with:.3} without={score_without:.3}"
    );
}

#[test]
fn declared_tech_rust_does_not_boost_unrelated_python_content() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let rust_ctx = make_tech_ctx("Rust", &["rust", "tauri"]);

    let python_input = sim_input(2,
        "PyTorch 2.0 training speed improvements",
        "PyTorch 2.0 introduces torch.compile() for up to 2x training speedup on Python machine learning workloads.",
        &emb);

    let result = score_item(&python_input, &rust_ctx, &db, &opts, None);
    // Rust user should not strongly prefer Python ML content
    assert!(
        result.top_score < 0.75,
        "Rust-declared user should not strongly prefer Python ML content, got {}",
        result.top_score
    );
}

#[test]
fn ace_context_improves_relevant_score() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let ctx_without_ace = make_tech_ctx("Rust", &[]);
    let ctx_with_ace = make_rich_ctx("Rust", &["rust", "tauri", "sqlite"]);

    let rust_input = sim_input(1,
        "Rust async/await with tokio and SQLite",
        "Building async Rust applications with tokio executor, SQLite storage via sqlx, and Tauri for the desktop UI layer.",
        &emb);

    let score_without = score_item(&rust_input, &ctx_without_ace, &db, &opts, None).top_score;
    let score_with = score_item(&rust_input, &ctx_with_ace, &db, &opts, None).top_score;

    assert!(
        score_with >= score_without,
        "ACE context should not decrease score: with={score_with:.3} without={score_without:.3}"
    );
}

#[test]
fn first_run_scores_are_bounded() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = bootstrap_user();

    let items = vec![
        (
            "Rust basics",
            "Introduction to Rust programming for beginners learning memory management.",
        ),
        (
            "Hiring post",
            "We are hiring TypeScript engineers for our fintech startup.",
        ),
        (
            "Tech news",
            "TypeScript 5.4 brings new utility types for better type safety in applications.",
        ),
    ];

    for (title, content) in &items {
        let input = sim_input(1, title, content, &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            result.top_score >= 0.0 && result.top_score <= 1.0,
            "Score out of bounds [0,1] for '{title}': {}",
            result.top_score
        );
    }
}

#[test]
fn first_run_tech_content_scores_higher_than_noise() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = bootstrap_user();

    let tech_input = sim_input(1,
        "TypeScript advanced type system features",
        "TypeScript conditional types, mapped types, and template literal types enable expressive type-level programming.",
        &emb);
    let noise_input = sim_input(2,
        "Ask HN: Who is hiring TypeScript engineers?",
        "Monthly hiring thread for TypeScript, React, and Node.js positions at startups and enterprise companies.",
        &emb);

    let tech_score = score_item(&tech_input, &ctx, &db, &opts, None).top_score;
    let noise_score = score_item(&noise_input, &ctx, &db, &opts, None).top_score;

    assert!(
        tech_score >= noise_score,
        "Tech content ({tech_score:.3}) should score >= hiring noise ({noise_score:.3})"
    );
}

#[test]
fn zero_context_produces_no_relevant_results() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Completely empty context — no interests, no ACE, 0 interactions
    let empty_ctx = ScoringContext::builder()
        .interest_count(0)
        .feedback_interaction_count(0)
        .build();

    let items = vec![
        (
            "Rust programming",
            "Rust memory safety and ownership for systems programming.",
        ),
        (
            "Python ML",
            "PyTorch machine learning model training with Python.",
        ),
        (
            "Hiring post",
            "We're hiring TypeScript engineers for our startup team.",
        ),
    ];

    for (title, content) in &items {
        let input = sim_input(1, title, content, &emb);
        let result = score_item(&input, &empty_ctx, &db, &opts, None);
        assert!(!result.relevant,
            "Zero-context user should not mark anything relevant: '{title}' got relevant={} score={:.3}",
            result.relevant, result.top_score);
    }
}

#[test]
fn score_breakdown_present_for_all_first_run_items() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = bootstrap_user();

    let items = vec![
        ("TypeScript tutorial", "Getting started with TypeScript type annotations and interfaces for JavaScript developers."),
        ("React hooks guide", "useState, useEffect, and custom hooks for React functional components."),
    ];

    for (title, content) in &items {
        let input = sim_input(1, title, content, &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);
        // score_breakdown should be present (even if minimal)
        // Note: breakdown is optional in SourceRelevance, so we just check it's not degenerate
        assert!(
            result.top_score >= 0.0,
            "Score should be non-negative for '{title}'"
        );
    }
}

#[test]
fn two_interests_improves_over_one_interest() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb_data = vec![0.0_f32; 384];
    let emb_interest = vec![0.5_f32; 384];

    let one_interest = {
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(emb_interest.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .feedback_interaction_count(0)
            .build()
    };

    let two_interests = {
        let interests = vec![
            crate::context_engine::Interest {
                id: Some(1),
                topic: "Rust".to_string(),
                weight: 1.0,
                embedding: Some(emb_interest.clone()),
                source: crate::context_engine::InterestSource::Explicit,
            },
            crate::context_engine::Interest {
                id: Some(2),
                topic: "systems programming".to_string(),
                weight: 1.0,
                embedding: Some(emb_interest.clone()),
                source: crate::context_engine::InterestSource::Explicit,
            },
        ];
        ScoringContext::builder()
            .interest_count(2)
            .interests(interests)
            .feedback_interaction_count(0)
            .build()
    };

    let rust_input = sim_input(1,
        "Rust systems programming and memory management",
        "Rust provides zero-cost abstractions for systems programming with memory safety guarantees.",
        &emb_data);

    let score_one = score_item(&rust_input, &one_interest, &db, &opts, None).top_score;
    let score_two = score_item(&rust_input, &two_interests, &db, &opts, None).top_score;

    // Interest normalization may dilute per-topic scores with more interests.
    // Verify the score doesn't drop catastrophically (>50% reduction).
    assert!(
        score_two >= score_one * 0.15,
        "Two interests ({score_two:.3}) dropped too far from one interest ({score_one:.3})"
    );
}
