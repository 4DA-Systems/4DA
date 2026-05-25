// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Quality gate validation for benchmark calibration results.

use tracing::warn;

use super::BenchmarkReport;

pub(super) fn model_meets_quality_gate(report: &BenchmarkReport) -> bool {
    let overall_ok = report.accuracy >= 0.80;

    let tp_ok = report
        .by_category
        .get("true_positive")
        .map_or(false, |c| c.accuracy >= 0.70);

    let tn_ok = report
        .by_category
        .get("true_negative")
        .map_or(false, |c| c.accuracy >= 0.90);

    let sec_ok = report
        .by_category
        .get("security")
        .map_or(false, |c| c.accuracy >= 0.90);

    if !overall_ok {
        warn!(
            "Quality gate: overall accuracy {:.1}% < 80%",
            report.accuracy * 100.0
        );
    }
    if !tp_ok {
        warn!(
            "Quality gate: true_positive accuracy {:.1}% < 70%",
            report
                .by_category
                .get("true_positive")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }
    if !tn_ok {
        warn!(
            "Quality gate: true_negative accuracy {:.1}% < 90%",
            report
                .by_category
                .get("true_negative")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }
    if !sec_ok {
        warn!(
            "Quality gate: security accuracy {:.1}% < 90%",
            report
                .by_category
                .get("security")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }

    overall_ok && tp_ok && tn_ok && sec_ok
}
