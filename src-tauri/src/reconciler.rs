//! Intelligence Mesh — Reconciler (Layer 3).
//!
//! The reconciler combines the authoritative pipeline score with zero or
//! more advisor signals into a final rank, a confidence, and a possible
//! disagreement flag. It REPLACES the pre-mesh 50/50 blend at
//! `analysis_rerank.rs:~261` that gave advisors equal weight with the
//! deterministic pipeline.
//!
//! ## Load-bearing rules
//!
//!   1. **Pipeline is authoritative.** Advisors can adjust the final rank
//!      by at most `ADVISOR_ADJUSTMENT_CAP` (±0.15). The pipeline score
//!      plus bounded adjustment is the final rank.
//!
//!   2. **Disagreement is a signal, never an override.** When pipeline
//!      and an advisor's normalized score differ by more than
//!      `DISAGREEMENT_THRESHOLD` (0.30), we emit a `DisagreementKind`
//!      for the UI to render as "judges split." The final rank is
//!      still the bounded-adjustment result — the flag is informative.
//!
//!   3. **No hard rejects.** An advisor cannot zero out an item the
//!      pipeline rated relevant. The worst an advisor can do is push the
//!      rank down by the cap. Items the pipeline thinks are relevant
//!      stay surfaced; the user (and feedback loop) decide.
//!
//!   4. **Multiple advisors sum, then clamp.** In the multi-advisor
//!      ensemble case (reserved for Phase 9 post-launch), individual
//!      (advisor_normalized - pipeline_score) deltas are summed first,
//!      then clamped to the adjustment cap. This naturally dampens
//!      extreme single-advisor opinions when others moderate them.
//!
//! See `docs/strategy/INTELLIGENCE-MESH.md` §2 Layer 3 for the full
//! design and `§5.2 Phase 2` for the migration plan.

use crate::types::{AdvisorSignal, DisagreementKind};

/// Maximum total adjustment advisors can make to the pipeline score.
/// A single advisor's |normalized - pipeline| can exceed this, but the
/// FINAL adjustment applied is clamped here. This is the single most
/// load-bearing constant in the mesh — loosen only with awe_transmute.
pub const ADVISOR_ADJUSTMENT_CAP: f32 = 0.15;

/// |pipeline - advisor_normalized| above this flags a disagreement for
/// UI surfacing ("judges split"). Deliberately lower than the cap so
/// disagreements are visible without being acted on.
pub const DISAGREEMENT_THRESHOLD: f32 = 0.30;

/// Output of reconciling pipeline + advisors.
///
/// `pipeline_score` and `applied_adjustment` are currently unread by the
/// callers wired in Phase 2 (the rerank path only needs `final_rank` and
/// `disagreement`). They are kept on the struct because the Phase 7
/// receipts UI ("Why this score?") will render the adjustment explicitly,
/// and the tests exercise them as a regression guard for the math.
#[derive(Debug, Clone)]
#[allow(dead_code)] // pipeline_score + applied_adjustment consumed by Phase 7 receipts UI
pub struct Reconciled {
    /// The final rank. `pipeline_score + clamped_adjustment`, in [0.0, 1.0].
    pub final_rank: f32,
    /// `pipeline_score` as provided (for caller's explanation code).
    pub pipeline_score: f32,
    /// The adjustment actually applied (post-clamp). Negative means
    /// advisors pushed the rank down; positive means up.
    pub applied_adjustment: f32,
    /// `Some(kind)` when pipeline and advisor(s) disagreed by more than
    /// the threshold; `None` when they agreed or no advisor spoke.
    pub disagreement: Option<DisagreementKind>,
}

