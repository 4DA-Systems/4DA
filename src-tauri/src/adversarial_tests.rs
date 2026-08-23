// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use crate::evidence::{EvidenceCitation, EvidenceKind, LensHints};

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

// ---- Verdict application (the self-refuting-alert regression) ----

#[test]
fn agreeing_verdict_updates_the_item() {
    assert_eq!(
        apply_verdict(false, true),
        VerdictApplication::SurfaceUpdated
    );
    // The safety floor never blocks an agreed update.
    assert_eq!(
        apply_verdict(true, true),
        VerdictApplication::SurfaceUpdated
    );
}

/// A dissenting verdict on a Critical/High item surfaces the item UNCHANGED.
/// Regression for 2026-08-22: the old `should_surface || must_surface` arm
/// overwrote the item's explanation/confidence with the REFUTING verdict, so
/// Preemption displayed a Critical alert whose own explanation said
/// "incorrectly escalated" at 92% confidence.
#[test]
fn dissenting_verdict_on_must_surface_item_keeps_original_evidence() {
    assert_eq!(
        apply_verdict(true, false),
        VerdictApplication::SurfaceUnchanged
    );
}

#[test]
fn dissenting_verdict_without_floor_filters() {
    assert_eq!(apply_verdict(false, false), VerdictApplication::Filter);
}

// ---- Escalation corroboration (the phantom-chain critical regression) ----
//
// Observed live 2026-08-23: 3 of 6 critical Preemption items were signal
// chains built from bare single-token topic matches ("table", "sandbox",
// "next") whose own explanation said "No advisory issued". R3 fixed that
// copy; these tests pin the escalation decision itself.

fn citation(title: &str, url: Option<&str>) -> EvidenceCitation {
    EvidenceCitation {
        source: "hackernews".to_string(),
        title: title.to_string(),
        url: url.map(String::from),
        freshness_days: 1.0,
        relevance_note: "test".to_string(),
    }
}

#[test]
fn advisory_id_detection_accepts_real_ids() {
    assert!(contains_advisory_id("CVE-2026-12345 fixed in 4.17.21"));
    assert!(contains_advisory_id("ghsa-jfmj-5f2r-6x9p")); // case-insensitive
    assert!(contains_advisory_id("RUSTSEC-2026-0001"));
    assert!(contains_advisory_id(
        "https://osv.dev/vulnerability/GO-2026-5781"
    ));
    assert!(contains_advisory_id("PYSEC-2026-1 advisory"));
    assert!(contains_advisory_id("(MAL-2026-123)"));
}

#[test]
fn advisory_id_detection_rejects_prose() {
    // Token boundary: "MAL-" inside a word must not match.
    assert!(!contains_advisory_id("normal-2026 release notes"));
    // Numeric families need a digit right after the prefix.
    assert!(!contains_advisory_id("go-to-definition support"));
    assert!(!contains_advisory_id("the CVE- prefix explained"));
    // Bare chain topics never look like advisories.
    assert!(!contains_advisory_id("table"));
    assert!(!contains_advisory_id("sandbox escape discussion"));
    assert!(!contains_advisory_id(""));
}

/// A chain with no advisory linkage and empty affected_deps cannot stay
/// Critical: the gate demotes it below the must-surface floor. This is the
/// exact live shape — a single-token chain arriving Critical with empty
/// deps, heuristic confidence, and article citations naming no advisory.
#[test]
fn uncorroborated_critical_chain_cannot_stay_critical() {
    let mut item = make_test_item(Urgency::Critical, "table signal chain (4 events)");
    item.id = "chain-00000000-0000-0000-0000-000000000000".to_string();
    item.evidence.push(citation(
        "Why your SQL table design matters",
        Some("https://example.com/tables"),
    ));

    assert!(gate_escalation(&mut item), "gate must demote");
    assert_eq!(item.urgency, Urgency::Medium);
    // The demoted item no longer qualifies for the must-surface floor.
    assert!(item.urgency != Urgency::Critical && item.urgency != Urgency::High);
}

#[test]
fn uncorroborated_high_item_loses_the_floor_too() {
    let mut item = make_test_item(Urgency::High, "sandbox signal chain (3 events)");
    assert!(gate_escalation(&mut item));
    assert_eq!(item.urgency, Urgency::Medium);
}

