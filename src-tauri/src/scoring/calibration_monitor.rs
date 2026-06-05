// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Continuous per-developer calibration monitor — Phase 5 of the scoring relevance funnel.
//!
//! The strategic goal is scoring that is immaculate for EVERY developer, not just the
//! founder. A fixed probe set (the existing 28-probe calibration) cannot represent the
//! real diversity of developers — a mobile dev, an ML researcher, an embedded engineer,
//! a security analyst all have different relevance universes. The only thing that is
//! sharp for everyone is a system that calibrates against each developer's OWN stack and
//! behavior, continuously, on their own machine.
//!
//! This module computes a `CalibrationSnapshot` from the developer's real feedback:
//!
//! * **precision_miss_rate** — items the developer DISMISSED (feedback.relevant = 0) that
//!   the scorer rated relevant (≥ threshold). These are false positives we surfaced.
//! * **recall_miss_rate** — items the developer ENGAGED with (feedback.relevant = 1) that
//!   the scorer rated as noise (< threshold). These are false negatives we'd have buried —
//!   the most important failure for an intelligence product.
//! * **discrimination** — avg score of engaged items minus avg score of dismissed items.
//!   Large and positive means the scorer cleanly separates what this developer wants from
//!   what they don't.
//!
//! These three signals are inherently per-developer (they come from the developer's own
//! engagement) and need nothing global. Cold-start aware (intelligence-doctrine rule 6):
//! `has_sufficient_feedback` is false and `health()` is `None` until enough feedback
//! accrues — the system stays honestly silent rather than fabricating a calibration grade.
//!
//! NOTE (deliberately NOT here): a "high-stakes scored as noise" structural signal is
//! valuable at cold start, but it is only meaningful when scoped to the developer's
//! dependency graph — a CVE in a package they don't use SHOULD score low. Live data
//! confirmed an unscoped version flags ~86% of advisories (general security volume, not
//! recall bugs). Scoping requires the live ACE dep-graph + `match_dependencies`, so that
//! check belongs in the scheduled job (Phase 5b), not this pure-SQL analyzer.
//!
//! Side-effect free: pure reads, no scoring or storage mutation.

use rusqlite::Result as SqliteResult;

use crate::db::Database;

use super::{match_dependencies, ScoringContext};

/// Minimum feedback events before the engagement-derived metrics are trustworthy.
/// Below this the rates are reported but `has_sufficient_feedback` is false.
const MIN_FEEDBACK_FOR_METRICS: i64 = 10;

/// A point-in-time, per-developer calibration reading.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct CalibrationSnapshot {
    pub relevance_threshold: f32,

    // ── Engagement-derived (this developer's feedback) ──
    pub feedback_count: i64,
    pub has_sufficient_feedback: bool,
    /// Dismissed items that scored ≥ threshold ÷ all scored dismissed items. Lower is better.
    pub precision_miss_rate: f32,
    /// Engaged items that scored < threshold ÷ all scored engaged items. Lower is better.
    pub recall_miss_rate: f32,
    /// avg score(engaged) − avg score(dismissed). Larger positive is better.
    pub discrimination: f32,
}

impl CalibrationSnapshot {
    /// A 0..1 calibration-health score for drift detection / observability, or `None`
    /// when there isn't enough feedback to assess it honestly (cold start). Penalises
    /// recall misses at double weight — false negatives (relevant items buried) are the
    /// worst failure for an intelligence product. 1.0 = clean separation.
    pub fn health(&self) -> Option<f32> {
        if !self.has_sufficient_feedback {
            return None;
        }
        let score = 1.0 - self.precision_miss_rate * 0.25 - self.recall_miss_rate * 0.50;
        Some(score.clamp(0.0, 1.0))
    }
}

