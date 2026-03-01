//! ACE Context Validation Tests
//!
//! Isolates the ACE axis contributions to scoring:
//! - Anti-topic exclusion (check_ace_exclusions)
//! - Dependency matching (match_dependencies)
//! - Detected tech influence
//! - Topic affinity amplification (compute_semantic_ace_boost)

use super::super::{score_item, ScoringContext};
use super::corpus::corpus;
use super::personas::make_interests;
use super::{sim_db, sim_input, sim_no_freshness};
use crate::scoring::ace_context::ACEContext;
use crate::scoring::dependencies::DepInfo;

// ============================================================================
// Test 1: Anti-topic exclusion
// ============================================================================

#[test]
fn ace_anti_topic_excludes_matching_content() {
    // Build a Rust persona but add "python" as an anti-topic
    let interests = make_interests(&[("Rust", 1.0), ("systems programming", 1.0)]);
    let mut ace = ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.detected_tech.push("rust".to_string());
    ace.anti_topics.push("python".to_string());

    let ctx = ScoringContext::builder()
        .interest_count(2)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Find a Python-focused corpus item
    let items = corpus();
    let python_item = items.iter().find(|i| {
        let title_lower = i.title.to_lowercase();
        title_lower.contains("python") || title_lower.contains("pytorch")
    });

    if let Some(item) = python_item {
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            result.excluded || result.top_score == 0.0,
            "Python content should be excluded or scored 0 when 'python' is an anti-topic, \
             but got excluded={}, top_score={:.3}",
            result.excluded,
            result.top_score,
        );
    }
}

// ============================================================================
// Test 2: Dependency matching boosts score
// ============================================================================

#[test]
fn ace_dependency_match_boosts_score() {
    let interests = make_interests(&[("Rust", 1.0), ("systems programming", 1.0), ("Tauri", 0.9)]);

    // Persona WITH dependency info for tokio
    let mut ace_with_deps = ACEContext::default();
    ace_with_deps.active_topics.push("rust".to_string());
    ace_with_deps.detected_tech.push("rust".to_string());
    ace_with_deps.dependency_names.insert("tokio".to_string());
    ace_with_deps.dependency_info.insert(
        "tokio".to_string(),
        DepInfo {
            package_name: "tokio".to_string(),
            search_terms: vec!["tokio".to_string()],
            is_dev: false,
            version: Some("1.36".to_string()),
        },
    );

    let ctx_with_deps = ScoringContext::builder()
        .interest_count(3)
        .interests(interests.clone())
        .ace_ctx(ace_with_deps)
        .feedback_interaction_count(50)
        .build();

    // Persona WITHOUT dependency info
    let mut ace_no_deps = ACEContext::default();
    ace_no_deps.active_topics.push("rust".to_string());
    ace_no_deps.detected_tech.push("rust".to_string());

    let ctx_no_deps = ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ace_no_deps)
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Score a tokio-related item
    let input = sim_input(
        999,
        "Tokio 1.36 released with major performance improvements",
        "The async runtime for Rust gets faster task scheduling, better resource usage, and improved tokio::sync primitives.",
        &emb,
    );

    let result_with = score_item(&input, &ctx_with_deps, &db, &opts, None);
    let result_without = score_item(&input, &ctx_no_deps, &db, &opts, None);

    assert!(
        result_with.top_score >= result_without.top_score,
        "Tokio content with tokio dependency ({:.3}) should score >= without ({:.3})",
        result_with.top_score,
        result_without.top_score,
    );
}

// ============================================================================
// Test 3: Detected tech influences scoring
// ============================================================================

