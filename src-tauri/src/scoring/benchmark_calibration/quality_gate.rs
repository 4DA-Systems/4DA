// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Quality gate validation for benchmark calibration results.
//!
//! ## Ratchet semantics (2026-08-22)
//!
//! These floors are a RATCHET locked to the best measured state, not generous
//! minimums. The original thresholds (overall 80 / TP 70 / TN 90 / sec 90)
//! sat far below what the pipeline actually achieved, so cumulative drift
//! passed silently: between v7 (2026-06, score-range 98.7%, TP 95%, sec 100%)
//! and v21 (2026-08-21, 92.3% / 80% / 92%) FIVE new under-scoring failures
//! accumulated — each individual change was "gated", the erosion never was.
//!
//! Contract: the floors sit AT the currently-achieved state. Any change that
//! regresses a category fails the gate and must either be fixed or must
//! consciously lower the floor in the same PR — silent drift is the one thing
//! this gate exists to prevent. After a genuine improvement, raise the floor
//! to the new state in the same PR.
//!
//! Current achieved state (2026-08-22, snowflake-arctic-embed-m, live run):
//! overall 92.3% · TP 16/20 (80%) · TN 20/20 (100%) · security 11/12 (91.7%)
//! · cold_start 11/12 (91.7%) · edge_case 14/14 (100%).

use tracing::warn;

use super::BenchmarkReport;

/// Overall score-range floor — achieved 92.3%.
const OVERALL_FLOOR: f64 = 0.92;
/// True-positive floor — achieved 16/20.
const TP_FLOOR: f64 = 0.80;
/// True-negative floor — 100% is the precision-first hard gate (held since
/// 2026-06; a single false positive here is a doctrine violation, not drift).
const TN_FLOOR: f64 = 1.00;
/// Security floor — achieved 11/12.
const SEC_FLOOR: f64 = 0.91;
/// Cold-start floor — achieved 11/12. Previously ungated: cold-start is a
/// non-negotiable product surface (intelligence-doctrine rule 6).
const COLD_FLOOR: f64 = 0.91;

pub(super) fn model_meets_quality_gate(report: &BenchmarkReport) -> bool {
    let category = |name: &str| -> f64 {
        report
            .by_category
            .get(name)
            .map_or(0.0, |c| c.accuracy as f64)
    };

    let overall_ok = report.accuracy as f64 >= OVERALL_FLOOR;
    let tp_ok = category("true_positive") >= TP_FLOOR;
    let tn_ok = category("true_negative") >= TN_FLOOR;
    let sec_ok = category("security") >= SEC_FLOOR;
    // cold_start may be absent in reduced fixture reports (unit tests build
    // partial category maps); only gate it when the category ran.
    let cold_ok = report
        .by_category
        .get("cold_start")
        .is_none_or(|c| c.accuracy as f64 >= COLD_FLOOR);

    let mut check = |ok: bool, name: &str, actual: f64, floor: f64| {
        if !ok {
            warn!(
                "Quality gate RATCHET: {} accuracy {:.1}% < locked floor {:.1}% — \
                 fix the regression or consciously lower the floor in the same PR",
                name,
                actual * 100.0,
                floor * 100.0
            );
        }
    };
    check(overall_ok, "overall", report.accuracy as f64, OVERALL_FLOOR);
    check(tp_ok, "true_positive", category("true_positive"), TP_FLOOR);
    check(tn_ok, "true_negative", category("true_negative"), TN_FLOOR);
    check(sec_ok, "security", category("security"), SEC_FLOOR);
    check(cold_ok, "cold_start", category("cold_start"), COLD_FLOOR);

    overall_ok && tp_ok && tn_ok && sec_ok && cold_ok
}
