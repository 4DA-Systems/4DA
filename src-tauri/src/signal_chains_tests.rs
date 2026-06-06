// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for signal_chains — EvidenceItem conversion (Intelligence Reconciliation,
//! Phase 5) and the grounding policy (chain_policy). Split out of signal_chains.rs to
//! keep the implementation file under the size limit; included via `#[path]` so these
//! remain a child module with access to the parent's private items.

use super::*;

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
