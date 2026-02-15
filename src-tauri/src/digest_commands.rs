//! Digest, AI Briefing, and Update Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains digest generation/preview,
//! AI briefing synthesis, and app update commands.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tracing::{error, info};

use crate::scoring::get_ace_context;
use crate::{get_analysis_state, get_database, get_settings_manager};

/// Cached latest briefing text for TTS and handoff features
static LATEST_BRIEFING: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Get the latest generated briefing text (used by TTS and handoff)
pub(crate) fn get_latest_briefing_text() -> Option<String> {
    LATEST_BRIEFING.lock().clone()
}

// ============================================================================
// Update Commands
// ============================================================================
// ============================================================================
// Digest Commands
// ============================================================================

/// Get digest configuration
#[tauri::command]
pub async fn get_digest_config() -> Result<serde_json::Value, String> {
    // Clone data out of lock immediately to avoid holding across async boundary
    let json = {
        let settings_guard = get_settings_manager().lock();
        let digest = &settings_guard.get().digest;
        serde_json::json!({
            "enabled": digest.enabled,
            "frequency": digest.frequency,
            "email": digest.email,
            "save_local": digest.save_local,
            "min_score": digest.min_score,
            "max_items": digest.max_items,
            "last_sent": digest.last_sent,
            "generate_summaries": digest.generate_summaries
        })
    };
    Ok(json)
}

/// Update digest configuration
#[tauri::command]
pub async fn set_digest_config(
    enabled: Option<bool>,
    frequency: Option<String>,
    email: Option<String>,
    save_local: Option<bool>,
    min_score: Option<f64>,
    max_items: Option<usize>,
) -> Result<serde_json::Value, String> {
    // Mutate and save within scoped lock, then release before returning
    let json = {
        let mut settings_guard = get_settings_manager().lock();
        let digest = &mut settings_guard.get_mut().digest;

        if let Some(e) = enabled {
            digest.enabled = e;
        }
        if let Some(f) = frequency {
            digest.frequency = f;
        }
        if let Some(e) = email {
            digest.email = Some(e);
        }
        if let Some(s) = save_local {
            digest.save_local = s;
        }
        if let Some(s) = min_score {
            digest.min_score = s;
        }
        if let Some(m) = max_items {
            digest.max_items = m;
        }

        settings_guard.save()?;

        let digest = &settings_guard.get().digest;
        info!(
            target: "4da::digest",
            enabled = digest.enabled,
            frequency = %digest.frequency,
            "Digest config updated"
        );

        serde_json::json!({
            "success": true,
            "config": {
                "enabled": digest.enabled,
                "frequency": digest.frequency,
                "email": digest.email,
                "save_local": digest.save_local,
                "min_score": digest.min_score,
                "max_items": digest.max_items
            }
        })
    };
    Ok(json)
}
// ============================================================================
// AI Briefing Commands
// ============================================================================

/// Generate an AI-powered briefing from recent relevant items
/// Uses the configured LLM (Ollama by default) to synthesize insights
#[tauri::command]
pub async fn generate_ai_briefing() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    info!(target: "4da::briefing", "Generating AI briefing");

    // Drain batched notifications from monitoring (items that were silently queued)
    let batched = {
        let state = crate::get_monitoring_state();
        crate::monitoring::drain_batched_notifications(state)
    };

    if !batched.is_empty() {
        info!(
            target: "4da::briefing",
            count = batched.len(),
            "Including batched notifications in briefing"
        );
    }

    // Get LLM settings
    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    // Check if LLM is configured
    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "No LLM configured. Set up Ollama or add an API key in Settings.",
            "briefing": null
        }));
    }

    // Use in-memory analysis results (scored + filtered) when available
    // Capture both DigestSourceItem and explanation/score data for richer briefing
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

    // Fall back to DB query (wider window) if no in-memory results
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

    // Get ACE context for personalization
    let ace_ctx = get_ace_context();

    // Format items with content snippets and explanations
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

    // Build context summary
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

    // Create the prompt
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

    // Build batched items section if there are silently queued items
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

    let user_prompt = format!(
        "My active projects and context:\n\
         - Tech stack: {tech}\n\
         - Currently working on: {topics}\n\
         - Skip these topics: {anti}\n\n\
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
        count = items.len(),
        items = items_text,
        batched = batched_section,
    );

    // Call the LLM
    let llm_client = crate::llm::LLMClient::new(llm_settings.clone());
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let start_time = std::time::Instant::now();

    match llm_client.complete(system_prompt, messages).await {
        Ok(response) => {
            let elapsed = start_time.elapsed();
            info!(
                target: "4da::briefing",
                tokens = response.input_tokens + response.output_tokens,
                elapsed_ms = elapsed.as_millis(),
                "AI briefing generated"
            );

            // Cache for TTS and handoff
            *LATEST_BRIEFING.lock() = Some(response.content.clone());

            Ok(serde_json::json!({
                "success": true,
                "briefing": response.content,
                "item_count": items.len(),
                "model": llm_settings.model,
                "tokens_used": response.input_tokens + response.output_tokens,
                "latency_ms": elapsed.as_millis()
            }))
        }
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to generate briefing");

            // Provide helpful error message
            let error_msg = if e.contains("Connection refused") || e.contains("connect") {
                "Ollama is not running. Start it with 'ollama serve' or check your LLM settings."
            } else if e.contains("model") {
                "The configured model may not be available. Try 'ollama pull llama3.1:8b-instruct-q8_0'."
            } else {
                &e
            };

            Ok(serde_json::json!({
                "success": false,
                "error": error_msg,
                "briefing": null
            }))
        }
    }
}
