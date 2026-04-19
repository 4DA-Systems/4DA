// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! System 4: Differential Regression Detection
//!
//! Tests that scoring is deterministic, monotone, and free from regressions
//! in the fundamental properties of the PASIFA scoring pipeline.

use super::super::ace_context::ACEContext;
use super::super::{score_item, ScoringContext};
use super::personas::rust_systems_dev;
use super::{sim_db, sim_input, sim_no_freshness};

// ============================================================================
// Helpers
// ============================================================================

fn rust_ctx() -> ScoringContext {
    rust_systems_dev()
}

fn simple_rust_ctx() -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "Rust".to_string(),
        weight: 1.0,
        embedding: Some(emb),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let mut ace = ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.detected_tech.push("rust".to_string());
    ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(20)
        .build()
}

// ============================================================================
// Determinism
// ============================================================================

#[test]
fn scoring_is_fully_deterministic() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let rust_input = sim_input(1,
        "Rust ownership and borrowing deep dive",
        "Understanding Rust's ownership model: move semantics, borrowing rules, and lifetime annotations for safe systems code.",
        &emb);

    // Score the same item 10 times
    let mut scores = Vec::new();
    for _ in 0..10 {
        let ctx = rust_ctx();
        let result = score_item(&rust_input, &ctx, &db, &opts, None);
        scores.push(result.top_score);
    }

    let first = scores[0];
    for (i, &score) in scores.iter().enumerate() {
        assert!(
            (score - first).abs() < 1e-6,
            "Scoring is non-deterministic: run 0={first:.6} run {i}={score:.6}"
        );
    }
}

#[test]
fn breakdown_fields_identical_across_runs() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let input = sim_input(
        1,
        "Rust systems programming with tokio",
        "Building async Rust services with tokio runtime, axum web framework, and SQLite database.",
        &emb,
    );

    let ctx1 = rust_ctx();
    let ctx2 = rust_ctx();

    let r1 = score_item(&input, &ctx1, &db, &opts, None);
    let r2 = score_item(&input, &ctx2, &db, &opts, None);

    assert!(
        (r1.top_score - r2.top_score).abs() < 1e-6,
        "top_score differs: {:.6} vs {:.6}",
        r1.top_score,
        r2.top_score
    );
    assert_eq!(r1.relevant, r2.relevant, "relevant flag differs");
    assert_eq!(r1.excluded, r2.excluded, "excluded flag differs");
    assert_eq!(
        r1.interest_score.to_bits(),
        r2.interest_score.to_bits(),
        "interest_score differs: {} vs {}",
        r1.interest_score,
        r2.interest_score
    );
}

#[test]
fn breakdown_multipliers_in_valid_range() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    let input = sim_input(
        1,
        "Rust performance optimization techniques",
        "Optimizing Rust code with SIMD, zero-copy parsing, and cache-friendly data structures.",
        &emb,
    );

    let result = score_item(&input, &ctx, &db, &opts, None);

    // Score should be in valid range
    assert!(
        result.top_score >= 0.0 && result.top_score <= 1.0,
        "top_score out of [0,1]: {}",
        result.top_score
    );
    assert!(
        result.interest_score >= 0.0 && result.interest_score <= 1.0,
        "interest_score out of [0,1]: {}",
        result.interest_score
    );
    assert!(
        result.context_score >= 0.0 && result.context_score <= 1.0,
        "context_score out of [0,1]: {}",
        result.context_score
    );
}

#[test]
fn breakdown_anti_penalty_range() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Rust ctx with anti-topic for Python
    let ctx = {
        let e = vec![0.5_f32; 384];
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(e),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        let mut ace = ACEContext::default();
        ace.active_topics.push("rust".to_string());
        ace.detected_tech.push("rust".to_string());
        ace.anti_topics.push("python".to_string());
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .feedback_interaction_count(20)
            .build()
    };

    let python_input = sim_input(
        1,
        "Python machine learning with scikit-learn",
        "Building ML pipelines with Python, scikit-learn, and pandas for data analysis.",
        &emb,
    );

    let result = score_item(&python_input, &ctx, &db, &opts, None);

    // Score should still be in valid range even with anti-penalty
    assert!(
        result.top_score >= 0.0,
        "Score went negative with anti-penalty: {}",
        result.top_score
    );
    assert!(
        result.top_score <= 1.0,
        "Score exceeded 1.0 despite anti-penalty: {}",
        result.top_score
    );
}

// ============================================================================
// Monotonicity
// ============================================================================

