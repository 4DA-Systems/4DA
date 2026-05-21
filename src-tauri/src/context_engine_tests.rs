// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use parking_lot::Mutex;
use std::sync::Arc;

/// Create an in-memory ContextEngine for testing.
fn test_engine() -> ContextEngine {
    let conn = Connection::open_in_memory().expect("in-memory DB");
    ContextEngine::new(Arc::new(Mutex::new(conn))).expect("context engine init")
}

#[test]
fn test_embedding_conversion() {
    let original = vec![1.0, 2.5, -0.5, 0.0];
    let blob = embedding_to_blob(&original);
    let restored = blob_to_embedding(&blob);
    assert_eq!(original, restored);
}

#[test]
fn test_embedding_conversion_empty() {
    let original: Vec<f32> = vec![];
    let blob = embedding_to_blob(&original);
    let restored = blob_to_embedding(&blob);
    assert!(
        restored.is_empty(),
        "Empty embedding should round-trip to empty"
    );
}

#[test]
fn test_embedding_conversion_384_dim() {
    let original: Vec<f32> = (0..crate::EMBEDDING_DIMS)
        .map(|i| (i as f32) * 0.001)
        .collect();
    let blob = embedding_to_blob(&original);
    let restored = blob_to_embedding(&blob);
    assert_eq!(original.len(), restored.len());
    for (a, b) in original.iter().zip(restored.iter()) {
        assert!(
            (a - b).abs() < f32::EPSILON,
            "Mismatch at value: {a} vs {b}"
        );
    }
}

// ========================================================================
// Interest merge: Explicit should win over Inferred for same topic
// ========================================================================

/// When the same topic is added as both Inferred and Explicit, the last
/// write wins due to UPSERT (ON CONFLICT DO UPDATE). So adding Explicit
/// AFTER Inferred should result in the interest being Explicit.
/// This is the correct behavior: explicit user declarations override inferences.
#[test]
fn test_interest_merge_prefers_explicit() {
    let engine = test_engine();

    // Add "rust" as Inferred with weight 0.5
    engine
        .add_interest("rust", 0.5, None, InterestSource::Inferred)
        .expect("add inferred interest");

    // Verify it's Inferred
    let interests = engine.get_interests().expect("get interests");
    assert_eq!(interests.len(), 1);
    assert_eq!(interests[0].source, InterestSource::Inferred);
    assert!((interests[0].weight - 0.5).abs() < f32::EPSILON);

    // Add "rust" as Explicit with weight 1.0 — should UPSERT and override
    engine
        .add_interest("rust", 1.0, None, InterestSource::Explicit)
        .expect("add explicit interest");

    // Verify it's now Explicit with updated weight
    let interests = engine.get_interests().expect("get interests after merge");
    assert_eq!(
        interests.len(),
        1,
        "UPSERT should not create a duplicate — got {} interests",
        interests.len()
    );
    assert_eq!(
        interests[0].source,
        InterestSource::Explicit,
        "Explicit should override Inferred after UPSERT"
    );
    assert!(
        (interests[0].weight - 1.0).abs() < f32::EPSILON,
        "Weight should be updated to 1.0, got {}",
        interests[0].weight
    );
}

/// The reverse case: adding Inferred AFTER Explicit should also replace,
/// but in practice the system should avoid this (Explicit always wins).
/// This test documents the current UPSERT behavior: last-write-wins.
#[test]
fn test_interest_merge_last_write_wins() {
    let engine = test_engine();

    engine
        .add_interest("typescript", 1.0, None, InterestSource::Explicit)
        .expect("add explicit");

    engine
        .add_interest("typescript", 0.3, None, InterestSource::Inferred)
        .expect("add inferred");

    let interests = engine.get_interests().expect("get interests");
    assert_eq!(
        interests.len(),
        1,
        "Should still be one interest after UPSERT"
    );
    // Last write wins — this is the current behavior
    assert_eq!(
        interests[0].source,
        InterestSource::Inferred,
        "Last-write-wins: Inferred overwrites Explicit (caller must enforce ordering)"
    );
    assert!(
        (interests[0].weight - 0.3).abs() < f32::EPSILON,
        "Weight should be 0.3 from the second insert, got {}",
        interests[0].weight
    );
}

// ========================================================================
// Interest weight bounds: weights from DB should be in valid range
// ========================================================================

/// Weights added via add_interest should be preserved exactly.
/// The system trusts callers to provide valid weights, but we verify
/// that round-tripping through the DB doesn't corrupt values.
#[test]
fn test_interest_weight_bounds() {
    let engine = test_engine();

    // Add interests at boundary weights
    engine
        .add_interest("min_weight", 0.0, None, InterestSource::Explicit)
        .expect("add 0.0 weight");
    engine
        .add_interest("max_weight", 1.0, None, InterestSource::Explicit)
        .expect("add 1.0 weight");
    engine
        .add_interest("mid_weight", 0.5, None, InterestSource::Inferred)
        .expect("add 0.5 weight");

    let interests = engine.get_interests().expect("get interests");
    assert_eq!(interests.len(), 3);

    for interest in &interests {
        assert!(
            interest.weight >= 0.0 && interest.weight <= 1.0,
            "Interest '{}' weight {} should be in [0.0, 1.0]",
            interest.topic,
            interest.weight
        );
    }

    // Verify specific values round-tripped correctly
    let min = interests.iter().find(|i| i.topic == "min_weight").unwrap();
    assert!(
        (min.weight - 0.0).abs() < f32::EPSILON,
        "0.0 weight should round-trip exactly, got {}",
        min.weight
    );

    let max = interests.iter().find(|i| i.topic == "max_weight").unwrap();
    assert!(
        (max.weight - 1.0).abs() < f32::EPSILON,
        "1.0 weight should round-trip exactly, got {}",
        max.weight
    );
}

