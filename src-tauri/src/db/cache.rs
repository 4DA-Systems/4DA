// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Caching, projection data, LLM content retrieval, briefings, and digest operations.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};
use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

/// Item for digest purposes
#[derive(Debug, Clone)]
pub struct DigestSourceItem {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub relevance_score: Option<f64>,
    pub topics: Vec<String>,
}

// ============================================================================
// LLM Content Retrieval
// ============================================================================

impl Database {
    /// Get first N chars of content for an item (for LLM judging)
    pub fn get_item_content_snippet(&self, item_id: i64, max_chars: usize) -> Result<String> {
        let conn = self.conn.lock();
        let content: String = conn.query_row(
            "SELECT COALESCE(content, '') FROM source_items WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )?;

        if content.len() <= max_chars {
            Ok(content)
        } else {
            let truncated: String = content.chars().take(max_chars).collect();
            Ok(truncated)
        }
    }

    /// Get full content + source_type for an item. Returns (content, source_type, char_count).
    pub fn get_item_content(&self, item_id: i64) -> Result<Option<(String, String, usize)>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT COALESCE(content, ''), source_type FROM source_items WHERE id = ?1",
                params![item_id],
                |row| {
                    let content: String = row.get(0)?;
                    let source_type: String = row.get(1)?;
                    let char_count = content.len();
                    Ok((content, source_type, char_count))
                },
            )
            .optional()?)
    }

    /// Get cached AI summary for an item.
    pub fn get_item_summary(&self, item_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT summary FROM source_items WHERE id = ?1",
                params![item_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(std::option::Option::flatten)?)
    }

    /// Cache an AI summary for an item.
    pub fn set_item_summary(&self, item_id: i64, summary: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE source_items SET summary = ?1 WHERE id = ?2",
            params![summary, item_id],
        )?;
        Ok(())
    }

    /// Get title for a source item.
    pub fn get_item_title(&self, item_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT title FROM source_items WHERE id = ?1",
                params![item_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    // ========================================================================
    // Digest Operations
    // ========================================================================

    /// Get recent source items since a given date, for digest generation.
    /// Filters by user language and minimum relevance score.
    pub fn get_relevant_items_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        min_score: f64,
        limit: usize,
        user_lang: &str,
    ) -> SqliteResult<Vec<DigestSourceItem>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, title, url, source_type, created_at, content, relevance_score
             FROM source_items
             WHERE created_at >= ?1
               AND COALESCE(detected_lang, 'en') = ?3
               AND COALESCE(relevance_score, 0.0) >= ?4
             ORDER BY CASE WHEN relevance_score IS NULL THEN 1 ELSE 0 END,
                      relevance_score DESC, created_at DESC
             LIMIT ?2",
        )?;

        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

        let rows = stmt.query_map(
            params![since_str, limit as i64, user_lang, min_score],
            |row| {
                let content: String = row.get(5)?;
                let topics: Vec<String> = content
                    .split_whitespace()
                    .filter(|w| w.len() > 4)
                    .take(5)
                    .map(str::to_lowercase)
                    .collect();

                Ok(DigestSourceItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    url: row.get(2)?,
                    source_type: row.get(3)?,
                    created_at: parse_datetime(row.get::<_, String>(4)?),
                    relevance_score: row.get(6)?,
                    topics,
                })
            },
        )?;

        rows.collect()
    }

    /// Persist relevance scores from in-memory analysis back to the database.
    /// Called after scoring completes so the DB fallback path has real scores.
    pub fn persist_analysis_scores(&self, scores: &[(i64, f32)]) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE source_items SET relevance_score = ?1 WHERE id = ?2")?;
            for &(id, score) in scores {
                stmt.execute(params![score as f64, id])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    // ========================================================================
    // Void Position Cache Operations
    // ========================================================================

    /// Upsert a projected 3D position for an item
    pub fn upsert_void_position(
        &self,
        item_id: i64,
        item_type: &str,
        x: f32,
        y: f32,
        z: f32,
        projection_version: i64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO void_positions (item_id, item_type, x, y, z, projection_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(item_id, item_type) DO UPDATE SET
                x = excluded.x, y = excluded.y, z = excluded.z,
                projection_version = excluded.projection_version",
            params![item_id, item_type, x, y, z, projection_version],
        )?;
        Ok(())
    }

    /// Batch upsert positions (much faster than individual calls)
    pub fn upsert_void_positions_batch(
        &self,
        positions: &[(i64, &str, f32, f32, f32)],
        projection_version: i64,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO void_positions (item_id, item_type, x, y, z, projection_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(item_id, item_type) DO UPDATE SET
                    x = excluded.x, y = excluded.y, z = excluded.z,
                    projection_version = excluded.projection_version",
            )?;
            for (id, item_type, x, y, z) in positions {
                stmt.execute(params![id, item_type, x, y, z, projection_version])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Get cached position for a single item
    pub fn get_void_position(
        &self,
        item_id: i64,
        item_type: &str,
    ) -> SqliteResult<Option<(f32, f32, f32, i64)>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT x, y, z, projection_version FROM void_positions
             WHERE item_id = ?1 AND item_type = ?2",
            params![item_id, item_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
    }

    /// Get all cached positions for a given projection version
    #[allow(clippy::type_complexity)]
    pub fn get_void_positions(
        &self,
        projection_version: i64,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, f32, f32, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT item_id, item_type, x, y, z FROM void_positions
             WHERE projection_version = ?1
             ORDER BY item_id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![projection_version, limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Delete positions for a specific projection version (for cache invalidation)
    pub fn clear_void_positions(&self, projection_version: Option<i64>) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        if let Some(version) = projection_version {
            conn.execute(
                "DELETE FROM void_positions WHERE projection_version = ?1",
                params![version],
            )
        } else {
            conn.execute("DELETE FROM void_positions", [])
        }
    }

    /// Count cached positions
    pub fn void_position_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM void_positions", [], |row| row.get(0))
    }

    // ========================================================================
    // Void Universe Data Queries
    // ========================================================================

    /// Get source items with embeddings for projection (lightweight: no content text).
    #[allow(clippy::type_complexity)]
    pub fn get_source_items_for_projection(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, Option<String>, Vec<f32>, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, title, url, embedding,
                    (julianday('now') - julianday(last_seen)) * 24.0 as age_hours
             FROM source_items
             ORDER BY last_seen DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(4)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                blob_to_embedding(&embedding_blob),
                row.get::<_, f64>(5).unwrap_or(0.0) as f32,
            ))
        })?;

        rows.collect()
    }

    /// Get context chunks with embeddings for projection (lightweight).
    #[allow(clippy::type_complexity)]
    pub fn get_context_chunks_for_projection(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, Vec<f32>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_file, substr(text, 1, 100), embedding
             FROM context_chunks
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                blob_to_embedding(&embedding_blob),
            ))
        })?;

        rows.collect()
    }

    /// Get a single source item by ID (full detail for particle selection)
    pub fn get_source_item_by_id(&self, id: i64) -> SqliteResult<Option<StoredSourceItem>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en')
             FROM source_items WHERE id = ?1",
            params![id],
            |row| {
                let embedding_blob: Vec<u8> = row.get(7)?;
                Ok(StoredSourceItem {
                    id: row.get(0)?,
                    source_type: row.get(1)?,
                    source_id: row.get(2)?,
                    url: row.get(3)?,
                    title: row.get(4)?,
                    content: row.get(5)?,
                    content_hash: row.get(6)?,
                    embedding: blob_to_embedding(&embedding_blob),
                    created_at: parse_datetime(row.get::<_, String>(8)?),
                    last_seen: parse_datetime(row.get::<_, String>(9)?),
                    detected_lang: row.get::<_, String>(10).unwrap_or_else(|_| "en".to_string()),
                })
            },
        ).optional()
    }

    /// Get created_at timestamps for multiple source items in a single query.
    /// Used by free-tier history gate to avoid N+1 per-item lookups.
    pub fn get_created_at_batch(
        &self,
        ids: &[i64],
    ) -> rusqlite::Result<std::collections::HashMap<i64, chrono::DateTime<chrono::Utc>>> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.conn.lock();
        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, created_at FROM source_items WHERE id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(ids.iter());
        let mut result = std::collections::HashMap::with_capacity(ids.len());
        let rows = stmt.query_map(params, |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, created_at_str) = row?;
            result.insert(id, parse_datetime(created_at_str));
        }
        Ok(result)
    }

    // ========================================================================
    // Briefing Persistence
    // ========================================================================

    /// Save a briefing to the database, pruning to keep only the last 10.
    pub fn save_briefing(
        &self,
        content: &str,
        model: Option<&str>,
        item_count: usize,
        tokens_used: Option<u64>,
        latency_ms: Option<u64>,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO briefings (content, model, item_count, tokens_used, latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                content,
                model,
                item_count as i64,
                tokens_used.map(|v| v as i64),
                latency_ms.map(|v| v as i64),
            ],
        )?;
        let id = conn.last_insert_rowid();

        conn.execute(
            "DELETE FROM briefings WHERE id NOT IN (
                SELECT id FROM briefings ORDER BY created_at DESC LIMIT 10
            )",
            [],
        )?;

        Ok(id)
    }

    /// Get the most recent briefing.
    #[allow(clippy::type_complexity)]
    pub fn get_latest_briefing(
        &self,
    ) -> SqliteResult<Option<(String, Option<String>, i64, String)>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT content, model, item_count, created_at
             FROM briefings ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{insert_test_item, test_db};

    #[test]
    fn test_store_and_retrieve_briefing() {
        let db = test_db();

        // No briefing yet
        let latest = db.get_latest_briefing().unwrap();
        assert!(latest.is_none(), "Empty DB should have no briefings");

        // Save a briefing
        let id = db
            .save_briefing(
                "Today's briefing content",
                Some("gpt-4"),
                5,
                Some(1200),
                Some(350),
            )
            .unwrap();
        assert!(id > 0);

        // Retrieve it
        let latest = db.get_latest_briefing().unwrap().unwrap();
        assert_eq!(latest.0, "Today's briefing content");
        assert_eq!(latest.1, Some("gpt-4".to_string()));
        assert_eq!(latest.2, 5);
    }

    #[test]
    fn test_get_item_content_snippet_truncation() {
        let db = test_db();
        let long_content = "A".repeat(500);
        let id = insert_test_item(
            &db,
            "hackernews",
            "trunc_1",
            "Truncation Test",
            &long_content,
        );

        // Request snippet shorter than content
        let snippet = db.get_item_content_snippet(id, 100).unwrap();
        assert_eq!(
            snippet.len(),
            100,
            "Snippet should be truncated to max_chars"
        );

        // Request snippet longer than content
        let snippet = db.get_item_content_snippet(id, 1000).unwrap();
        assert_eq!(
            snippet.len(),
            500,
            "Snippet should return full content when under max_chars"
        );
    }

    #[test]
    fn test_get_item_content_missing_item() {
        let db = test_db();

        // Query a non-existent item id
        let result = db.get_item_content_snippet(99999, 100);
        assert!(result.is_err(), "Missing item should return an error");
    }
}
