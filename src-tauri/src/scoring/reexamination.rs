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
//! 1. Hash the developer's PINS into a "dep epoch": every included project's dependency,
//!    version and direct/dev flags. Since v37 a registry release is graded against each
//!    project's pinned version (`release_grade`), so a version bump, a new project that
//!    pins an existing package, or a dependency turning direct all change what a release
//!    means — the old names-only hash saw none of them.
//! 2. When the epoch changes, re-queue (a) every dependency release and (b) the buried
//!    releases and security/breaking advisories a dependency match would now flip, and
//!    withdraw their SCORE-derived verdicts: the drain writes scores, never verdicts, and
//!    the risen sweep only re-admits superseded-version verdicts, so a verdict stamped at
//!    the current version would otherwise outlive the re-score forever (live 2026-09-25:
//!    `crates.io: ed25519-dalek v3.0.0` re-scored 0.892 as a breaking upgrade for a
//!    project whose pin was scanned minutes after its verdict was stamped, and stayed out
//!    of the feed).
//! 3. Converge in the same step: drain the re-queued rows and run the verdict sweeps, so
//!    the feed never loses its dependency releases for a cycle.
//! 4. Casual mentions (discussions) are deliberately NOT re-queued: a dep match doesn't
//!    change their verdict, so re-scoring them is wasted work.
//!
//! Loop-safe: the reset is a one-shot batched update gated on the epoch CHANGING. After
//! re-scoring, items are at the current version again; the epoch is unchanged, so they
//! are not re-selected until the pins change again.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tracing::{info, warn};

use crate::db::Database;

use super::{match_dependencies, ScoringContext};

/// Cap on items re-queued per epoch change — generous (the candidate set is small:
/// dependency releases plus noise-scored releases/advisories), but bounds a
/// pathological case.
const MAX_REQUEUE: usize = 5000;

/// Drain chunk and chunk cap for the in-step convergence.
const CONVERGE_CHUNK: usize = 500;
const CONVERGE_MAX_CHUNKS: usize = 12;

/// Stable hash of the developer's pins: every included project's (path, package,
/// version, direct, dev), sorted. `DefaultHasher` is seeded deterministically, so the
/// value is stable across runs; a Rust-version change to the algorithm would at worst
/// trigger one extra (harmless) re-examination. Masked to 63 bits (kept from the
/// scheduler_state era; see `scheduler_state::persist_dep_epoch_hash`).
pub(crate) fn dependency_pin_epoch(db: &Database) -> u64 {
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    let mut pins: Vec<(String, String, String, bool, bool)> = db
        .all_dependency_pins()
        .unwrap_or_default()
        .into_iter()
        .filter(|(path, ..)| {
            !crate::project_inclusion::is_excluded_from_intelligence(path, &user_excluded)
        })
        .map(|(path, name, version, direct, dev)| {
            (
                path.replace('\\', "/").to_lowercase(),
                name.replace('_', "-").to_lowercase(),
                version.unwrap_or_default(),
                direct,
                dev,
            )
        })
        .collect();
    pins.sort_unstable();
    pins.dedup();
    let mut hasher = DefaultHasher::new();
    pins.len().hash(&mut hasher);
    for pin in &pins {
        pin.hash(&mut hasher);
    }
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

/// Re-queue every dependency release plus the buried releases/advisories the current
/// dependency graph may have made relevant, and withdraw their score-derived verdicts.
/// Returns the number of items reset for re-scoring.
pub(crate) fn requeue_reexaminable_items(
    db: &Database,
    ctx: &ScoringContext,
    threshold: f32,
) -> usize {
    let mut ids: Vec<i64> = db
        .get_reexaminable_candidates(threshold, MAX_REQUEUE)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, title, content)| {
            let (matches, _) = match_dependencies(title, content, &[], &ctx.ace_ctx);
            !matches.is_empty()
        })
        .map(|(id, _, _)| id)
        .collect();
    ids.extend(
        db.dependency_release_item_ids(MAX_REQUEUE)
            .unwrap_or_default(),
    );
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(MAX_REQUEUE);
    db.requeue_and_clear_score_verdicts(&ids).unwrap_or(0)
}

/// The whole trigger, for the engine cycle and the GUI monitor alike: when the pin
/// epoch has changed, re-queue and converge. `None` when the pins are unchanged (the
/// common case — one indexed read plus a hash).
pub(crate) async fn reexamine_if_pins_changed(db: &Database) -> Option<usize> {
    let epoch = dependency_pin_epoch(db);
    if epoch == crate::scheduler_state::get_dep_epoch_hash() {
        return None;
    }
    let ctx = match super::build_scoring_context(db).await {
        Ok(ctx) => ctx,
        Err(e) => {
            warn!(target: "4da::reexamination", error = %e, "Re-examination skipped (context build failed)");
            return None;
        }
    };
    let requeued = requeue_reexaminable_items(db, &ctx, crate::get_relevance_threshold());
    crate::scheduler_state::persist_dep_epoch_hash(epoch);
    if requeued > 0 {
        converge(requeued).await;
    }
    Some(requeued)
}

/// Re-score the re-queued rows and run the verdict sweeps now, so a withdrawn verdict is
/// replaced in this step rather than a cycle later. `drain_stale_version_cycle` runs the
/// verdict reconciliation (risen sweep included) BEFORE each score chunk, so one more
/// reconciliation after the last chunk admits what the last chunk raised.
async fn converge(requeued: usize) {
    let mut rescored = 0usize;
    for _ in 0..CONVERGE_MAX_CHUNKS {
        match crate::analysis_backfill::drain_stale_version_cycle(CONVERGE_CHUNK).await {
            Ok(progress) => {
                rescored += progress.scored_this_cycle;
                if progress.done {
                    break;
                }
            }
            Err(e) => {
                warn!(target: "4da::reexamination", error = %e, "Re-examination drain chunk failed");
                break;
            }
        }
    }
    let _ = crate::analysis_verdicts::reconcile_stale_verdicts_logged().await;
    info!(
        target: "4da::reexamination",
        requeued,
        rescored,
        "Dependency pins changed — re-examined and re-judged dependency releases"
    );
}

#[cfg(test)]
#[path = "reexamination_tests.rs"]
mod tests;