/// Reconcile a pipeline score with advisor signals.
///
/// Produces a final rank bounded by the adjustment cap and, when
/// applicable, a disagreement flag. This is the single function that
/// should combine pipeline and advisor output across the codebase — do
/// not add ad-hoc blending anywhere else.
pub fn reconcile(pipeline_score: f32, advisors: &[AdvisorSignal]) -> Reconciled {
    if advisors.is_empty() {
        return Reconciled {
            final_rank: pipeline_score.clamp(0.0, 1.0),
            pipeline_score,
            applied_adjustment: 0.0,
            disagreement: None,
        };
    }

    // Summed delta across all advisors: each contributes (normalized - pipeline).
    // This is NOT an average — it lets a single confident advisor make the full
    // ±cap adjustment, while multiple disagreeing advisors cancel.
    let raw_adjustment: f32 = advisors
        .iter()
        .map(|a| a.normalized_score - pipeline_score)
        .sum();
    let applied_adjustment = raw_adjustment.clamp(-ADVISOR_ADJUSTMENT_CAP, ADVISOR_ADJUSTMENT_CAP);
    let final_rank = (pipeline_score + applied_adjustment).clamp(0.0, 1.0);

    // Disagreement: flag the largest single-advisor deviation that exceeds
    // the threshold. For the Phase 2 single-advisor case this is just the
    // one advisor; for multi-advisor ensembles (Phase 9+) we also consider
    // advisor-vs-advisor disagreement, detected here as the spread across
    // advisors' normalized scores.
    let disagreement = detect_disagreement(pipeline_score, advisors);

    Reconciled {
        final_rank,
        pipeline_score,
        applied_adjustment,
        disagreement,
    }
}

