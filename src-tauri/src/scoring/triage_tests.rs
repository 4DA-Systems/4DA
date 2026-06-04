// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the cheap relevance triage gate.
//!
//! The gate's defining property is HIGH RECALL: it must never drop something that
//! could be relevant. These tests pin each keep-path plus the one drop-path, and the
//! two safety carve-outs (high-stakes always kept; no-embedding fails open).

use super::{triage_item, TriageReason, TriageThresholds};
use crate::scoring::ace_context::ACEContext;
use crate::scoring::dependencies::{extract_search_terms, DepInfo};
use crate::scoring::ScoringContext;

const DIM: usize = crate::EMBEDDING_DIMS;

/// A 768-dim unit-ish vector with all weight on one axis (orthogonal to other axes).
fn axis_vec(axis: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; DIM];
    v[axis % DIM] = 1.0;
    v
}

fn dep_info(name: &str, ecosystem: &str) -> DepInfo {
    DepInfo {
        package_name: name.to_string(),
        version: None,
        is_dev: false,
        is_direct: true,
        search_terms: extract_search_terms(name),
        ecosystem: ecosystem.to_string(),
    }
}

fn ctx_with_taste(taste_axis: usize) -> ScoringContext {
    ScoringContext::builder()
        .taste_embedding(Some(axis_vec(taste_axis)))
        .build()
}

#[test]
fn high_stakes_cve_kept_even_when_semantically_far() {
    // Item embedding orthogonal to taste, no dep match — would normally be deferred.
    let ctx = ctx_with_taste(0);
    let v = triage_item(
        &axis_vec(500),
        "Some advisory",
        "details",
        Some("security_advisory"),
        Some("CVE-2026-0001"),
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(v.keep, "a security advisory must never be gated out");
    assert_eq!(v.reason, TriageReason::HighStakes);
}

#[test]
fn breaking_change_content_type_kept() {
    let ctx = ctx_with_taste(0);
    let v = triage_item(
        &axis_vec(500),
        "X 2.0 breaking changes",
        "migration guide",
        Some("breaking_change"),
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(v.keep);
    assert_eq!(v.reason, TriageReason::HighStakes);
}

#[test]
fn dependency_match_kept() {
    // User has tokio in their stack; an off-taste item about tokio must be kept.
    let mut ace = ACEContext::default();
    ace.dependency_info
        .insert("tokio".to_string(), dep_info("tokio", "rust"));
    let ctx = ScoringContext::builder()
        .taste_embedding(Some(axis_vec(0))) // taste is elsewhere
        .ace_ctx(ace)
        .build();

    let v = triage_item(
        &axis_vec(500), // far from taste
        "tokio 1.52 released",
        "async runtime update with new scheduler",
        Some("release_notes"),
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(
        v.keep,
        "an item matching the user's dependency graph must be kept"
    );
    assert_eq!(v.reason, TriageReason::DepMatch);
}

#[test]
fn taste_similar_kept() {
    // Item embedding identical to taste centroid → cosine 1.0 ≥ taste_min.
    let ctx = ctx_with_taste(3);
    let v = triage_item(
        &axis_vec(3),
        "Something the user likes",
        "body",
        None,
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(v.keep);
    assert_eq!(v.reason, TriageReason::TasteSimilar);
    assert!(v.similarity > 0.99);
}

#[test]
fn topic_similar_kept_without_taste() {
    // No taste centroid, but the item matches a tracked topic embedding.
    let mut topic_embeddings = std::collections::HashMap::new();
    topic_embeddings.insert("rust".to_string(), axis_vec(7));
    let ctx = ScoringContext::builder()
        .topic_embeddings(topic_embeddings)
        .build();

    let v = triage_item(
        &axis_vec(7),
        "A rust topic match",
        "body",
        None,
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(v.keep);
    assert_eq!(v.reason, TriageReason::TopicSimilar);
}

#[test]
fn far_from_everything_is_deferred_not_deleted() {
    // Off-taste, no deps, no topic, ordinary content type → DEFER (keep == false).
    let ctx = ctx_with_taste(0);
    let v = triage_item(
        &axis_vec(600),
        "Unrelated marketing fluff",
        "nothing to do with the user",
        Some("discussion"),
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(!v.keep, "a truly unrelated item should be deferred");
    assert_eq!(v.reason, TriageReason::Deferred);
}

#[test]
fn zero_embedding_fails_open() {
    // A fallback (all-zero) embedding can't be judged semantically — keep it rather
    // than silently drop something we couldn't evaluate.
    let ctx = ctx_with_taste(0);
    let v = triage_item(
        &vec![0.0f32; DIM],
        "Item with no usable embedding",
        "body",
        Some("discussion"),
        None,
        &ctx,
        &TriageThresholds::default(),
    );
    assert!(v.keep, "no-embedding items must fail open (high recall)");
    assert_eq!(v.reason, TriageReason::NoEmbedding);
}

#[test]
fn threshold_controls_taste_keep() {
    // Construct a partial-similarity item and show the threshold gates it.
    let mut taste = axis_vec(0);
    // Blend two axes so cosine to pure-axis-0 item is ~0.7.
    taste[1] = 1.0;
    let ctx = ScoringContext::builder()
        .taste_embedding(Some(taste))
        .build();
    let item = axis_vec(0); // cosine to the blended taste ≈ 1/sqrt(2) ≈ 0.707

    let loose = TriageThresholds {
        taste_min: 0.5,
        topic_min: 0.55,
    };
    let strict = TriageThresholds {
        taste_min: 0.9,
        topic_min: 0.55,
    };
    assert!(triage_item(&item, "t", "b", None, None, &ctx, &loose).keep);
    assert!(!triage_item(&item, "t", "b", None, None, &ctx, &strict).keep);
}
