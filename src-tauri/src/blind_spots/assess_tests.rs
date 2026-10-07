// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! AI assessment: persisted stable-key cache, evidence-only prompt input,
//! and the verdict/wording guards (audit 2026-10-07, wave 4c).

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn input(name: &str, why: &str, force_worth: bool, has_advisory: bool) -> AssessInput {
    AssessInput {
        name: name.to_string(),
        why: why.to_string(),
        force_worth,
        has_advisory,
        has_signals: true,
    }
}

fn gap(name: &str, risk: &str, days_since: u32, available: u32) -> UncoveredDep {
    UncoveredDep {
        name: name.to_string(),
        dep_type: "crates.io".to_string(),
        projects_using: vec!["/a".into(), "/b".into(), "/c".into()],
        days_since_last_signal: days_since,
        available_signal_count: available,
        risk_level: risk.to_string(),
        match_type: "exact_registry".to_string(),
        coverage_reason: None,
        adapters_searched: Vec::new(),
        platform_active: true,
    }
}

fn breakdown(releases: u32, security: u32, advisories: u32, fp: u64) -> DepSignalBreakdown {
    DepSignalBreakdown {
        releases,
        security,
        advisories,
        security_fingerprint: fp,
        ..Default::default()
    }
}

fn persisted(key: &str) -> PersistedAssessment {
    PersistedAssessment {
        version: PERSIST_VERSION,
        key: key.to_string(),
        assessment: BlindSpotAssessment {
            assessments: vec![DepAssessment {
                dep_name: "openssl (crates.io)".into(),
                worth_reviewing: true,
                recommendation: "Review the advisory.".into(),
            }],
            model: "claude-sonnet".into(),
            assessed_at: 1,
            from_cache: false,
            stale: false,
        },
    }
}

// ─── (a) persisted, stable-key cache ─────────────────────────────────────

#[test]
fn persisted_assessment_survives_a_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("4da.db");
    {
        let db = Database::new(&path).expect("open db");
        store_persisted_in(&db, &persisted("v1;openssl|1|1|00000000000000aa"));
    } // process "exits": the connection is gone
    let db = Database::new(&path).expect("reopen db");
    let back = load_persisted_from(&db).expect("assessment survives the restart");
    assert_eq!(back.key, "v1;openssl|1|1|00000000000000aa");
    assert_eq!(back.assessment.assessments.len(), 1);
    assert_eq!(back.assessment.model, "claude-sonnet");
}

#[tokio::test]
async fn unchanged_stable_key_makes_zero_llm_calls() {
    let calls = AtomicUsize::new(0);
    let key = "v1;openssl|1|1|00000000000000aa";
    let inputs = vec![input(
        "openssl (crates.io)",
        "1 unreviewed signal",
        true,
        true,
    )];
    let out = assess_core(&inputs, key, Some(persisted(key)), false, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok(("[]".to_string(), "m".to_string())) }
    })
    .await
    .expect("cache hit");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "unchanged key must not call the model"
    );
    assert!(out.assessment.from_cache);
    assert!(out.persist.is_none(), "a cache hit rewrites nothing");
}

#[tokio::test]
async fn changed_key_or_force_calls_the_model_once_and_persists() {
    let inputs = vec![input(
        "openssl (crates.io)",
        "1 unreviewed signal",
        true,
        true,
    )];
    let resp = r#"[{"id":1,"worth_reviewing":true,"recommendation":"Review it."}]"#;
    for (cached_key, force) in [("v1;old", false), ("v1;new", true)] {
        let calls = AtomicUsize::new(0);
        let out = assess_core(
            &inputs,
            "v1;new",
            Some(persisted(cached_key)),
            force,
            |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok((resp.to_string(), "m".to_string())) }
            },
        )
        .await
        .expect("model result");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!out.assessment.from_cache);
        assert_eq!(out.persist.map(|p| p.key), Some("v1;new".to_string()));
    }
}

#[tokio::test]
async fn an_unusable_model_answer_is_not_persisted() {
    let inputs = vec![input("x (npm)", "no signals", false, false)];
    let out = assess_core(&inputs, "v1;", None, false, |_| async {
        Ok(("not json".to_string(), "m".to_string()))
    })
    .await
    .expect("ok");
    assert!(out.persist.is_none(), "garbage must not pin the cache shut");
}

#[test]
fn stable_key_ignores_release_churn_and_tracks_security_evidence() {
    let base = vec![(
        "openssl (crates.io)".to_string(),
        Some(breakdown(0, 1, 1, 7)),
    )];
    let mut churned = base.clone();
    churned.push((
        "schemars (crates.io)".to_string(),
        Some(breakdown(1, 0, 0, 0)),
    ));
    churned.push(("quiet (crates.io)".to_string(), None));
    assert_eq!(
        stable_assessment_key(&base),
        stable_assessment_key(&churned),
        "release-only / zero-signal deps entering the window must not change the key"
    );
    let new_advisory = vec![(
        "openssl (crates.io)".to_string(),
        Some(breakdown(0, 2, 2, 9)),
    )];
    assert_ne!(
        stable_assessment_key(&base),
        stable_assessment_key(&new_advisory)
    );
}

