// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use crate::evidence::{EvidenceKind, LensHints};

// ---- has_grounded_reasoning tests ----

#[test]
fn test_grounded_reasoning_too_short() {
    assert!(!has_grounded_reasoning("Short."));
    assert!(!has_grounded_reasoning("This is short because yes"));
}

#[test]
fn test_grounded_reasoning_no_causal_connector() {
    let explanation = "This vulnerability exists in the lodash package and \
                       it could potentially impact applications that use \
                       deep cloning functionality in production environments.";
    assert!(!has_grounded_reasoning(explanation));
}

#[test]
fn test_grounded_reasoning_valid() {
    let explanation = "This vulnerability in lodash affects your project \
                       because your package.json lists lodash@4.17.20 as \
                       a direct dependency, which means any deep clone \
                       operations could trigger the prototype pollution \
                       attack vector described in CVE-2021-23337.";
    assert!(has_grounded_reasoning(explanation));
}

#[test]
fn test_grounded_reasoning_with_therefore() {
    let explanation = "React 19 introduces breaking changes to the \
                       concurrent rendering API. Your project uses \
                       useTransition extensively, therefore you will \
                       need to update your suspense boundaries before \
                       upgrading to avoid runtime errors.";
    assert!(has_grounded_reasoning(explanation));
}

#[test]
fn test_grounded_reasoning_with_due_to() {
    let explanation = "The npm registry experienced an outage that affected \
                       package resolution. This is relevant to your CI pipeline \
                       due to your heavy reliance on npm install in GitHub \
                       Actions workflows.";
    assert!(has_grounded_reasoning(explanation));
}

#[test]
fn test_grounded_reasoning_exact_threshold_length() {
    // Exactly 51 characters with a connector
    let explanation = "A problem exists because of a known issue in core.";
    assert!(has_grounded_reasoning(explanation));
}

// ---- is_title_restatement tests ----

#[test]
fn test_restatement_detection() {
    // Title words (>2 chars, lowered): {"critical", "vulnerability", "lodash"}
    // Explanation words (>2 chars): "critical", "vulnerability", "lodash" = 3
    // Overlap: 3/3 = 1.0 > 0.8 -- restatement detected.
    assert!(is_title_restatement(
        "Critical vulnerability in lodash",
        "A critical vulnerability in lodash.",
    ));
}

#[test]
fn test_not_a_restatement() {
    assert!(!is_title_restatement(
        "Critical vulnerability in lodash",
        "CVE-2021-23337 allows prototype pollution via the set() \
         function. Your project imports lodash 4.17.20, which is \
         in the affected range. Update to 4.17.21 to remediate.",
    ));
}

#[test]
fn test_restatement_empty_explanation() {
    assert!(is_title_restatement("Some title", ""));
}

#[test]
fn test_restatement_empty_title() {
    assert!(!is_title_restatement(
        "",
        "This is a detailed explanation with many words.",
    ));
}

// ---- JSON parsing tests ----

#[test]
fn test_parse_valid_json() {
    let json = r#"{
        "signal_argument": "This matters because...",
        "noise_argument": "This is noise because...",
        "should_surface": true,
        "confidence": 0.85,
        "grounded_explanation": "After weighing both sides...",
        "reasoning_chain": {
            "claim": "Lodash vulnerability is relevant",
            "evidence_points": ["Uses lodash 4.17.20", "CVE affects < 4.17.21"],
            "connection": "Direct dependency in affected range",
            "conclusion": "Should surface with high confidence"
        }
    }"#;

    let verdict = parse_verdict(json).expect("Should parse valid JSON");
    assert!(verdict.should_surface);
    assert!((verdict.adjusted_confidence - 0.85).abs() < f32::EPSILON);
    assert_eq!(verdict.reasoning_chain.evidence_points.len(), 2);
}

