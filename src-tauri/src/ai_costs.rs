// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! AI usage tracking and cost estimation.
//!
//! Records all LLM API calls with token counts and estimated costs.
//! Enables cost transparency and model routing recommendations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiUsageRecord {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub task_type: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub estimated_cost_usd: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiUsageSummary {
    pub period: String,
    pub total_cost_usd: f64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub by_provider: Vec<ProviderUsage>,
    pub by_task: Vec<TaskUsage>,
    pub recommendation: Option<ModelRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderUsage {
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub request_count: u32,
    pub tokens_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskUsage {
    pub task_type: String,
    pub cost_usd: f64,
    pub request_count: u32,
    pub avg_tokens: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModelRecommendation {
    pub current_provider: String,
    pub current_model: String,
    pub recommended_provider: String,
    pub recommended_model: String,
    pub estimated_savings_usd: f64,
    pub quality_match_pct: f32,
    pub reason: String,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Estimate cost per API call in USD.
///
/// Priced from the model registry first — the same source the daily cost cap
/// uses (`LLMClient::estimate_cost_millicents`). Until 2026-09-25 this ledger
/// priced by substring alone, so `ai_usage` disagreed with the cap: Opus 4.6/5
/// at Opus 4's $15/$75, Sonnet 5 at $3/$15, Fable at the $1/$3 fallback. And
/// because the cap is re-seeded from `ai_usage` at every restart, the wrong
/// ledger leaked into enforcement. The substring table below is only the
/// fallback for models the registry does not know.
pub(crate) fn estimate_cost(provider: &str, model: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    if let Some(millicents) = crate::model_registry::estimate_cost_millicents(
        provider,
        model,
        u64::from(tokens_in),
        u64::from(tokens_out),
    ) {
        // 1 USD = 100,000 millicents.
        return (millicents as f64 / 100_000.0 * 10_000.0).round() / 10_000.0;
    }
    substring_price_cost(provider, model, tokens_in, tokens_out)
}

/// Fallback pricing (per 1M tokens) by name pattern, for models the registry
/// does not know. More specific patterns come first.
fn substring_price_cost(provider: &str, model: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    let (cost_in_per_m, cost_out_per_m) = match (provider, model) {
        // OpenAI embeddings
        ("openai", m) if m.contains("text-embedding-3-small") => (0.02, 0.0),
        ("openai", m) if m.contains("text-embedding-3-large") => (0.13, 0.0),
        // OpenAI chat
        ("openai", m) if m.contains("gpt-4o-mini") => (0.15, 0.60),
        ("openai", m) if m.contains("gpt-4o") => (2.50, 10.00),
        ("openai", m) if m.contains("gpt-4.1-nano") => (0.10, 0.40),
        ("openai", m) if m.contains("gpt-4.1-mini") => (0.40, 1.60),
        ("openai", m) if m.contains("gpt-4.1") => (2.00, 8.00),
        // Anthropic
        ("anthropic", m) if m.contains("haiku") => (1.00, 5.00),
        ("anthropic", m) if m.contains("sonnet-5") => (2.00, 10.00),
        ("anthropic", m) if m.contains("sonnet") => (3.00, 15.00),
        ("anthropic", m) if m.contains("fable") => (10.00, 50.00),
        ("anthropic", m) if m.contains("opus-5-5") => (4.00, 20.00),
        // Opus 4.5 and later: $5/$25. Opus 4 / 4.1: $15/$75.
        ("anthropic", m) if m.contains("opus-4-5") || m.contains("opus-4-6") => (5.00, 25.00),
        ("anthropic", m) if m.contains("opus-4") => (15.00, 75.00),
        ("anthropic", m) if m.contains("opus") => (5.00, 25.00),
        // Local models (free)
        ("ollama", _) => (0.0, 0.0),
        // Conservative fallback
        _ => (1.00, 3.00),
    };

    let cost = (tokens_in as f64 * cost_in_per_m / 1_000_000.0)
        + (tokens_out as f64 * cost_out_per_m / 1_000_000.0);
    (cost * 10000.0).round() / 10000.0
}

/// Summarize usage records into a report.
pub(crate) fn summarize_usage(records: &[AiUsageRecord], period: &str) -> AiUsageSummary {
    let mut by_provider_map: HashMap<(String, String), (f64, u32, u64)> = HashMap::new();
    let mut by_task_map: HashMap<String, (f64, u32, u64)> = HashMap::new();
    let mut total_cost = 0.0;
    let mut total_in: u64 = 0;
    let mut total_out: u64 = 0;

    for r in records {
        total_cost += r.estimated_cost_usd;
        total_in += r.tokens_in as u64;
        total_out += r.tokens_out as u64;

        let pe = by_provider_map
            .entry((r.provider.clone(), r.model.clone()))
            .or_default();
        pe.0 += r.estimated_cost_usd;
        pe.1 += 1;
        pe.2 += (r.tokens_in + r.tokens_out) as u64;

        let te = by_task_map.entry(r.task_type.clone()).or_default();
        te.0 += r.estimated_cost_usd;
        te.1 += 1;
        te.2 += (r.tokens_in + r.tokens_out) as u64;
    }

    let by_provider: Vec<ProviderUsage> = by_provider_map
        .into_iter()
        .map(|((provider, model), (cost, count, tokens))| ProviderUsage {
            provider,
            model,
            cost_usd: cost,
            request_count: count,
            tokens_total: tokens,
        })
        .collect();

    let by_task: Vec<TaskUsage> = by_task_map
        .into_iter()
        .map(|(task_type, (cost, count, tokens))| TaskUsage {
            task_type,
            cost_usd: cost,
            request_count: count,
            avg_tokens: if count > 0 {
                tokens as f32 / count as f32
            } else {
                0.0
            },
        })
        .collect();

    let recommendation = generate_recommendation(records);

    AiUsageSummary {
        period: period.to_string(),
        total_cost_usd: total_cost,
        total_tokens_in: total_in,
        total_tokens_out: total_out,
        by_provider,
        by_task,
        recommendation,
    }
}

/// Generate a cost-saving recommendation based on usage patterns.
pub(crate) fn generate_recommendation(usage: &[AiUsageRecord]) -> Option<ModelRecommendation> {
    let mut costs: HashMap<(String, String), (f64, u32)> = HashMap::new();
    for r in usage {
        let e = costs
            .entry((r.provider.clone(), r.model.clone()))
            .or_default();
        e.0 += r.estimated_cost_usd;
        e.1 += 1;
    }

    let mut candidates: Vec<_> = costs
        .iter()
        .filter(|((provider, _), _)| provider.as_str() != "ollama")
        .collect();
    candidates.sort_by(|a, b| {
        b.1 .0
            .partial_cmp(&a.1 .0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(((provider, model), (cost, _))) = candidates.first() {
        if *cost > 0.10 {
            let is_embedding = model.contains("embed");
            if is_embedding {
                return Some(ModelRecommendation {
                    current_provider: provider.clone(),
                    current_model: model.clone(),
                    recommended_provider: "ollama".to_string(),
                    recommended_model: "nomic-embed-text".to_string(),
                    estimated_savings_usd: *cost,
                    quality_match_pct: 94.0,
                    reason: "Local embeddings via Ollama match cloud quality at 94% agreement. Zero ongoing cost.".to_string(),
                });
            }
        }
    }

    None
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_ai_usage_summary(period: Option<String>) -> crate::error::Result<serde_json::Value> {
    let p = period.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT id, provider, model, task_type, tokens_in, tokens_out, estimated_cost_usd, created_at \
         FROM ai_usage WHERE created_at LIKE ?1 ORDER BY created_at DESC",
    )?;
    let pattern = format!("{p}%");
    let records: Vec<AiUsageRecord> = stmt
        .query_map(rusqlite::params![pattern], |row| {
            Ok(AiUsageRecord {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                task_type: row.get(3)?,
                tokens_in: row.get(4)?,
                tokens_out: row.get(5)?,
                estimated_cost_usd: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    let summary = summarize_usage(&records, &p);
    Ok(serde_json::to_value(summary)?)
}

#[tauri::command]
pub fn get_ai_cost_estimate(
    provider: String,
    model: String,
    tokens_in: u32,
    tokens_out: u32,
) -> crate::error::Result<serde_json::Value> {
    let cost = estimate_cost(&provider, &model, tokens_in, tokens_out);
    Ok(serde_json::json!({
        "provider": provider,
        "model": model,
        "tokens_in": tokens_in,
        "tokens_out": tokens_out,
        "estimated_cost_usd": cost,
    }))
}

#[tauri::command]
pub fn get_ai_cost_recommendation() -> crate::error::Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT id, provider, model, task_type, tokens_in, tokens_out, estimated_cost_usd, created_at \
         FROM ai_usage ORDER BY created_at DESC LIMIT 500",
    )?;
    let records: Vec<AiUsageRecord> = stmt
        .query_map([], |row| {
            Ok(AiUsageRecord {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                task_type: row.get(3)?,
                tokens_in: row.get(4)?,
                tokens_out: row.get(5)?,
                estimated_cost_usd: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    match generate_recommendation(&records) {
        Some(rec) => Ok(serde_json::to_value(rec)?),
        None => Ok(serde_json::json!({
            "message": "No cost-saving recommendations at this time"
        })),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_embedding_cost() {
        let cost = estimate_cost("openai", "text-embedding-3-small", 1_000_000, 0);
        assert!((cost - 0.02).abs() < 0.001);
    }

    #[test]
    fn test_ollama_free() {
        let cost = estimate_cost("ollama", "nomic-embed-text", 1_000_000, 0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_anthropic_sonnet_cost() {
        let cost = estimate_cost("anthropic", "claude-sonnet-4-6", 1000, 500);
        assert!(cost > 0.0);
        // 1000 * 3.00/1M + 500 * 15.00/1M = 0.003 + 0.0075 = 0.0105
        assert!(cost < 0.02);
    }

    #[test]
    fn test_claude_five_prices_match_the_cap_ledger() {
        // Per 1M input / 1M output tokens.
        let cases = [
            ("claude-sonnet-5", 2.0, 10.0),
            ("claude-opus-5", 5.0, 25.0),
            ("claude-opus-5-5", 4.0, 20.0),
            ("claude-fable-5-1", 10.0, 50.0),
            ("claude-opus-4-6", 5.0, 25.0),
        ];
        for (model, input, output) in cases {
            let cost_in = estimate_cost("anthropic", model, 1_000_000, 0);
            let cost_out = estimate_cost("anthropic", model, 0, 1_000_000);
            assert!((cost_in - input).abs() < 1e-6, "{model} input {cost_in}");
            assert!(
                (cost_out - output).abs() < 1e-6,
                "{model} output {cost_out}"
            );
        }
    }

    #[test]
    fn test_substring_fallback_prices_unknown_claude_ids() {
        // Dated ids the registry does not carry still get their family's price.
        let fable = substring_price_cost("anthropic", "claude-fable-5-2-20270101", 1_000_000, 0);
        assert!((fable - 10.0).abs() < 1e-6);
        let opus4 = substring_price_cost("anthropic", "claude-opus-4-1-20250805", 1_000_000, 0);
        assert!((opus4 - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_summarize_empty() {
        let summary = summarize_usage(&[], "2026-03");
        assert_eq!(summary.total_cost_usd, 0.0);
        assert!(summary.by_provider.is_empty());
    }

    #[test]
    fn test_summarize_with_records() {
        let records = vec![
            AiUsageRecord {
                id: 1,
                provider: "openai".into(),
                model: "text-embedding-3-small".into(),
                task_type: "embedding".into(),
                tokens_in: 5000,
                tokens_out: 0,
                estimated_cost_usd: 0.0001,
                created_at: "2026-03-19".into(),
            },
            AiUsageRecord {
                id: 2,
                provider: "anthropic".into(),
                model: "claude-haiku-4-5".into(),
                task_type: "briefing".into(),
                tokens_in: 2000,
                tokens_out: 500,
                estimated_cost_usd: 0.001,
                created_at: "2026-03-19".into(),
            },
        ];

        let summary = summarize_usage(&records, "2026-03");
        assert_eq!(summary.by_provider.len(), 2);
        assert_eq!(summary.by_task.len(), 2);
        assert!(summary.total_cost_usd > 0.0);
    }

    #[test]
    fn test_recommendation_for_expensive_embeddings() {
        let records = vec![AiUsageRecord {
            id: 1,
            provider: "openai".into(),
            model: "text-embedding-3-small".into(),
            task_type: "embedding".into(),
            tokens_in: 5_000_000,
            tokens_out: 0,
            estimated_cost_usd: 0.50,
            created_at: "2026-03-19".into(),
        }];

        let rec = generate_recommendation(&records);
        assert!(rec.is_some());
        let rec = rec.unwrap();
        assert_eq!(rec.recommended_provider, "ollama");
        assert_eq!(rec.quality_match_pct, 94.0);
    }

    #[test]
    fn test_no_recommendation_for_cheap_usage() {
        let records = vec![AiUsageRecord {
            id: 1,
            provider: "openai".into(),
            model: "text-embedding-3-small".into(),
            task_type: "embedding".into(),
            tokens_in: 1000,
            tokens_out: 0,
            estimated_cost_usd: 0.001,
            created_at: "2026-03-19".into(),
        }];

        let rec = generate_recommendation(&records);
        assert!(rec.is_none());
    }
}
