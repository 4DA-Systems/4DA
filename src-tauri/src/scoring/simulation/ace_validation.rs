// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE Context Validation Tests
//!
//! Isolates the ACE axis contributions to scoring:
//! - Dependency matching (match_dependencies)
//! - Detected tech influence
//!
//! (The former "anti-topic exclusion" test was removed in v20b: the ACE
//! anti-topic auto-exclusion mechanism it asserted was removed from scoring
//! in v19 (AD-029), and the test had passed vacuously ever since — a pure-Rust
//! persona scores an off-topic python item near zero regardless.)

use super::super::{score_item, ScoringContext};
use super::corpus::corpus;
use super::personas::make_interests;
use super::{sim_db, sim_input, sim_no_freshness};
use crate::scoring::ace_context::ACEContext;
use crate::scoring::dependencies::DepInfo;

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
            is_direct: true,
            version: Some("1.36".to_string()),
            ecosystem: "rust".to_string(),
            project_paths: Vec::new(),
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
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

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

    // Verify the breakdown shows actual dependency contribution
    if let Some(ref bd) = result_with.score_breakdown {
        assert!(
            bd.dep_match_score > 0.0,
            "Breakdown should show positive dep_match_score, got {:.3}",
            bd.dep_match_score
        );
        assert!(
            bd.matched_deps.iter().any(|d| d.contains("tokio")),
            "Breakdown should list tokio in matched_deps: {:?}",
            bd.matched_deps
        );
    }
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
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

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

        // Verify detected tech produces confirmation signals
        if let Some(ref bd) = result_with.score_breakdown {
            assert!(
                bd.signal_count >= 1,
                "Detected tech should produce >= 1 confirmation signal, got {}",
                bd.signal_count
            );
        }
    }
}

// ============================================================================
// Test 4: ACE boost is non-zero with calibrated embeddings
// ============================================================================

#[test]
fn ace_semantic_boost_nonzero_with_embeddings() {
    let interests = make_interests(&[("Rust", 1.0), ("systems programming", 1.0), ("Tauri", 0.9)]);

    let mut ace = ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.detected_tech.push("rust".to_string());
    ace.detected_tech.push("tauri".to_string());

    let ctx = ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();

    {
        let embeddings = super::domain_embeddings::corpus_embeddings();
        let fallback = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let items = corpus();
        // Find a Rust corpus item (IDs 1-10 are Systems/Rust)
        let rust_item = items.iter().find(|i| i.id >= 1 && i.id <= 10);
        if let Some(item) = rust_item {
            let emb = embeddings.get((item.id - 1) as usize).unwrap_or(&fallback);
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            if let Some(ref bd) = result.score_breakdown {
                assert!(
                    bd.ace_boost > 0.0,
                    "ACE boost should be > 0 with domain embeddings and active ACE context, got {:.4}",
                    bd.ace_boost
                );
            }
        }
    }

    // With zero embeddings, ACE boost may still activate via keyword path
    let emb_zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let items = corpus();
    let rust_item = items.iter().find(|i| i.id >= 1 && i.id <= 10);
    if let Some(item) = rust_item {
        let input = sim_input(item.id, item.title, item.content, &emb_zero);
        let result = score_item(&input, &ctx, &db, &opts, None);
        // At minimum, the item should be scored (not zero)
        assert!(
            result.top_score > 0.0,
            "Rust content with full ACE context should score > 0, got {:.4}",
            result.top_score
        );
    }
}

// ============================================================================
// Test 5: ACE context increases confirmation signal count
// ============================================================================

#[test]
fn ace_context_increases_confirmation_signals() {
    let interests = make_interests(&[("Rust", 1.0), ("systems programming", 1.0), ("Tauri", 0.9)]);

    // Full ACE persona: topics + tech + deps
    let mut ace_full = ACEContext::default();
    ace_full.active_topics.push("rust".to_string());
    ace_full.active_topics.push("tauri".to_string());
    ace_full.detected_tech.push("rust".to_string());
    ace_full.detected_tech.push("tauri".to_string());
    ace_full.dependency_names.insert("tokio".to_string());
    ace_full.dependency_info.insert(
        "tokio".to_string(),
        DepInfo {
            package_name: "tokio".to_string(),
            search_terms: vec!["tokio".to_string()],
            is_dev: false,
            is_direct: true,
            version: Some("1.36".to_string()),
            ecosystem: "rust".to_string(),
            project_paths: Vec::new(),
        },
    );

    let ctx_full = ScoringContext::builder()
        .interest_count(3)
        .interests(interests.clone())
        .ace_ctx(ace_full)
        .feedback_interaction_count(50)
        .build();

    // Empty ACE persona: same interests but no ACE context
    let ctx_empty = ScoringContext::builder()
        .interest_count(3)
        .interests(interests)
        .ace_ctx(ACEContext::default())
        .feedback_interaction_count(50)
        .build();

    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    // Score content that mentions tokio + rust
    let input = sim_input(
        998,
        "Tokio async runtime improvements for Rust systems",
        "Major performance boost in tokio 1.36 for async task scheduling in Rust.",
        &emb,
    );

    let result_full = score_item(&input, &ctx_full, &db, &opts, None);
    let result_empty = score_item(&input, &ctx_empty, &db, &opts, None);

    if let (Some(ref bd_full), Some(ref bd_empty)) =
        (&result_full.score_breakdown, &result_empty.score_breakdown)
    {
        assert!(
            bd_full.signal_count >= bd_empty.signal_count,
            "Full ACE signal_count ({}) should >= empty ACE signal_count ({})",
            bd_full.signal_count,
            bd_empty.signal_count
        );
    }

    // Score with full ACE should be >= empty ACE
    assert!(
        result_full.top_score >= result_empty.top_score,
        "Full ACE score ({:.3}) should >= empty ACE score ({:.3})",
        result_full.top_score,
        result_empty.top_score
    );
}
