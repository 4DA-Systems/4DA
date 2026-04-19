// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! API Cost Monitor Sun -- tracks LLM API usage costs (hourly).

use super::SunResult;

pub fn execute() -> SunResult {
    let (tokens_today, cost_today, daily_limit) = {
        let sm = crate::get_settings_manager().lock();
        let usage = sm.get_usage();
        let limit = sm.get().rerank.daily_cost_limit_cents;
        (usage.tokens_today, usage.cost_today_cents, limit)
    };

    let cost_percentage = if daily_limit > 0 {
        (cost_today as f64 / daily_limit as f64) * 100.0
    } else {
        0.0
    };

    // Alert if approaching limit
    if cost_percentage > 80.0 {
        let alert_msg = format!(
            "API cost at {cost_percentage:.0}% of daily limit ({cost_today}c / {daily_limit}c)"
        );
        super::store_sun_alert("api_cost_monitor", "cost_warning", &alert_msg);
    }

    SunResult {
        success: true,
        message: format!("Tokens today: {tokens_today}, Cost: {cost_today}c"),
        data: Some(serde_json::json!({
            "tokens_today": tokens_today,
            "cost_today_cents": cost_today,
            "daily_limit_cents": daily_limit,
            "cost_percentage": cost_percentage,
        })),
    }
}
