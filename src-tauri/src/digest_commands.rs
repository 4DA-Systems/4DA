//! AI Briefing Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains AI briefing synthesis.
//! Digest configuration, briefing cache, and decision context are in digest_config.rs.

use tracing::{error, info};

use crate::error::Result;
use crate::scoring::get_ace_context;
use crate::{get_analysis_state, get_database, get_settings_manager};

// Re-export so that `crate::digest_commands::get_latest_briefing_text` still resolves
// for callers that haven't been updated — canonical home is digest_config.
pub(crate) use crate::digest_config::get_latest_briefing_text;

// ============================================================================
// AI Briefing Commands
// ============================================================================

/// Get the latest persisted briefing from the database (survives restarts)
#[tauri::command]
pub async fn get_latest_briefing() -> Result<serde_json::Value> {
    crate::settings::require_pro_feature("get_latest_briefing")?;
    let db = get_database()?;
    match db.get_latest_briefing() {
        Ok(Some((content, model, item_count, created_at))) => Ok(serde_json::json!({
            "content": content,
            "model": model,
            "item_count": item_count,
            "created_at": created_at,
        })),
        Ok(None) => Ok(serde_json::Value::Null),
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to load persisted briefing");
            Ok(serde_json::Value::Null)
        }
    }
}

/// Internal briefing generation -- called by both the Tauri command and auto-trigger.
/// `auto_triggered`: when true, suppresses Pro gate check and adjusts logging.
/// `anomaly_context`: optional unresolved anomaly descriptions to inject into the prompt.
pub(crate) async fn generate_briefing_internal(
    auto_triggered: bool,
    anomaly_context: Option<Vec<String>>,
) -> Result<serde_json::Value> {
    use chrono::{Duration, Utc};

    let trigger = if auto_triggered { "auto" } else { "manual" };
    info!(target: "4da::briefing", trigger = trigger, "Generating AI briefing");

    // Drain batched notifications
    let batched = {
        let state = crate::get_monitoring_state();
        crate::monitoring::drain_batched_notifications(state)
    };
    if !batched.is_empty() {
        info!(target: "4da::briefing", count = batched.len(), "Including batched notifications");
    }

    // Get LLM settings
    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "No LLM configured. Set up Ollama or add an API key in Settings.",
            "briefing": null
        }));
    }

    // Get items from analysis state or DB
    let (mem_items, explanations): (
        Vec<crate::db::DigestSourceItem>,
        std::collections::HashMap<i64, String>,
    ) = {
        let state = get_analysis_state().lock();
        if let Some(ref results) = state.results {
            let items: Vec<crate::db::DigestSourceItem> = results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(30)
                .map(|r| crate::db::DigestSourceItem {
                    id: r.id as i64,
                    title: r.title.clone(),
                    url: r.url.clone(),
                    source_type: r.source_type.clone(),
                    created_at: Utc::now(),
                    relevance_score: Some(r.top_score as f64),
                    topics: vec![],
                })
                .collect();
            let expl: std::collections::HashMap<i64, String> = results
                .iter()
                .filter(|r| r.explanation.is_some())
                .map(|r| (r.id as i64, r.explanation.clone().unwrap_or_default()))
                .collect();
            (items, expl)
        } else {
            (vec![], std::collections::HashMap::new())
        }
    };

    let items = if mem_items.is_empty() {
        let db = get_database()?;
        let period_start = Utc::now() - Duration::hours(72);
        db.get_relevant_items_since(period_start, 0.1, 30)
            .map_err(|e| format!("Failed to fetch items: {}", e))?
    } else {
        mem_items
    };

    if items.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "briefing": "No items found. Run an analysis first to fetch and score content.",
            "item_count": 0,
            "model": llm_settings.model
        }));
    }

    let ace_ctx = get_ace_context();

    let items_text: String = items
        .iter()
        .take(20)
        .enumerate()
        .map(|(i, item)| {
            let explanation = explanations
                .get(&item.id)
                .map(|s| s.as_str())
                .unwrap_or("No context match");
            format!(
                "{}. [{}] {} (score: {:.0}%)\n   URL: {}\n   Why matched: {}",
                i + 1,
                item.source_type,
                item.title,
                item.relevance_score.unwrap_or(0.0) * 100.0,
                item.url.as_deref().unwrap_or("N/A"),
                explanation,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let tech_summary = if ace_ctx.detected_tech.is_empty() {
        "Not detected".to_string()
    } else {
        ace_ctx
            .detected_tech
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let topics_summary = if ace_ctx.active_topics.is_empty() {
        "None active".to_string()
    } else {
        ace_ctx
            .active_topics
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let anti_topics = ace_ctx
        .anti_topics
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    let system_prompt = r#"You are the user's personal intelligence analyst. You have deep knowledge of their active projects and tech stack. Your briefing should feel like a senior colleague who read everything and is telling you what matters.

Structure your briefing as:

## Action Required
[Items the user should read/act on TODAY — max 3. Each gets 2-3 sentences explaining WHY it matters to their specific work, not just what it is.]

## Worth Knowing
[3-5 items that are genuinely useful context. One sentence each with the key takeaway.]

## Filtered Out
[Brief note on what categories you filtered out and why, so the user trusts the filter.]

Rules:
- Reference the user's specific projects and tech by name
- "This affects your Tauri app" not "This may be relevant to developers"
- Include concrete details from the articles, not just titles
- If nothing is truly important, say so — don't manufacture urgency
- Max 500 words"#;

    let batched_section = if batched.is_empty() {
        String::new()
    } else {
        let items_text: String = batched
            .iter()
            .map(|b| {
                format!(
                    "- [{}] {} (score: {:.0}%)",
                    b.source_type,
                    b.title,
                    b.score * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\n\nSince your last check, {} items were queued silently:\n{}\n",
            batched.len(),
            items_text
        )
    };

    let decision_context = crate::digest_config::build_decision_context_for_briefing();

    // Improvement C: Inject anomaly context if provided
    let anomaly_section = match anomaly_context {
        Some(ref anomalies) if !anomalies.is_empty() => {
            let list = anomalies
                .iter()
                .map(|a| format!("  - {}", a))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "\n- Unresolved system anomalies (mention if relevant):\n{}",
                list
            )
        }
        _ => String::new(),
    };

    let user_prompt = format!(
        "My active projects and context:\n\
         - Tech stack: {tech}\n\
         - Currently working on: {topics}\n\
         - Skip these topics: {anti}\n\
         {decisions}{anomalies}\n\n\
         Today's {count} items (sorted by relevance):\n\n\
         {items}{batched}\n\n\
         Give me my intelligence briefing.",
        tech = tech_summary,
        topics = topics_summary,
        anti = if anti_topics.is_empty() {
            "None specified".to_string()
        } else {
            anti_topics
        },
        decisions = decision_context,
        anomalies = anomaly_section,
        count = items.len(),
        items = items_text,
        batched = batched_section,
    );

    let llm_client = crate::llm::LLMClient::new(llm_settings.clone());
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];
    let start_time = std::time::Instant::now();

    match llm_client.complete(system_prompt, messages).await {
        Ok(response) => {
            let elapsed = start_time.elapsed();
            info!(target: "4da::briefing",
                tokens = response.input_tokens + response.output_tokens,
                elapsed_ms = elapsed.as_millis(),
                trigger = trigger,
                "AI briefing generated"
            );
            *crate::digest_config::LATEST_BRIEFING.lock() = Some(response.content.clone());

            if let Ok(db) = get_database() {
                let total_tokens = response.input_tokens + response.output_tokens;
                if let Err(e) = db.save_briefing(
                    &response.content,
                    Some(&llm_settings.model),
                    items.len(),
                    Some(total_tokens),
                    Some(elapsed.as_millis() as u64),
                ) {
                    error!(target: "4da::briefing", error = %e, "Failed to persist briefing");
                }
            }

            Ok(serde_json::json!({
                "success": true,
                "briefing": response.content,
                "item_count": items.len(),
                "model": llm_settings.model,
                "tokens_used": response.input_tokens + response.output_tokens,
                "latency_ms": elapsed.as_millis(),
                "auto_triggered": auto_triggered,
            }))
        }
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to generate briefing");
            let e_str = e.to_string();
            let error_msg = if e_str.contains("Connection refused") || e_str.contains("connect") {
                "Ollama is not running. Start it with 'ollama serve' or check your LLM settings."
                    .to_string()
            } else if e_str.contains("model") {
                "The configured model may not be available. Try 'ollama pull llama3.1:8b-instruct-q8_0'.".to_string()
            } else {
                e_str
            };
            Ok(serde_json::json!({
                "success": false,
                "error": error_msg,
                "briefing": null
            }))
        }
    }
}

/// Generate an AI-powered briefing from recent relevant items
/// Uses the configured LLM (Ollama by default) to synthesize insights
#[tauri::command]
pub async fn generate_ai_briefing(app: tauri::AppHandle) -> Result<serde_json::Value> {
    crate::settings::require_pro_feature("generate_ai_briefing")?;
    // Improvement C: Gather unresolved anomalies for context injection
    let anomalies = {
        if let Ok(ace) = crate::get_ace_engine() {
            let conn = ace.get_conn().lock();
            crate::anomaly::get_unresolved(&conn).ok().map(|list| {
                list.iter()
                    .map(|a| a.description.clone())
                    .collect::<Vec<_>>()
            })
        } else {
            None
        }
    };
    let result = generate_briefing_internal(false, anomalies).await;

    // GAME: track briefing generation on success
    if let Ok(ref val) = result {
        if val
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Ok(db) = crate::get_database() {
                for a in crate::game_engine::increment_counter(db, "briefings", 1) {
                    crate::events::emit_achievement_unlocked(&app, &a);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    // ========================================================================
    // Briefing JSON response structure tests
    // ========================================================================

    #[test]
    fn briefing_no_llm_response_shape() {
        // Simulates the response shape when no LLM is configured
        let response = serde_json::json!({
            "success": false,
            "error": "No LLM configured. Set up Ollama or add an API key in Settings.",
            "briefing": null
        });
        assert_eq!(response["success"], false);
        assert!(response["error"].as_str().unwrap().contains("No LLM"));
        assert!(response["briefing"].is_null());
    }

    #[test]
    fn briefing_empty_items_response_shape() {
        // Simulates the response when no items are found
        let model = "llama3.2:latest";
        let response = serde_json::json!({
            "success": true,
            "briefing": "No items found. Run an analysis first to fetch and score content.",
            "item_count": 0,
            "model": model
        });
        assert_eq!(response["success"], true);
        assert_eq!(response["item_count"], 0);
        assert_eq!(response["model"], model);
        assert!(response["briefing"].as_str().unwrap().contains("No items"));
    }

    #[test]
    fn briefing_success_response_has_required_fields() {
        let response = serde_json::json!({
            "success": true,
            "briefing": "## Action Required\nNothing urgent today.",
            "item_count": 5,
            "model": "claude-3-haiku",
            "tokens_used": 1500,
            "latency_ms": 2300,
            "auto_triggered": false,
        });
        assert_eq!(response["success"], true);
        assert!(response["briefing"].is_string());
        assert!(response["item_count"].is_number());
        assert!(response["model"].is_string());
        assert!(response["tokens_used"].is_number());
        assert!(response["latency_ms"].is_number());
        assert_eq!(response["auto_triggered"], false);
    }
}
