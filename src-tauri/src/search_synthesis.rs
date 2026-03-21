//! Search Synthesis — LLM-powered briefings for Intelligence Console queries.
//!
//! Signal-gated. Takes a natural language query, gathers deep context from the
//! local DB (search results, tech stack, active decisions, knowledge gaps),
//! and calls the configured LLM to produce a grounded intelligence briefing
//! that references the user's specific context.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::debug;

use crate::error::{Result, ResultExt};
use crate::llm::{LLMClient, Message};
use crate::natural_language_search::extract_keywords;

// ============================================================================
// Types
// ============================================================================

/// Lightweight input gathered from the DB for synthesis context.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SynthesisItem {
    id: i64,
    title: String,
    preview: String,
    source_type: String,
    url: Option<String>,
}

/// Synthesis response with citation sources for trust/verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResponse {
    /// The synthesized text with [N] citation markers
    pub text: String,
    /// Sources referenced in the synthesis (1-indexed)
    pub sources: Vec<SynthesisSource>,
    /// How many sources the LLM actually cited
    pub grounding_count: usize,
    /// Total sources provided to the LLM
    pub total_sources: usize,
}

/// A source that can be cited in synthesis text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisSource {
    /// 1-based index matching [N] markers in synthesis text
    pub index: usize,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
}

/// Active decision relevant to the query.
#[derive(Debug, Clone)]
struct DecisionContext {
    subject: String,
    decision: String,
}

/// Knowledge gap relevant to the query.
#[derive(Debug, Clone)]
struct GapContext {
    technology: String,
    severity: String,
}

// ============================================================================
// Context gathering
// ============================================================================

