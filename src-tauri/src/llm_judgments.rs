// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! LLM judgment engine — evaluates source items at ingestion time for Tier 2 intelligence.
//!
//! After ingestion, items scoring above a threshold are sent to the user's configured LLM
//! for relevance evaluation with full ACE context. Judgments are stored in `llm_judgments`
//! and later read by preemption/blind_spots feeds.

use crate::db::Database;
use crate::error::Result;
use crate::llm::{LLMClient, Message};
use crate::settings::LLMProvider;
use serde::Deserialize;
use tracing::{debug, info, warn};

const PROMPT_VERSION: &str = "v1";
const INGESTION_THRESHOLD: f64 = 0.25;
const BATCH_SIZE: usize = 5;

#[derive(Debug, Deserialize)]
struct JudgmentResponse {
    relevance: Option<f64>,
    explanation: Option<String>,
    actions: Option<Vec<String>>,
    confidence: Option<f64>,
}

/// Evaluate a batch of source items and store judgments.
/// Called after ingestion when new items arrive.
pub(crate) async fn evaluate_pending_items(db: &Database) -> Result<usize> {
    if crate::state::is_llm_limit_reached() {
        debug!(target: "4da::llm_judgments", "LLM daily limit reached, skipping judgment batch");
        return Ok(0);
    }

    let unjudged = db
        .get_unjudged_item_ids(INGESTION_THRESHOLD, BATCH_SIZE * 4)
        .map_err(|e| {
            crate::error::FourDaError::Internal(format!("Failed to get unjudged items: {e}"))
        })?;

    if unjudged.is_empty() {
        return Ok(0);
    }

    let llm_settings = get_llm_settings();
    let Some(provider) = llm_settings else {
        debug!(target: "4da::llm_judgments", "No LLM provider configured, skipping judgments");
        return Ok(0);
    };

    let model_name = provider.model.clone();
    let client = LLMClient::new(provider);
    let user_context = crate::adversarial::build_user_context_summary();

    let mut judged = 0;
    for chunk in unjudged.chunks(BATCH_SIZE) {
        let items = load_items_for_judgment(db, chunk)?;
        if items.is_empty() {
            continue;
        }

        match evaluate_batch(&client, &items, &user_context).await {
            Ok(results) => {
                for (item_id, response) in results {
                    let relevance = response.relevance.unwrap_or(0.0).clamp(0.0, 1.0);
                    let explanation = response.explanation.unwrap_or_default();
                    let confidence = response.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
                    let actions_json = response
                        .actions
                        .map(|a| serde_json::to_string(&a).unwrap_or_default());

                    if let Err(e) = db.upsert_llm_judgment(
                        item_id,
                        relevance,
                        &explanation,
                        actions_json.as_deref(),
                        confidence,
                        &model_name,
                        PROMPT_VERSION,
                    ) {
                        warn!(target: "4da::llm_judgments", error = %e, item_id, "Failed to store judgment");
                    } else {
                        judged += 1;
                    }
                }
            }
            Err(e) => {
                warn!(target: "4da::llm_judgments", error = %e, "Batch evaluation failed");
                break;
            }
        }
    }

    if judged > 0 {
        info!(target: "4da::llm_judgments", judged, "Stored LLM judgments for ingested items");
    }

    Ok(judged)
}

// ============================================================================
// Internal Types
// ============================================================================

struct ItemForJudgment {
    id: i64,
    title: String,
    content: Option<String>,
    source_type: String,
    relevance_score: f64,
}

// ============================================================================
// Helpers
// ============================================================================

/// Get the LLM provider from settings.
/// Returns `None` if no provider is configured or the API key is missing
/// (for non-Ollama providers).
fn get_llm_settings() -> Option<LLMProvider> {
    let mgr = crate::get_settings_manager();
    let guard = mgr.lock();
    let provider = guard.get().llm.clone();

    // Ollama doesn't need an API key; cloud providers do
    if provider.provider != "ollama" && provider.api_key.is_empty() {
        return None;
    }

    Some(provider)
}

fn load_items_for_judgment(db: &Database, ids: &[i64]) -> Result<Vec<ItemForJudgment>> {
    let conn = db.conn.lock();
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, title, content, source_type, COALESCE(relevance_score, 0.0)
         FROM source_items WHERE id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| {
        crate::error::FourDaError::Internal(format!("Failed to prepare query: {e}"))
    })?;

    let params: Vec<&dyn rusqlite::types::ToSql> = ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok(ItemForJudgment {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                source_type: row.get(3)?,
                relevance_score: row.get(4)?,
            })
        })
        .map_err(|e| crate::error::FourDaError::Internal(format!("Failed to query items: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::llm_judgments", error = %e, "Failed to read item row"),
        }
    }
    Ok(items)
}

async fn evaluate_batch(
    client: &LLMClient,
    items: &[ItemForJudgment],
    user_context: &str,
) -> Result<Vec<(i64, JudgmentResponse)>> {
    let items_block: String = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let content_preview = item
                .content
                .as_deref()
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let truncated: String = c.chars().take(500).collect();
                    format!("\nContent: {truncated}")
                })
                .unwrap_or_default();
            format!(
                "--- Item {} (id={}) ---\nTitle: {}\nSource: {}\nScore: {:.2}{content_preview}",
                i + 1,
                item.id,
                item.title,
                item.source_type,
                item.relevance_score
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = format!(
        "You are an intelligence relevance evaluator for a developer tool. \
         Evaluate each item's relevance to this specific user.\n\n\
         {user_context}\n\n\
         For each item, respond with a JSON array where each element has:\n\
         - \"id\": the item id\n\
         - \"relevance\": 0.0-1.0 (how relevant to THIS user specifically)\n\
         - \"explanation\": one sentence explaining WHY this matters to this user \
           (must reference a specific fact from the item AND the user's context)\n\
         - \"actions\": array of suggested actions (e.g. [\"review_security\", \"investigate\"])\n\
         - \"confidence\": 0.0-1.0 (how confident you are in your relevance assessment)\n\n\
         Rules:\n\
         - Explanation MUST reference something specific from the item (a package name, CVE, etc.) \
           AND something from the user's context\n\
         - Generic explanations like \"relevant to your interests\" score 0 confidence\n\
         - If the item has no clear connection to the user's stack/topics, relevance should be < 0.3\n\
         - Return ONLY a valid JSON array, no other text"
    );

    let user_msg = format!("Evaluate these items:\n\n{items_block}");

    let messages = vec![Message {
        role: "user".to_string(),
        content: user_msg,
    }];

    let response = client.complete(&system_prompt, messages).await?;

    let text = response.content.trim();
    // Extract JSON from potential markdown code block
    let json_text = if text.starts_with("```") {
        text.lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text.to_string()
    };

    #[derive(Deserialize)]
    struct BatchItem {
        id: Option<i64>,
        relevance: Option<f64>,
        explanation: Option<String>,
        actions: Option<Vec<String>>,
        confidence: Option<f64>,
    }

    let parsed: Vec<BatchItem> = serde_json::from_str(&json_text)?;

    let results: Vec<(i64, JudgmentResponse)> = parsed
        .into_iter()
        .filter_map(|bi| {
            let id = bi.id?;
            Some((
                id,
                JudgmentResponse {
                    relevance: bi.relevance,
                    explanation: bi.explanation,
                    actions: bi.actions,
                    confidence: bi.confidence,
                },
            ))
        })
        .collect();

    Ok(results)
}
