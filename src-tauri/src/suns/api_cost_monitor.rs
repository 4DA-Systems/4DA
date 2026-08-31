// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! API Cost Monitor Sun -- tracks LLM API usage costs (hourly).
//!
//! Ledger truth (2026-08-31 audit): this sun used to read the
//! settings-manager usage ledger against `rerank.daily_cost_limit_cents` —
//! but that ledger is fed ONLY by rerank passes and cloud embeddings
//! (`SettingsManager::record_usage` call sites), so after #553 moved the
//! bulk spend elsewhere its hourly "96% of daily limit" alerts measured a
//! sliver of the bill against the wrong limit. It now reads the persisted
//! `ai_usage` table (every provider call from BOTH `fourda` and
//! `fourda-engine`, per-feature tagged since #553) against the GLOBAL
//! `llm_limits.daily_cost_limit_cents` — the same limit the preflight cost
//! wall enforces. In-process counters are the fallback when the DB read
//! fails, never fabricated zeros presented as spend.

use super::SunResult;

/// Warn once cost passes this share of the daily limit. The alert itself is
/// deduped by `store_sun_alert` (24h window), so a standing condition stays
/// ONE row whose message tracks the latest observation.
const WARN_AT_PERCENT: f64 = 80.0;

pub fn execute() -> SunResult {
    let daily_limit = {
        let sm = crate::get_settings_manager().lock();
        sm.get().llm_limits.daily_cost_limit_cents
    };

    // Cross-process truth first (ai_usage covers the engine's spend too);
    // this process's live counters as the fallback.
    let (tokens_today, cost_today_cents) = match crate::llm::todays_persisted_usage() {
        Some((tokens, millicents)) => (tokens, millicents / 1000),
        None => {
            let (tokens, _) = crate::state::get_llm_token_usage();
            let (cost_cents, _) = crate::state::get_llm_cost_usage();
            (tokens, cost_cents)
        }
    };

    let cost_percentage = if daily_limit > 0 {
        (cost_today_cents as f64 / daily_limit as f64) * 100.0
    } else {
        0.0
    };

    if let Some(alert_msg) = build_cost_warning(cost_today_cents, daily_limit) {
        super::store_sun_alert("api_cost_monitor", "cost_warning", &alert_msg);
    }

    SunResult {
        success: true,
        message: format!("Tokens today: {tokens_today}, Cost: {cost_today_cents}c"),
        data: Some(serde_json::json!({
            "tokens_today": tokens_today,
            "cost_today_cents": cost_today_cents,
            "daily_limit_cents": daily_limit,
            "cost_percentage": cost_percentage,
        })),
    }
}

/// The warning line for a day's spend against the global limit, or `None`
/// while under [`WARN_AT_PERCENT`] (or with no limit configured). Pure so the
/// threshold is unit-testable.
fn build_cost_warning(cost_today_cents: u64, daily_limit_cents: u64) -> Option<String> {
    if daily_limit_cents == 0 {
        return None;
    }
    let pct = (cost_today_cents as f64 / daily_limit_cents as f64) * 100.0;
    if pct <= WARN_AT_PERCENT {
        return None;
    }
    Some(format!(
        "API cost at {pct:.0}% of daily limit ({cost_today_cents}c / {daily_limit_cents}c)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_only_past_eighty_percent() {
        assert_eq!(
            build_cost_warning(40, 50),
            None,
            "80% exactly is not yet a warning"
        );
        let msg = build_cost_warning(48, 50).expect("96% warns");
        assert!(msg.contains("96%"), "got: {msg}");
        assert!(msg.contains("48c / 50c"), "got: {msg}");
    }

    #[test]
    fn over_limit_names_the_overshoot() {
        let msg = build_cost_warning(53, 50).expect("106% warns");
        assert!(msg.contains("106%"), "got: {msg}");
    }

    #[test]
    fn no_limit_means_no_warning() {
        assert_eq!(build_cost_warning(10_000, 0), None);
        assert_eq!(build_cost_warning(0, 50), None);
    }
}