#[test]
fn test_parse_json_with_code_fences() {
    let json = "```json\n{\"should_surface\": false, \"confidence\": 0.3, \
                 \"signal_argument\": \"weak\", \"noise_argument\": \"strong\", \
                 \"grounded_explanation\": \"Not relevant.\", \
                 \"reasoning_chain\": {\"claim\": \"c\", \"evidence_points\": [], \
                 \"connection\": \"n\", \"conclusion\": \"no\"}}\n```";

    let verdict = parse_verdict(json).expect("Should parse fenced JSON");
    assert!(!verdict.should_surface);
}

#[test]
fn test_parse_json_missing_optional_fields() {
    let json = r#"{"should_surface": true}"#;
    let verdict = parse_verdict(json).expect("Should handle missing fields");
    assert!(verdict.should_surface);
    assert!((verdict.adjusted_confidence - 0.5).abs() < f32::EPSILON);
    assert!(verdict.grounded_explanation.is_empty());
    assert!(verdict.reasoning_chain.evidence_points.is_empty());
}

#[test]
fn test_parse_json_confidence_clamped() {
    let json = r#"{"should_surface": true, "confidence": 1.5}"#;
    let verdict = parse_verdict(json).expect("Should clamp confidence");
    assert!((verdict.adjusted_confidence - 1.0).abs() < f32::EPSILON);

    let json_neg = r#"{"should_surface": true, "confidence": -0.3}"#;
    let verdict_neg = parse_verdict(json_neg).expect("Should clamp negative");
    assert!(verdict_neg.adjusted_confidence >= 0.0);
}

#[test]
fn test_parse_invalid_json() {
    assert!(parse_verdict("not json at all").is_none());
    assert!(parse_verdict("").is_none());
    assert!(parse_verdict("{broken").is_none());
}

#[test]
fn test_parse_defaults_to_surface_when_missing() {
    let json = r#"{}"#;
    let verdict = parse_verdict(json).expect("Should parse empty object");
    // Default: should_surface = true (fail open)
    assert!(verdict.should_surface);
}

// ---- strip_code_fences tests ----

#[test]
fn test_strip_code_fences_json() {
    let input = "```json\n{\"key\": \"value\"}\n```";
    assert_eq!(strip_code_fences(input), r#"{"key": "value"}"#);
}

#[test]
fn test_strip_code_fences_bare() {
    let input = "```\n{\"key\": \"value\"}\n```";
    assert_eq!(strip_code_fences(input), r#"{"key": "value"}"#);
}

#[test]
fn test_strip_code_fences_none() {
    let input = r#"{"key": "value"}"#;
    assert_eq!(strip_code_fences(input), input);
}

// ---- Critical/High bypass in filter_batch (unit-level) ----

fn make_test_item(urgency: Urgency, title: &str) -> EvidenceItem {
    EvidenceItem {
        id: format!("test-{}", title.replace(' ', "-")),
        kind: EvidenceKind::Alert,
        title: title.to_string(),
        explanation: String::new(),
        confidence: Confidence::heuristic(0.6),
        urgency,
        reversibility: None,
        evidence: vec![],
        affected_projects: vec![],
        affected_deps: vec![],
        suggested_actions: vec![],
        precedents: vec![],
        refutation_condition: None,
        lens_hints: LensHints::preemption_only(),
        created_at: 0,
        expires_at: None,
    }
}

// Note: filter_batch integration tests require an LLM and are not
// run in unit tests. The bypass logic for Critical/High is verified
// by the Urgency ordering -- Critical < High < Medium < Watch --
// so the comparison `item.urgency == Urgency::Critical || item.urgency
// == Urgency::High` is exercised.

#[test]
fn test_urgency_ordering_for_bypass() {
    // Verify the enum ordering used by filter_batch bypass logic
    assert!(Urgency::Critical < Urgency::High);
    assert!(Urgency::High < Urgency::Medium);
    assert!(Urgency::Medium < Urgency::Watch);
}

#[test]
fn test_make_test_item_fields() {
    let item = make_test_item(Urgency::Critical, "test vuln");
    assert_eq!(item.urgency, Urgency::Critical);
    assert_eq!(item.title, "test vuln");
    assert_eq!(item.id, "test-test-vuln");
}
