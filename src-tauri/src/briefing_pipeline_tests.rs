// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! End-to-end briefing pipeline tests.
//!
//! These tests drive the quality → dedupe → groundedness chain as a
//! single unit so we lock in the observable behavior from the user's
//! perspective: "if I feed the pipeline junk, I get an abstention; if
//! I feed it real signal, I get a grounded synthesis-ready input list."
//!
//! They do NOT call the live LLM — the LLM call itself is mocked away
//! by invoking only the pre-synthesis and post-synthesis gates. This
//! keeps the tests:
//!
//! - **Deterministic** — no flaky "the LLM wrote slightly different
//!   prose today" false negatives.
//! - **Fast** — ~1 ms per test, no 14 s transmute subprocess spawns.
//! - **CI-friendly** — no API keys or Ollama dependency.
//!
//! The *actual* production bug that Screenshot_1976 surfaced is
//! captured as a regression test below so if the gate ever loosens,
//! this test will catch it in pre-commit.

#![cfg(test)]

use crate::briefing_dedupe::dedupe_briefing_items;
use crate::briefing_groundedness::validate_groundedness;
use crate::briefing_quality::is_briefing_worthy;
use crate::monitoring_briefing::BriefingItem;

// ============================================================================
// Helpers
// ============================================================================

fn mk(title: &str, source: &str, score: f32) -> BriefingItem {
    BriefingItem {
        title: title.to_string(),
        source_type: source.to_string(),
        score,
        signal_type: None,
        url: None,
        item_id: None,
        signal_priority: None,
        description: None,
        matched_deps: vec![],
        content_type: None,
        corroboration_count: 0,
        alt_sources: vec![],
    }
}

/// Run the pre-synthesis pipeline: quality gate → fuzzy dedupe.
/// Returns the items the LLM would see.
fn prepare_briefing_items(raw: Vec<BriefingItem>) -> Vec<BriefingItem> {
    let quality_filtered: Vec<BriefingItem> = raw
        .into_iter()
        .filter(|item| is_briefing_worthy(&item.title, &item.source_type).is_ok())
        .collect();
    dedupe_briefing_items(quality_filtered)
}

// ============================================================================
// Golden dataset — production-realistic input scenarios
// ============================================================================

/// Scenario 1: The "everything is garbage" morning.
/// All items fail the quality gate. Pipeline must produce an empty
/// list so the upstream caller can emit the abstention message.
#[test]
fn scenario_all_junk_yields_empty_list() {
    let raw = vec![
        mk(
            "Tip on marketing isnt back into job re-validate (5+ V/D/E DMS, run-off)",
            "rss",
            0.72,
        ),
        mk("We're hiring senior Rust engineers", "rss", 0.65),
        mk(
            "You won't believe what this developer discovered",
            "devto",
            0.58,
        ),
        mk("Home » Blog » Category » Archive » Tag » Item", "rss", 0.55),
        mk("Post navigation for April 2026 archives", "rss", 0.50),
    ];
    let out = prepare_briefing_items(raw);
    assert!(
        out.is_empty(),
        "all items should be rejected by quality gate, got {out:?}"
    );
}

/// Scenario 2: Mixed garbage + real signal.
/// Only the legitimate items survive.
#[test]
fn scenario_mixed_keeps_only_legitimate() {
    let raw = vec![
        mk(
            "Tip on marketing isnt back into job re-validate",
            "rss",
            0.75,
        ),
        mk(
            "Rust 1.80 released with const generics improvements",
            "hn",
            0.85,
        ),
        mk("We're hiring senior Rust engineers", "rss", 0.60),
        mk("CVE-2026-1234: Critical RCE in tokio 1.38", "cve", 0.90),
    ];
    let out = prepare_briefing_items(raw);
    assert_eq!(out.len(), 2);
    assert!(out.iter().any(|i| i.title.contains("Rust 1.80")));
    assert!(out.iter().any(|i| i.title.contains("CVE-2026-1234")));
}

/// Scenario 3: Cross-source duplicates.
/// HN + Reddit both carry the same story. Only the highest-scoring
/// variant survives the dedupe pass.
#[test]
fn scenario_cross_source_duplicates_collapse() {
    let raw = vec![
        mk(
            "React 19.2.3 released with concurrent rendering",
            "hackernews",
            0.78,
        ),
        mk(
            "React 19.2.3 released with concurrent rendering",
            "reddit",
            0.42,
        ),
        mk(
            "React 19.2.3 released with concurrent rendering!",
            "lobsters",
            0.65,
        ),
        mk("Postgres 17 ships pg_logical v2", "hackernews", 0.61),
    ];
    let out = prepare_briefing_items(raw);
    assert_eq!(out.len(), 2);
    // The highest-scoring React item must win
    let react = out.iter().find(|i| i.title.contains("React")).unwrap();
    assert_eq!(react.source_type, "hackernews");
    assert!((react.score - 0.78).abs() < 0.001);
}

