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
    audit_signal_axes, detect_user_domain, run_probe_calibration, ProbeResults, SignalAudit,
};

// ============================================================================
// 4-Dimension Score Computation
// ============================================================================

pub(crate) fn compute_infrastructure_score(rig: &RigRequirements) -> u32 {
    let mut score = 0u32;
    if rig.ollama_running {
        score += 8;
    }
    if rig.embedding_available {
        score += 12;
    }
    if rig.gpu_detected {
        score += 5;
    }
    score.min(25)
}

pub(crate) fn compute_context_score(ctx: &ScoringContext) -> u32 {
    let mut score = 0.0_f64;
    // Interests: min(count, 5) * 2.5
    let interest_pts = (ctx.interest_count.min(5) as f64) * 2.5;
    score += interest_pts;
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

pub(crate) fn compute_signal_score(audit: &SignalAudit) -> u32 {
    let mut score = 0u32;
    if audit.context_fires {
        score += 5;
    }
    if audit.interest_fires {
        score += 5;
    }
    if audit.ace_fires {
        score += 5;
    }
    if audit.learned_fires {
        score += 5;
    }
    if audit.dependency_fires {
        score += 5;
    }
    score.min(25)
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