// ========================================================================
// InterestSource serialization
// ========================================================================

/// Verify all InterestSource variants survive the DB round-trip.
#[test]
fn test_interest_source_round_trip_all_variants() {
    let engine = test_engine();

    let sources = vec![
        ("explicit_topic", InterestSource::Explicit),
        ("github_topic", InterestSource::GitHub),
        ("import_topic", InterestSource::Import),
        ("inferred_topic", InterestSource::Inferred),
    ];

    for (topic, source) in &sources {
        engine
            .add_interest(topic, 0.8, None, source.clone())
            .expect(&format!("add {topic}"));
    }

    let interests = engine.get_interests().expect("get interests");
    assert_eq!(interests.len(), 4);

    for (topic, expected_source) in &sources {
        let interest = interests
            .iter()
            .find(|i| i.topic == *topic)
            .unwrap_or_else(|| panic!("Missing interest: {topic}"));
        assert_eq!(
            &interest.source, expected_source,
            "Source for '{topic}' should be {expected_source:?}, got {:?}",
            interest.source
        );
    }
}

// ========================================================================
// Embedding round-trip through add_interest / get_interests
// ========================================================================

/// Verify that embeddings stored via add_interest are correctly
/// retrieved via get_interests (blob encoding round-trip).
#[test]
fn test_interest_embedding_round_trip() {
    let engine = test_engine();

    let embedding: Vec<f32> = (0..crate::EMBEDDING_DIMS)
        .map(|i| (i as f32) * 0.002 - 0.384)
        .collect();
    engine
        .add_interest("rust", 1.0, Some(&embedding), InterestSource::Explicit)
        .expect("add interest with embedding");

    let interests = engine.get_interests().expect("get interests");
    assert_eq!(interests.len(), 1);

    let stored = interests[0]
        .embedding
        .as_ref()
        .expect("embedding should be present");
    assert_eq!(
        stored.len(),
        crate::EMBEDDING_DIMS,
        "Embedding dimension should match EMBEDDING_DIMS"
    );

    for (i, (a, b)) in embedding.iter().zip(stored.iter()).enumerate() {
        assert!(
            (a - b).abs() < f32::EPSILON,
            "Mismatch at index {i}: {a} vs {b}"
        );
    }
}

/// Interest added without embedding should have None embedding.
#[test]
fn test_interest_no_embedding_returns_none() {
    let engine = test_engine();

    engine
        .add_interest("python", 0.7, None, InterestSource::Explicit)
        .expect("add interest without embedding");

    let interests = engine.get_interests().expect("get interests");
    assert_eq!(interests.len(), 1);
    assert!(
        interests[0].embedding.is_none(),
        "Interest without embedding should have None, got Some({} dims)",
        interests[0].embedding.as_ref().map_or(0, |e| e.len())
    );
}

// ========================================================================
// Exclusion management
// ========================================================================

#[test]
fn test_exclusion_add_and_remove() {
    let engine = test_engine();

    engine.add_exclusion("crypto").expect("add exclusion");
    engine.add_exclusion("blockchain").expect("add exclusion");

    let exclusions = engine.get_exclusions().expect("get exclusions");
    assert_eq!(exclusions.len(), 2);
    assert!(exclusions.contains(&"blockchain".to_string()));
    assert!(exclusions.contains(&"crypto".to_string()));

    engine.remove_exclusion("crypto").expect("remove exclusion");
    let exclusions = engine.get_exclusions().expect("get exclusions");
    assert_eq!(exclusions.len(), 1);
    assert_eq!(exclusions[0], "blockchain");
}

#[test]
fn test_exclusion_duplicate_is_ignored() {
    let engine = test_engine();

    engine.add_exclusion("spam").expect("add first");
    engine.add_exclusion("spam").expect("add duplicate");

    let count = engine.exclusion_count().expect("count");
    assert_eq!(
        count, 1,
        "Duplicate exclusion should be ignored via INSERT OR IGNORE"
    );
}

// ========================================================================
// Static identity aggregate
// ========================================================================

#[test]
fn test_static_identity_aggregates_all_layers() {
    let engine = test_engine();

    engine
        .set_role(Some("Backend Developer"))
        .expect("set role");
    engine.add_technology("rust").expect("add tech");
    engine.add_technology("typescript").expect("add tech");
    engine
        .add_interest("distributed systems", 1.0, None, InterestSource::Explicit)
        .expect("add interest");
    engine.add_exclusion("crypto").expect("add exclusion");

    let identity = engine.get_static_identity().expect("get identity");
    assert_eq!(identity.role.as_deref(), Some("Backend Developer"));
    assert_eq!(identity.tech_stack.len(), 2);
    assert_eq!(identity.interests.len(), 1);
    assert_eq!(identity.interests[0].topic, "distributed systems");
    assert_eq!(identity.exclusions.len(), 1);
    assert_eq!(identity.exclusions[0], "crypto");
}

// Note: test_cosine_similarity and test_exclusion_filter were removed
// as they tested the removed ContextMembrane functionality.
// ACE module provides comprehensive relevance scoring tests.
