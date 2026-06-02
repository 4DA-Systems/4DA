// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Calibration Probes — re-exports from probes_corpus + probes_engine,
//! plus 4-dimension score computation.
//!
//! Split structure:
//!   probes_corpus.rs  — 28 probe items, Domain/ProbeExpected types
//!   probes_engine.rs  — domain detection, probe selection, execution, signal audit
//!   calibration_probes.rs (this file) — dimension scores, grade computation, re-exports

use crate::calibration_commands::RigRequirements;
use crate::scoring::ScoringContext;

// Re-export items used by calibration_commands.rs at runtime
pub(crate) use crate::probes_corpus::domain_name;
pub(crate) use crate::probes_engine::{
    detect_user_domain, run_probe_calibration, select_probes_for_user, ProbeResults,
};

// ============================================================================
// 4-Dimension Score Computation
// ============================================================================

pub(crate) fn compute_infrastructure_score(rig: &RigRequirements) -> u32 {
    let mut score = 0u32;
    // Working semantic embeddings are the dominant infrastructure signal — and they
    // count regardless of provider (built-in fastembed, Ollama, or cloud). Without
    // them scoring falls back to keyword matching. This is provider-agnostic on
    // purpose: the previous logic only credited Ollama, so a user on 4DA's bundled
    // local model scored zero here despite having fully working embeddings.
    if rig.embedding_available {
        score += 18;
    }
    // A real GPU (detected via hardware probe, not a model-count proxy) accelerates
    // local models.
    if rig.gpu_detected {
        score += 4;
    }
    // A local Ollama engine running is a privacy/offline bonus on top of the above.
    if rig.ollama_running {
        score += 3;
    }
    score.min(25)
}

pub(crate) fn compute_context_score(ctx: &ScoringContext) -> u32 {
    let mut score = 0.0_f64;
    // Interests: first 5 at 2.5 each (12.5), interests 6-10 at 0.5 each (diminishing,
    // +2.5 max). Credits power users with many interests instead of silently ignoring
    // everything past the 5th — without letting interest count dominate the dimension.
    let core_interest_pts = (ctx.interest_count.min(5) as f64) * 2.5;
    let extra_interest_pts = (ctx.interest_count.saturating_sub(5).min(5) as f64) * 0.5;
    score += core_interest_pts + extra_interest_pts;
    // Stack profiles active
    if ctx.composed_stack.active {
        score += 5.0;
    }
    // ACE active topics exist
    if !ctx.ace_ctx.active_topics.is_empty() {
        score += 3.0;
    }
    // Feedback: min(count / 2, 4.5)
    let feedback_pts = (ctx.feedback_interaction_count as f64 / 2.0).min(4.5);
    score += feedback_pts;
    (score as u32).min(25)
}

/// 5 points per signal axis that actually contributed to the probe battery.
/// `fired_axes` comes straight from [`ProbeResults`] — measured contribution,
/// never a data-existence proxy.
pub(crate) fn compute_signal_score(fired_axes: &[String]) -> u32 {
    ((fired_axes.len() as u32) * 5).min(25)
}

pub(crate) fn compute_discrimination_score(probes: &ProbeResults) -> u32 {
    // F1 * 15 + separation_gap.clamp(0, 1) * 10
    let f1_pts = probes.f1 * 15.0;
    let sep_pts = probes.separation_gap.clamp(0.0, 1.0) * 10.0;
    ((f1_pts + sep_pts) as u32).min(25)
}

pub(crate) fn compute_grade_from_dimensions(
    infra: u32,
    context: u32,
    signal: u32,
    discrimination: u32,
) -> (String, u32) {
    let score = (infra + context + signal + discrimination).min(100);
    let grade = match score {
        90..=100 => "A",
        80..=89 => "B+",
        70..=79 => "B",
        60..=69 => "C+",
        50..=59 => "C",
        40..=49 => "D",
        _ => "F",
    }
    .to_string();
    (grade, score)
}
