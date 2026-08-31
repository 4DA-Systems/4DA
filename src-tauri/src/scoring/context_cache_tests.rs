// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Exactness tests for the incremental context-match merge.
//!
//! The merge is the one place a cached grounding result can silently diverge
//! from what the vector index would say, so every branch of it is pinned here.
//! The live equivalence check against a real corpus is
//! `drain_cost_profile::live_context_cache_equivalence`.

use super::*;

fn delta(id: i64, embedding: &[f32], grounds: bool, deleted: bool) -> ContextDelta {
    ContextDelta {
        context_id: id,
        deleted,
        grounds,
        embedding: embedding.to_vec(),
    }
}

/// vec0 reports plain Euclidean distance, not squared — pinned against the
/// value the linked 0.1.9 build actually returned for orthogonal unit vectors.
#[test]
fn l2_matches_vec0_semantics() {
    let d = l2_distance(&[1.0, 0.0, 0.0, 0.0], &[0.0, 1.0, 0.0, 0.0]);
    assert!(
        (d - std::f32::consts::SQRT_2).abs() < 1e-6,
        "orthogonal unit vectors must be sqrt(2) apart, got {d}"
    );
    assert!(l2_distance(&[0.5, 0.5], &[0.5, 0.5]).abs() < 1e-7);
}

/// A newly-added chunk closer than the worst cached one displaces it, and the
/// list stays sorted and capped.
#[test]
fn closer_new_chunk_displaces_the_worst() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.10), (11, 0.20), (12, 0.90)];
    // [0.999, 0.0] is ~0.001 away from the item — closer than everything.
    let d = [delta(99, &[0.999, 0.0], true, false)];

    let merged = merge_context_delta(&cached, &item, &d, 3).expect("mergeable");
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].0, 99, "the new chunk is now nearest");
    assert_eq!(merged[1].0, 10);
    assert_eq!(merged[2].0, 11);
    assert!(
        !merged.iter().any(|(id, _)| *id == 12),
        "the old worst entry was displaced"
    );
}

/// A new chunk farther than the K-th cached entry cannot enter the top-K.
#[test]
fn farther_new_chunk_is_ignored_when_full() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.10), (11, 0.11), (12, 0.12)];
    let d = [delta(99, &[0.0, 1.0], true, false)]; // sqrt(2) away

    let merged = merge_context_delta(&cached, &item, &d, 3).expect("mergeable");
    assert_eq!(
        merged, cached,
        "a full top-K is unchanged by a farther candidate"
    );
}

/// An UNDER-FILLED list (the corpus genuinely holds fewer than K eligible
/// chunks) still accepts additions — this is the case the pre-schema-113
/// over-fetch made unsound, because a short list could also mean "we stopped
/// looking", and merging into that is wrong.
#[test]
fn underfilled_list_accepts_additions() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.50)];
    let d = [delta(99, &[0.0, 1.0], true, false)];

    let merged = merge_context_delta(&cached, &item, &d, 3).expect("mergeable");
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].0, 10);
    assert_eq!(merged[1].0, 99);
}

/// A chunk ALREADY in the cached list turning up in the delta means its own
/// distance may have moved in either direction — the ordering can no longer be
/// repaired by adding candidates, so the merge must refuse and let the caller
/// recompute.
#[test]
fn touched_cached_chunk_refuses_the_merge() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.10), (11, 0.20)];

    for touched in [
        delta(10, &[0.0, 1.0], true, false),    // re-embedded
        delta(10, &[], true, true),             // deleted
        delta(10, &[0.999, 0.0], false, false), // reclassified out of grounding
    ] {
        assert!(
            merge_context_delta(&cached, &item, &[touched], 3).is_none(),
            "a delta touching a cached entry must force a recompute"
        );
    }
}

/// Deleted and grounding-ineligible chunks that are NOT in the cached list are
/// simply not candidates.
#[test]
fn deleted_and_ineligible_candidates_are_skipped() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.50)];
    let d = [
        delta(98, &[0.999, 0.0], true, true), // very close, but deleted
        delta(97, &[0.998, 0.0], false, false), // very close, but a doc chunk
    ];

    let merged = merge_context_delta(&cached, &item, &d, 3).expect("mergeable");
    assert_eq!(merged, cached, "neither candidate may enter the list");
}

/// Replaying a delta already reflected in the list is a no-op, so an entry
/// stamped at a stale generation is repaired rather than duplicated. (The
/// scoring context is TTL-cached, so an entry can legitimately be written
/// against a corpus newer than the generation it is stamped with.)
#[test]
fn replaying_an_applied_delta_is_idempotent() {
    let item = [1.0, 0.0];
    let d = [delta(99, &[0.999, 0.0], true, false)];

    let once = merge_context_delta(&[(10, 0.50)], &item, &d, 3).expect("first");
    assert!(once.iter().any(|(id, _)| *id == 99));
    // The list now CONTAINS 99, so a replay hits the refuse-branch rather than
    // duplicating it — which is the safe outcome: recompute, never corrupt.
    assert!(
        merge_context_delta(&once, &item, &d, 3).is_none(),
        "replaying onto a list that already holds the chunk forces a recompute"
    );
}

/// A dimension mismatch (a corpus mid re-embed at a different dimension) is
/// skipped rather than producing a garbage distance.
#[test]
fn dimension_mismatch_is_skipped() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.50)];
    let d = [delta(99, &[0.9, 0.0, 0.0, 0.0], true, false)];

    let merged = merge_context_delta(&cached, &item, &d, 3).expect("mergeable");
    assert_eq!(merged, cached);
}

/// An empty delta leaves the list exactly as it was — the common case on a
/// static context corpus, and the one that makes a drain nearly free.
#[test]
fn empty_delta_is_the_identity() {
    let item = [1.0, 0.0];
    let cached = vec![(10, 0.10), (11, 0.20)];
    assert_eq!(
        merge_context_delta(&cached, &item, &[], 3).expect("mergeable"),
        cached
    );
}
