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
        matched_window_label: None,
        age_hours: 0.0,
        content_type: None,
        strongly_grounded: false,
        version_affected: None,
    }
}

#[test]
fn test_version_negative_advisory_is_awareness_only() {
    // A long-fixed advisory for a dep the user has already patched must not
    // page as if it endangers today's build (the 2026-07-09 OSV backfill
    // flooded 34 historical axios advisories, all claiming "affects you").
    let inputs = NecessityInputs {
        dep_match_score: 0.7,
        matched_deps: vec!["axios".to_string()],
        signal_type: Some("security_alert".to_string()),
        cve_severity: Some("CRITICAL".to_string()),
        version_affected: Some(false),
        strongly_grounded: true,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score <= 0.25,
        "patched advisory must be awareness-only, got {}",
        result.score
    );
    assert_eq!(result.urgency, Urgency::Awareness);
    assert!(
        result.reason.contains("not affected"),
        "reason must state the honest verdict: {}",
        result.reason
    );
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
        strongly_grounded: true,
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
fn test_weak_text_match_does_not_fire_stack_update() {
    // The 2026-07-13 junk-crate class: a THIRD-PARTY release whose description
    // merely mentions a dep name produces a nonzero dep_match_score but is NOT
    // strongly grounded. It must not become "New release in your stack".
    let inputs = NecessityInputs {
        dep_match_score: 0.5,
        matched_deps: vec!["tauri".to_string()],
        content_type: Some("release_notes".to_string()),
        age_hours: 4.0,
        strongly_grounded: false,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_ne!(
        result.category,
        NecessityCategory::EcosystemShift,
        "un-grounded release must not claim the user's stack: {:?}",
        result.reason
    );
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
        matched_window_label: Some("Choose message queue".to_string()),
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert!(
        result.score > 0.60,
        "Decision-relevant with window_boost 0.18 should score > 0.60, got {}",
        result.score
    );
    assert_eq!(result.category, NecessityCategory::DecisionRelevant);
    assert_eq!(
        result.reason, "Relevant to open decision: Choose message queue",
        "reason must name the matched decision window"
    );
}

#[test]
fn test_decision_relevant_dep_fallback_when_no_label() {
    // No window label reachable, but a dependency overlap connected the item to the
    // window — the reason names that evidence instead of a canned claim.
    let inputs = NecessityInputs {
        window_boost: 0.18,
        dep_match_score: 0.4,
        matched_deps: vec!["axum".to_string()],
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(result.category, NecessityCategory::DecisionRelevant);
    assert_eq!(
        result.reason,
        "Touches axum while a related decision is open"
    );
}

#[test]
fn test_decision_relevant_skill_gap_fallback_when_no_label_or_deps() {
    let inputs = NecessityInputs {
        window_boost: 0.18,
        skill_gap_boost: 0.15,
        matched_skill_gaps: vec!["kubernetes".to_string()],
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(
        result.category,
        NecessityCategory::DecisionRelevant,
        "decision path still takes precedence over blind spot"
    );
    assert_eq!(
        result.reason,
        "Covers kubernetes while a related decision is open"
    );
}

#[test]
fn test_decision_relevant_without_nameable_evidence_stays_silent() {
    // Window boost with NO label, deps, or skill gaps: the old constant headline
    // "Relevant to an active architectural decision" was unfalsifiable — the path
    // now abstains rather than emit a claim the breakdown can't support.
    let inputs = NecessityInputs {
        window_boost: 0.18,
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(result.category, NecessityCategory::None);
    assert!(
        result.score < 0.01,
        "label-less window match with no evidence must not emit necessity, got {}",
        result.score
    );
    assert!(result.reason.is_empty());
}

#[test]
fn test_decision_relevant_blank_label_falls_through() {
    // A whitespace-only label must not produce "Relevant to open decision: ".
    let inputs = NecessityInputs {
        window_boost: 0.18,
        matched_window_label: Some("   ".to_string()),
        dep_match_score: 0.4,
        matched_deps: vec!["tokio".to_string()],
        ..default_inputs()
    };
    let result = compute_necessity(&inputs);
    assert_eq!(
        result.reason,
        "Touches tokio while a related decision is open"
    );
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

// ============================================================================
// persist_from_results (necessity persistence)
// ============================================================================

fn neutral_breakdown() -> crate::types::ScoreBreakdown {
    crate::types::ScoreBreakdown {
        context_score: 0.0,
        interest_score: 0.0,
        keyword_score: 0.0,
        score_ceiling: None,
        ace_boost: 0.0,
        affinity_mult: 1.0,
        anti_penalty: 0.0,
        freshness_mult: 1.0,
        feedback_boost: 0.0,
        source_quality_boost: 0.0,
        confidence_by_signal: std::collections::HashMap::new(),
        signal_count: 0,
        confirmed_signals: vec![],
        confirmation_mult: 1.0,
        dep_match_score: 0.0,
        matched_deps: vec![],
        strongly_grounded: false,
        degraded_inputs: vec![],
        domain_relevance: 1.0,
        content_quality_mult: 1.0,
        novelty_mult: 1.0,
        intent_boost: 0.0,
        content_type: None,
        content_dna_mult: 1.0,
        competing_mult: 1.0,
        llm_score: None,
        llm_reason: None,
        stack_boost: 0.0,
        ecosystem_shift_mult: 1.0,
        stack_competing_mult: 1.0,
        window_boost: 0.0,
        matched_window_id: None,
        skill_gap_boost: 0.0,
        necessity_score: 0.0,
        necessity_reason: None,
        necessity_category: None,
        necessity_urgency: None,
        signal_strength_bonus: 0.0,
        content_analysis_mult: 1.0,
        advisor_signals: vec![],
        disagreement: None,
        advisory_source: None,
        cvss_score: None,
        cvss_severity: None,
        affected_versions: None,
        fixed_version: None,
        installed_version: None,
        is_version_affected: None,
        dependency_path: None,
        affected_project_count: None,
        negative_stack_prior: 1.0,
        explanation_factors: vec![],
    }
}

/// Minimal SourceRelevance carrying only what persist_from_results reads.
fn relevance_with_necessity(
    id: u64,
    necessity: f32,
    reason: Option<&str>,
    with_breakdown: bool,
) -> crate::SourceRelevance {
    let breakdown = with_breakdown.then(|| {
        let mut b = neutral_breakdown();
        b.necessity_score = necessity;
        b.necessity_reason = reason.map(str::to_string);
        b.necessity_category = (necessity > 0.0).then(|| "security_vulnerability".to_string());
        b.necessity_urgency = (necessity > 0.0).then(|| "immediate".to_string());
        b
    });
    crate::SourceRelevance {
        id,
        title: format!("item {id}"),
        url: None,
        top_score: 0.5,
        matches: vec![],
        relevant: true,
        context_score: 0.0,
        interest_score: 0.0,
        excluded: false,
        excluded_by: None,
        source_type: "test".into(),
        explanation: None,
        confidence: None,
        score_breakdown: breakdown,
        signal_type: None,
        signal_priority: None,
        signal_action: None,
        signal_triggers: None,
        signal_horizon: None,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        streets_engine: None,
        decision_window_match: None,
        decision_boost_applied: 0.0,
        created_at: None,
        detected_lang: String::new(),
        is_critical_alert: false,
        applicability: None,
        advisory_id: None,
        primary_topic: None,
        evidence_score: 0.5,
        rank_factors: None,
    }
}

#[test]
fn test_persist_from_results_writes_only_nonzero_necessity() {
    let db = crate::test_utils::test_db();
    let hot = crate::test_utils::insert_test_item(&db, "cve", "n1", "CVE hits axum", "body");
    let cold = crate::test_utils::insert_test_item(&db, "hackernews", "n2", "Listicle", "body");
    let bare = crate::test_utils::insert_test_item(&db, "hackernews", "n3", "No breakdown", "body");

    let results = vec![
        relevance_with_necessity(
            hot as u64,
            0.85,
            Some("Security vulnerability affects axum"),
            true,
        ),
        relevance_with_necessity(cold as u64, 0.0, None, true),
        relevance_with_necessity(bare as u64, 0.9, Some("unreachable"), false),
    ];

    persist_from_results(&db, &results);

    let conn = db.conn.lock();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM item_necessity", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "only the non-zero-necessity item with a breakdown persists"
    );

    let (score, reason, category, urgency): (f64, Option<String>, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT necessity_score, necessity_reason, necessity_category, necessity_urgency
             FROM item_necessity WHERE source_item_id = ?1",
            rusqlite::params![hot],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert!((score - 0.85).abs() < 0.001);
    assert_eq!(
        reason.as_deref(),
        Some("Security vulnerability affects axum")
    );
    assert_eq!(category.as_deref(), Some("security_vulnerability"));
    assert_eq!(urgency.as_deref(), Some("immediate"));
}

#[test]
fn test_persist_from_results_upserts_on_rescore() {
    let db = crate::test_utils::test_db();
    let id = crate::test_utils::insert_test_item(&db, "cve", "n4", "CVE", "body");

    persist_from_results(
        &db,
        &[relevance_with_necessity(
            id as u64,
            0.85,
            Some("first"),
            true,
        )],
    );
    persist_from_results(
        &db,
        &[relevance_with_necessity(
            id as u64,
            0.40,
            Some("decayed"),
            true,
        )],
    );

    let conn = db.conn.lock();
    let (count, score, reason): (i64, f64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), necessity_score, necessity_reason
             FROM item_necessity WHERE source_item_id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(count, 1, "upsert must not duplicate rows");
    assert!((score - 0.40).abs() < 0.001, "re-score refreshes the row");
    assert_eq!(reason.as_deref(), Some("decayed"));
}
