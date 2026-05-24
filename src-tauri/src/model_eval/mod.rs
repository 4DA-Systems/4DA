// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Model evaluation harness — tests local models against curated fixtures
//! to detect hallucination, version fabrication, and false security claims.

pub(crate) mod fixtures;
pub(crate) mod runner;

use runner::{EvalReport, EvalSummary};
use tracing::info;

pub(crate) async fn run_eval(model_tag: &str, provider: &str) -> Result<EvalSummary, String> {
    let all = fixtures::all_fixtures();
    let mut reports: Vec<EvalReport> = Vec::with_capacity(all.len());

    for fixture in &all {
        let briefing = runner::build_briefing_from_fixture(fixture);
        let start = std::time::Instant::now();

        let synthesis_result =
            crate::monitoring_briefing::synthesize_morning_briefing(&briefing).await;

        let elapsed = start.elapsed().as_millis() as u64;

        match synthesis_result {
            Ok(result) => {
                let mut report = runner::check_output(fixture, &result.prose);
                report.duration_ms = elapsed;
                info!(
                    target: "4da::model_eval",
                    fixture = fixture.name,
                    passed = report.passed,
                    violations = report.violations.len(),
                    duration_ms = elapsed,
                    "Fixture evaluated"
                );
                reports.push(report);
            }
            Err(e) => {
                info!(
                    target: "4da::model_eval",
                    fixture = fixture.name,
                    error = %e,
                    "Fixture synthesis failed"
                );
                reports.push(EvalReport {
                    fixture_name: fixture.name.to_string(),
                    passed: false,
                    violations: vec![],
                    missing_required: vec!["(synthesis failed)".to_string()],
                    synthesis_text: format!("ERROR: {e}"),
                    duration_ms: elapsed,
                });
            }
        }
    }

    let summary = runner::summarize(model_tag, provider, reports);

    info!(
        target: "4da::model_eval",
        model = model_tag,
        provider = provider,
        passed = summary.passed,
        failed = summary.failed,
        critical = summary.critical_violations,
        verdict = ?summary.verdict,
        "Eval complete"
    );

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_to_briefing_round_trip() {
        let fixtures = fixtures::all_fixtures();
        for f in &fixtures {
            let briefing = runner::build_briefing_from_fixture(f);
            assert_eq!(briefing.items.len(), f.signals.len());
        }
    }
}
