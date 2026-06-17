// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;

/// Helper to create default inputs with everything zeroed/empty.
fn default_inputs() -> NecessityInputs {
    NecessityInputs {
        dep_match_score: 0.0,
        matched_deps: vec![],
        signal_type: None,
        signal_priority: None,
        cve_severity: None,
        cvss_score: None,
        affected_project_count: 0,
        skill_gap_boost: 0.0,
        matched_skill_gaps: vec![],
        window_boost: 0.0,
        age_hours: 0.0,
        content_type: None,
        contradiction_boost: 0.0,
    }
}

#[test]
fn test_stack_update_release_of_a_dependency_surfaces() {
    // A new release of something in the user's stack (e.g. "crates.io: axum v0.8.9")
    // must surface as an actionable stack update — NOT decay into a 0.17 blind-spot.
    let inputs = NecessityInputs {
        dep_match_score: 0.6,
        matched_deps: vec!["axum".to_string()],
        content_type: Some("release_notes".to_string()),
        age_hours: 24.0,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(result.category, NecessityCategory::EcosystemShift);
    assert!(
        result.score >= 0.40,
        "A fresh release of a stack dependency should score >= 0.40, got {}",
        result.score
    );
    assert!(result.reason.contains("axum"));
}

#[test]
fn test_release_without_dep_match_does_not_fire_stack_update() {
    // A release of something the user does NOT depend on must not hijack the
    // stack-update path (preserves the necessity-over-want doctrine).
    let inputs = NecessityInputs {
        dep_match_score: 0.0,
        content_type: Some("release_notes".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_ne!(result.category, NecessityCategory::EcosystemShift);
}

#[test]
fn test_critical_cve_with_dep_match() {
    let inputs = NecessityInputs {
        dep_match_score: 0.7,
        matched_deps: vec!["lodash".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("CRITICAL".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.90,
        "Critical CVE + dep match should score > 0.90, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::SecurityVulnerability);
    assert_eq!(result.urgency, Urgency::Immediate);
    assert!(result.reason.contains("lodash"));
}

#[test]
fn test_high_cve_without_dep_match() {
    let inputs = NecessityInputs {
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("HIGH".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.40,
        "High CVE without dep match should score < 0.40, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::SecurityVulnerability);
    assert_eq!(result.urgency, Urgency::Awareness);
}

#[test]
fn test_high_cve_with_dep_match() {
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["serde".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("HIGH".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.80,
        "High CVE + dep match should score > 0.80, got {}",
        result.score
    );
    assert_eq!(result.urgency, Urgency::ThisWeek);
}

#[test]
fn test_breaking_change_with_dep_match() {
    let inputs = NecessityInputs {
        dep_match_score: 0.6,
        matched_deps: vec!["react".to_string()],
        signal_type: Some("breaking_change".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.70,
        "Breaking change + dep match should score > 0.70, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::BreakingChange);
    assert_eq!(result.urgency, Urgency::ThisWeek);
}

#[test]
fn test_breaking_change_without_dep_match() {
    let inputs = NecessityInputs {
        signal_type: Some("breaking_change".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.30,
        "Breaking change without dep match should score < 0.30, got {}",
        result.score
    );
}

#[test]
fn test_blind_spot_boost() {
    let inputs = NecessityInputs {
        skill_gap_boost: 0.15,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.40,
        "Blind spot with skill_gap 0.15 should score > 0.40, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::BlindSpot);
    assert_eq!(result.urgency, Urgency::Awareness);
}

#[test]
fn test_decision_relevant() {
    let inputs = NecessityInputs {
        window_boost: 0.18,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.60,
        "Decision-relevant with window_boost 0.18 should score > 0.60, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::DecisionRelevant);
}

#[test]
fn test_multi_project_amplification() {
    let inputs = NecessityInputs {
        dep_match_score: 0.6,
        matched_deps: vec!["tokio".to_string()],
        signal_type: Some("breaking_change".to_string()),
        affected_project_count: 4,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    // Base score 0.80 * amplification (1.0 + 3*0.1 = 1.3) = 1.04, clamped to 1.0
    assert!(
        result.score > 0.80,
        "Multi-project should amplify score above base 0.80, got {}",
        result.score
    );
}

#[test]
fn test_recency_decay_non_security() {
    // Breaking change that is 5 days old
    let inputs = NecessityInputs {
        dep_match_score: 0.6,
        matched_deps: vec!["react".to_string()],
        signal_type: Some("breaking_change".to_string()),
        age_hours: 120.0, // 5 days
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    // Base 0.80 * decay max(0.5, 1.0 - 120/168) = 0.80 * 0.286 -> but floor at 0.5
    // So 0.80 * 0.5 = 0.40 approximately
    assert!(
        result.score < 0.80,
        "5-day-old breaking change should decay below 0.80, got {}",
        result.score
    );
    assert!(
        result.score >= 0.30,
        "Should not decay too aggressively, got {}",
        result.score
    );
}

#[test]
fn test_security_no_recency_decay() {
    // Critical security item that is 5 days old — should NOT decay
    let inputs = NecessityInputs {
        dep_match_score: 0.7,
        matched_deps: vec!["lodash".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("CRITICAL".to_string()),
        age_hours: 120.0, // 5 days
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.90,
        "Security items should not decay with age, got {}",
        result.score
    );
}

#[test]
fn test_no_necessity_item() {
    let inputs = default_inputs();
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.01,
        "No-signal item should score near 0.0, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::None);
    assert_eq!(result.urgency, Urgency::None);
}

#[test]
fn test_medium_cve_with_dep_match() {
    let inputs = NecessityInputs {
        dep_match_score: 0.4,
        matched_deps: vec!["express".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("MEDIUM".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score >= 0.55 && result.score <= 0.65,
        "Medium CVE + dep match should be ~0.60, got {}",
        result.score
    );
    assert_eq!(result.urgency, Urgency::Awareness);
}

#[test]
fn test_multi_project_capped_amplification() {
    // 10 projects affected — amplification capped at 1.5x
    let inputs = NecessityInputs {
        dep_match_score: 0.6,
        matched_deps: vec!["tokio".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("HIGH".to_string()),
        affected_project_count: 10,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    // Base 0.85 * 1.5 = 1.275, clamped to 1.0
    assert_eq!(
        result.score, 1.0,
        "10-project amplification on high CVE should cap at 1.0"
    );
}

#[test]
fn test_skill_gap_too_low_no_match() {
    let inputs = NecessityInputs {
        skill_gap_boost: 0.05, // below 0.10 threshold
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.01,
        "Skill gap below threshold should not trigger, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::None);
}

#[test]
fn test_window_boost_too_low_no_match() {
    let inputs = NecessityInputs {
        window_boost: 0.08, // below 0.10 threshold
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.01,
        "Window boost below threshold should not trigger, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::None);
}

#[test]
fn test_deprecation_with_dep_match() {
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["moment".to_string()],
        signal_type: Some("deprecation".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.60,
        "Deprecation + dep match should score > 0.60, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::DeprecationNotice);
    assert_eq!(result.urgency, Urgency::ThisWeek);
}

#[test]
fn test_security_takes_priority_over_breaking_change() {
    // Item classified as both security AND breaking — security path should win
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["openssl".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("CRITICAL".to_string()),
        window_boost: 0.15,    // also decision relevant
        skill_gap_boost: 0.15, // also blind spot
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(
        result.category,
        NecessityCategory::SecurityVulnerability,
        "Security should take priority"
    );
    assert!(result.score > 0.90);
}

#[test]
fn test_contradiction_boost_triggers() {
    let inputs = NecessityInputs {
        contradiction_boost: 0.5, // single topic match
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.40,
        "Contradiction with 0.5 boost should score > 0.40, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::BlindSpot);
    assert_eq!(result.urgency, Urgency::Awareness);
    assert!(result.reason.contains("conflicting signals"));
}

#[test]
fn test_contradiction_strong_boost() {
    let inputs = NecessityInputs {
        contradiction_boost: 1.0, // multiple topic matches
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score >= 0.65,
        "Strong contradiction should score >= 0.65, got {}",
        result.score
    );
}

#[test]
fn test_contradiction_no_boost() {
    let inputs = NecessityInputs {
        contradiction_boost: 0.0,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score < 0.01,
        "No contradiction boost should not trigger, got {}",
        result.score
    );
}

#[test]
fn test_security_takes_priority_over_contradiction() {
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["openssl".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("CRITICAL".to_string()),
        contradiction_boost: 1.0, // also has contradiction
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(
        result.category,
        NecessityCategory::SecurityVulnerability,
        "Security should take priority over contradiction"
    );
    assert!(result.score > 0.90);
}

#[test]
fn test_cvss_score_promotes_severity_when_no_priority() {
    // Bug J regression: a real critical CVE that reaches the security path with NO
    // signal priority and NO cve_severity (e.g. a dev-only dep that didn't trip the
    // classifier) must use the CVSS base score, not silently fall back to "medium".
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["serde".to_string()],
        content_type: Some("security_advisory".to_string()),
        signal_priority: None,
        cve_severity: None,
        cvss_score: Some(9.8),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(result.category, NecessityCategory::SecurityVulnerability);
    assert!(
        result.score > 0.90,
        "CVSS 9.8 with dep match must score critical (>0.90), not medium 0.60, got {}",
        result.score
    );
    assert_eq!(result.urgency, Urgency::Immediate);
}

#[test]
fn test_signal_priority_still_wins_over_cvss() {
    // The CVSS fallback must NOT override a present signal priority — the trust gate
    // deliberately downgrades transitive criticals to a lower priority, and that must
    // be respected. A "medium" priority with a high CVSS stays medium-scored.
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["openssl".to_string()],
        content_type: Some("security_advisory".to_string()),
        signal_priority: Some("medium".to_string()),
        cve_severity: None,
        cvss_score: Some(9.8),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score >= 0.55 && result.score <= 0.65,
        "present signal_priority=medium must win over CVSS 9.8, got {}",
        result.score
    );
}
