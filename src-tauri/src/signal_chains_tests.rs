// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for signal_chains — EvidenceItem conversion (Intelligence Reconciliation,
//! Phase 5) and the grounding policy (chain_policy). Split out of signal_chains.rs to
//! keep the implementation file under the size limit; included via `#[path]` so these
//! remain a child module with access to the parent's private items.

use super::*;

fn sample_link(signal_type: &str, title: &str) -> ChainLink {
    ChainLink {
        signal_type: signal_type.to_string(),
        source_item_id: 1,
        title: title.to_string(),
        timestamp: "2026-04-15 12:00:00".to_string(),
        description: format!("Signal from {signal_type}"),
    }
}

fn sample_chain_with_prediction() -> SignalChainWithPrediction {
    SignalChainWithPrediction {
        chain: SignalChain {
            id: "chain_tokio_cve".to_string(),
            chain_name: "tokio CVE disclosure + patch sequence".to_string(),
            links: vec![
                sample_link("cve", "CVE-2026-1234 disclosed"),
                sample_link("blog", "Tokio maintainers respond"),
                sample_link("release", "Tokio 1.37 released with fix"),
            ],
            overall_priority: "high".to_string(),
            resolution: ChainResolution::Open,
            suggested_action: "Upgrade tokio to 1.37 immediately.".to_string(),
            confidence: 0.78,
            created_at: "2026-04-15 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
            verified_dep: Some("tokio".to_string()),
        },
        prediction: ChainPrediction {
            phase: ChainPhase::Escalating,
            intervals_hours: vec![36.0, 24.0, 12.0],
            acceleration: -0.3,
            predicted_next_hours: Some(8.0),
            confidence: 0.72,
            forecast: "Escalating — expect patch guidance within the day.".to_string(),
        },
    }
}

#[test]
fn chain_maps_to_chain_kind() {
    let item = sample_chain_with_prediction().to_evidence_item();
    assert_eq!(item.kind, crate::evidence::EvidenceKind::Chain);
}

#[test]
fn chain_priority_maps_to_urgency() {
    let mut c = sample_chain_with_prediction();
    c.chain.overall_priority = "critical".to_string();
    assert_eq!(
        c.to_evidence_item().urgency,
        crate::evidence::Urgency::Critical
    );
    c.chain.overall_priority = "high".to_string();
    assert_eq!(c.to_evidence_item().urgency, crate::evidence::Urgency::High);
    c.chain.overall_priority = "medium".to_string();
    assert_eq!(
        c.to_evidence_item().urgency,
        crate::evidence::Urgency::Medium
    );
    c.chain.overall_priority = "low".to_string();
    assert_eq!(
        c.to_evidence_item().urgency,
        crate::evidence::Urgency::Watch
    );
}

#[test]
fn chain_forecast_is_explanation() {
    let item = sample_chain_with_prediction().to_evidence_item();
    assert!(item.explanation.contains("Escalating"));
}

#[test]
fn affected_deps_use_verified_dep_only_not_chain_name_tokens() {
    // The chain_name is "tokio CVE disclosure + patch sequence" — the OLD regex split
    // would have emitted ["tokio","cve","disclosure"] (boilerplate/topic words as fake
    // deps). Now affected_deps is exactly the verified dependency, nothing else.
    let item = sample_chain_with_prediction().to_evidence_item();
    assert_eq!(item.affected_deps, vec!["tokio".to_string()]);

    // When the topic was NOT a verified dependency, claim no affected dep at all
    // (never fabricate one from the chain name).
    let mut c = sample_chain_with_prediction();
    c.chain.verified_dep = None;
    c.chain.chain_name = "security vulnerability updates signal chain (3 events)".to_string();
    let item = c.to_evidence_item();
    assert!(
        item.affected_deps.is_empty(),
        "no verified dep → no fabricated affected deps, got {:?}",
        item.affected_deps
    );
}

#[test]
fn chain_falls_back_to_suggested_action_when_no_forecast() {
    let mut c = sample_chain_with_prediction();
    c.prediction.forecast.clear();
    let item = c.to_evidence_item();
    assert_eq!(item.explanation, "Upgrade tokio to 1.37 immediately.");
}

#[test]
fn chain_citations_built_from_links() {
    let item = sample_chain_with_prediction().to_evidence_item();
    assert_eq!(item.evidence.len(), 3);
    assert_eq!(item.evidence[0].source, "cve");
}

