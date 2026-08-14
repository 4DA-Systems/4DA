// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Rerank outcome reporting and daily-budget pacing.
//!
//! Split out of `analysis_rerank.rs` on 2026-08-15: that file crossed the
//! 1000-line ERROR threshold in `scripts/check-file-sizes.cjs`. The gate runs
//! in CI's Frontend job, which `Detect changes` skips on Rust-only PRs — so the
//! violation reached main unnoticed and would have failed the next
//! frontend-touching PR rather than the one that caused it.
//!
//! The two concerns here travel together, and both are genuinely separate from
//! "run the LLM rerank": deciding whether a pass may spend budget, and
//! reporting honestly what a pass actually did.

use tracing::{info, warn};

/// Fraction of the daily budget released up-front, before the day has elapsed,
/// so the first pass after the UTC rollover can actually run.
const BUDGET_PACE_HEADROOM: f64 = 0.05;

/// How much of the daily budget may be spent by this point in the UTC day.
///
/// Without pacing the budget is consumed as fast as the scheduler can spend it.
/// Measured on the live app 2026-08-14: the analysis loop runs every ~10.5min at
/// ~11.4k tokens per rerank, so a 100k/day budget was exhausted by 01:36Z — the
/// day resets at 00:00Z, meaning the flagship reranker was DEAD for 22.4 of
/// every 24 hours while the logs still said "LLM rerank phase complete".
/// Pacing converts "9 passes clustered in 96 minutes, then nothing" into
/// "~9 passes spread evenly across the day".
pub(crate) fn budget_allowance_by_now(limit: u64, secs_into_utc_day: u64) -> u64 {
    if limit == 0 {
        return u64::MAX; // 0 means "no limit" throughout this module
    }
    let elapsed_fraction = (secs_into_utc_day as f64 / 86_400.0).clamp(0.0, 1.0);
    let share = (elapsed_fraction + BUDGET_PACE_HEADROOM).min(1.0);
    (limit as f64 * share) as u64
}

pub(crate) fn secs_into_utc_day() -> u64 {
    use chrono::Timelike;
    let now = chrono::Utc::now();
    u64::from(now.hour()) * 3600 + u64::from(now.minute()) * 60 + u64::from(now.second())
}

/// Why a rerank pass did not run.
///
/// Every variant is logged by the caller. Previously ALL of these paths returned
/// a bare `None` — several with no log line whatsoever — and the caller printed
/// "LLM rerank phase complete elapsed_ms=0", which reads as success. A skipped
/// rerank is now indistinguishable from a real one only if you don't read the
/// logs, which is the opposite of the old behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RerankSkip {
    /// Disabled in settings, or no usable LLM provider/key configured.
    Disabled,
    /// Daily token or cost ceiling already reached.
    BudgetExhausted {
        tokens_today: u64,
        token_limit: u64,
        cost_today_cents: u64,
        cost_limit_cents: u64,
    },
    /// Budget intact, but spending now would burn the day's allowance early.
    BudgetPaced {
        tokens_today: u64,
        allowed_by_now: u64,
    },
    NoContext,
    NoDatabase,
    /// No item cleared `rerank.min_embedding_score`.
    NoCandidates,
    UnsupportedTier(String),
    /// Every LLM batch failed.
    NoJudgments,
    /// Judge returned one identical score for every item (2026-08-11 incident).
    NonDiscriminating,
}

impl RerankSkip {
    /// Stable machine-readable tag for log filtering.
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::BudgetExhausted { .. } => "budget_exhausted",
            Self::BudgetPaced { .. } => "budget_paced",
            Self::NoContext => "no_context",
            Self::NoDatabase => "no_database",
            Self::NoCandidates => "no_candidates",
            Self::UnsupportedTier(_) => "unsupported_model_tier",
            Self::NoJudgments => "all_batches_failed",
            Self::NonDiscriminating => "non_discriminating_judge",
        }
    }

    /// Human-readable detail, including the numbers that explain the decision.
    pub(crate) fn detail(&self) -> String {
        match self {
            Self::Disabled => "rerank disabled or no LLM configured".to_string(),
            Self::BudgetExhausted {
                tokens_today,
                token_limit,
                cost_today_cents,
                cost_limit_cents,
            } => format!(
                "daily budget spent: {tokens_today}/{token_limit} tokens, {cost_today_cents}/{cost_limit_cents} cents — resets at 00:00 UTC"
            ),
            Self::BudgetPaced {
                tokens_today,
                allowed_by_now,
            } => format!(
                "pacing the daily budget: {tokens_today} tokens used, {allowed_by_now} allowed by this point in the UTC day"
            ),
            Self::NoContext => "no user context available to rank against".to_string(),
            Self::NoDatabase => "database unavailable".to_string(),
            Self::NoCandidates => {
                "no item cleared rerank.min_embedding_score this pass".to_string()
            }
            Self::UnsupportedTier(tier) => {
                format!("model tier '{tier}' cannot produce structured judgments")
            }
            Self::NoJudgments => "every LLM batch failed".to_string(),
            Self::NonDiscriminating => {
                "judge returned an identical score for every item — pass discarded".to_string()
            }
        }
    }
}

/// Result of a rerank attempt. `Skipped` carries WHY, so no caller can report a
/// no-op as a completed phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RerankOutcome {
    Reranked { judged: usize },
    Skipped(RerankSkip),
}

impl RerankOutcome {
    /// Emit the single honest log line for this outcome.
    pub(crate) fn log(&self, elapsed_ms: u128, phase: &str) {
        match self {
            Self::Reranked { judged } => {
                info!(
                    target: "4da::rerank",
                    phase,
                    judged,
                    elapsed_ms,
                    "LLM rerank applied"
                );
            }
            Self::Skipped(skip) => {
                warn!(
                    target: "4da::rerank",
                    phase,
                    reason = skip.reason(),
                    detail = %skip.detail(),
                    elapsed_ms,
                    "LLM rerank SKIPPED — items carry pipeline scores only, no LLM judgment"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{budget_allowance_by_now, secs_into_utc_day, RerankOutcome, RerankSkip};

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
        let secs_at_0136z = 3600 + 36 * 60 + 30;
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
}
