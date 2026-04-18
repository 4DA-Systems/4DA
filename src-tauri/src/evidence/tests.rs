// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Unit tests for EvidenceItem: schema roundtrip + validate_item coverage.

use super::types::*;
use super::validate::{validate_item, ValidationError};

fn base_citation() -> EvidenceCitation {
    EvidenceCitation {
        source: "hackernews".into(),
        title: "Some article".into(),
        url: Some("https://example.com/x".into()),
        freshness_days: 1.0,
        relevance_note: "matches tokio in Cargo.toml".into(),
    }
}

fn base_action() -> Action {
    Action {
        action_id: "acknowledge".into(),
        label: "OK".into(),
        description: "Acknowledge this item".into(),
    }
}

fn good_alert() -> EvidenceItem {
    EvidenceItem {
        id: "ev_alert_1".into(),
        kind: EvidenceKind::Alert,
        title: "CVE-2026-1234 affects tokio 1.x".into(),
        explanation: "".into(),
        confidence: Confidence::heuristic(0.72),
        urgency: Urgency::High,
        reversibility: Some(0.2),
        evidence: vec![base_citation()],
        affected_projects: vec!["4da".into()],
        affected_deps: vec!["tokio".into()],
        suggested_actions: vec![base_action()],
        precedents: vec![],
        refutation_condition: None,
        lens_hints: LensHints::preemption_only(),
        created_at: 1_700_000_000_000,
        expires_at: None,
    }
}

// --- Roundtrip ----------------------------------------------------------------

#[test]
fn serialize_deserialize_roundtrips() {
    let original = good_alert();
    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: EvidenceItem = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, original);
}

#[test]
fn urgency_serializes_lowercase() {
    let json = serde_json::to_string(&Urgency::Critical).unwrap();
    assert_eq!(json, "\"critical\"");
}

#[test]
fn evidence_kind_serializes_snake_case() {
    let json = serde_json::to_string(&EvidenceKind::MissedSignal).unwrap();
    assert_eq!(json, "\"missed_signal\"");
}

#[test]
fn confidence_provenance_serializes_snake_case() {
    let json = serde_json::to_string(&ConfidenceProvenance::LlmAssessed).unwrap();
    assert_eq!(json, "\"llm_assessed\"");
}

// --- Urgency ordering (used by lens sorters) ---------------------------------

#[test]
fn urgency_ordering_is_most_urgent_first() {
    assert!(Urgency::Critical < Urgency::High);
    assert!(Urgency::High < Urgency::Medium);
    assert!(Urgency::Medium < Urgency::Watch);
}

// --- Confidence constructors -------------------------------------------------

#[test]
fn confidence_constructors_set_provenance() {
    assert_eq!(
        Confidence::checklist(0.5).provenance,
        ConfidenceProvenance::Checklist
    );
    assert_eq!(
        Confidence::heuristic(0.5).provenance,
        ConfidenceProvenance::Heuristic
    );
    assert_eq!(
        Confidence::calibrated(0.5, 42).provenance,
        ConfidenceProvenance::Calibrated
    );
    assert_eq!(
        Confidence::llm_assessed(0.5).provenance,
        ConfidenceProvenance::LlmAssessed
    );
}

#[test]
fn confidence_calibrated_captures_sample_size() {
    let c = Confidence::calibrated(0.8, 47);
    assert_eq!(c.sample_size, Some(47));
}

// --- validate_item: happy paths ----------------------------------------------

#[test]
fn valid_alert_passes() {
    assert!(validate_item(&good_alert()).is_ok());
}

#[test]
fn retrospective_without_evidence_passes() {
    let mut it = good_alert();
    it.kind = EvidenceKind::Retrospective;
    it.evidence.clear();
    it.suggested_actions.clear(); // Retrospective is not actionable
    assert!(validate_item(&it).is_ok());
}

// --- validate_item: rejection paths ------------------------------------------

#[test]
fn empty_id_rejected() {
    let mut it = good_alert();
    it.id.clear();
    assert_eq!(validate_item(&it), Err(ValidationError::IdEmpty));
}

#[test]
fn empty_title_rejected() {
    let mut it = good_alert();
    it.title.clear();
    assert_eq!(validate_item(&it), Err(ValidationError::TitleEmpty));
}

#[test]
fn title_over_120_rejected() {
    let mut it = good_alert();
    it.title = "x".repeat(121);
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::TitleTooLong { .. })
    ));
}

#[test]
fn title_ending_in_period_rejected() {
    let mut it = good_alert();
    it.title = "Something.".into();
    assert_eq!(
        validate_item(&it),
        Err(ValidationError::TitleTrailingPeriod)
    );
}

