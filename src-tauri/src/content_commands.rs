//! Content commands for 4DA — article reader, AI summaries, saved items.

use crate::error::{FourDaError, Result};
use crate::llm::{LLMClient, Message};
use crate::{get_database, get_settings_manager, open_db_connection};
use rusqlite::params;
use serde::Serialize;
use tracing::{debug, info};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ItemContent {
    pub content: String,
    pub source_type: String,
    pub word_count: usize,
    pub has_summary: bool,
    pub summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ItemSummary {
    pub summary: String,
    pub cached: bool,
}

#[derive(Debug, Serialize)]
pub struct SavedItem {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub saved_at: String,
    pub summary: Option<String>,
    pub content_preview: Option<String>,
}

// ============================================================================
// Commands
// ============================================================================

/// Fetch full article content for a source item.
#[tauri::command]
pub async fn get_item_content(item_id: i64) -> Result<ItemContent> {
    let db = get_database().map_err(FourDaError::Internal)?;

    let (content, source_type, _char_count) = db
        .get_item_content(item_id)
        .map_err(FourDaError::Internal)?
        .ok_or_else(|| FourDaError::Internal(format!("Item {} not found", item_id)))?;

    let word_count = content.split_whitespace().count();

    let summary = db
        .get_item_summary(item_id)
        .map_err(FourDaError::Internal)?;

    Ok(ItemContent {
        content,
        source_type,
        word_count,
        has_summary: summary.is_some(),
        summary,
    })
}

/// Get cached AI summary for an item. Returns error if no summary cached.
#[tauri::command]
pub async fn get_item_summary(item_id: i64) -> Result<ItemSummary> {
    let db = get_database().map_err(FourDaError::Internal)?;

    match db
        .get_item_summary(item_id)
        .map_err(FourDaError::Internal)?
    {
        Some(summary) => Ok(ItemSummary {
            summary,
            cached: true,
        }),
        None => Err(FourDaError::Internal("No summary cached".to_string())),
    }
}

/// Generate AI summary for an item. Uses cache if available.
#[tauri::command]
pub async fn generate_item_summary(item_id: i64) -> Result<ItemSummary> {
    let db = get_database().map_err(FourDaError::Internal)?;

    // Check cache first
    if let Some(summary) = db
        .get_item_summary(item_id)
        .map_err(FourDaError::Internal)?
    {
        return Ok(ItemSummary {
            summary,
            cached: true,
        });
    }

    // Get content snippet for summarization
    let content_snippet = db
        .get_item_content_snippet(item_id, 2000)
        .map_err(FourDaError::Internal)?;

    if content_snippet.trim().is_empty() {
        return Err(FourDaError::Internal(
            "No content available to summarize".to_string(),
        ));
    }

    let title = db
        .get_item_title(item_id)
        .map_err(FourDaError::Internal)?
        .unwrap_or_default();

    // Get LLM config
    let llm_config = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if llm_config.provider.is_empty()
        || (llm_config.api_key.is_empty() && llm_config.provider != "ollama")
    {
        return Err(FourDaError::Llm(
            "No LLM configured. Set up a provider in Settings to generate summaries.".to_string(),
        ));
    }

    debug!(target: "4da::content", item_id = item_id, "Generating AI summary");

    let client = LLMClient::new(llm_config);
    let system_prompt = "You are a concise technical summarizer. Given an article title and content, produce a 2-3 sentence summary that captures the key technical insight. Focus on what a developer needs to know. Do not use markdown formatting.";

    let user_message = format!("Title: {}\n\nContent:\n{}", title, content_snippet);

    let response = client
        .complete(
            system_prompt,
            vec![Message {
                role: "user".to_string(),
                content: user_message,
            }],
        )
        .await
        .map_err(FourDaError::Llm)?;

    let summary = response.content.trim().to_string();

    // Cache it
    if let Err(e) = db.set_item_summary(item_id, &summary) {
        debug!(target: "4da::content", error = %e, "Failed to cache summary (non-fatal)");
    }

    info!(target: "4da::content", item_id = item_id, tokens = response.input_tokens + response.output_tokens, "Generated AI summary");

    Ok(ItemSummary {
        summary,
        cached: false,
    })
}

/// Get all saved items (from ACE interactions table).
#[tauri::command]
pub async fn get_saved_items() -> Result<Vec<SavedItem>> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;

    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT i.item_id, si.title, si.url, si.source_type,
                    i.timestamp, si.summary, SUBSTR(si.content, 1, 200) as preview
             FROM interactions i
             JOIN source_items si ON si.id = i.item_id
             WHERE i.action_type = 'save'
             ORDER BY i.timestamp DESC
             LIMIT 100",
        )
        .map_err(FourDaError::Db)?;

    let items = stmt
        .query_map([], |row| {
            Ok(SavedItem {
                item_id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                source_type: row.get(3)?,
                saved_at: row.get::<_, String>(4).unwrap_or_default(),
                summary: row.get(5)?,
                content_preview: row.get(6)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

/// Remove a saved item (delete save interaction).
#[tauri::command]
pub async fn remove_saved_item(item_id: i64) -> Result<()> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;

    conn.execute(
        "DELETE FROM interactions WHERE action_type = 'save' AND item_id = ?1",
        params![item_id],
    )
    .map_err(FourDaError::Db)?;

    info!(target: "4da::content", item_id = item_id, "Removed saved item");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ItemContent construction & serialization ----

    #[test]
    fn test_item_content_serialization() {
        let item = ItemContent {
            content: "This is the full article content with several words.".to_string(),
            source_type: "hackernews".to_string(),
            word_count: 9,
            has_summary: true,
            summary: Some("A brief summary.".to_string()),
        };
        let json = serde_json::to_value(&item).expect("serialize");
        assert_eq!(json["source_type"], "hackernews");
        assert_eq!(json["word_count"], 9);
        assert_eq!(json["has_summary"], true);
        assert_eq!(json["summary"], "A brief summary.");
    }

    #[test]
    fn test_item_content_without_summary() {
        let item = ItemContent {
            content: "Some content".to_string(),
            source_type: "reddit".to_string(),
            word_count: 2,
            has_summary: false,
            summary: None,
        };
        let json = serde_json::to_value(&item).expect("serialize");
        assert_eq!(json["has_summary"], false);
        assert!(json["summary"].is_null());
    }

    // ---- ItemSummary construction & serialization ----

    #[test]
    fn test_item_summary_cached() {
        let summary = ItemSummary {
            summary: "This article covers Rust async patterns.".to_string(),
            cached: true,
        };
        let json = serde_json::to_value(&summary).expect("serialize");
        assert_eq!(json["cached"], true);
        assert!(json["summary"].as_str().expect("str").contains("Rust"));
    }

    #[test]
    fn test_item_summary_fresh() {
        let summary = ItemSummary {
            summary: "Freshly generated summary.".to_string(),
            cached: false,
        };
        let json = serde_json::to_value(&summary).expect("serialize");
        assert_eq!(json["cached"], false);
    }

    // ---- SavedItem construction & serialization ----

    #[test]
    fn test_saved_item_full_serialization() {
        let item = SavedItem {
            item_id: 42,
            title: "Understanding SQLite-vec".to_string(),
            url: Some("https://example.com/sqlite-vec".to_string()),
            source_type: "hackernews".to_string(),
            saved_at: "2025-12-01 10:00:00".to_string(),
            summary: Some("Guide to sqlite-vec KNN queries.".to_string()),
            content_preview: Some("SQLite-vec enables vector...".to_string()),
        };
        let json = serde_json::to_value(&item).expect("serialize");
        assert_eq!(json["item_id"], 42);
        assert_eq!(json["title"], "Understanding SQLite-vec");
        assert_eq!(json["url"], "https://example.com/sqlite-vec");
        assert_eq!(json["source_type"], "hackernews");
    }

    #[test]
    fn test_saved_item_with_none_fields() {
        let item = SavedItem {
            item_id: 1,
            title: "Minimal Item".to_string(),
            url: None,
            source_type: "rss".to_string(),
            saved_at: "2025-12-01".to_string(),
            summary: None,
            content_preview: None,
        };
        let json = serde_json::to_value(&item).expect("serialize");
        assert!(json["url"].is_null());
        assert!(json["summary"].is_null());
        assert!(json["content_preview"].is_null());
    }

    // ---- word count logic ----

    #[test]
    fn test_word_count_matches_split_whitespace() {
        let text = "  Rust   is a systems   programming language  ";
        let word_count = text.split_whitespace().count();
        assert_eq!(word_count, 6);
    }

    #[test]
    fn test_word_count_empty_content() {
        let text = "";
        let word_count = text.split_whitespace().count();
        assert_eq!(word_count, 0);
    }
}
