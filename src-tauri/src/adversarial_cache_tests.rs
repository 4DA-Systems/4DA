// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Verdict cache and fail-closed batch settlement (audit L2, 2026-10-12).
//!
//! The model is injected as a closure that counts its calls, so every test
//! states exactly how many cloud calls a refresh would have made.

use std::cell::Cell;

use super::super::batch::{deliberate_batch, holds_back_unjudged, settle_from_cache, BatchOutcome};
use super::super::{DeliberationOutcome, DeliberationVerdict, ReasoningChain, Unavailable};
use super::*;
use crate::evidence::{Confidence, EvidenceItem, EvidenceKind, LensHints, Urgency};

const NOW: i64 = 1_760_000_000;
const PROMPT: &str = "system prompt";

fn item(id: &str, title: &str, confidence: Confidence, urgency: Urgency) -> EvidenceItem {
    EvidenceItem {
        id: id.to_string(),
        kind: EvidenceKind::Alert,
        title: title.to_string(),
        explanation: format!("3 sources discussing {title} over 2 days."),
        confidence,
        urgency,
        reversibility: None,
        evidence: vec![],
        evidence_total: None,
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

/// A signal chain: heuristic, Medium, a fresh uuid-style id every refresh.
fn chain(id: &str, title: &str) -> EvidenceItem {
    item(id, title, Confidence::heuristic(0.6), Urgency::Medium)
}

fn verdict(should_surface: bool) -> DeliberationVerdict {
    DeliberationVerdict {
        should_surface,
        adjusted_confidence: 0.7,
        grounded_explanation: "Grounded because the stack uses it.".to_string(),
        signal_argument: String::new(),
        noise_argument: String::new(),
        reasoning_chain: ReasoningChain {
            claim: String::new(),
            evidence_points: vec![],
            connection: String::new(),
            conclusion: String::new(),
        },
    }
}

fn keyer(item: &EvidenceItem) -> String {
    material_key(item, PROMPT, "anthropic", "claude-sonnet-4-6")
}

/// Run a batch against a model that answers `answer` and counts its calls.
fn run(
    items: Vec<EvidenceItem>,
    cache: &mut VerdictCache,
    now: i64,
    answer: DeliberationOutcome,
) -> (BatchOutcome, usize) {
    let calls = Cell::new(0usize);
    let outcome =
        tauri::async_runtime::block_on(deliberate_batch(items, cache, keyer, now, |_item| {
            calls.set(calls.get() + 1);
            let answer = answer.clone();
            async move { answer }
        }));
    (outcome, calls.get())
}

fn ids(outcome: &BatchOutcome) -> Vec<&str> {
    outcome.passed.iter().map(|i| i.id.as_str()).collect()
}

#[test]
fn cache_hit_makes_no_model_call() {
    let mut cache = VerdictCache::default();
    let (first, calls) = run(
        vec![chain("chain-a", "sandbox")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(true)),
    );
    assert_eq!(calls, 1, "first sight of the material is judged");
    assert_eq!(ids(&first), vec!["chain-a"]);

    // Next refresh: same material, NEW chain id (ids are minted per refresh).
    let (second, calls) = run(
        vec![chain("chain-b", "sandbox")],
        &mut cache,
        NOW + 600,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    assert_eq!(calls, 0, "unchanged material must not be sent again");
    assert_eq!(second.stats.cache_hits, 1);
    assert_eq!(ids(&second), vec!["chain-b"], "the stored approval applies");
    assert_eq!(
        second.passed[0].explanation, "Grounded because the stack uses it.",
        "the stored grounded explanation is adopted"
    );
}

#[test]
fn changed_material_is_judged_again() {
    let mut cache = VerdictCache::default();
    run(
        vec![chain("chain-a", "sandbox")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(true)),
    );
    let mut changed = chain("chain-b", "sandbox");
    changed.explanation = "4 sources discussing sandbox over 3 days.".to_string();
    let (outcome, calls) = run(
        vec![changed],
        &mut cache,
        NOW + 600,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    assert_eq!(calls, 1, "new material means a new verdict");
    assert!(
        outcome.passed.is_empty(),
        "the new verdict (reject) applies"
    );
}

#[test]
fn key_covers_model_and_prompt_but_not_the_item_id() {
    let a = chain("chain-1", "next");
    let b = chain("chain-2", "next");
    assert_eq!(keyer(&a), keyer(&b), "ids are per-refresh noise");
    assert_ne!(
        material_key(&a, PROMPT, "anthropic", "claude-sonnet-4-6"),
        material_key(&a, PROMPT, "anthropic", "claude-opus-4-1"),
        "another model re-judges"
    );
    assert_ne!(
        material_key(&a, PROMPT, "anthropic", "claude-sonnet-4-6"),
        material_key(&a, "edited prompt", "anthropic", "claude-sonnet-4-6"),
        "a prompt edit re-judges"
    );
    let mut escalated = a.clone();
    escalated.urgency = Urgency::High;
    assert_ne!(
        keyer(&a),
        keyer(&escalated),
        "urgency is part of what is judged"
    );
}

#[test]
fn rejected_chain_stays_suppressed_when_the_budget_is_spent() {
    let mut cache = VerdictCache::default();
    let (judged, _) = run(
        vec![chain("chain-a", "table")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    assert!(judged.passed.is_empty());

    // Later the same day the background budget share is gone.
    let (later, calls) = run(
        vec![chain("chain-b", "table")],
        &mut cache,
        NOW + 3600,
        DeliberationOutcome::Unavailable(Unavailable::Budget),
    );
    assert_eq!(calls, 0);
    assert!(
        later.passed.is_empty(),
        "a rejected chain must not come back when the budget runs out"
    );
    assert_eq!(later.stats.filtered, 1);

    // The same holds through a provider failure.
    let (failing, _) = run(
        vec![chain("chain-c", "table")],
        &mut cache,
        NOW + 7200,
        DeliberationOutcome::Unavailable(Unavailable::LlmError),
    );
    assert!(failing.passed.is_empty());
}

#[test]
fn unjudged_heuristics_are_held_back_but_grounded_items_still_surface() {
    let mut cache = VerdictCache::default();
    let items = vec![
        item(
            "osv-pkg-lodash-npm",
            "lodash: 2 known vulnerabilities",
            Confidence::osv_verified(0.9),
            Urgency::High,
        ),
        item(
            "llm-42",
            "next.js middleware bypass",
            Confidence::llm_assessed(0.7),
            Urgency::Medium,
        ),
        chain("chain-a", "sandbox"),
        chain("chain-b", "backend"),
        item(
            "chain-c",
            "vm2 escape",
            Confidence::heuristic(0.8),
            Urgency::Critical,
        ),
    ];
    let (outcome, calls) = run(
        items,
        &mut cache,
        NOW,
        DeliberationOutcome::Unavailable(Unavailable::Budget),
    );
    assert_eq!(calls, 1, "a budget stop is not retried for every item");
    assert_eq!(
        ids(&outcome),
        vec!["osv-pkg-lodash-npm", "llm-42", "chain-c"],
        "OSV-verified, LLM-assessed and corroborated Critical items surface; unvetted chains wait"
    );
    assert_eq!(outcome.stats.held_back, 2);
    assert_eq!(outcome.stats.unavailable, Some(Unavailable::Budget));
    assert_eq!(cache.len(), 0, "nothing is remembered without a verdict");
}

#[test]
fn an_unreadable_answer_does_not_stop_the_batch() {
    let mut cache = VerdictCache::default();
    let (_, calls) = run(
        vec![chain("chain-a", "a"), chain("chain-b", "b")],
        &mut cache,
        NOW,
        DeliberationOutcome::Unavailable(Unavailable::Unparseable),
    );
    assert_eq!(calls, 2);
}

#[test]
fn no_model_configured_passes_items_as_before() {
    let mut cache = VerdictCache::default();
    let (outcome, _) = run(
        vec![chain("chain-a", "sandbox")],
        &mut cache,
        NOW,
        DeliberationOutcome::NotConfigured,
    );
    assert_eq!(ids(&outcome), vec!["chain-a"]);
}

#[test]
fn verdicts_expire_after_the_ttl() {
    let mut cache = VerdictCache::default();
    run(
        vec![chain("chain-a", "sandbox")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    let key = keyer(&chain("x", "sandbox"));
    assert!(cache.fresh(&key, NOW + VERDICT_TTL_SECS - 1).is_some());
    assert!(cache.fresh(&key, NOW + VERDICT_TTL_SECS).is_none());

    let (outcome, calls) = run(
        vec![chain("chain-b", "sandbox")],
        &mut cache,
        NOW + VERDICT_TTL_SECS,
        DeliberationOutcome::Verdict(verdict(true)),
    );
    assert_eq!(calls, 1, "an expired verdict is judged again");
    assert_eq!(ids(&outcome), vec!["chain-b"]);

    assert_eq!(cache.prune(NOW + 2 * VERDICT_TTL_SECS), 1);
    assert_eq!(cache.len(), 0);
}

#[test]
fn fast_feed_applies_stored_verdicts_without_a_model() {
    let mut cache = VerdictCache::default();
    run(
        vec![chain("chain-a", "rejected"), chain("chain-b", "approved")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    // Flip the stored verdict for "approved".
    cache.insert(keyer(&chain("x", "approved")), verdict(true), NOW);
    let outcome = settle_from_cache(
        vec![
            chain("chain-1", "rejected"),
            chain("chain-2", "approved"),
            chain("chain-3", "never seen"),
        ],
        &cache,
        keyer,
        NOW + 60,
    );
    assert_eq!(ids(&outcome), vec!["chain-2"]);
    assert_eq!(outcome.stats.filtered, 1);
    assert_eq!(outcome.stats.held_back, 1);
}

#[test]
fn verdicts_persist_across_loads() {
    let db = crate::test_utils::test_db();
    let mut cache = VerdictCache::load(&db);
    run(
        vec![chain("chain-a", "sandbox")],
        &mut cache,
        NOW,
        DeliberationOutcome::Verdict(verdict(false)),
    );
    cache.save(&db, NOW);

    let mut reloaded = VerdictCache::load(&db);
    let (outcome, calls) = run(
        vec![chain("chain-b", "sandbox")],
        &mut reloaded,
        NOW + 60,
        DeliberationOutcome::Unavailable(Unavailable::Budget),
    );
    assert_eq!(calls, 0, "a restart does not re-judge");
    assert!(outcome.passed.is_empty());
}

#[test]
fn corrupt_store_starts_empty() {
    let db = crate::test_utils::test_db();
    db.set_kv(KV_KEY, "{not json").expect("write");
    assert_eq!(VerdictCache::load(&db).len(), 0);
}

#[test]
fn hold_back_rule_spares_vetted_and_escalated_items() {
    assert!(holds_back_unjudged(&chain("c", "t")));
    assert!(!holds_back_unjudged(&item(
        "l",
        "t",
        Confidence::llm_assessed(0.5),
        Urgency::Watch
    )));
    assert!(!holds_back_unjudged(&item(
        "h",
        "t",
        Confidence::heuristic(0.5),
        Urgency::High
    )));
}