#[test]
fn confidence_out_of_range_rejected() {
    let mut it = good_alert();
    it.confidence.value = 1.5;
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::ConfidenceOutOfRange(_))
    ));
}

#[test]
fn calibrated_missing_sample_size_rejected() {
    let mut it = good_alert();
    it.confidence = Confidence {
        value: 0.7,
        provenance: ConfidenceProvenance::Calibrated,
        sample_size: None,
    };
    assert_eq!(validate_item(&it), Err(ValidationError::CalibratedMissingN));
}

#[test]
fn calibrated_small_sample_size_rejected() {
    let mut it = good_alert();
    it.confidence = Confidence::calibrated(0.7, 5);
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::CalibratedNTooSmall { .. })
    ));
}

#[test]
fn calibrated_n_at_floor_accepted() {
    let mut it = good_alert();
    it.confidence = Confidence::calibrated(0.7, 10);
    assert!(validate_item(&it).is_ok());
}

#[test]
fn reversibility_out_of_range_rejected() {
    let mut it = good_alert();
    it.reversibility = Some(1.5);
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::ReversibilityOutOfRange(_))
    ));
}

#[test]
fn alert_without_evidence_rejected() {
    let mut it = good_alert();
    it.evidence.clear();
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::EvidenceRequired { .. })
    ));
}

#[test]
fn alert_without_actions_rejected() {
    let mut it = good_alert();
    it.suggested_actions.clear();
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::ActionsRequired { .. })
    ));
}

#[test]
fn unknown_action_id_rejected() {
    let mut it = good_alert();
    it.suggested_actions[0].action_id = "invented_action".into();
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::UnknownActionId(_))
    ));
}

#[test]
fn all_canonical_action_ids_accepted() {
    for id in ACTION_IDS {
        let mut it = good_alert();
        it.suggested_actions[0].action_id = (*id).into();
        assert!(
            validate_item(&it).is_ok(),
            "canonical action_id {id} should be accepted"
        );
    }
}

#[test]
fn citation_relevance_note_too_long_rejected() {
    let mut it = good_alert();
    it.evidence[0].relevance_note = "x".repeat(201);
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::CitationNoteTooLong { .. })
    ));
}

#[test]
fn precedent_similarity_out_of_range_rejected() {
    let mut it = good_alert();
    it.precedents.push(PrecedentRef {
        decision_id: "dc_1".into(),
        statement: "Adopted Tauri over Electron".into(),
        outcome: Some(PrecedentOutcome::Confirmed),
        origin: "user-history".into(),
        similarity: 1.3,
    });
    assert!(matches!(
        validate_item(&it),
        Err(ValidationError::PrecedentSimilarityOutOfRange(_))
    ));
}

// --- LensHints convenience ----------------------------------------------------

#[test]
fn lens_hints_defaults_all_false() {
    let h = LensHints::default();
    assert!(!h.briefing && !h.preemption && !h.blind_spots && !h.evidence);
}

#[test]
fn lens_hints_preemption_only() {
    let h = LensHints::preemption_only();
    assert!(h.preemption);
    assert!(!h.briefing && !h.blind_spots && !h.evidence);
}

// --- EvidenceFeed --------------------------------------------------------------

fn item_with_urgency(u: Urgency) -> EvidenceItem {
    let mut it = good_alert();
    it.urgency = u;
    it
}

#[test]
fn evidence_feed_counts_by_urgency() {
    let items = vec![
        item_with_urgency(Urgency::Critical),
        item_with_urgency(Urgency::Critical),
        item_with_urgency(Urgency::High),
        item_with_urgency(Urgency::Medium),
        item_with_urgency(Urgency::Watch),
    ];
    let feed = EvidenceFeed::from_items(items);
    assert_eq!(feed.total, 5);
    assert_eq!(feed.critical_count, 2);
    assert_eq!(feed.high_count, 1);
}

#[test]
fn evidence_feed_empty_has_zero_counts() {
    let feed = EvidenceFeed::from_items(vec![]);
    assert_eq!(feed.total, 0);
    assert_eq!(feed.critical_count, 0);
    assert_eq!(feed.high_count, 0);
}

#[test]
fn evidence_feed_without_score_is_none() {
    let feed = EvidenceFeed::from_items(vec![]);
    assert_eq!(feed.score, None);
}

#[test]
fn evidence_feed_with_score_clamps_to_0_100() {
    let under = EvidenceFeed::from_items_with_score(vec![], -12.0);
    let over = EvidenceFeed::from_items_with_score(vec![], 150.0);
    let in_range = EvidenceFeed::from_items_with_score(vec![], 42.5);
    assert_eq!(under.score, Some(0.0));
    assert_eq!(over.score, Some(100.0));
    assert_eq!(in_range.score, Some(42.5));
}