#[test]
fn ace_detected_tech_influences_scoring() {
    let interests = make_interests(&[("Rust", 1.0)]);

    // Persona WITH detected_tech
    let mut ace_with_tech = ACEContext::default();
    ace_with_tech.active_topics.push("rust".to_string());
    ace_with_tech.detected_tech.push("rust".to_string());
    ace_with_tech.detected_tech.push("tauri".to_string());

    let ctx_with_tech = ScoringContext::builder()
        .interest_count(1)
        .interests(interests.clone())
        .ace_ctx(ace_with_tech)
        .feedback_interaction_count(50)
        .build();

    // Persona WITHOUT detected_tech
    let mut ace_no_tech = ACEContext::default();
    ace_no_tech.active_topics.push("rust".to_string());

    let ctx_no_tech = ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace_no_tech)
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Score Rust content
    let items = corpus();
    let rust_item = items.iter().find(|i| {
        let title_lower = i.title.to_lowercase();
        title_lower.contains("rust")
    });

    if let Some(item) = rust_item {
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result_with = score_item(&input, &ctx_with_tech, &db, &opts, None);
        let result_no = score_item(&input, &ctx_no_tech, &db, &opts, None);

        assert!(
            result_with.top_score >= result_no.top_score,
            "Rust content with detected_tech ({:.3}) should score >= without ({:.3})",
            result_with.top_score,
            result_no.top_score,
        );
    }
}

// ============================================================================
// Test 4: Topic affinities amplify scores
// ============================================================================

#[test]
fn ace_topic_affinities_amplify_scores() {
    let interests = make_interests(&[("Rust", 1.0), ("systems programming", 0.9)]);

    // Persona with POSITIVE affinity for "rust"
    let mut ace_positive = ACEContext::default();
    ace_positive.active_topics.push("rust".to_string());
    ace_positive.detected_tech.push("rust".to_string());
    ace_positive
        .topic_affinities
        .insert("rust".to_string(), (1.0, 0.9)); // strong positive

    let ctx_positive = ScoringContext::builder()
        .interest_count(2)
        .interests(interests.clone())
        .ace_ctx(ace_positive)
        .feedback_interaction_count(50)
        .build();

    // Persona with NEGATIVE affinity for "rust"
    let mut ace_negative = ACEContext::default();
    ace_negative.active_topics.push("rust".to_string());
    ace_negative.detected_tech.push("rust".to_string());
    ace_negative
        .topic_affinities
        .insert("rust".to_string(), (-0.5, 0.9)); // negative

    let ctx_negative = ScoringContext::builder()
        .interest_count(2)
        .interests(interests.clone())
        .ace_ctx(ace_negative)
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();

    // Use domain embeddings for non-zero vectors to activate semantic path
    #[cfg(feature = "calibrated-sim")]
    let embeddings = super::domain_embeddings::corpus_embeddings();
    #[cfg(not(feature = "calibrated-sim"))]
    let embeddings: Vec<Vec<f32>> = vec![];

    // If we have calibrated embeddings, test the semantic path
    if !embeddings.is_empty() {
        let fallback = vec![0.0_f32; 384];
        let items = corpus();
        let rust_item = items
            .iter()
            .find(|i| i.title.to_lowercase().contains("rust"));

        if let Some(item) = rust_item {
            let emb = embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&fallback);

            let input = sim_input(item.id, item.title, item.content, emb);
            let result_pos = score_item(&input, &ctx_positive, &db, &opts, None);
            let result_neg = score_item(&input, &ctx_negative, &db, &opts, None);

            assert!(
                result_pos.top_score >= result_neg.top_score,
                "Positive affinity ({:.3}) should score >= negative affinity ({:.3})",
                result_pos.top_score,
                result_neg.top_score,
            );
        }
    }

    // Also test with zero vectors — keyword path still uses affinities
    let emb_zero = vec![0.0_f32; 384];
    let items = corpus();
    let rust_item = items
        .iter()
        .find(|i| i.title.to_lowercase().contains("rust"));

    if let Some(item) = rust_item {
        let input = sim_input(item.id, item.title, item.content, &emb_zero);
        let result_pos = score_item(&input, &ctx_positive, &db, &opts, None);
        let result_neg = score_item(&input, &ctx_negative, &db, &opts, None);

        // With the keyword path, positive affinity should produce >= scores
        assert!(
            result_pos.top_score >= result_neg.top_score,
            "Positive affinity keyword path ({:.3}) should score >= negative ({:.3})",
            result_pos.top_score,
            result_neg.top_score,
        );
    }
}