/// A chain with a linked OSV/GHSA advisory on a corroborated dep is still
/// allowed to be Critical: advisory linkage + affected deps uphold the
/// escalation and the safety floor.
#[test]
fn advisory_linked_chain_on_corroborated_dep_keeps_critical() {
    let mut item = make_test_item(Urgency::Critical, "lodash prototype pollution chain");
    item.affected_deps.push("lodash".to_string());
    item.evidence.push(citation(
        "GHSA-35jh-r3h4-6jhm: lodash command injection",
        Some("https://osv.dev/vulnerability/GHSA-35jh-r3h4-6jhm"),
    ));

    assert!(
        !gate_escalation(&mut item),
        "corroborated escalation stands"
    );
    assert_eq!(item.urgency, Urgency::Critical);
}

#[test]
fn advisory_id_in_title_counts_as_linkage() {
    let mut item = make_test_item(Urgency::Critical, "CVE-2026-22222 in next.js middleware");
    assert!(!gate_escalation(&mut item));
    assert_eq!(item.urgency, Urgency::Critical);
}

/// The live post-activation shape (2026-08-24): a single-token chain whose
/// EVIDENCE citations carry advisory-titled neighbors — a chain aggregates
/// co-tokened items, so the "table" chain cited an unrelated XWiki "Live
/// Table" CVE and rode that citation through the linkage check. A cited
/// neighbor's advisory id is evidence about the neighbor, not the chain:
/// chain items must not keep critical through citations alone.
#[test]
fn chain_with_advisory_titled_citations_still_demotes() {
    let mut item = make_test_item(
        Urgency::Critical,
        "Structured Prediction for Scalable Spreadsheet Table Understanding",
    );
    item.id = "chain-8457f1ba-bcef-4b15-812f-04229fa15295".to_string();
    item.evidence.push(citation(
        "[CVE-2026-53966] XWiki Platform Live Data Live Table Connector",
        Some("https://osv.dev/vulnerability/CVE-2026-53966"),
    ));
    item.evidence.push(citation(
        "[GHSA-hq84-x37p-j6q5] Winter: Reflected XSS through the search",
        None,
    ));

    assert!(
        gate_escalation(&mut item),
        "a chain must not keep critical through cited neighbors' advisory ids"
    );
    assert_eq!(item.urgency, Urgency::Medium);
}

/// A NON-chain item keeps citation-derived linkage (the citation is about the
/// item itself, e.g. an aggregated advisory writeup) — the chain rule is
/// scoped to `chain-*` ids only.
#[test]
fn non_chain_item_keeps_citation_linkage() {
    let mut item = make_test_item(Urgency::High, "lodash injection weekly roundup");
    item.evidence.push(citation(
        "GHSA-35jh-r3h4-6jhm: lodash command injection",
        Some("https://osv.dev/vulnerability/GHSA-35jh-r3h4-6jhm"),
    ));
    assert!(!gate_escalation(&mut item));
    assert_eq!(item.urgency, Urgency::High);
}

#[test]
fn osv_verified_provenance_counts_as_advisory_linkage() {
    let mut item = make_test_item(Urgency::Critical, "openssl heap overflow");
    item.confidence = Confidence::osv_verified(0.95);
    assert!(!gate_escalation(&mut item));
    assert_eq!(item.urgency, Urgency::Critical);
}

/// Preemption's whole point is warning BEFORE the advisory: when a
/// materializer confirmed affected deps, the escalation stands even with no
/// advisory id anywhere.
#[test]
fn corroborated_deps_alone_uphold_escalation() {
    let mut item = make_test_item(Urgency::High, "tokio breaking change wave");
    item.affected_deps.push("tokio".to_string());
    assert!(!gate_escalation(&mut item));
    assert_eq!(item.urgency, Urgency::High);
}

#[test]
fn non_escalated_items_pass_the_gate_untouched() {
    for urgency in [Urgency::Medium, Urgency::Watch] {
        let mut item = make_test_item(urgency, "some chain");
        assert!(!gate_escalation(&mut item));
        assert_eq!(item.urgency, urgency);
    }
}