#[test]
fn cached_view_is_stale_only_when_the_current_key_is_known_and_differs() {
    assert!(!cached_view(persisted("v1;a"), Some("v1;a")).stale);
    assert!(cached_view(persisted("v1;a"), Some("v1;b")).stale);
    assert!(
        !cached_view(persisted("v1;a"), None).stale,
        "unknown key never triggers a paid rerun"
    );
    assert!(cached_view(persisted("v1;a"), None).from_cache);
}

// ─── (b) evidence, not risk labels ───────────────────────────────────────

#[test]
fn release_only_dep_is_sent_as_evidence_never_as_risk() {
    // The live case: schemars, one routine release, never opened, 3 projects —
    // engagement risk said "critical".
    let d = gap("schemars (crates.io)", "critical", 999, 1);
    let why = describe_evidence(&d, Some(breakdown(1, 0, 0, 0)), &["0.8.21".to_string()]);
    assert_eq!(
        why,
        "1 unreviewed signal: 1 new release (you're on 0.8.21); never engaged"
    );
    let lower = why.to_lowercase();
    for banned in ["risk", "critical", "high", "999"] {
        assert!(!lower.contains(banned), "{banned:?} leaked into {why:?}");
    }
}

#[test]
fn advisory_evidence_names_the_advisory() {
    let d = gap("openssl (crates.io)", "high", 12, 2);
    let why = describe_evidence(&d, Some(breakdown(1, 1, 1, 3)), &[]);
    assert!(why.starts_with("2 unreviewed signals: 1 advisory affecting your installed version"));
    assert!(why.ends_with("last opened an item about it 12 days ago"));
}

#[test]
fn prompt_forbids_risk_words_without_an_advisory() {
    assert!(BS_ASSESS_SYSTEM_PROMPT.contains("NEVER describe a dependency as \"critical\""));
    assert!(!BS_ASSESS_SYSTEM_PROMPT.contains("risk="));
}

#[test]
fn risk_wording_on_routine_evidence_is_replaced() {
    let routine = input("schemars (crates.io)", "1 unreviewed signal", false, false);
    let rec = enforce_risk_vocabulary(
        "Critical risk signal on a core schema library, review it now",
        &routine,
    );
    assert!(!rec.to_lowercase().contains("critical"), "{rec}");
    let advisory = input("openssl (crates.io)", "1 unreviewed signal", true, true);
    let kept = enforce_risk_vocabulary("Critical advisory, upgrade now", &advisory);
    assert_eq!(kept, "Critical advisory, upgrade now");
}

// ─── (c) the force-worth override keys on security evidence only ─────────

#[test]
fn a_no_action_verdict_on_routine_evidence_stays_out_of_needs_attention() {
    // Before: risk=critical forced worth_reviewing=true over "No action needed".
    let deps = vec![input(
        "schemars (crates.io)",
        "1 unreviewed signal",
        false,
        false,
    )];
    let resp = r#"[{"id":1,"worth_reviewing":false,"recommendation":"Routine release, no action needed."}]"#;
    let out = parse_dep_assessments(resp, &deps);
    assert_eq!(out.len(), 1);
    assert!(
        !out[0].worth_reviewing,
        "no-action item must not land in Needs attention"
    );
}

#[test]
fn security_evidence_keeps_worth_reviewing_and_drops_a_contradicting_sentence() {
    let deps = vec![input(
        "openssl (crates.io)",
        "1 unreviewed signal",
        true,
        true,
    )];
    let resp =
        r#"[{"id":1,"worth_reviewing":false,"recommendation":"Looks fine, no action needed."}]"#;
    let out = parse_dep_assessments(resp, &deps);
    assert!(
        out[0].worth_reviewing,
        "the AI can add attention, never remove it"
    );
    assert!(!out[0].recommendation.to_lowercase().contains("no action"));
}

#[test]
fn parse_dep_assessments_joins_by_index_and_tolerates_garbage() {
    let deps = vec![
        input("libc (crates.io)", "no signals", false, false),
        input("react (npm)", "3 unreviewed signals", false, false),
    ];
    let resp = r#"Sure: [{"id":1,"worth_reviewing":false,"recommendation":"Stable libc, no action."},{"id":2,"worth_reviewing":true,"recommendation":"Review the v19 breaking changes."}]"#;
    let out = parse_dep_assessments(resp, &deps);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].dep_name, "libc (crates.io)");
    assert!(!out[0].worth_reviewing);
    assert_eq!(out[1].dep_name, "react (npm)");
    assert!(out[1].worth_reviewing);
    assert!(out[1].recommendation.contains("v19"));
    // Malformed response -> empty (UI shows "couldn't assess"), never panic.
    assert!(parse_dep_assessments("not json at all", &deps).is_empty());
    // Out-of-range / zero ids are dropped, not joined to a wrong dep.
    let oob = r#"[{"id":99,"worth_reviewing":true,"recommendation":"x"}]"#;
    assert!(parse_dep_assessments(oob, &deps).is_empty());
}

#[test]
fn hallucinated_signal_count_is_corrected_against_the_evidence_total() {
    let deps = vec![input(
        "react (npm)",
        "7 unreviewed signals: 7 new releases; never engaged",
        false,
        false,
    )];
    let resp = r#"[{"id":1,"worth_reviewing":true,"recommendation":"React has 73 unreviewed signals to skim."}]"#;
    let out = parse_dep_assessments(resp, &deps);
    assert!(
        out[0].recommendation.contains("7 unreviewed signals"),
        "{}",
        out[0].recommendation
    );
}