/// Scenario 4: Legitimate signal with no cross-source overlap.
/// Everything passes through untouched, preserving the score order.
#[test]
fn scenario_pristine_input_preserves_everything() {
    let raw = vec![
        mk(
            "CVE-2026-7777: kernel privilege escalation via eBPF",
            "cve",
            0.95,
        ),
        mk(
            "Postgres 17 ships pg_logical v2 with streaming",
            "hackernews",
            0.82,
        ),
        mk(
            "TanStack Start now supports React Server Components",
            "hn",
            0.78,
        ),
        mk(
            "Tauri 2.1 released with improved IPC performance",
            "devto",
            0.74,
        ),
        mk(
            "tokio 1.38.5 patches critical RCE in hyper integration",
            "cve",
            0.91,
        ),
    ];
    let out = prepare_briefing_items(raw);
    assert_eq!(out.len(), 5);
    // Score-descending order preserved
    for w in out.windows(2) {
        assert!(w[0].score >= w[1].score);
    }
}

// ============================================================================
// Groundedness — post-synthesis validation
// ============================================================================

/// Scenario 5: The "VAR and Stripe" hallucination regression.
/// If the LLM emits a recommendation about vendors not in the corpus,
/// the groundedness check must flag it.
#[test]
fn scenario_var_and_stripe_hallucination_is_caught() {
    let corpus = vec![
        "TanStack Start now supports React Server Components".to_string(),
        "npm: react v19.2.3".to_string(),
        "npm: typescript v5.6".to_string(),
    ];
    let bad_output = "SITUATION: React Server Components gain TanStack Start support.\n\n\
                      PRIORITY: Recommend update of your strategy for non-test architecture, \
                      including a 5+ year migration from VAR and Stripe.";
    let report = validate_groundedness(bad_output, &corpus);
    assert!(
        !report.ungrounded_terms.is_empty(),
        "VAR/Stripe should be flagged"
    );
    let found_stripe = report
        .ungrounded_terms
        .iter()
        .any(|t| t.eq_ignore_ascii_case("stripe"));
    assert!(
        found_stripe,
        "Stripe not flagged in {:?}",
        report.ungrounded_terms
    );
}

/// Scenario 6: A legitimate grounded briefing passes.
#[test]
fn scenario_grounded_synthesis_is_accepted() {
    let corpus = vec![
        "tokio 1.38 security advisory released".to_string(),
        "Postgres 17 ships pg_logical v2 with streaming".to_string(),
    ];
    let good_output = "SITUATION: Tokio released a security advisory [1]. \
                       Postgres 17 ships pg_logical v2 [2].\n\n\
                       PRIORITY: Upgrade tokio [1].";
    let report = validate_groundedness(good_output, &corpus);
    assert!(
        report.is_acceptable(0.65),
        "grounded output should pass: confidence {}, ungrounded {:?}",
        report.confidence,
        report.ungrounded_terms
    );
}

/// Scenario 7: Abstention text is intentionally generic.
/// The "Low signal" line has zero salient terms, so the specificity
/// floor correctly rejects it. This is expected — in production, the
/// abstention is a backend-generated fallback that bypasses groundedness
/// validation entirely. It's never sent through is_acceptable().
#[test]
fn scenario_abstention_is_generic_and_would_be_rejected() {
    let corpus: Vec<String> = vec![];
    let abstention = "Low signal — no noteworthy intelligence overnight.";
    let report = validate_groundedness(abstention, &corpus);
    assert!(
        !report.is_acceptable(0.8),
        "abstention has 0 salient terms — specificity floor should reject"
    );
}

/// Scenario 8: The specific production-screenshot bug.
/// This is the exact string observed in Screenshot_1976 plus the
/// source items visible in that screenshot. Used as a hard regression
/// gate — if this test ever passes without flagging, we've regressed.
#[test]
fn screenshot_1976_regression_guard() {
    let corpus_from_screenshot = vec![
        "npm: react v19.2.3".to_string(),
        "TanStack Start now support React Server Components and Composite".to_string(),
        "npm: typescript v5.6".to_string(),
    ];
    let observed_hallucination =
        "TanStack Start now support React Server Components and Composite. \
                                  Recommend update of your strategy for non-test architecture, \
                                  including a 5+ year migration from VAR and Stripe";
    let report = validate_groundedness(observed_hallucination, &corpus_from_screenshot);
    let ungrounded_lowered: Vec<String> = report
        .ungrounded_terms
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    // "Stripe" MUST appear in ungrounded (it doesn't exist in the corpus).
    assert!(
        ungrounded_lowered.iter().any(|s| s == "stripe"),
        "Stripe not flagged: {:?}",
        report.ungrounded_terms
    );
}
