// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Circuit-breaker, budget-pacing and skip-reporting tests for `analysis_rerank`.

use super::{
    budget_allowance_by_now, rerank_pass_is_uniform, secs_into_utc_day, RerankOutcome, RerankSkip,
};
use crate::llm::RelevanceJudgment;

fn j(confidence: f32) -> RelevanceJudgment {
    RelevanceJudgment {
        item_id: "x".to_string(),
        relevant: confidence >= 0.6,
        confidence,
        raw_confidence: None,
        reasoning: String::new(),
        key_connections: vec![],
    }
}

#[test]
fn uniform_full_pass_trips_the_breaker() {
    // The incident signature: 48 judgments, every confidence 1.0.
    let judgments: Vec<_> = (0..48).map(|_| j(1.0)).collect();
    assert!(rerank_pass_is_uniform(&judgments));
}

#[test]
fn discriminating_pass_does_not_trip() {
    let mut judgments: Vec<_> = (0..47).map(|_| j(0.2)).collect();
    judgments.push(j(0.8));
    assert!(!rerank_pass_is_uniform(&judgments));
}

#[test]
fn small_pass_is_exempt() {
    let judgments: Vec<_> = (0..7).map(|_| j(1.0)).collect();
    assert!(
        !rerank_pass_is_uniform(&judgments),
        "fewer than 8 judgments cannot prove non-discrimination"
    );
}

// ── Budget pacing ────────────────────────────────────────────────────
// Reproduces the live 2026-08-14 failure: a 100k/day budget consumed by
// 01:36Z, leaving the reranker dead for 22.4 of every 24 hours.

const DAY: u64 = 86_400;

#[test]
fn pacing_releases_headroom_at_the_utc_rollover() {
    // At 00:00Z some budget must be available or the first pass of the day
    // could never run.
    let allowed = budget_allowance_by_now(100_000, 0);
    assert_eq!(allowed, 5_000, "expected the 5% up-front headroom");
    assert!(allowed > 0);
}

#[test]
fn pacing_would_have_blocked_the_live_burn() {
    // Live numbers: 102,677 tokens spent by 01:36:30Z against a 100k limit.
    let secs_at_0136z = 1 * 3600 + 36 * 60 + 30;
    let allowed = budget_allowance_by_now(100_000, secs_at_0136z);

    assert!(
        allowed < 102_677,
        "pacing must forbid the observed burn: allowed {allowed} at 01:36Z"
    );
    // One pass (~11.4k) fits inside the headroom; the run-away does not.
    assert!(
        allowed >= 11_400,
        "a single pass must still fit early in the day, got {allowed}"
    );
}

#[test]
fn pacing_grows_monotonically_across_the_day() {
    let mut previous = 0;
    for hour in 0..=24 {
        let allowed = budget_allowance_by_now(100_000, hour * 3600);
        assert!(
            allowed >= previous,
            "allowance must never shrink: hour {hour} gave {allowed} after {previous}"
        );
        previous = allowed;
    }
}

#[test]
fn pacing_reaches_the_full_budget_by_end_of_day() {
    assert_eq!(budget_allowance_by_now(100_000, DAY), 100_000);
    // And is clamped, never exceeding the limit even past the boundary.
    assert_eq!(budget_allowance_by_now(100_000, DAY * 2), 100_000);
}

#[test]
fn pacing_treats_zero_limit_as_unlimited() {
    // 0 means "no limit" everywhere else in this module; pacing must agree,
    // otherwise setting no limit would paradoxically disable reranking.
    assert_eq!(budget_allowance_by_now(0, 0), u64::MAX);
    assert_eq!(budget_allowance_by_now(0, DAY), u64::MAX);
}

#[test]
fn secs_into_utc_day_is_within_bounds() {
    assert!(secs_into_utc_day() < DAY);
}

// ── Skip reporting ───────────────────────────────────────────────────

#[test]
fn every_skip_reason_is_tagged_and_explained() {
    let skips = [
        RerankSkip::Disabled,
        RerankSkip::BudgetExhausted {
            tokens_today: 102_677,
            token_limit: 100_000,
            cost_today_cents: 51,
            cost_limit_cents: 50,
        },
        RerankSkip::BudgetPaced {
            tokens_today: 50_000,
            allowed_by_now: 12_000,
        },
        RerankSkip::NoContext,
        RerankSkip::NoDatabase,
        RerankSkip::NoCandidates,
        RerankSkip::UnsupportedTier("basic".to_string()),
        RerankSkip::NoJudgments,
        RerankSkip::NonDiscriminating,
    ];

    let mut tags = std::collections::HashSet::new();
    for skip in &skips {
        assert!(!skip.reason().is_empty(), "{skip:?} has no tag");
        assert!(!skip.detail().is_empty(), "{skip:?} has no detail");
        assert!(
            tags.insert(skip.reason()),
            "{skip:?} reuses tag '{}' — skips must be distinguishable in logs",
            skip.reason()
        );
    }
}

#[test]
fn budget_exhausted_detail_carries_the_actual_numbers() {
    let detail = RerankSkip::BudgetExhausted {
        tokens_today: 102_677,
        token_limit: 100_000,
        cost_today_cents: 51,
        cost_limit_cents: 50,
    }
    .detail();

    // The operator must be able to read the cause straight out of the log
    // rather than going to look up usage.json.
    assert!(detail.contains("102677"), "missing spend: {detail}");
    assert!(detail.contains("100000"), "missing limit: {detail}");
    assert!(detail.contains("51"), "missing cost: {detail}");
    assert!(detail.contains("UTC"), "missing reset window: {detail}");
}

#[test]
fn a_skipped_outcome_is_never_a_reranked_one() {
    let skipped = RerankOutcome::Skipped(RerankSkip::Disabled);
    let done = RerankOutcome::Reranked { judged: 48 };
    assert_ne!(skipped, done);
    assert!(matches!(done, RerankOutcome::Reranked { judged } if judged == 48));
}
