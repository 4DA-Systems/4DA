// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Eval runner — feeds fixtures through synthesis and scores output quality.

use super::fixtures::{EvalFixture, EvalSignal, ForbiddenPattern, PatternSeverity};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EvalReport {
    pub fixture_name: String,
    pub passed: bool,
    pub violations: Vec<Violation>,
    pub missing_required: Vec<String>,
    pub synthesis_text: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Violation {
    pub pattern: String,
    pub reason: String,
    pub severity: PatternSeverity,
    pub matched_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EvalSummary {
    pub model_tag: String,
    pub provider: String,
    pub total_fixtures: usize,
    pub passed: usize,
    pub failed: usize,
    pub critical_violations: usize,
    pub reports: Vec<EvalReport>,
    pub verdict: EvalVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum EvalVerdict {
    Pass,
    PassWithWarnings,
    Fail,
}

pub(crate) fn build_briefing_from_fixture(
    fixture: &EvalFixture,
) -> crate::monitoring_briefing::BriefingNotification {
    use crate::monitoring_briefing::{BriefingItem, BriefingNotification};

    let items: Vec<BriefingItem> = fixture
        .signals
        .iter()
        .enumerate()
        .map(|(i, s)| signal_to_briefing_item(s, i))
        .collect();

    let total_relevant = items.len();

    BriefingNotification {
        title: format!("Eval: {}", fixture.name),
        items,
        total_relevant,
        ongoing_topics: vec![],
        knowledge_gaps: vec![],
        escalating_chains: vec![],
        synthesis: None,
        preemption_alerts: vec![],
        blind_spot_score: None,
        labels: None,
        personalization_context: None,
        data_freshness: None,
        corroboration_available: false,
        coverage_building: false,
    }
}

fn signal_to_briefing_item(
    signal: &EvalSignal,
    index: usize,
) -> crate::monitoring_briefing::BriefingItem {
    crate::monitoring_briefing::BriefingItem {
        title: signal.title.clone(),
        source_type: signal.source_type.clone(),
        score: signal.score,
        signal_type: None,
        url: None,
        item_id: Some(index as i64),
        signal_priority: None,
        description: Some(signal.description.clone()),
        matched_deps: vec![],
        content_type: None,
        corroboration_count: 0,
        alt_sources: vec![],
        section: None,
        triage_reason: None,
    }
}

pub(crate) fn check_output(fixture: &EvalFixture, synthesis: &str) -> EvalReport {
    let lower = synthesis.to_lowercase();

    let violations: Vec<Violation> = fixture
        .forbidden_patterns
        .iter()
        .filter_map(|fp| find_violation(fp, synthesis, &lower))
        .collect();

    let missing_required: Vec<String> = fixture
        .required_patterns
        .iter()
        .filter(|rp| !synthesis.contains(**rp))
        .map(|rp| (*rp).to_string())
        .collect();

    let has_critical = violations
        .iter()
        .any(|v| v.severity == PatternSeverity::Critical);
    let passed = !has_critical && missing_required.is_empty();

    EvalReport {
        fixture_name: fixture.name.to_string(),
        passed,
        violations,
        missing_required,
        synthesis_text: synthesis.to_string(),
        duration_ms: 0,
    }
}

fn find_violation(fp: &ForbiddenPattern, original: &str, lower: &str) -> Option<Violation> {
    let pattern_lower = fp.pattern.to_lowercase();
    let pos = lower.find(&pattern_lower)?;
    let end = (pos + fp.pattern.len()).min(original.len());
    let context_start = pos.saturating_sub(20);
    let context_end = (end + 20).min(original.len());
    let matched = &original[context_start..context_end];

    Some(Violation {
        pattern: fp.pattern.to_string(),
        reason: fp.reason.to_string(),
        severity: fp.severity,
        matched_text: matched.to_string(),
    })
}

pub(crate) fn summarize(model_tag: &str, provider: &str, reports: Vec<EvalReport>) -> EvalSummary {
    let total = reports.len();
    let passed = reports.iter().filter(|r| r.passed).count();
    let failed = total - passed;
    let critical = reports
        .iter()
        .flat_map(|r| &r.violations)
        .filter(|v| v.severity == PatternSeverity::Critical)
        .count();

    let verdict = if critical > 0 {
        EvalVerdict::Fail
    } else if failed > 0 {
        EvalVerdict::PassWithWarnings
    } else {
        EvalVerdict::Pass
    };

    EvalSummary {
        model_tag: model_tag.to_string(),
        provider: provider.to_string(),
        total_fixtures: total,
        passed,
        failed,
        critical_violations: critical,
        reports,
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_eval::fixtures::{all_fixtures, PatternSeverity};

    #[test]
    fn clean_synthesis_passes() {
        let fixture = &all_fixtures()[0]; // security_grounding
        let synthesis = "React 19.2 and Node.js security patch dominate this cycle. Update Node.js deployments promptly.";
        let report = check_output(fixture, synthesis);
        assert!(report.passed, "Clean synthesis should pass: {:?}", report);
        assert!(report.violations.is_empty());
        assert!(report.missing_required.is_empty());
    }

    #[test]
    fn hallucinated_cve_fails() {
        let fixture = &all_fixtures()[0];
        let synthesis =
            "CVE-2024-12345 affects Node.js with critical vulnerability in React dependencies.";
        let report = check_output(fixture, synthesis);
        assert!(!report.passed, "Hallucinated CVE should fail");
        assert!(report.violations.iter().any(|v| v.pattern == "CVE-"));
    }

    #[test]
    fn missing_required_term_fails() {
        let fixture = &all_fixtures()[0];
        let synthesis = "JavaScript frameworks continue to evolve with new features.";
        let report = check_output(fixture, synthesis);
        assert!(!report.passed, "Missing required terms should fail");
        assert!(!report.missing_required.is_empty());
    }

    #[test]
    fn summary_counts_correct() {
        let reports = vec![
            EvalReport {
                fixture_name: "a".into(),
                passed: true,
                violations: vec![],
                missing_required: vec![],
                synthesis_text: "ok".into(),
                duration_ms: 100,
            },
            EvalReport {
                fixture_name: "b".into(),
                passed: false,
                violations: vec![Violation {
                    pattern: "CVE-".into(),
                    reason: "hallucinated".into(),
                    severity: PatternSeverity::Critical,
                    matched_text: "CVE-2024-99999".into(),
                }],
                missing_required: vec![],
                synthesis_text: "bad".into(),
                duration_ms: 200,
            },
        ];

        let summary = summarize("qwen3:14b", "ollama", reports);
        assert_eq!(summary.total_fixtures, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.critical_violations, 1);
        assert_eq!(summary.verdict, EvalVerdict::Fail);
    }

    #[test]
    fn all_pass_gives_pass_verdict() {
        let reports = vec![EvalReport {
            fixture_name: "a".into(),
            passed: true,
            violations: vec![],
            missing_required: vec![],
            synthesis_text: "ok".into(),
            duration_ms: 50,
        }];

        let summary = summarize("qwen3:14b", "ollama", reports);
        assert_eq!(summary.verdict, EvalVerdict::Pass);
    }

    #[test]
    fn medium_violation_gives_warnings() {
        let reports = vec![EvalReport {
            fixture_name: "a".into(),
            passed: false,
            violations: vec![Violation {
                pattern: "migration".into(),
                reason: "not mentioned".into(),
                severity: PatternSeverity::Medium,
                matched_text: "migration path".into(),
            }],
            missing_required: vec![],
            synthesis_text: "mentions migration".into(),
            duration_ms: 50,
        }];

        let summary = summarize("qwen3:14b", "ollama", reports);
        assert_eq!(summary.verdict, EvalVerdict::PassWithWarnings);
    }

    #[test]
    fn briefing_from_fixture_has_correct_items() {
        let fixture = &all_fixtures()[0];
        let briefing = build_briefing_from_fixture(fixture);
        assert_eq!(briefing.items.len(), fixture.signals.len());
        assert_eq!(briefing.total_relevant, fixture.signals.len());
        for (item, signal) in briefing.items.iter().zip(&fixture.signals) {
            assert_eq!(item.title, signal.title);
            assert_eq!(item.source_type, signal.source_type);
        }
    }
}
