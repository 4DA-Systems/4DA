// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Embedding model management and re-embedding worker.
//!
//! Handles configurable embedding model selection, model change detection,
//! and background re-embedding when the model is switched (e.g., from
//! `nomic-embed-text` to `nomic-embed-text-v2-moe` for multilingual support).

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

// crate::error used indirectly via crate::embed_texts

/// Whether a re-embed operation is currently in progress.
static REEMBED_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Default embedding model name (English-optimized).
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text";

/// Get the currently configured embedding model name from settings.
/// Falls back to the default if the field is empty or unset.
pub(crate) fn get_embedding_model() -> String {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let model = &guard.get().llm.embedding_model;
    if model.is_empty() {
        DEFAULT_EMBEDDING_MODEL.to_string()
    } else {
        model.clone()
    }
}

/// Check if the embedding model has changed since last run.
/// Uses the `app_meta` table to persist the last-used model name.
/// Returns `true` if re-embedding is needed (model changed).
pub(crate) fn check_embedding_model_changed(conn: &rusqlite::Connection) -> bool {
    let current = get_embedding_model();

    let stored: String = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'embedding_model'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    if stored.is_empty() {
        // First run — store current model, no re-embed needed
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_meta (key, value) VALUES ('embedding_model', ?1)",
            rusqlite::params![&current],
        );
        return false;
    }

    if current != stored {
        // Model changed — update stored value and signal re-embed
        tracing::info!(
            target: "4da::embeddings",
            old_model = %stored,
            new_model = %current,
            "Embedding model changed — re-embed required"
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_meta (key, value) VALUES ('embedding_model', ?1)",
            rusqlite::params![&current],
        );
        return true;
    }

    false
}

/// Information about the current embedding model configuration.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingModelInfo {
    /// Currently configured model name
    pub model: String,
    /// Whether a re-embed operation is in progress
    pub reembed_in_progress: bool,
    /// Recommended multilingual model
    pub multilingual_model: String,
}

/// Tauri command: get embedding model info for the frontend.
#[tauri::command]
pub fn get_embedding_model_info() -> EmbeddingModelInfo {
    EmbeddingModelInfo {
        model: get_embedding_model(),
        reembed_in_progress: REEMBED_IN_PROGRESS.load(Ordering::Relaxed),
        multilingual_model: "nomic-embed-text-v2-moe".to_string(),
    }
}