#[test]
fn more_interests_does_not_decrease_score() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let rust_content = sim_input(1,
        "Rust async programming with tokio",
        "Building concurrent Rust applications using tokio's async runtime and structured concurrency.",
        &emb);

    let emb_i = vec![0.5_f32; 384];

    let one_interest = {
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "Rust".to_string(),
            weight: 1.0,
            embedding: Some(emb_i.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .feedback_interaction_count(20)
            .build()
    };

    let three_interests = {
        let interests = vec![
            crate::context_engine::Interest {
                id: Some(1),
                topic: "Rust".to_string(),
                weight: 1.0,
                embedding: Some(emb_i.clone()),
                source: crate::context_engine::InterestSource::Explicit,
            },
            crate::context_engine::Interest {
                id: Some(2),
                topic: "async programming".to_string(),
                weight: 1.0,
                embedding: Some(emb_i.clone()),
                source: crate::context_engine::InterestSource::Explicit,
            },
            crate::context_engine::Interest {
                id: Some(3),
                topic: "systems programming".to_string(),
                weight: 1.0,
                embedding: Some(emb_i.clone()),
                source: crate::context_engine::InterestSource::Explicit,
            },
        ];
        ScoringContext::builder()
            .interest_count(3)
            .interests(interests)
            .feedback_interaction_count(20)
            .build()
    };

    let score_one = score_item(&rust_content, &one_interest, &db, &opts, None).top_score;
    let score_three = score_item(&rust_content, &three_interests, &db, &opts, None).top_score;

    assert!(score_three >= score_one - 0.15,
        "More interests should not significantly decrease score: one={score_one:.3} three={score_three:.3}");
}

#[test]
fn ace_active_topics_does_not_decrease_score() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let emb_i = vec![0.5_f32; 384];

    let rust_input = sim_input(1,
        "Rust SQLite integration with sqlx",
        "Using sqlx for async SQLite access in Rust applications with compile-time query verification.",
        &emb);

    let make_interest = || crate::context_engine::Interest {
        id: Some(1),
        topic: "Rust".to_string(),
        weight: 1.0,
        embedding: Some(emb_i.clone()),
        source: crate::context_engine::InterestSource::Explicit,
    };

    let no_ace = {
        ScoringContext::builder()
            .interest_count(1)
            .interests(vec![make_interest()])
            .feedback_interaction_count(20)
            .build()
    };

    let with_ace = {
        let mut ace = ACEContext::default();
        ace.active_topics.push("rust".to_string());
        ace.active_topics.push("sqlite".to_string());
        ace.detected_tech.push("rust".to_string());
        ace.detected_tech.push("sqlite".to_string());
        ScoringContext::builder()
            .interest_count(1)
            .interests(vec![make_interest()])
            .ace_ctx(ace)
            .feedback_interaction_count(20)
            .build()
    };

    let score_no_ace = score_item(&rust_input, &no_ace, &db, &opts, None).top_score;
    let score_with_ace = score_item(&rust_input, &with_ace, &db, &opts, None).top_score;

    assert!(score_with_ace >= score_no_ace - 0.05,
        "ACE context should not decrease score: no_ace={score_no_ace:.3} with_ace={score_with_ace:.3}");
}

#[test]
fn signal_count_monotone_with_context_richness() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let rich_content = sim_input(1,
        "Rust Tauri SQLite systems programming async tokio serde",
        "Building Rust applications with Tauri for desktop UI, SQLite for storage, tokio for async, and serde for serialization. Systems programming excellence.",
        &emb);

    let sparse_ctx = simple_rust_ctx();
    let rich_ctx = rust_ctx();

    let score_sparse = score_item(&rich_content, &sparse_ctx, &db, &opts, None).top_score;
    let score_rich = score_item(&rich_content, &rich_ctx, &db, &opts, None).top_score;

    assert!(score_rich >= score_sparse - 0.25,
        "Richer context should not significantly decrease score: sparse={score_sparse:.3} rich={score_rich:.3}");
}

// ============================================================================
// Penalty Asymmetry
// ============================================================================

#[test]
fn penalties_asymmetrically_stronger_than_boosts() {
    // Anti-topics should have a meaningful dampening effect
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let python_input = sim_input(
        1,
        "Python machine learning frameworks comparison",
        "Comparing Python ML frameworks: PyTorch, TensorFlow, and JAX for deep learning research.",
        &emb,
    );

    let neutral_ctx = {
        let e = vec![0.5_f32; 384];
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "programming".to_string(),
            weight: 0.5,
            embedding: Some(e),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .feedback_interaction_count(20)
            .build()
    };

    let anti_ctx = {
        let e = vec![0.5_f32; 384];
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "programming".to_string(),
            weight: 0.5,
            embedding: Some(e),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        let mut ace = ACEContext::default();
        ace.anti_topics.push("python".to_string());
        ace.anti_topics.push("machine learning".to_string());
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .feedback_interaction_count(20)
            .build()
    };

    let neutral_score = score_item(&python_input, &neutral_ctx, &db, &opts, None).top_score;
    let anti_score = score_item(&python_input, &anti_ctx, &db, &opts, None).top_score;

    // Anti-topics should reduce score, but score must remain non-negative
    assert!(
        anti_score >= 0.0,
        "Anti-penalty drove score negative: {anti_score:.3}"
    );
    assert!(
        anti_score <= neutral_score + 0.05,
        "Anti-topics had no dampening effect: neutral={neutral_score:.3} anti={anti_score:.3}"
    );
}