/// Pull the top matching source_items for the query keywords.
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
        "SELECT s.id, s.title, s.content, s.source_type, s.url
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
        let id: i64 = row.get(0)?;
        let title: String = row.get(1)?;
        let content: String = row.get(2)?;
        let source_type: String = row.get(3)?;
        let url: Option<String> = row.get(4)?;
        let preview = if content.len() > 300 {
            format!("{}...", &content[..300])
        } else {
            content
        };
        Ok(SynthesisItem {
            id,
            title,
            preview,
            source_type,
            url,
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

/// Find active decisions whose subject overlaps with query keywords.
fn gather_decision_context(
    conn: &rusqlite::Connection,
    keywords: &[String],
) -> Vec<DecisionContext> {
    if keywords.is_empty() {
        return Vec::new();
    }

    let sql = "SELECT subject, decision FROM decisions WHERE status != 'superseded' LIMIT 50";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let all: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    all.into_iter()
        .filter(|(subject, _)| {
            let lower = subject.to_lowercase();
            keywords.iter().any(|k| lower.contains(k.as_str()))
        })
        .take(3)
        .map(|(subject, decision)| DecisionContext { subject, decision })
        .collect()
}

/// Find knowledge gaps whose technology matches query keywords.
fn gather_gap_context(conn: &rusqlite::Connection, keywords: &[String]) -> Vec<GapContext> {
    if keywords.is_empty() {
        return Vec::new();
    }

    // Knowledge gaps are computed, not stored — check detected_tech with
    // no recent signals as a lightweight proxy.
    let sql = "SELECT dt.name, \
               CASE \
                 WHEN NOT EXISTS (SELECT 1 FROM source_items s \
                   WHERE (LOWER(s.title) LIKE '%' || LOWER(dt.name) || '%' \
                     OR LOWER(s.content) LIKE '%' || LOWER(dt.name) || '%') \
                   AND s.created_at >= datetime('now', '-21 days')) \
                 THEN 'stale' ELSE 'ok' END as gap_status \
               FROM detected_tech dt \
               WHERE dt.confidence >= 0.5";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let techs: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    techs
        .into_iter()
        .filter(|(name, status)| {
            status == "stale"
                && keywords
                    .iter()
                    .any(|k| name.to_lowercase().contains(k.as_str()))
        })
        .take(3)
        .map(|(technology, _)| GapContext {
            technology,
            severity: "stale".to_string(),
        })
        .collect()
}

// ============================================================================
// Prompt construction
// ============================================================================

fn build_system_prompt(
    stack_summary: &str,
    decisions: &[DecisionContext],
    gaps: &[GapContext],
) -> String {
    let mut prompt = String::from(
        "You are a senior technical advisor who knows this developer's exact setup. \
         Your job: synthesize search results into a sharp, specific briefing they can act on.\n\n\
         Rules:\n\
         - 2-4 sentences max. Dense, not verbose.\n\
         - Reference specific technologies, versions, and dates from the results.\n\
         - Reference sources by number: [1], [2], etc. Every factual claim must cite its source.\n\
         - Example: \"React 19 brings compiler optimizations [1] affecting server hydration [3].\"\n\
         - If something affects their stack, say which technology and why.\n\
         - If a result contradicts or supports one of their decisions, flag it.\n\
         - If they have a knowledge gap in a queried area, mention it.\n\
         - ONLY use information from the provided results. Never fabricate.\n\
         - Write like a colleague in Slack, not a chatbot. No greetings or filler.",
    );

    if !stack_summary.is_empty() {
        prompt.push_str(&format!("\n\nTheir tech stack: {stack_summary}"));
    }

    if !decisions.is_empty() {
        prompt.push_str("\n\nTheir active decisions:");
        for d in decisions {
            prompt.push_str(&format!("\n- {}: {}", d.subject, d.decision));
        }
    }

    if !gaps.is_empty() {
        prompt.push_str("\n\nKnowledge gaps (no recent signals):");
        for g in gaps {
            prompt.push_str(&format!("\n- {} ({})", g.technology, g.severity));
        }
    }

    prompt
}

fn build_user_message(query: &str, items: &[SynthesisItem]) -> String {
    if items.is_empty() {
        return format!(
            "Query: \"{query}\"\n\nNo matching results found. \
             Respond with a brief note that no signals were found for this query."
        );
    }

    let mut parts = vec![format!("Query: \"{query}\"\n\nSignals:")];
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

/// Count which citation markers [1]..[max] appear in the synthesis text.
fn count_citations(text: &str, max: usize) -> usize {
    (1..=max)
        .filter(|i| text.contains(&format!("[{i}]")))
        .count()
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn synthesize_search(
    app: tauri::AppHandle,
    query_text: String,
) -> Result<SynthesisResponse> {
    crate::settings::require_signal_feature("synthesize_search")?;

    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query cannot be empty".into());
    }

    // Check LLM provider is configured
    let provider = {
        let manager = crate::get_settings_manager();
        let guard = manager.lock();
        guard.get().llm.clone()
    };

    if provider.provider.is_empty() || provider.provider == "none" {
        return Err(
            "No LLM provider configured. Set up Ollama (free, local) or add an API key in Settings."
                .into(),
        );
    }

    debug!(target: "4da::synthesis", query = %query_text, "Starting search synthesis");

    // Gather deep context from DB
    let keywords = extract_keywords(&query_text);
    let (items, stack_summary, decisions, gaps) = {
        let conn = crate::open_db_connection()?;
        let items = gather_result_context(&conn, &keywords, 7);
        let stack = gather_stack_summary(&conn);
        let decisions = gather_decision_context(&conn, &keywords);
        let gaps = gather_gap_context(&conn, &keywords);
        (items, stack, decisions, gaps)
    };

    debug!(
        target: "4da::synthesis",
        results = items.len(),
        stack = %stack_summary,
        decisions = decisions.len(),
        gaps = gaps.len(),
        "Context gathered"
    );

    // Build prompts
    let system = build_system_prompt(&stack_summary, &decisions, &gaps);
    let user_msg = build_user_message(&query_text, &items);

    // Emit start event
    if let Err(e) = app.emit("search-synthesis-start", &query_text) {
        tracing::warn!("Failed to emit 'search-synthesis-start': {e}");
    }

    // Call LLM with streaming (tokens emitted progressively via Tauri events)
    let client = LLMClient::new(provider);
    let app_for_stream = app.clone();
    let response = client
        .stream_complete(
            &system,
            vec![Message {
                role: "user".to_string(),
                content: user_msg,
            }],
            move |token| {
                if let Err(e) = app_for_stream.emit("synthesis-token", token) {
                    tracing::warn!("Failed to emit 'synthesis-token': {e}");
                }
            },
        )
        .await
        .context("Synthesis failed")?;

    let synthesis = response.content.trim().to_string();

    let sources: Vec<SynthesisSource> = items
        .iter()
        .enumerate()
        .map(|(i, item)| SynthesisSource {
            index: i + 1,
            title: item.title.clone(),
            url: item.url.clone(),
            source_type: item.source_type.clone(),
        })
        .collect();

    let total_sources = sources.len();
    let grounding_count = count_citations(&synthesis, total_sources);

    debug!(
        target: "4da::synthesis",
        input_tokens = response.input_tokens,
        output_tokens = response.output_tokens,
        len = synthesis.len(),
        grounding = %format!("{}/{}", grounding_count, total_sources),
        "Synthesis complete"
    );

    // Emit completion event
    if let Err(e) = app.emit("search-synthesis-complete", &synthesis) {
        tracing::warn!("Failed to emit 'search-synthesis-complete': {e}");
    }

    Ok(SynthesisResponse {
        text: synthesis,
        sources,
        grounding_count,
        total_sources,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_system_prompt_includes_stack_summary() {
        let prompt = build_system_prompt("Rust, React, SQLite", &[], &[]);
        assert!(
            prompt.contains("Rust, React, SQLite"),
            "Prompt should include the stack summary"
        );
        assert!(
            prompt.contains("Their tech stack:"),
            "Prompt should have the stack section header"
        );
    }

    #[test]
    fn build_system_prompt_includes_decisions() {
        let decisions = vec![DecisionContext {
            subject: "Database".to_string(),
            decision: "Use SQLite with sqlite-vec".to_string(),
        }];
        let prompt = build_system_prompt("", &decisions, &[]);
        assert!(
            prompt.contains("Their active decisions:"),
            "Prompt should have the decisions section"
        );
        assert!(
            prompt.contains("Database: Use SQLite with sqlite-vec"),
            "Prompt should include the decision detail"
        );
    }

    #[test]
    fn build_system_prompt_includes_gaps() {
        let gaps = vec![GapContext {
            technology: "React".to_string(),
            severity: "stale".to_string(),
        }];
        let prompt = build_system_prompt("", &[], &gaps);
        assert!(
            prompt.contains("Knowledge gaps"),
            "Prompt should have the gaps section"
        );
        assert!(
            prompt.contains("React (stale)"),
            "Prompt should include the gap detail"
        );
    }

    #[test]
    fn build_system_prompt_omits_empty_sections() {
        let prompt = build_system_prompt("", &[], &[]);
        assert!(
            !prompt.contains("Their tech stack:"),
            "Empty stack should not produce a stack section"
        );
        assert!(
            !prompt.contains("Their active decisions:"),
            "Empty decisions should not produce a decisions section"
        );
        assert!(
            !prompt.contains("Knowledge gaps"),
            "Empty gaps should not produce a gaps section"
        );
    }

    #[test]
    fn build_user_message_formats_items() {
        let items = vec![
            SynthesisItem {
                id: 1,
                title: "Rust 2024 Edition".to_string(),
                preview: "New features in Rust".to_string(),
                source_type: "hackernews".to_string(),
                url: Some("https://example.com/rust".to_string()),
            },
            SynthesisItem {
                id: 2,
                title: "React Server Components".to_string(),
                preview: "RSC deep dive".to_string(),
                source_type: "reddit".to_string(),
                url: None,
            },
        ];
        let msg = build_user_message("rust updates", &items);
        assert!(msg.contains("Query: \"rust updates\""));
        assert!(msg.contains("1. [hackernews] Rust 2024 Edition"));
        assert!(msg.contains("2. [reddit] React Server Components"));
        assert!(msg.contains("New features in Rust"));
    }

    #[test]
    fn build_user_message_handles_empty_items() {
        let msg = build_user_message("nonexistent topic", &[]);
        assert!(msg.contains("No matching results found"));
        assert!(msg.contains("nonexistent topic"));
    }

    #[test]
    fn count_citations_finds_markers() {
        assert_eq!(count_citations("This [1] and that [3] point.", 5), 2);
        assert_eq!(count_citations("No citations here.", 3), 0);
        assert_eq!(count_citations("[1][2][3] all cited", 3), 3);
    }

    #[test]
    fn build_system_prompt_includes_citation_instruction() {
        let prompt = build_system_prompt("Rust", &[], &[]);
        assert!(
            prompt.contains("[1], [2]"),
            "Prompt should instruct LLM to use citation markers"
        );
    }
}
