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
            "API cost at {:.0}% of daily limit ({}c / {}c)",
            cost_percentage, cost_today, daily_limit
        );
        super::store_sun_alert("api_cost_monitor", "cost_warning", &alert_msg);
    }

    SunResult {
        success: true,
        message: format!("Tokens today: {}, Cost: {}c", tokens_today, cost_today),
        data: Some(serde_json::json!({
            "tokens_today": tokens_today,
            "cost_today_cents": cost_today,
            "daily_limit_cents": daily_limit,
            "cost_percentage": cost_percentage,
        })),
    }
}