/// Re-embed all source items with the current embedding model.
/// Called when the embedding model changes. Runs in background.
/// Uses batched processing (32 items per API call) to avoid memory spikes.
pub(crate) async fn reembed_all_items() {
    if REEMBED_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!(
            target: "4da::embeddings",
            "Re-embed already in progress — skipping duplicate request"
        );
        return;
    }

    // Guard clears the flag on drop (including panic)
    struct ReembedGuard;
    impl Drop for ReembedGuard {
        fn drop(&mut self) {
            REEMBED_IN_PROGRESS.store(false, Ordering::SeqCst);
        }
    }
    let _guard = ReembedGuard;

    let db = match crate::state::get_database() {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(
                target: "4da::embeddings",
                error = %e,
                "Cannot re-embed: database unavailable"
            );
            return;
        }
    };

    let items: Vec<(i64, String, String)> = {
        let conn = db.conn.lock();
        conn.prepare("SELECT id, title, content FROM source_items ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .and_then(|rows| rows.collect::<std::result::Result<Vec<_>, _>>())
            })
            .unwrap_or_default()
    };

    let total = items.len();
    if total == 0 {
        tracing::info!(target: "4da::embeddings", "No items to re-embed");
        return;
    }

    tracing::info!(
        target: "4da::embeddings",
        total,
        model = %get_embedding_model(),
        "Starting re-embed of all items"
    );

    let mut success_count = 0usize;
    let mut fail_count = 0usize;

    for (batch_idx, chunk) in items.chunks(32).enumerate() {
        let texts: Vec<String> = chunk
            .iter()
            .map(|(_, title, content)| crate::build_embedding_text(title, content))
            .collect();

        match crate::embed_texts(&texts).await {
            Ok(embeddings) => {
                let conn = db.conn.lock();
                for (i, (id, _, _)) in chunk.iter().enumerate() {
                    if let Some(embedding) = embeddings.get(i) {
                        let blob: Vec<u8> =
                            embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
                        let result = conn.execute(
                            "UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2",
                            rusqlite::params![blob, id],
                        );
                        match result {
                            Ok(_) => success_count += 1,
                            Err(e) => {
                                tracing::warn!(
                                    target: "4da::embeddings",
                                    item_id = id,
                                    error = %e,
                                    "Failed to update embedding for item"
                                );
                                fail_count += 1;
                            }
                        }
                    }
                }
                tracing::debug!(
                    target: "4da::embeddings",
                    batch = batch_idx,
                    items = chunk.len(),
                    "Re-embedded batch"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "4da::embeddings",
                    batch = batch_idx,
                    error = %e,
                    "Re-embed batch failed"
                );
                fail_count += chunk.len();
            }
        }
    }

    tracing::info!(
        target: "4da::embeddings",
        total,
        success = success_count,
        failed = fail_count,
        "Re-embed complete"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Test 1: get_embedding_model returns default when settings empty
    // ========================================================================
    #[test]
    fn test_get_embedding_model_default() {
        // When the settings have an empty embedding_model, we should get the default
        let model = get_embedding_model();
        // Should return a non-empty string (either user config or default)
        assert!(!model.is_empty(), "Embedding model should never be empty");
        // On a fresh settings instance, should be the default
        assert_eq!(
            model, DEFAULT_EMBEDDING_MODEL,
            "Default model should be nomic-embed-text"
        );
    }

    // ========================================================================
    // Test 2: check_embedding_model_changed — first run stores model
    // ========================================================================
    #[test]
    fn test_check_model_changed_first_run_stores_model() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .unwrap();

        // First call on empty table should return false (no re-embed needed)
        let changed = check_embedding_model_changed(&conn);
        assert!(!changed, "First run should not trigger re-embed");

        // Verify the model was stored
        let stored: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'embedding_model'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            stored,
            get_embedding_model(),
            "Stored model should match current model"
        );
    }

    // ========================================================================
    // Test 3: check_embedding_model_changed — same model returns false
    // ========================================================================
    #[test]
    fn test_check_model_changed_same_model_no_reembed() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .unwrap();

        let current = get_embedding_model();
        conn.execute(
            "INSERT INTO app_meta (key, value) VALUES ('embedding_model', ?1)",
            rusqlite::params![&current],
        )
        .unwrap();

        let changed = check_embedding_model_changed(&conn);
        assert!(!changed, "Same model should not trigger re-embed");
    }

    // ========================================================================
    // Test 4: check_embedding_model_changed — different model returns true
    // ========================================================================
    #[test]
    fn test_check_model_changed_different_model_triggers_reembed() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .unwrap();

        // Store a different model than what's currently configured
        conn.execute(
            "INSERT INTO app_meta (key, value) VALUES ('embedding_model', 'some-other-model')",
            [],
        )
        .unwrap();

        let changed = check_embedding_model_changed(&conn);
        assert!(changed, "Different model should trigger re-embed");

        // Verify the stored value was updated
        let stored: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'embedding_model'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            stored,
            get_embedding_model(),
            "Stored model should be updated to current model"
        );
    }

    // ========================================================================
    // Test 5: EmbeddingModelInfo serialization and defaults
    // ========================================================================
    #[test]
    fn test_embedding_model_info_structure() {
        let info = get_embedding_model_info();

        // Model should be non-empty
        assert!(!info.model.is_empty(), "Model name should not be empty");

        // Re-embed should not be in progress by default
        assert!(
            !info.reembed_in_progress,
            "Re-embed should not be in progress at test start"
        );

        // Multilingual model recommendation should be set
        assert_eq!(
            info.multilingual_model, "nomic-embed-text-v2-moe",
            "Should recommend multilingual v2 model"
        );

        // Should serialize to JSON without error
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("model"));
        assert!(json.contains("reembed_in_progress"));
        assert!(json.contains("multilingual_model"));
    }
}
