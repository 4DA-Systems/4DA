// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
    let db = get_database()?;

    let (content, source_type, _char_count) = db
        .get_item_content(item_id)?
        .ok_or_else(|| FourDaError::Internal(format!("Item {item_id} not found")))?;

    let word_count = content.split_whitespace().count();

    let summary = db.get_item_summary(item_id)?;

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
    let db = get_database()?;

    match db.get_item_summary(item_id)? {
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
    let db = get_database()?;

    // Check cache first
    if let Some(summary) = db.get_item_summary(item_id)? {
        return Ok(ItemSummary {
            summary,
            cached: true,
        });
    }

    // Get content snippet for summarization
    let content_snippet = db.get_item_content_snippet(item_id, 2000)?;

    if content_snippet.trim().is_empty() {
        return Err(FourDaError::Internal(
            "No content available to summarize".to_string(),
        ));
    }

    let title = db.get_item_title(item_id)?.unwrap_or_default();

    let llm_config = {
        let mut guard = get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    if !crate::llm_gate::compute_has_llm(&llm_config.provider, &llm_config.api_key) {
        return Err(FourDaError::Llm(
            "No LLM configured. Set up a provider in Settings to generate summaries.".to_string(),
        ));
    }

    // A summary written from the title alone would be invented, so under
    // titles_only an off-machine model is not asked for one.
    if !crate::llm_egress::body_allowed(&llm_config) {
        return Err(FourDaError::Llm(
            "Summaries need the article text, and your privacy setting sends only titles to cloud AI providers. Change it in Settings → Privacy, or use a local Ollama model.".to_string(),
        ));
    }

    debug!(target: "4da::content", item_id = item_id, "Generating AI summary");

    let client = LLMClient::with_purpose(llm_config, "content_analysis");
    let system_prompt = "You are a concise technical summarizer. Given an article title and content, produce a 2-3 sentence summary that captures the key technical insight. Focus on what a developer needs to know. Do not use markdown formatting.";

    let user_message = format!("Title: {title}\n\nContent:\n{content_snippet}");

    let response = client
        .complete(
            system_prompt,
            vec![Message {
                role: "user".to_string(),
                content: user_message,
            }],
        )
        .await?;

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
    let conn = open_db_connection()?;

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
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in content_commands: {e}");
                None
            }
        })
        .collect();

    Ok(items)
}

/// The rows one Save writes land within this many seconds of its `save`
/// interaction (three IPC calls fired together; generous for a slow disk).
const SAVE_ROW_WINDOW_SECS: i64 = 120;

/// What an unsave removed, per table.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RetractedSave {
    pub saves: usize,
    pub accuracy_feedback: usize,
    pub feedback: usize,
}

/// Retract a Save, in one transaction. A Save writes three rows: the `save`
/// interaction, an `accuracy_feedback` interaction scored 1.0 (save is the
/// only feedback type mapped to 1.0) and a `feedback` row with `relevant = 1`
/// (the calibration fitter's ground truth). Unsave used to delete only the
/// first, so the learning the save taught stayed (audit 2026-10-07). Removes
/// exactly the 1.0 accuracy rows and relevant=1 feedback rows written
/// alongside one of this item's saves — never a click, dismiss or a
/// relevance label given at another time.
pub(crate) fn retract_save(
    conn: &mut rusqlite::Connection,
    item_id: i64,
) -> rusqlite::Result<RetractedSave> {
    let tx = conn.transaction()?;
    let feedback = tx.execute(
        "DELETE FROM feedback
          WHERE source_item_id = ?1 AND relevant = 1
            AND EXISTS (SELECT 1 FROM interactions s
                         WHERE s.item_id = ?1 AND s.action_type = 'save'
                           AND ABS(julianday(feedback.created_at) - julianday(s.timestamp)) * 86400 <= ?2)",
        params![item_id, SAVE_ROW_WINDOW_SECS],
    )?;
    let accuracy_feedback = tx.execute(
        "DELETE FROM interactions
          WHERE item_id = ?1 AND action_type = 'accuracy_feedback'
            AND json_extract(action_data, '$.actual_score') = 1.0
            AND EXISTS (SELECT 1 FROM interactions s
                         WHERE s.item_id = ?1 AND s.action_type = 'save'
                           AND ABS(julianday(interactions.timestamp) - julianday(s.timestamp)) * 86400 <= ?2)",
        params![item_id, SAVE_ROW_WINDOW_SECS],
    )?;
    let saves = tx.execute(
        "DELETE FROM interactions WHERE action_type = 'save' AND item_id = ?1",
        params![item_id],
    )?;
    tx.commit()?;
    Ok(RetractedSave {
        saves,
        accuracy_feedback,
        feedback,
    })
}

