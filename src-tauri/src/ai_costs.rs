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
// SQL Schema
// ============================================================================

#[allow(dead_code)]
pub(crate) const AI_USAGE_SQL: &str = "
CREATE TABLE IF NOT EXISTS ai_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    task_type TEXT NOT NULL,
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    estimated_cost_usd REAL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_ai_usage_provider ON ai_usage(provider, model);
CREATE INDEX IF NOT EXISTS idx_ai_usage_task ON ai_usage(task_type);
CREATE INDEX IF NOT EXISTS idx_ai_usage_date ON ai_usage(created_at);
";

// ============================================================================
// Core Functions
// ============================================================================

/// Estimate cost per API call based on known model pricing (per 1M tokens).
pub(crate) fn estimate_cost(provider: &str, model: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    let (cost_in_per_m, cost_out_per_m) = match (provider, model) {
        // OpenAI embeddings
        ("openai", m) if m.contains("text-embedding-3-small") => (0.02, 0.0),
        ("openai", m) if m.contains("text-embedding-3-large") => (0.13, 0.0),
        // OpenAI chat
        ("openai", m) if m.contains("gpt-4o-mini") => (0.15, 0.60),
        ("openai", m) if m.contains("gpt-4o") => (2.50, 10.00),
        ("openai", m) if m.contains("gpt-4.1") => (2.00, 8.00),
        // Anthropic
        ("anthropic", m) if m.contains("haiku") => (0.25, 1.25),
        ("anthropic", m) if m.contains("sonnet") => (3.00, 15.00),
        ("anthropic", m) if m.contains("opus") => (15.00, 75.00),
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
        .filter(|((provider, _), _)| provider != "ollama")
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
    let pattern = format!("{}%", p);
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
        .filter_map(|r| r.ok())
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
        .filter_map(|r| r.ok())
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
