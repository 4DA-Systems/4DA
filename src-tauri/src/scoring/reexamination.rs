// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Re-examination triggers — Phase 3 of the scoring relevance funnel.
//!
//! Relevance is a function of (item × developer-state × time): a release of a crate you
//! don't use yet is noise today and critical the day you adopt it. The product promise —
//! "yesterday's noise becomes tomorrow's signal" — only holds if buried items are
//! re-examined when the developer's stack changes. The `PIPELINE_VERSION` drain already
//! re-scores everything when the SCORING logic changes; this closes the other half:
//! re-scoring when the PROFILE changes.
//!
//! Mechanism — cheap and loop-safe:
//! 1. Hash the developer's dependency-name set into a "dep epoch". It changes iff a dep
//!    is added or removed.
//! 2. When the epoch changes, re-queue the buried items a dep match would actually FLIP —
//!    releases and security/breaking advisories scored as noise — by resetting their
//!    pipeline version to 0. The Phase-2 backfill then re-scores them (prioritized)
//!    against the new dep graph, so a now-tracked dependency's releases/CVEs surface.
//! 3. Casual mentions (discussions) are deliberately NOT re-queued: a dep match doesn't
//!    change their verdict, so re-scoring them is wasted work.
//!
//! Loop-safe: the reset is a one-shot batched update gated on the epoch CHANGING. After
//! re-scoring, items are at the current version again; the epoch is unchanged, so they
//! are not re-selected until the stack changes again. An item that mentions a dep but
//! legitimately scores low (a compound-prefix false match) re-scores low once and is
//! then left alone — no churn.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::db::Database;

use super::{match_dependencies, ScoringContext};

/// Cap on items re-queued per epoch change — generous (the candidate set is small:
/// only noise-scored releases/advisories), but bounds a pathological case.
const MAX_REQUEUE: usize = 5000;

/// Stable hash of the developer's dependency set. Changes iff deps are added/removed.
/// `DefaultHasher` is seeded deterministically (not randomised), so the value is stable
/// across runs; a Rust-version change to the algorithm would at worst trigger one extra
/// (harmless) re-examination.
pub(crate) fn dep_epoch_hash(ctx: &ScoringContext) -> u64 {
    let mut names: Vec<&str> = ctx
        .ace_ctx
        .dependency_info
        .keys()
        .map(String::as_str)
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut hasher = DefaultHasher::new();
    names.len().hash(&mut hasher);
    for name in names {
        name.hash(&mut hasher);
    }
    // Mask to 63 bits: the epoch is persisted via scheduler_state, which stores values
    // as a signed i64 and clamps negatives to 0. A full u64 would fail to round-trip
    // (the gate would never close → re-queue every cycle). 63 bits keeps collisions
    // astronomically unlikely while guaranteeing a safe non-negative round-trip.
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

/// Re-queue buried releases/advisories that the current dependency graph may have made
/// relevant. Returns the number of items reset for re-scoring. Pure trigger — the actual
/// re-scoring is done by the backfill worker.
pub(crate) fn requeue_reexaminable_items(
    db: &Database,
    ctx: &ScoringContext,
    threshold: f32,
) -> usize {
    let candidates = db
        .get_reexaminable_candidates(threshold, MAX_REQUEUE)
        .unwrap_or_default();
    if candidates.is_empty() {
        return 0;
    }
    let ids: Vec<i64> = candidates
        .iter()
        .filter(|(_, title, content)| {
            let (matches, _) = match_dependencies(title, content, &[], &ctx.ace_ctx);
            !matches.is_empty()
        })
        .map(|(id, _, _)| *id)
        .collect();
    db.requeue_items_by_ids(&ids).unwrap_or(0)
}

#[cfg(test)]
#[path = "reexamination_tests.rs"]
mod tests;