/// Remove a saved item and retract what saving it taught (see `retract_save`).
#[tauri::command]
pub async fn remove_saved_item(item_id: i64) -> Result<()> {
    let mut conn = open_db_connection()?;
    let removed = retract_save(&mut conn, item_id).map_err(FourDaError::Db)?;
    if removed.feedback > 0 {
        crate::db::invalidate_feedback_topic_cache();
    }

    info!(
        target: "4da::content",
        item_id = item_id,
        saves = removed.saves,
        accuracy_feedback = removed.accuracy_feedback,
        feedback = removed.feedback,
        "Removed saved item"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- retract_save (unsave retracts what the save taught) ----

    fn unsave_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE interactions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, item_id INTEGER, action_type TEXT,
                 action_data TEXT, signal_strength REAL DEFAULT 0.5,
                 timestamp TEXT DEFAULT (datetime('now')));
             CREATE TABLE feedback (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, source_item_id INTEGER NOT NULL,
                 relevant INTEGER NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')));",
        )
        .expect("schema");
        conn
    }

    /// The three rows one Save writes (feedback-slice recordInteraction).
    fn save(conn: &rusqlite::Connection, item: i64, at: &str) {
        conn.execute(
            "INSERT INTO interactions (item_id, action_type, signal_strength, timestamp) VALUES (?1, 'save', 1.0, ?2)",
            params![item, at],
        )
        .expect("save row");
        conn.execute(
            "INSERT INTO interactions (item_id, action_type, action_data, signal_strength, timestamp)
             VALUES (?1, 'accuracy_feedback', '{\"predicted_score\":0.6,\"actual_score\":1.0,\"calibration_error\":0.4}', 1.0, ?2)",
            params![item, at],
        )
        .expect("accuracy row");
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant, created_at) VALUES (?1, 1, ?2)",
            params![item, at],
        )
        .expect("feedback row");
    }

    fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).expect("count")
    }

    #[test]
    fn retract_save_removes_exactly_the_rows_the_save_wrote() {
        let mut conn = unsave_db();
        save(&conn, 7, "2026-10-07 10:00:00");
        // Unrelated learning that must survive the unsave:
        save(&conn, 8, "2026-10-07 10:00:00"); // another item's save
        conn.execute_batch(
            "INSERT INTO interactions (item_id, action_type, action_data, timestamp)
               VALUES (7, 'accuracy_feedback', '{\"actual_score\":0.7}', '2026-10-07 10:00:01'); -- a click
             INSERT INTO interactions (item_id, action_type, timestamp) VALUES (7, 'click', '2026-10-07 10:00:01');
             INSERT INTO feedback (source_item_id, relevant, created_at) VALUES (7, 1, '2026-10-01 09:00:00'); -- older label
             INSERT INTO feedback (source_item_id, relevant, created_at) VALUES (7, 0, '2026-10-07 10:00:00');",
        )
        .expect("noise");

        let removed = retract_save(&mut conn, 7).expect("retract");
        assert_eq!(
            removed,
            RetractedSave {
                saves: 1,
                accuracy_feedback: 1,
                feedback: 1
            }
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM interactions WHERE item_id = 7 AND action_type = 'save'"
            ),
            0
        );
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM interactions WHERE item_id = 7 AND action_type = 'accuracy_feedback'"),
            1,
            "the click's accuracy row stays"
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM interactions WHERE item_id = 7 AND action_type = 'click'"
            ),
            1
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM feedback WHERE source_item_id = 7"
            ),
            2,
            "the older label and the relevant=0 row stay"
        );
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM interactions WHERE item_id = 8"),
            2
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM feedback WHERE source_item_id = 8"
            ),
            1
        );
    }

    #[test]
    fn retract_save_is_a_no_op_when_nothing_was_saved() {
        let mut conn = unsave_db();
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (3, 1)",
            [],
        )
        .expect("label");
        assert_eq!(
            retract_save(&mut conn, 3).expect("retract"),
            RetractedSave::default()
        );
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM feedback"), 1);
    }

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
