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
//! Current achieved state (2026-08-24, snowflake-arctic-embed-m, live run at
//! 62ffbacf + Wave 1-2 audit fixes + the item-22a harness wiring — item ages,
//! registry source_id, and the 7 harness_coverage scenarios now measured):
//! overall 82/85 (96.5%) · TP 17/20 (85%) · TN 20/20 (100%) · security 12/12
//! (100%) · cold_start 12/12 (100%) · edge_case 14/14 (100%) ·
//! harness_coverage 7/7 (100%). Residual failures (root-caused, Phase-2
//! recall-arc scope): tp_systems_programming 0.182 (interest corroboration
//! 0.28 < the 0.35 own-stack bar), reg_tauri_title_boost 0.171 (corroboration
//! 0.15), reg_desktop_packaging 0.078 (1-signal cap + domain token filter).
//!
//! Enforcement: soft-warn locally; HARD-FAIL when FOURDA_REQUIRE_REAL_
//! EMBEDDINGS=1 (the CI real-embedding step) — see full_calibration_with_
//! real_embeddings.
//!
//! ## Cross-machine noise margin (2026-08-25)
//!
//! The achieved state is measured on ONE machine. Hosted CI runners take
//! different CPU/ONNX float paths, shifting embedding cosines by thousandths
//! — and near a confirmation threshold that STEPS a scenario's score by
//! ~0.1 (the signal-gate multiplier jump), not by float dust. Observed live:
//! PR #527 (zero scoring changes) failed this gate on ubuntu-hosted with
//! edge_deprecated_tech at 0.413 (band max 0.30) while THREE byte-identical
//! local runs scored it in-band — run 32734979346. The overall floor
//! therefore sits exactly ONE scenario below the achieved state: a single
//! cross-machine threshold-flip passes, a second failure (or any category-
//! floor breach) is real drift and stays red. Do not "fix" a one-scenario
//! CI failure by widening that scenario's band — the band is the meaning;
//! the tolerance lives here, sized to the measured phenomenon.

use tracing::warn;

use super::BenchmarkReport;

/// Overall score-range floor — achieved 82/85 (96.5%); floor sits ONE
/// scenario below (81/85 = 95.3%) as the cross-machine noise margin (see
/// module doc). Raised from 0.92 by the 2026-08-24 ratchet after the Wave
/// 1-2 recall fixes recovered sec_serde_advisory + reg_multi_stack_title.
const OVERALL_FLOOR: f64 = 0.95;
/// True-positive floor — achieved 17/20, raised from 0.80 (16/20).
const TP_FLOOR: f64 = 0.85;
/// True-negative floor — 100% is the precision-first hard gate (held since
/// 2026-06; a single false positive here is a doctrine violation, not drift).
const TN_FLOOR: f64 = 1.00;
/// Security floor — achieved 12/12, raised from 0.91 (11/12): the family/
/// sub-crate map recovered sec_serde_advisory (0.414 → 0.544). A security
/// scenario regression is now categorically red.
const SEC_FLOOR: f64 = 1.00;
/// Cold-start floor — achieved 12/12, raised from 0.91 (11/12) after
/// cold_single_interest_match's synthetic-era 0.60 ceiling was re-derived to
/// 0.635 (ratcheted BELOW the audit-era 0.639 measurement — see the scenario's
/// notes). Cold-start is a non-negotiable product surface (doctrine rule 6).
const COLD_FLOOR: f64 = 1.00;
/// Harness-coverage floor — the 7 scenarios that exercise the paths the
/// 2026-08-23 audit found structurally untested (UGC caps from age 0,
/// engagement escape hatch, v18 registry grounding via source_id, stale
/// published_at discount, post-bootstrap dep-release surfacing). All banded
/// from measurement; a regression here reopens a closed audit blind spot.
const HARNESS_FLOOR: f64 = 1.00;
/// Edge-case floor — achieved 14/14; floored at 13/14 (92.8%) because the
/// one near-threshold scenario in this category (edge_deprecated_tech) is
/// exactly the cross-machine flip the module doc describes. Was previously
/// the only category with NO floor of its own; a second edge regression is
/// red both here and via the overall floor.
const EDGE_FLOOR: f64 = 0.92;

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
    // cold_start / harness_coverage may be absent in reduced fixture reports
    // (unit tests build partial category maps); only gate them when they ran.
    let cold_ok = report
        .by_category
        .get("cold_start")
        .is_none_or(|c| c.accuracy as f64 >= COLD_FLOOR);
    let harness_ok = report
        .by_category
        .get("harness_coverage")
        .is_none_or(|c| c.accuracy as f64 >= HARNESS_FLOOR);
    let edge_ok = report
        .by_category
        .get("edge_case")
        .is_none_or(|c| c.accuracy as f64 >= EDGE_FLOOR);

    let check = |ok: bool, name: &str, actual: f64, floor: f64| {
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
    check(
        harness_ok,
        "harness_coverage",
        category("harness_coverage"),
        HARNESS_FLOOR,
    );
    check(edge_ok, "edge_case", category("edge_case"), EDGE_FLOOR);

    overall_ok && tp_ok && tn_ok && sec_ok && cold_ok && harness_ok && edge_ok
}