fn detect_disagreement(
    pipeline_score: f32,
    advisors: &[AdvisorSignal],
) -> Option<DisagreementKind> {
    // Largest |pipeline - advisor| pair.
    let (max_abs, signed) = advisors
        .iter()
        .map(|a| a.normalized_score - pipeline_score)
        .fold((0.0_f32, 0.0_f32), |(max_abs, signed), d| {
            if d.abs() > max_abs {
                (d.abs(), d)
            } else {
                (max_abs, signed)
            }
        });

    if max_abs > DISAGREEMENT_THRESHOLD {
        return Some(if signed < 0.0 {
            DisagreementKind::AdvisorSkeptical
        } else {
            DisagreementKind::AdvisorEnthusiastic
        });
    }

    // Advisor-vs-advisor spread (reserved: only fires with >=2 advisors).
    if advisors.len() >= 2 {
        let max = advisors
            .iter()
            .map(|a| a.normalized_score)
            .fold(f32::MIN, f32::max);
        let min = advisors
            .iter()
            .map(|a| a.normalized_score)
            .fold(f32::MAX, f32::min);
        if (max - min) > DISAGREEMENT_THRESHOLD {
            return Some(DisagreementKind::AdvisorsInternal);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny helper to build an AdvisorSignal with only the fields the
    /// reconciler reads. Keeps test setup noise low.
    fn advisor(normalized: f32) -> AdvisorSignal {
        AdvisorSignal {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            task: "judge".to_string(),
            raw_score: normalized,
            normalized_score: normalized,
            confidence: normalized,
            reason: None,
            prompt_version: None,
            calibration_id: None,
        }
    }

    #[test]
    fn no_advisors_returns_pipeline_score_unchanged() {
        let r = reconcile(0.73, &[]);
        assert!((r.final_rank - 0.73).abs() < 1e-5);
        assert!((r.applied_adjustment - 0.0).abs() < 1e-5);
        assert!(r.disagreement.is_none());
    }

    #[test]
    fn small_agreement_applies_full_delta_within_cap() {
        // Pipeline 0.70, advisor 0.75 → delta +0.05, within cap → final 0.75.
        let r = reconcile(0.70, &[advisor(0.75)]);
        assert!((r.final_rank - 0.75).abs() < 1e-5);
        assert!((r.applied_adjustment - 0.05).abs() < 1e-5);
        assert!(r.disagreement.is_none());
    }

    #[test]
    fn positive_delta_beyond_cap_is_clamped_upward() {
        // Pipeline 0.50, advisor 0.95 → delta +0.45, clamped to +0.15 → final 0.65.
        // Disagreement fires because |delta| > 0.30.
        let r = reconcile(0.50, &[advisor(0.95)]);
        assert!((r.final_rank - 0.65).abs() < 1e-5);
        assert!((r.applied_adjustment - 0.15).abs() < 1e-5);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorEnthusiastic));
    }

    #[test]
    fn negative_delta_beyond_cap_is_clamped_downward() {
        // Pipeline 0.80, advisor 0.20 → delta -0.60, clamped to -0.15 → final 0.65.
        // Disagreement fires (advisor skeptical).
        let r = reconcile(0.80, &[advisor(0.20)]);
        assert!((r.final_rank - 0.65).abs() < 1e-5);
        assert!((r.applied_adjustment - (-0.15)).abs() < 1e-5);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorSkeptical));
    }

    #[test]
    fn pipeline_cannot_go_above_one() {
        // Pipeline 0.95, advisor 1.0 → delta +0.05, final would be 1.00.
        let r = reconcile(0.95, &[advisor(1.0)]);
        assert!((r.final_rank - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pipeline_cannot_go_below_zero() {
        // Pipeline 0.05, advisor 0.0 → delta -0.05, final 0.0.
        let r = reconcile(0.05, &[advisor(0.0)]);
        assert!((r.final_rank - 0.0).abs() < 1e-5);
    }

    #[test]
    fn advisor_cannot_hard_reject_pipeline_relevant_items() {
        // A core invariant: no matter how skeptical an advisor, a
        // pipeline-confident item never drops below pipeline - cap.
        let r = reconcile(0.85, &[advisor(0.0)]);
        assert!(r.final_rank >= 0.85 - ADVISOR_ADJUSTMENT_CAP - 1e-5);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorSkeptical));
    }

    #[test]
    fn advisor_cannot_rescue_low_pipeline_items_past_cap() {
        // The dual invariant: an advisor's enthusiasm can't lift an
        // item above pipeline + cap.
        let r = reconcile(0.10, &[advisor(1.0)]);
        assert!(r.final_rank <= 0.10 + ADVISOR_ADJUSTMENT_CAP + 1e-5);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorEnthusiastic));
    }

    #[test]
    fn two_advisors_agreeing_sum_deltas_then_clamp() {
        // Two advisors both slightly above pipeline: 0.70 + 0.05 + 0.05 = 0.80,
        // within cap (0.10 ≤ 0.15) so no clamp, no disagreement.
        let r = reconcile(0.70, &[advisor(0.75), advisor(0.75)]);
        assert!((r.final_rank - 0.80).abs() < 1e-5);
        assert_eq!(r.disagreement, None);
    }

    #[test]
    fn two_advisors_disagreeing_cancel_each_other() {
        // Advisor A wants +0.10, advisor B wants -0.10 → net 0.
        let r = reconcile(0.50, &[advisor(0.60), advisor(0.40)]);
        assert!((r.final_rank - 0.50).abs() < 1e-5);
        assert_eq!(r.disagreement, None);
    }

    #[test]
    fn two_advisors_spread_flags_internal_disagreement() {
        // Both advisors are close-ish to pipeline individually (neither
        // exceeds DISAGREEMENT_THRESHOLD from pipeline), but they
        // disagree strongly with each other.
        // Pipeline 0.55, advisor A 0.80, advisor B 0.30.
        // A-pipeline = +0.25 (under 0.30). B-pipeline = -0.25 (under 0.30).
        // A-B spread = 0.50 (exceeds 0.30) → AdvisorsInternal.
        let r = reconcile(0.55, &[advisor(0.80), advisor(0.30)]);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorsInternal));
    }

    #[test]
    fn disagreement_threshold_boundary_exclusive() {
        // |delta| == DISAGREEMENT_THRESHOLD (0.30) should NOT flag.
        // Pipeline 0.50, advisor 0.80 → exactly 0.30.
        let r = reconcile(0.50, &[advisor(0.80)]);
        assert_eq!(r.disagreement, None);
    }

    #[test]
    fn disagreement_threshold_just_above_flags() {
        let r = reconcile(0.50, &[advisor(0.81)]);
        assert_eq!(r.disagreement, Some(DisagreementKind::AdvisorEnthusiastic));
    }

    #[test]
    fn adjustment_cap_constant_matches_design_doc() {
        // Guard: if this constant changes, the design doc must be updated
        // in lockstep. Bumping the cap loosens advisor power and is a
        // high-stakes architectural decision — route through awe_transmute.
        assert!((ADVISOR_ADJUSTMENT_CAP - 0.15).abs() < 1e-6);
    }

    #[test]
    fn disagreement_threshold_constant_matches_design_doc() {
        assert!((DISAGREEMENT_THRESHOLD - 0.30).abs() < 1e-6);
    }
}