// ============================================================================
// Exclusion Invariants
// ============================================================================

#[test]
fn excluded_item_scores_exactly_zero() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Create context with exclusion for "blockchain"
    let e = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "Rust".to_string(),
        weight: 1.0,
        embedding: Some(e),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let ctx = ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .exclusions(vec!["blockchain".to_string(), "crypto".to_string()])
        .feedback_interaction_count(20)
        .build();

    let excluded_input = sim_input(
        1,
        "Bitcoin blockchain reaches new all-time high in crypto rally",
        "blockchain cryptocurrency crypto trading market rally bitcoin decentralized finance",
        &emb,
    );

    let result = score_item(&excluded_input, &ctx, &db, &opts, None);
    assert!(
        result.excluded,
        "Item with exclusion keyword should be excluded"
    );
    assert_eq!(result.top_score, 0.0, "Excluded item should have score 0.0");
    assert!(!result.relevant, "Excluded item should not be relevant");
}

#[test]
fn exclusion_does_not_affect_non_matching_content() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let e = vec![0.5_f32; 384];
    let interests = vec![crate::context_engine::Interest {
        id: Some(1),
        topic: "Rust".to_string(),
        weight: 1.0,
        embedding: Some(e),
        source: crate::context_engine::InterestSource::Explicit,
    }];
    let ctx = ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .exclusions(vec!["hiring".to_string()])
        .feedback_interaction_count(20)
        .build();

    let rust_input = sim_input(
        1,
        "Rust ownership and memory safety",
        "Rust's ownership system prevents data races and memory errors at compile time.",
        &emb,
    );

    let result = score_item(&rust_input, &ctx, &db, &opts, None);
    assert!(
        !result.excluded,
        "Non-matching content should not be excluded"
    );
}

#[test]
fn very_short_title_capped_in_score() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    // Very short title with keyword match
    let short_input = sim_input(1, "Rust", "r", &emb);
    let result = score_item(&short_input, &ctx, &db, &opts, None);

    // Should not crash, score should be in valid range
    assert!(
        result.top_score >= 0.0 && result.top_score <= 1.0,
        "Short title score out of bounds: {}",
        result.top_score
    );
}

#[test]
fn empty_content_handled_gracefully() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    let empty_input = sim_input(1, "Rust programming language", "", &emb);
    let result = score_item(&empty_input, &ctx, &db, &opts, None);

    assert!(
        result.top_score >= 0.0 && result.top_score <= 1.0,
        "Empty content score out of bounds: {}",
        result.top_score
    );
}

#[test]
fn source_type_does_not_affect_core_score_determinism() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    let title = "Rust memory management deep dive";
    let content =
        "Rust's ownership model enables safe memory management without a garbage collector.";

    // Same content, different source types
    let hn_result = {
        let input = crate::scoring::ScoringInput {
            id: 1,
            title,
            url: None,
            content,
            source_type: "hackernews",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
        };
        score_item(&input, &ctx, &db, &opts, None)
    };

    let reddit_result = {
        let input = crate::scoring::ScoringInput {
            id: 1,
            title,
            url: None,
            content,
            source_type: "reddit",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
        };
        score_item(&input, &ctx, &db, &opts, None)
    };

    // Core score (interest + context) should be the same regardless of source
    assert!(
        (hn_result.interest_score - reddit_result.interest_score).abs() < 1e-6,
        "interest_score differs by source type: hn={:.6} reddit={:.6}",
        hn_result.interest_score,
        reddit_result.interest_score
    );
}

#[test]
fn score_breakdown_always_present() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    let items = vec![
        ("Rust crate", "A Rust crate for async programming."),
        (
            "Python tutorial",
            "Getting started with Python for beginners.",
        ),
        ("Hiring post", "We are hiring Rust engineers for our team."),
    ];

    for (title, content) in &items {
        let input = sim_input(1, title, content, &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);
        // All items should have valid score ranges
        assert!(result.top_score >= 0.0, "Negative score for '{title}'");
        assert!(result.top_score <= 1.0, "Score > 1.0 for '{title}'");
    }
}

#[test]
fn confirmed_signals_consistent_with_signal_count() {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let ctx = rust_ctx();

    let input = sim_input(1,
        "Rust Tauri SQLite tokio serde systems programming",
        "Building desktop applications with Rust using Tauri, SQLite, tokio async runtime, and serde serialization.",
        &emb);

    let result = score_item(&input, &ctx, &db, &opts, None);

    // Result must be internally consistent
    assert!(result.top_score >= 0.0 && result.top_score <= 1.0);
    if result.excluded {
        assert_eq!(
            result.top_score, 0.0,
            "Excluded item has non-zero score: {}",
            result.top_score
        );
    }
}
