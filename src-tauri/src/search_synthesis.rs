//! Search Synthesis — LLM-powered briefings for Intelligence Console queries.
//!
//! Pro-gated. Takes a natural language query, gathers lightweight context
//! from the local DB, and calls the configured LLM to produce a 2-3 sentence
//! intelligence briefing grounded in the user's stack and search results.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::debug;

use crate::llm::{LLMClient, Message};
use crate::natural_language_search::extract_keywords;

// ============================================================================
// Types
// ============================================================================

/// Lightweight input gathered from the DB for synthesis context.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SynthesisItem {
    title: String,
    preview: String,
    source_type: String,
}

// ============================================================================
// Context gathering
// ============================================================================

/// Pull the top matching source_items for the query keywords (lightweight — no
/// embedding or vector search, just LIKE matching on title/content).
fn gather_result_context(
    conn: &rusqlite::Connection,
    keywords: &[String],
    limit: usize,
) -> Vec<SynthesisItem> {
    if keywords.is_empty() {
        return Vec::new();
    }

    let conditions: Vec<String> = keywords
        .iter()
        .map(|k| {
            format!(
                "(LOWER(s.title) LIKE '%{kw}%' OR LOWER(s.content) LIKE '%{kw}%')",
                kw = k.replace('\'', "''")
            )
        })
        .collect();

    let where_clause = conditions.join(" OR ");
    let sql = format!(
        "SELECT s.title, s.content, s.source_type
         FROM source_items s
         WHERE ({where_clause})
         ORDER BY s.last_seen DESC
         LIMIT {limit}"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            debug!(target: "4da::synthesis", error = %e, "Failed to prepare context query");
            return Vec::new();
        }
    };

    let rows = match stmt.query_map([], |row| {
        let title: String = row.get(0)?;
        let content: String = row.get(1)?;
        let source_type: String = row.get(2)?;
        let preview = if content.len() > 300 {
            format!("{}...", &content[..300])
        } else {
            content
        };
        Ok(SynthesisItem {
            title,
            preview,
            source_type,
        })
    }) {
        Ok(r) => r,
        Err(e) => {
            debug!(target: "4da::synthesis", error = %e, "Context query failed");
            return Vec::new();
        }
    };

    rows.flatten().collect()
}

/// Build a comma-separated string of the user's detected tech stack.
fn gather_stack_summary(conn: &rusqlite::Connection) -> String {
    let sql =
        "SELECT name FROM detected_tech WHERE confidence >= 0.5 ORDER BY confidence DESC LIMIT 15";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    names.join(", ")
}

// ============================================================================
// Prompt construction
// ============================================================================

fn build_system_prompt(stack_summary: &str) -> String {
    let stack_part = if stack_summary.is_empty() {
        String::new()
    } else {
        format!(" You know their stack: {}.", stack_summary)
    };

    format!(
        "You are the user's technical intelligence advisor.{stack_part} \
         Synthesize the search results into a 2-3 sentence briefing. \
         Be specific about versions, dates, and actionable items. \
         Reference their stack directly. Do not hallucinate — only reference \
         information from the provided results. \
         If no results are relevant, say so briefly."
    )
}

fn build_user_message(query: &str, items: &[SynthesisItem]) -> String {
    if items.is_empty() {
        return format!(
            "Query: \"{query}\"\n\nNo matching results were found in the local database."
        );
    }

    let mut parts = vec![format!("Query: \"{query}\"\n\nSearch results:")];
    for (i, item) in items.iter().enumerate() {
        parts.push(format!(
            "{}. [{}] {}\n   {}",
            i + 1,
            item.source_type,
            item.title,
            item.preview
        ));
    }
    parts.join("\n\n")
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn synthesize_search(
    app: tauri::AppHandle,
    query_text: String,
) -> Result<String, String> {
    crate::settings::require_pro_feature("synthesize_search")?;

    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query cannot be empty".to_string());
    }

    // Check LLM provider is configured
    let provider = {
        let manager = crate::get_settings_manager();
        let guard = manager.lock();
        guard.get().llm.clone()
    };

    if provider.provider.is_empty() || provider.provider == "none" {
        return Err(
            "No LLM provider configured. Set up Ollama (free, local) or a cloud provider in Settings."
                .to_string(),
        );
    }

    debug!(target: "4da::synthesis", query = %query_text, "Starting search synthesis");

    // Gather context from DB
    let keywords = extract_keywords(&query_text);
    let (items, stack_summary) = {
        let conn = crate::open_db_connection()?;
        let items = gather_result_context(&conn, &keywords, 7);
        let stack = gather_stack_summary(&conn);
        (items, stack)
    };

    debug!(
        target: "4da::synthesis",
        result_count = items.len(),
        stack = %stack_summary,
        "Context gathered"
    );

    // Build prompts
    let system = build_system_prompt(&stack_summary);
    let user_msg = build_user_message(&query_text, &items);

    // Emit start event
    let _ = app.emit("search-synthesis-start", &query_text);

    // Call LLM
    let client = LLMClient::new(provider);
    let response = client
        .complete(
            &system,
            vec![Message {
                role: "user".to_string(),
                content: user_msg,
            }],
        )
        .await
        .map_err(|e| format!("Synthesis failed: {e}"))?;

    let synthesis = response.content.trim().to_string();

    debug!(
        target: "4da::synthesis",
        input_tokens = response.input_tokens,
        output_tokens = response.output_tokens,
        len = synthesis.len(),
        "Synthesis complete"
    );

    // Emit completion event
    let _ = app.emit("search-synthesis-complete", &synthesis);

    Ok(synthesis)
}