/// Compute the per-developer calibration snapshot from the live DB. Pure reads.
pub(crate) fn compute_calibration_snapshot(
    db: &Database,
    threshold: f32,
) -> SqliteResult<CalibrationSnapshot> {
    let conn = db.conn.lock();
    let t = threshold as f64;

    // ── Engagement-derived metrics (join feedback → source_items) ──
    // Engaged = feedback.relevant = 1; dismissed = feedback.relevant = 0. Only items that
    // were actually scored (relevance_score IS NOT NULL) count toward the rates.
    let (engaged_total, engaged_low, engaged_avg): (i64, i64, f64) = conn.query_row(
        "SELECT
             COUNT(*),
             COALESCE(SUM(CASE WHEN si.relevance_score < ?1 THEN 1 ELSE 0 END), 0),
             COALESCE(AVG(si.relevance_score), 0.0)
         FROM feedback f
         JOIN source_items si ON si.id = f.source_item_id
         WHERE f.relevant = 1 AND si.relevance_score IS NOT NULL",
        [t],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let (dismissed_total, dismissed_high, dismissed_avg): (i64, i64, f64) = conn.query_row(
        "SELECT
             COUNT(*),
             COALESCE(SUM(CASE WHEN si.relevance_score >= ?1 THEN 1 ELSE 0 END), 0),
             COALESCE(AVG(si.relevance_score), 0.0)
         FROM feedback f
         JOIN source_items si ON si.id = f.source_item_id
         WHERE f.relevant = 0 AND si.relevance_score IS NOT NULL",
        [t],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let feedback_count = engaged_total + dismissed_total;
    let recall_miss_rate = ratio(engaged_low, engaged_total);
    let precision_miss_rate = ratio(dismissed_high, dismissed_total);
    let discrimination = (engaged_avg - dismissed_avg) as f32;

    Ok(CalibrationSnapshot {
        relevance_threshold: threshold,
        feedback_count,
        has_sufficient_feedback: feedback_count >= MIN_FEEDBACK_FOR_METRICS,
        precision_miss_rate,
        recall_miss_rate,
        discrimination,
    })
}

#[inline]
fn ratio(num: i64, denom: i64) -> f32 {
    if denom <= 0 {
        0.0
    } else {
        num as f32 / denom as f32
    }
}

/// Dep-scoped high-stakes recall (Phase 5b) — the cold-start-capable structural signal
/// that needs the live dependency graph (so it lives here, not in the pure-SQL snapshot).
///
/// A security/breaking advisory that affects the developer's OWN stack should never score
/// as noise; if it does, that's a concrete recall bug. The dep-graph scoping is what makes
/// this honest: an unscoped "high-stakes scored low" count flagged ~86% on live data
/// (a CVE in a package you don't use SHOULD score low). Here the denominator is ONLY the
/// high-stakes items that match a current dependency.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct HighStakesRecall {
    /// High-stakes (security/breaking/CVE) items that match a current dependency.
    pub dep_matched_total: usize,
    /// ...of those, how many scored below threshold (a buried advisory for your stack).
    pub misscored: usize,
    /// `misscored / dep_matched_total`. Should be ~0; anything above is a real recall bug.
    pub miss_rate: f32,
}

/// Compute dep-scoped high-stakes recall from the live corpus + dependency graph.
/// Bounded scan (most-recent high-stakes items); read-only.
pub(crate) fn compute_high_stakes_recall(
    db: &Database,
    ctx: &ScoringContext,
    threshold: f32,
) -> SqliteResult<HighStakesRecall> {
    let items = db.get_scored_high_stakes_items(2000)?;
    let t = threshold as f64;
    let mut dep_matched_total = 0usize;
    let mut misscored = 0usize;
    for (_, title, content, relevance) in &items {
        let (matches, _) = match_dependencies(title, content, &[], &ctx.ace_ctx);
        if !matches.is_empty() {
            dep_matched_total += 1;
            if *relevance < t {
                misscored += 1;
            }
        }
    }
    let miss_rate = if dep_matched_total > 0 {
        misscored as f32 / dep_matched_total as f32
    } else {
        0.0
    };
    Ok(HighStakesRecall {
        dep_matched_total,
        misscored,
        miss_rate,
    })
}

#[cfg(test)]
#[path = "calibration_monitor_tests.rs"]
mod tests;