#[test]
fn chain_without_links_synthesizes_citation() {
    let mut c = sample_chain_with_prediction();
    c.chain.links.clear();
    let item = c.to_evidence_item();
    assert_eq!(item.evidence.len(), 1);
    assert_eq!(item.evidence[0].source, "signal-chain-detector");
}

#[test]
fn chain_caps_citations_at_5() {
    let mut c = sample_chain_with_prediction();
    c.chain.links = (0..10)
        .map(|i| sample_link("hn", &format!("link #{i}")))
        .collect();
    let item = c.to_evidence_item();
    assert_eq!(item.evidence.len(), 5);
}

#[test]
fn chain_lens_hints_preemption_and_evidence() {
    let item = sample_chain_with_prediction().to_evidence_item();
    assert!(item.lens_hints.preemption);
    assert!(item.lens_hints.evidence);
    assert!(!item.lens_hints.briefing);
    assert!(!item.lens_hints.blind_spots);
}

#[test]
fn chain_passes_schema_validation() {
    let item = sample_chain_with_prediction().to_evidence_item();
    assert!(crate::evidence::validate_item(&item).is_ok());
}

#[test]
fn chain_confidence_clamps_out_of_range() {
    let mut c = sample_chain_with_prediction();
    c.prediction.confidence = 1.5;
    let item = c.to_evidence_item();
    assert!(item.confidence.value >= 0.0 && item.confidence.value <= 1.0);
}

// ------------------------------------------------------------------------
// Grounding policy (chain_policy) — keyword-inferred severity must not mint a
// critical alert for a topic the user does not actually depend on.
// ------------------------------------------------------------------------

#[test]
fn ungrounded_keyword_security_cannot_be_critical() {
    // Topic is NOT an installed dep (dep_match = 0). Even a fully-corroborated
    // "security" chain (5 links) must stay awareness-only, never "critical".
    let p = chain_policy(true, false, 0.0, 5);
    assert_eq!(
        p.priority, "watch",
        "ungrounded keyword-security chain must not be critical"
    );
}

#[test]
fn ungrounded_breaking_cannot_be_alert() {
    let p = chain_policy(false, true, 0.0, 5);
    assert_eq!(p.priority, "watch");
}

#[test]
fn grounded_security_is_critical() {
    // Same security signal, but now the topic IS an installed dependency.
    let p = chain_policy(true, false, 0.6, 3);
    assert_eq!(p.priority, "critical");
}

#[test]
fn grounded_breaking_is_alert() {
    let p = chain_policy(false, true, 0.6, 3);
    assert_eq!(p.priority, "alert");
}

#[test]
fn grounded_thin_vs_corroborated_non_security() {
    // Installed dep, no security/breaking: 3+ links → advisory, fewer → watch.
    assert_eq!(chain_policy(false, false, 0.6, 3).priority, "advisory");
    assert_eq!(chain_policy(false, false, 0.6, 2).priority, "watch");
}

#[test]
fn ungrounded_confidence_capped_below_grounded_band() {
    // The worst pre-fix case: a 2-link "security" chain on a non-dep topic used to
    // surface at "critical" with confidence ~0.32. Confidence is now capped, and the
    // cap sits strictly below the floor any grounded chain can reach.
    let ungrounded = chain_policy(true, false, 0.0, 5);
    assert!(
        ungrounded.confidence <= UNGROUNDED_CONFIDENCE_CAP + f64::EPSILON,
        "ungrounded confidence {} exceeded cap {}",
        ungrounded.confidence,
        UNGROUNDED_CONFIDENCE_CAP
    );

    // Weakest possible grounded chain (min dep_match 0.5, 2 links, learning severity).
    let grounded_floor = chain_policy(false, false, 0.5, 2);
    assert!(
        grounded_floor.confidence > UNGROUNDED_CONFIDENCE_CAP,
        "grounded floor {} should exceed ungrounded cap {}",
        grounded_floor.confidence,
        UNGROUNDED_CONFIDENCE_CAP
    );
}

#[test]
fn grounded_chains_retain_dependency_weighted_confidence() {
    // More dependency matches → higher confidence (dep relevance is the 50% term).
    let one = chain_policy(false, false, 0.5, 3).confidence;
    let many = chain_policy(false, false, 0.9, 3).confidence;
    assert!(many > one);
}
