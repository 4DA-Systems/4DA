//! Digest, AI Briefing, and Update Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains digest generation/preview,
//! AI briefing synthesis, and app update commands.

use tracing::{debug, error, info, warn};

use crate::scoring::get_ace_context;
use crate::{digest, get_analysis_state, get_database, get_settings_manager};

// ============================================================================
// Update Commands
// ============================================================================

/// Check for available updates
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app
        .updater()
        .map_err(|e| format!("Updater not available: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            info!(
                target: "4da::updater",
                current = %update.current_version,
                available = %update.version,
                "Update available"
            );
            Ok(serde_json::json!({
                "update_available": true,
                "current_version": update.current_version,
                "new_version": update.version,
                "body": update.body
            }))
        }
        Ok(None) => {
            debug!(target: "4da::updater", "No update available");
            Ok(serde_json::json!({
                "update_available": false,
                "current_version": env!("CARGO_PKG_VERSION")
            }))
        }
        Err(e) => {
            warn!(target: "4da::updater", error = %e, "Update check failed");
            Ok(serde_json::json!({
                "update_available": false,
                "error": e.to_string(),
                "current_version": env!("CARGO_PKG_VERSION")
            }))
        }
    }
}

/// Get current app version
#[tauri::command]
pub async fn get_current_version() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
        "description": env!("CARGO_PKG_DESCRIPTION")
    }))
}

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

/// Generate a digest from recent relevant items
#[tauri::command]
pub async fn generate_digest() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    // Get settings (clone to avoid holding lock during DB operations)
    let digest_config = {
        let settings_guard = get_settings_manager().lock();
        settings_guard.get().digest.clone()
    };

    let db = get_database()?;

    // Get digest period
    let period_end = Utc::now();
    let period_start = digest_config
        .last_sent
        .unwrap_or(period_end - Duration::hours(24));

    // Fetch recent relevant items from source_items table
    let items = db
        .get_relevant_items_since(
            period_start,
            digest_config.min_score,
            digest_config.max_items,
        )
        .map_err(|e| format!("Failed to fetch items: {}", e))?;

    if items.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "digest": null,
            "message": "No relevant items found for this period"
        }));
    }

    // Convert to digest items
    let digest_items: Vec<digest::DigestItem> = items
        .into_iter()
        .map(|item| digest::DigestItem {
            id: item.id,
            title: item.title,
            url: item.url,
            source: item.source_type,
            relevance_score: item.relevance_score.unwrap_or(0.0),
            matched_topics: item.topics,
            discovered_at: item.created_at,
            summary: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
        })
        .collect();

    // Create digest
    let digest_obj = digest::Digest::new(digest_items, period_start, period_end);

    // Save locally if configured
    let saved_path = if digest_config.save_local {
        let manager = digest::DigestManager::new(digest_config.clone());
        match manager.save_local(&digest_obj) {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(e) => {
                warn!(target: "4da::digest", error = %e, "Failed to save digest locally");
                None
            }
        }
    } else {
        None
    };

    // Update last_sent timestamp
    {
        let mut settings_guard = get_settings_manager().lock();
        settings_guard.get_mut().digest.last_sent = Some(Utc::now());
        settings_guard.save()?;
    }

    info!(
        target: "4da::digest",
        items = digest_obj.summary.total_items,
        avg_relevance = %format!("{:.1}%", digest_obj.summary.avg_relevance * 100.0),
        "Digest generated"
    );

    Ok(serde_json::json!({
        "success": true,
        "digest": {
            "id": digest_obj.id,
            "created_at": digest_obj.created_at,
            "period_start": digest_obj.period_start,
            "period_end": digest_obj.period_end,
            "summary": digest_obj.summary,
            "item_count": digest_obj.items.len()
        },
        "saved_path": saved_path,
        "text": digest_obj.to_text(),
        "markdown": digest_obj.to_markdown(),
        "html": digest_obj.to_html()
    }))
}

/// Preview what would be in a digest without generating it
#[tauri::command]
pub async fn preview_digest() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    // Get settings (clone to avoid holding lock during DB operations)
    let digest_config = {
        let settings_guard = get_settings_manager().lock();
        settings_guard.get().digest.clone()
    };

    let db = get_database()?;

    let period_end = Utc::now();
    let period_start = digest_config
        .last_sent
        .unwrap_or(period_end - Duration::hours(24));

    let items = db
        .get_relevant_items_since(
            period_start,
            digest_config.min_score,
            digest_config.max_items,
        )
        .map_err(|e| format!("Failed to fetch items: {}", e))?;

    Ok(serde_json::json!({
        "period_start": period_start,
        "period_end": period_end,
        "item_count": items.len(),
        "min_score": digest_config.min_score,
        "items": items.iter().take(5).map(|i| serde_json::json!({
            "title": i.title,
            "source": i.source_type,
            "score": i.relevance_score
        })).collect::<Vec<_>>()
    }))
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
    let mem_items: Vec<crate::db::DigestSourceItem> = {
        let state = get_analysis_state().lock();
        if let Some(ref results) = state.results {
            results
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
                .collect()
        } else {
            vec![]
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

    // Format items for the prompt
    let items_text: String = items
        .iter()
        .take(15) // Limit to top 15 for context window
        .enumerate()
        .map(|(i, item)| {
            format!(
                "{}. [{}] {} (score: {:.0}%)\n   URL: {}",
                i + 1,
                item.source_type,
                item.title,
                item.relevance_score.unwrap_or(0.0) * 100.0,
                item.url.as_deref().unwrap_or("N/A")
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

    // Create the prompt
    let system_prompt = r#"You are a personalized research assistant for a software developer. Your job is to synthesize the day's relevant content into actionable insights.

Be concise, direct, and useful. Focus on:
1. What's worth reading NOW (pick top 2-3)
2. Emerging patterns or themes
3. Anything that needs immediate attention

Format your response as:
## Top Picks
[2-3 items that deserve attention, with 1-sentence why]

## Themes
[Brief patterns you notice]

## Quick Takes
[Any other notable items in 1 line each]

Keep it under 300 words. No fluff."#;

    let user_prompt = format!(
        r#"Here's my context:
- Tech stack: {}
- Active topics: {}

Today's items (sorted by relevance):

{}

Give me my personalized briefing."#,
        tech_summary, topics_summary, items_text
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
