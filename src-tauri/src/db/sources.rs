// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source item CRUD, feedback, source registry, and health tracking.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use std::sync::{LazyLock, Mutex};
use tracing::info;

use super::StoredSourceItem;
use super::{
    blob_to_embedding, embedding_to_blob, hash_content_parts, parse_datetime, Database,
    ScoringStatsAggregate,
};

/// Maximum history depth for free-tier users (30 days in hours).
/// Signal and trial users have unlimited history access.
pub const FREE_HISTORY_LIMIT_HOURS: i64 = 30 * 24;

// Cache for feedback topic summary — invalidated on feedback writes.
static FEEDBACK_TOPIC_CACHE: LazyLock<Mutex<Option<Vec<FeedbackTopicSummary>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Invalidate the feedback topic summary cache.
/// Must be called after any feedback write (record_feedback, etc.).
pub fn invalidate_feedback_topic_cache() {
    let mut cache = FEEDBACK_TOPIC_CACHE.lock().unwrap_or_else(|e| {
        tracing::warn!("FEEDBACK_TOPIC_CACHE mutex poisoned, recovering");
        e.into_inner()
    });
    *cache = None;
}

// ============================================================================
// Types
// ============================================================================

/// Source info tuple: (source_type, name, enabled, last_fetch)
pub type SourceInfo = (String, String, bool, Option<String>);

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceHealthRecord {
    pub source_type: String,
    pub status: String,
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub error_count: i64,
    pub consecutive_failures: i64,
    pub items_fetched: i64,
    pub response_time_ms: i64,
    pub checked_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeedHealth {
    pub feed_origin: String,
    pub source_type: String,
    pub consecutive_failures: i64,
    pub total_successes: i64,
    pub total_failures: i64,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_error: Option<String>,
    pub circuit_open: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeedbackTopicSummary {
    pub topic: String,
    pub saves: i64,
    pub dismissals: i64,
    pub net_score: f64,
}

// ============================================================================
// Source Item Operations
// ============================================================================

impl Database {
    /// Store or update a source item (also updates vec0 index).
    /// Language is auto-detected from title text via `whichlang`.
    pub fn upsert_source_item(
        &self,
        source_type: &str,
        source_id: &str,
        url: Option<&str>,
        title: &str,
        content: &str,
        embedding: &[f32],
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let content_hash = hash_content_parts(&[title, content]);
        let embedding_blob = embedding_to_blob(embedding);
        let detected_lang = crate::language_detect::detect_language(title);

        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
                params![source_type, source_id],
                |row| row.get(0),
            )
            .ok();

        let tx = conn.unchecked_transaction()?;
        if let Some(id) = existing_id {
            tx.execute(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embedding = ?5, detected_lang = ?6, last_seen = datetime('now') WHERE id = ?7",
                params![url, title, content, content_hash, embedding_blob, detected_lang, id],
            )?;
            tx.execute(
                "UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            tx.commit()?;
            Ok(id)
        } else {
            tx.execute(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, detected_lang, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
                params![source_type, source_id, url, title, content, content_hash, embedding_blob, detected_lang],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
                params![id, embedding_blob],
            )?;
            tx.commit()?;
            Ok(id)
        }
    }

    /// Batch upsert source items in a transaction (much faster than individual calls).
    /// Tuple: (source_type, source_id, url, title, content, embedding, detected_lang, content_type, cve_ids, feed_origin).
    #[allow(clippy::type_complexity)]
    pub fn batch_upsert_source_items(
        &self,
        items: &[(
            String,
            String,
            Option<String>,
            String,
            String,
            Vec<f32>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut check_stmt = tx.prepare_cached(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            )?;
            let mut update_stmt = tx.prepare_cached(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, \
                 embedding = ?5, detected_lang = ?6, \
                 content_type = COALESCE(?7, source_items.content_type), \
                 cve_ids = COALESCE(?8, source_items.cve_ids), \
                 feed_origin = COALESCE(?9, source_items.feed_origin), \
                 last_seen = datetime('now') WHERE id = ?10",
            )?;
            let mut update_vec_stmt =
                tx.prepare_cached("UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2")?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, \
                 embedding, detected_lang, content_type, cve_ids, feed_origin, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
            )?;
            let mut insert_vec_stmt =
                tx.prepare_cached("INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)")?;

            for (
                source_type,
                source_id,
                url,
                title,
                content,
                embedding,
                detected_lang,
                content_type,
                cve_ids,
                feed_origin,
            ) in items
            {
                let content_hash = hash_content_parts(&[title, content]);
                let embedding_blob = embedding_to_blob(embedding);

                let existing_id: Option<i64> = check_stmt
                    .query_row(params![source_type, source_id], |row| row.get(0))
                    .ok();

                if let Some(id) = existing_id {
                    update_stmt.execute(params![
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embedding_blob,
                        detected_lang,
                        content_type,
                        cve_ids,
                        feed_origin,
                        id
                    ])?;
                    update_vec_stmt.execute(params![embedding_blob, id])?;
                } else {
                    insert_stmt.execute(params![
                        source_type,
                        source_id,
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embedding_blob,
                        detected_lang,
                        content_type,
                        cve_ids,
                        feed_origin
                    ])?;
                    let id = tx.last_insert_rowid();
                    insert_vec_stmt.execute(params![id, embedding_blob])?;
                }
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Batch upsert source items that failed embedding (stored as pending for retry)
    #[allow(clippy::type_complexity)]
    pub fn batch_upsert_pending_source_items(
        &self,
        items: &[(String, String, Option<String>, String, String, String)],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut check_stmt = tx.prepare_cached(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            )?;
            let mut update_stmt = tx.prepare_cached(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embed_text = ?5, embedding_status = 'pending', last_seen = datetime('now') WHERE id = ?6",
            )?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, embedding_status, embed_text, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, X'', 'pending', ?7, datetime('now'))",
            )?;

            for (source_type, source_id, url, title, content, embed_text) in items {
                let content_hash = hash_content_parts(&[title, content]);

                let existing_id: Option<i64> = check_stmt
                    .query_row(params![source_type, source_id], |row| row.get(0))
                    .ok();

                if let Some(id) = existing_id {
                    update_stmt.execute(params![
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embed_text,
                        id
                    ])?;
                } else {
                    insert_stmt.execute(params![
                        source_type,
                        source_id,
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embed_text
                    ])?;
                }
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Get items with pending embeddings for retry
    pub fn get_pending_embedding_items(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, COALESCE(embed_text, title || ' ' || content)
             FROM source_items
             WHERE embedding_status = 'pending'
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        rows.collect()
    }

    /// Upgrade a pending item to complete after successful re-embedding
    pub fn upgrade_pending_to_complete(&self, id: i64, embedding: &[f32]) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let embedding_blob = embedding_to_blob(embedding);

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE source_items SET embedding = ?1, embedding_status = 'complete', embed_text = NULL WHERE id = ?2",
            params![embedding_blob, id],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
            params![id, embedding_blob],
        )?;
        tx.commit()?;

        Ok(())
    }

    /// Check if a source item exists (for incremental updates)
    pub fn source_item_exists(&self, source_type: &str, source_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get a single source item by type and id (for cache lookup)
    pub fn get_source_item(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> SqliteResult<Option<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en'), feed_origin
             FROM source_items
             WHERE source_type = ?1 AND source_id = ?2
             AND (embedding_status IS NULL OR embedding_status = 'complete')"
        )?;

        let mut rows = stmt.query_map(params![source_type, source_id], |row| {
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
                detected_lang: row
                    .get::<_, String>(10)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(11).ok().flatten(),
            })
        })?;

        match rows.next() {
            Some(Ok(item)) => Ok(Some(item)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Update last_seen timestamp for an existing item
    pub fn touch_source_item(&self, source_type: &str, source_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE source_items SET last_seen = datetime('now') WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
        )?;
        Ok(())
    }

    /// Get recent source_id values for a given source type (for duplicate detection).
    pub fn get_recent_source_item_ids(
        &self,
        source_type: &str,
        limit: usize,
    ) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT source_id FROM source_items WHERE source_type = ?1 ORDER BY last_seen DESC LIMIT ?2",
        )?;
        let ids = stmt
            .query_map(params![source_type, limit as i64], |row| {
                row.get::<_, String>(0)
            })?
            .filter_map(std::result::Result::ok)
            .collect();
        Ok(ids)
    }

    /// Get source items by type
    pub fn get_source_items(
        &self,
        source_type: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en'), feed_origin
             FROM source_items
             WHERE source_type = ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![source_type, limit as i64], |row| {
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
                detected_lang: row
                    .get::<_, String>(10)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(11).ok().flatten(),
            })
        })?;

        rows.collect()
    }

    /// Get recent source items respecting the free-tier history gate.
    ///
    /// Free users are limited to 30 days of history (`FREE_HISTORY_LIMIT_HOURS`).
    /// Signal / trial users receive the full requested window.
    /// The gate is applied at the SQL level so older items are never loaded.
    ///
    /// **IMPORTANT:** This now routes through `get_items_balanced_by_source`
    /// to guarantee per-source diversity. Previously, high-volume sources
    /// (Reddit/Lobsters/Devto) would flood the LIMIT budget and starve
    /// low-volume chapters (Security/Research/Dependencies). The balanced
    /// query caps each source at `limit / 10` items minimum, leaving
    /// headroom for up to 20 sources while still honoring the overall cap.
    pub fn get_items_tiered(
        &self,
        requested_hours: i64,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let effective_hours = if crate::settings::is_signal() {
            requested_hours
        } else {
            requested_hours.min(FREE_HISTORY_LIMIT_HOURS)
        };
        // Per-source cap: at minimum 50, at most limit/5 — ensures every source
        // that has items gets representation, while heavy sources can still
        // contribute more than their fair share when they dominate.
        let per_source = (limit / 5).max(50);
        self.get_items_balanced_by_source(effective_hours, per_source, limit)
    }

    /// Get recent items with **per-source-type diversity** — guarantees each
    /// active source contributes up to `per_source_cap` of its most recent
    /// items, then the global `overall_limit` trims the union.
    ///
    /// This prevents the "one noisy source drowns out the rest" problem:
    /// without this, `ORDER BY last_seen DESC LIMIT 1000` lets Reddit/Lobsters
    /// monopolize the budget and leaves Security/Research/Dependencies chapters
    /// empty. The balanced query uses a window function partitioned by
    /// `source_type` so each source gets a fair slice.
    pub fn get_items_balanced_by_source(
        &self,
        hours: i64,
        per_source_cap: usize,
        overall_limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.read_conn();
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare(
            "WITH ranked AS (
                SELECT id, source_type, source_id, url, title, content, content_hash,
                       embedding, created_at, last_seen,
                       COALESCE(detected_lang, 'en') AS detected_lang,
                       feed_origin,
                       ROW_NUMBER() OVER (
                         PARTITION BY source_type
                         ORDER BY last_seen DESC
                       ) AS rn
                FROM source_items
                WHERE last_seen >= ?1
             )
             SELECT id, source_type, source_id, url, title, content, content_hash,
                    embedding, created_at, last_seen, detected_lang, feed_origin
             FROM ranked
             WHERE rn <= ?2
             ORDER BY last_seen DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(
            params![cutoff_str, per_source_cap as i64, overall_limit as i64],
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
                    detected_lang: row
                        .get::<_, String>(10)
                        .unwrap_or_else(|_| "en".to_string()),
                    feed_origin: row.get(11).ok().flatten(),
                })
            },
        )?;

        rows.collect()
    }

    /// Get recent source items within a time window (hours)
    pub fn get_items_since_hours(
        &self,
        hours: i64,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.read_conn();
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en'), feed_origin
             FROM source_items
             WHERE last_seen >= ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![cutoff_str, limit as i64], |row| {
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
                detected_lang: row
                    .get::<_, String>(10)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(11).ok().flatten(),
            })
        })?;

        rows.collect()
    }

    /// Get items added since a specific ISO timestamp (for differential analysis).
    /// Respects free-tier 30-day history gate.
    pub fn get_items_since_timestamp_tiered(
        &self,
        since: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        if crate::settings::is_signal() {
            self.get_items_since_timestamp(since, limit)
        } else {
            // Free tier: clamp to 30-day floor even if `since` is older
            let cutoff_30d = chrono::Utc::now() - chrono::Duration::hours(FREE_HISTORY_LIMIT_HOURS);
            let cutoff_str = cutoff_30d.format("%Y-%m-%d %H:%M:%S").to_string();
            // Use whichever is more recent: the requested timestamp or the 30-day cutoff
            let effective_since = if since > cutoff_str.as_str() {
                since.to_string()
            } else {
                cutoff_str
            };
            self.get_items_since_timestamp(&effective_since, limit)
        }
    }

    /// Get items added since a specific ISO timestamp (for differential analysis)
    pub fn get_items_since_timestamp(
        &self,
        since: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en'), feed_origin
             FROM source_items
             WHERE last_seen > ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![since, limit as i64], |row| {
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
                detected_lang: row
                    .get::<_, String>(10)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(11).ok().flatten(),
            })
        })?;

        rows.collect()
    }

    /// Count items by source type
    pub fn source_item_count(&self, source_type: &str) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE source_type = ?1",
            params![source_type],
            |row| row.get(0),
        )
    }

    /// Count total items
    pub fn total_item_count(&self) -> SqliteResult<i64> {
        let conn = self.read_conn();
        conn.query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
    }

    /// Prune oldest items when the database exceeds the total item cap.
    /// Keeps the newest `max_items` by `created_at`, deletes the rest.
    /// Returns the number of items deleted.
    pub fn prune_items_if_needed(&self, max_items: usize) -> SqliteResult<usize> {
        let total = self.total_item_count()? as usize;
        if total <= max_items {
            return Ok(0);
        }

        let to_delete = total - max_items;
        let conn = self.conn.lock();

        // Delete oldest items (by created_at) that aren't bookmarked/saved
        let deleted = conn.execute(
            "DELETE FROM source_items WHERE id IN (
                SELECT id FROM source_items
                WHERE id NOT IN (SELECT source_item_id FROM saved_items)
                ORDER BY created_at ASC
                LIMIT ?1
            )",
            params![to_delete],
        )?;

        if deleted > 0 {
            tracing::info!(
                target: "4da::db",
                deleted,
                total,
                cap = max_items,
                "Auto-pruned oldest items to stay within capacity"
            );
        }

        Ok(deleted)
    }

    // ========================================================================
    // Feedback Operations
    // ========================================================================

    /// Record user feedback
    pub fn record_feedback(&self, source_item_id: i64, relevant: bool) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
            params![source_item_id, relevant as i32],
        )?;
        drop(conn);
        invalidate_feedback_topic_cache();
        Ok(())
    }

    // ========================================================================
    // Source Registry
    // ========================================================================

    /// Register a source
    pub fn register_source(&self, source_type: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO sources (source_type, name) VALUES (?1, ?2)",
            params![source_type, name],
        )?;
        Ok(())
    }

    /// Update last fetch time for a source
    pub fn update_source_fetch_time(&self, source_type: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sources SET last_fetch = datetime('now') WHERE source_type = ?1",
            params![source_type],
        )?;
        Ok(())
    }

    /// Check if a specific source is enabled (defaults to true if not in DB)
    pub fn is_source_enabled(&self, source_type: &str) -> bool {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT enabled FROM sources WHERE source_type = ?1",
            params![source_type],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(true)
    }

    /// Toggle source enabled/disabled
    pub fn toggle_source_enabled(&self, source_type: &str, enabled: bool) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sources SET enabled = ?1 WHERE source_type = ?2",
            params![enabled as i64, source_type],
        )?;
        Ok(())
    }

    /// Get all sources with their enabled status
    pub fn get_all_sources(&self) -> SqliteResult<Vec<SourceInfo>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT source_type, name, enabled, last_fetch FROM sources ORDER BY source_type",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect()
    }

    /// Get last fetch time for a source
    pub fn get_source_last_fetch(&self, source_type: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT last_fetch FROM sources WHERE source_type = ?1",
            params![source_type],
            |row| row.get(0),
        )
        .optional()
        .map(std::option::Option::flatten)
    }

    // ========================================================================
    // Source Health
    // ========================================================================

    /// Record source health after a fetch
    pub fn record_source_health(
        &self,
        source_type: &str,
        success: bool,
        items_fetched: i64,
        response_time_ms: i64,
        error_msg: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();

        if success {
            conn.execute(
                "INSERT INTO source_health (source_type, status, last_success, items_fetched, response_time_ms, consecutive_failures, checked_at)
                 VALUES (?1, 'healthy', datetime('now'), ?2, ?3, 0, datetime('now'))
                 ON CONFLICT(source_type) DO UPDATE SET
                   status = 'healthy', last_success = datetime('now'),
                   items_fetched = ?2, response_time_ms = ?3,
                   consecutive_failures = 0, checked_at = datetime('now')",
                params![source_type, items_fetched, response_time_ms],
            )?;
        } else {
            conn.execute(
                "INSERT INTO source_health (source_type, status, last_error, error_count, consecutive_failures, checked_at)
                 VALUES (?1, 'error', ?2, 1, 1, datetime('now'))
                 ON CONFLICT(source_type) DO UPDATE SET
                   status = CASE WHEN consecutive_failures + 1 >= 5 THEN 'circuit_open' ELSE 'error' END,
                   last_error = ?2,
                   error_count = error_count + 1,
                   consecutive_failures = consecutive_failures + 1,
                   checked_at = datetime('now')",
                params![source_type, error_msg.unwrap_or("Unknown error")],
            )?;
        }

        Ok(())
    }

    /// Get source health for all sources
    pub fn get_source_health(&self) -> SqliteResult<Vec<SourceHealthRecord>> {
        let conn = self.read_conn();

        let mut stmt = conn.prepare(
            "SELECT source_type, status, last_success, last_error, error_count,
                    consecutive_failures, items_fetched, response_time_ms, checked_at
             FROM source_health ORDER BY source_type",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SourceHealthRecord {
                source_type: row.get(0)?,
                status: row.get(1)?,
                last_success: row.get(2)?,
                last_error: row.get(3)?,
                error_count: row.get(4)?,
                consecutive_failures: row.get(5)?,
                items_fetched: row.get(6)?,
                response_time_ms: row.get(7)?,
                checked_at: row.get(8)?,
            })
        })?;

        rows.collect()
    }

    /// Check if circuit breaker is open for a source (5+ consecutive failures).
    /// Auto-resets after 10 minutes cooldown to allow retry after transient outages.
    pub fn is_circuit_open(&self, source_type: &str) -> bool {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT consecutive_failures, checked_at FROM source_health WHERE source_type = ?1",
            params![source_type],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok((failures, checked_at)) if failures >= 5 => {
                let stale = conn
                    .query_row(
                        "SELECT datetime(?1, '+10 minutes') <= datetime('now')",
                        params![checked_at],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap_or(false);
                if stale {
                    if let Err(e) = conn.execute(
                        "UPDATE source_health SET consecutive_failures = 0, status = 'error' WHERE source_type = ?1",
                        params![source_type],
                    ) {
                        tracing::warn!(target: "4da::db", error = %e, source = source_type, "Failed to auto-reset circuit breaker");
                    }
                    tracing::info!(target: "4da::health", source = source_type, "Circuit breaker auto-reset after cooldown");
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    // ========================================================================
    // Per-Feed Health Tracking (circuit breaker per feed/channel/handle)
    // ========================================================================

    /// Record a successful fetch for a specific feed/channel/handle.
    /// Resets consecutive failures and closes the circuit.
    pub fn record_feed_success(&self, feed_origin: &str, source_type: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, total_successes, last_success_at, updated_at)
             VALUES (?1, ?2, 1, datetime('now'), datetime('now'))
             ON CONFLICT(feed_origin, source_type) DO UPDATE SET
               consecutive_failures = 0,
               total_successes = total_successes + 1,
               last_success_at = datetime('now'),
               circuit_open = 0,
               circuit_opened_at = NULL,
               updated_at = datetime('now')",
            params![feed_origin, source_type],
        )?;
        Ok(())
    }

    /// Record a failed fetch for a specific feed/channel/handle.
    /// Opens the circuit after 5 consecutive failures.
    pub fn record_feed_failure(
        &self,
        feed_origin: &str,
        source_type: &str,
        error: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, total_failures, last_failure_at, last_error, updated_at)
             VALUES (?1, ?2, 1, 1, datetime('now'), ?3, datetime('now'))
             ON CONFLICT(feed_origin, source_type) DO UPDATE SET
               consecutive_failures = consecutive_failures + 1,
               total_failures = total_failures + 1,
               last_failure_at = datetime('now'),
               last_error = ?3,
               circuit_open = CASE WHEN consecutive_failures + 1 >= 5 THEN 1 ELSE circuit_open END,
               circuit_opened_at = CASE WHEN consecutive_failures + 1 >= 5 AND circuit_open = 0 THEN datetime('now') ELSE circuit_opened_at END,
               updated_at = datetime('now')",
            params![feed_origin, source_type, error],
        )?;
        Ok(())
    }

    /// Check if a specific feed's circuit breaker is open.
    /// Auto-resets after 30 minutes (half-open state).
    pub fn is_feed_circuit_open(&self, feed_origin: &str, source_type: &str) -> bool {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT circuit_open, circuit_opened_at FROM feed_health WHERE feed_origin = ?1 AND source_type = ?2",
            params![feed_origin, source_type],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        );
        match result {
            Ok((1, Some(opened_at))) => {
                let stale = conn
                    .query_row(
                        "SELECT datetime(?1, '+30 minutes') <= datetime('now')",
                        params![opened_at],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap_or(false);
                if stale {
                    let _ = conn.execute(
                        "UPDATE feed_health SET circuit_open = 0, consecutive_failures = 0, updated_at = datetime('now') WHERE feed_origin = ?1 AND source_type = ?2",
                        params![feed_origin, source_type],
                    );
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Get health record for a specific feed.
    pub fn get_feed_health(&self, feed_origin: &str, source_type: &str) -> Option<FeedHealth> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT feed_origin, source_type, consecutive_failures, total_successes, total_failures, last_success_at, last_failure_at, last_error, circuit_open
             FROM feed_health WHERE feed_origin = ?1 AND source_type = ?2",
            params![feed_origin, source_type],
            |row| {
                Ok(FeedHealth {
                    feed_origin: row.get(0)?,
                    source_type: row.get(1)?,
                    consecutive_failures: row.get(2)?,
                    total_successes: row.get(3)?,
                    total_failures: row.get(4)?,
                    last_success_at: row.get(5)?,
                    last_failure_at: row.get(6)?,
                    last_error: row.get(7)?,
                    circuit_open: row.get::<_, i64>(8)? != 0,
                })
            },
        )
        .ok()
    }

    /// Get all feed health records for a source type (for UI listing).
    pub fn get_all_feed_health(&self, source_type: &str) -> Vec<FeedHealth> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT feed_origin, source_type, consecutive_failures, total_successes, total_failures, last_success_at, last_failure_at, last_error, circuit_open
             FROM feed_health WHERE source_type = ?1 ORDER BY feed_origin",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        stmt.query_map(params![source_type], |row| {
            Ok(FeedHealth {
                feed_origin: row.get(0)?,
                source_type: row.get(1)?,
                consecutive_failures: row.get(2)?,
                total_successes: row.get(3)?,
                total_failures: row.get(4)?,
                last_success_at: row.get(5)?,
                last_failure_at: row.get(6)?,
                last_error: row.get(7)?,
                circuit_open: row.get::<_, i64>(8)? != 0,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Reset a feed's circuit breaker (manual user override).
    pub fn reset_feed_health(&self, feed_origin: &str, source_type: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE feed_health SET consecutive_failures = 0, circuit_open = 0, circuit_opened_at = NULL, updated_at = datetime('now') WHERE feed_origin = ?1 AND source_type = ?2",
            params![feed_origin, source_type],
        )?;
        Ok(())
    }

    /// Get feedback summary aggregated by topic for scoring boost.
    /// Results are cached until invalidated by a feedback write.
    pub fn get_feedback_topic_summary(&self) -> SqliteResult<Vec<FeedbackTopicSummary>> {
        // Check cache first
        {
            let cache = FEEDBACK_TOPIC_CACHE.lock().unwrap_or_else(|e| {
                tracing::warn!("FEEDBACK_TOPIC_CACHE mutex poisoned, recovering");
                e.into_inner()
            });
            if let Some(ref cached) = *cache {
                return Ok(cached.clone());
            }
        }
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT si.title, f.relevant
             FROM feedback f
             JOIN source_items si ON f.source_item_id = si.id
             WHERE f.created_at > datetime('now', '-30 days')
             ORDER BY f.created_at DESC
             LIMIT 500",
        )?;

        let rows: Vec<(String, bool)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
            })?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in db_sources: {e}");
                    None
                }
            })
            .collect();

        let mut topic_map: std::collections::HashMap<String, (i64, i64)> =
            std::collections::HashMap::new();
        for (title, relevant) in &rows {
            let words: Vec<String> = title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .filter(|w| {
                    ![
                        "the", "and", "for", "with", "that", "this", "from", "have", "been",
                        "will", "what", "when", "where", "which", "about", "into", "your", "more",
                        "some", "show",
                    ]
                    .contains(w)
                })
                .map(std::string::ToString::to_string)
                .collect();

            for word in words.into_iter().take(5) {
                let entry = topic_map.entry(word).or_insert((0, 0));
                if *relevant {
                    entry.0 += 1;
                } else {
                    entry.1 += 1;
                }
            }
        }

        let mut summaries: Vec<FeedbackTopicSummary> = topic_map
            .into_iter()
            .filter(|(_, (saves, dismissals))| saves + dismissals >= 2)
            .map(|(topic, (saves, dismissals))| FeedbackTopicSummary {
                topic,
                saves,
                dismissals,
                net_score: (saves as f64 - dismissals as f64) / (saves + dismissals) as f64,
            })
            .collect();

        summaries.sort_by(|a, b| {
            b.net_score
                .partial_cmp(&a.net_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Store in cache
        {
            let mut cache = FEEDBACK_TOPIC_CACHE.lock().unwrap_or_else(|e| {
                tracing::warn!("FEEDBACK_TOPIC_CACHE mutex poisoned, recovering");
                e.into_inner()
            });
            *cache = Some(summaries.clone());
        }

        Ok(summaries)
    }

    /// Count total feedback interactions (used for bootstrap mode detection)
    pub fn query_feedback_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
    }

    // ========================================================================
    // Scoring Stats
    // ========================================================================

    /// Record scoring run statistics (rejection rate measurement)
    pub fn record_scoring_stats(
        &self,
        run_type: &str,
        total_scored: usize,
        relevant_count: usize,
        excluded_count: usize,
    ) -> SqliteResult<()> {
        if total_scored == 0 {
            return Ok(());
        }
        let rejection_rate = (total_scored - relevant_count) as f64 / total_scored as f64;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO scoring_stats (run_type, total_scored, relevant_count, excluded_count, rejection_rate)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run_type,
                total_scored as i64,
                relevant_count as i64,
                excluded_count as i64,
                rejection_rate
            ],
        )?;
        info!(
            target: "4da::scoring",
            run_type,
            total_scored,
            relevant_count,
            rejection_rate = format!("{:.1}%", rejection_rate * 100.0),
            "Scoring stats recorded"
        );
        Ok(())
    }

    /// Persist necessity scores for items so MCP server can query them.
    /// Uses INSERT OR REPLACE (upsert) — safe to call repeatedly.
    pub fn persist_necessity_scores(
        &self,
        items: &[(u64, f32, Option<String>, Option<String>, Option<String>)],
    ) -> SqliteResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock();
        // Ensure table exists (graceful for pre-migration DBs)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS item_necessity (
                source_item_id INTEGER PRIMARY KEY REFERENCES source_items(id),
                necessity_score REAL NOT NULL DEFAULT 0.0,
                necessity_reason TEXT,
                necessity_category TEXT,
                necessity_urgency TEXT,
                scored_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )?;
        // Explicit transaction — without this, each execute() autocommits
        // individually, causing 1000+ WAL writes for a typical scoring batch.
        // A single transaction reduces I/O by ~99%.
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO item_necessity (source_item_id, necessity_score, necessity_reason, necessity_category, necessity_urgency, scored_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            )?;
            for (id, score, reason, category, urgency) in items {
                stmt.execute(params![id, score, reason, category, urgency])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Get aggregate scoring stats (lifetime rejection rate)
    pub fn get_scoring_stats(&self) -> SqliteResult<ScoringStatsAggregate> {
        let conn = self.conn.lock();
        let (total_runs, total_scored, total_relevant): (i64, i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_scored), 0), COALESCE(SUM(relevant_count), 0)
                 FROM scoring_stats",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let lifetime_rejection_rate = if total_scored > 0 {
            (total_scored - total_relevant) as f64 / total_scored as f64
        } else {
            0.0
        };
        let last_run_rejection: Option<f64> = conn
            .query_row(
                "SELECT rejection_rate FROM scoring_stats ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(ScoringStatsAggregate {
            total_runs,
            total_scored,
            total_relevant,
            lifetime_rejection_rate,
            last_run_rejection_rate: last_run_rejection,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{insert_test_item, seed_embedding, test_db};

    #[test]
    fn test_upsert_source_item_insert_and_update() {
        let db = test_db();
        let emb = seed_embedding("hn:42");

        // Insert
        let id1 = db
            .upsert_source_item("hackernews", "42", None, "Original Title", "content", &emb)
            .unwrap();
        assert!(id1 > 0);

        // Verify original title
        let item = db.get_source_item("hackernews", "42").unwrap().unwrap();
        assert_eq!(item.title, "Original Title");

        // Update with same source_type + source_id
        let id2 = db
            .upsert_source_item(
                "hackernews",
                "42",
                None,
                "Updated Title",
                "new content",
                &emb,
            )
            .unwrap();
        assert_eq!(
            id1, id2,
            "Upsert should return same id for same source_type+source_id"
        );

        // Verify title changed
        let item = db.get_source_item("hackernews", "42").unwrap().unwrap();
        assert_eq!(item.title, "Updated Title");
    }

    #[test]
    fn test_upsert_duplicate_source_id() {
        let db = test_db();
        let emb = seed_embedding("reddit:abc");

        let id1 = db
            .upsert_source_item("reddit", "abc", None, "Title A", "content a", &emb)
            .unwrap();
        let id2 = db
            .upsert_source_item("reddit", "abc", None, "Title B", "content b", &emb)
            .unwrap();

        assert_eq!(
            id1, id2,
            "Same source_type+source_id should return same row id"
        );
        assert_eq!(db.total_item_count().unwrap(), 1);
    }

    #[test]
    fn test_get_sources_info_empty_db() {
        let db = test_db();
        let sources = db.get_all_sources().unwrap();
        assert!(
            sources.is_empty(),
            "Empty DB should have no registered sources"
        );
    }

    #[test]
    fn test_record_and_get_source_health() {
        let db = test_db();

        // Record a healthy fetch
        db.record_source_health("hackernews", true, 25, 150, None)
            .unwrap();

        let health = db.get_source_health().unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].source_type, "hackernews");
        assert_eq!(health[0].status, "healthy");
        assert_eq!(health[0].items_fetched, 25);
        assert_eq!(health[0].consecutive_failures, 0);

        // Record an error
        db.record_source_health("hackernews", false, 0, 0, Some("timeout"))
            .unwrap();

        let health = db.get_source_health().unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].status, "error");
        assert_eq!(health[0].consecutive_failures, 1);
    }

    #[test]
    fn test_record_feedback_positive_and_negative() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "fb_1", "Feedback Test", "Some content");

        // Record positive feedback
        db.record_feedback(id, true).unwrap();
        assert_eq!(db.query_feedback_count().unwrap(), 1);

        // Record negative feedback
        db.record_feedback(id, false).unwrap();
        assert_eq!(db.query_feedback_count().unwrap(), 2);
    }

    // ------------------------------------------------------------------------
    // Balanced-by-source query tests — prove we fix the "community floods
    // the chapters" bug that left Security/News/Research at 0/0.
    // ------------------------------------------------------------------------

    #[test]
    fn test_balanced_query_prevents_high_volume_source_starvation() {
        let db = test_db();

        // Simulate realistic volumes:
        //   Reddit (community): 100 items
        //   Lobsters (community): 80 items
        //   CVE (security): 5 items
        //   ArXiv (research): 3 items
        //   Npm (package_registry): 2 items
        for i in 0..100 {
            insert_test_item(
                &db,
                "reddit",
                &format!("r_{i}"),
                &format!("Reddit item {i}"),
                "content",
            );
        }
        for i in 0..80 {
            insert_test_item(
                &db,
                "lobsters",
                &format!("l_{i}"),
                &format!("Lobsters item {i}"),
                "content",
            );
        }
        for i in 0..5 {
            insert_test_item(
                &db,
                "cve",
                &format!("cve_{i}"),
                &format!("CVE-2026-{i:04}"),
                "content",
            );
        }
        for i in 0..3 {
            insert_test_item(
                &db,
                "arxiv",
                &format!("a_{i}"),
                &format!("arXiv paper {i}"),
                "content",
            );
        }
        for i in 0..2 {
            insert_test_item(
                &db,
                "npm_registry",
                &format!("n_{i}"),
                &format!("npm release {i}"),
                "content",
            );
        }

        // OLD behavior: get_items_since_hours with limit 50 would return
        // ~45 reddit + 5 lobsters and zero from CVE/arXiv/npm.
        let naive = db.get_items_since_hours(168, 50).unwrap();
        let naive_sources: std::collections::HashSet<&str> =
            naive.iter().map(|i| i.source_type.as_str()).collect();
        // Not asserting exact breakdown on naive — we just want to prove the
        // balanced version is better.

        // NEW behavior: balanced query with per-source cap of 10, overall 50
        let balanced = db.get_items_balanced_by_source(168, 10, 50).unwrap();
        let balanced_sources: std::collections::HashSet<&str> =
            balanced.iter().map(|i| i.source_type.as_str()).collect();

        // EVERY source that has items must be represented
        assert!(
            balanced_sources.contains("reddit"),
            "Balanced query must include reddit"
        );
        assert!(
            balanced_sources.contains("lobsters"),
            "Balanced query must include lobsters"
        );
        assert!(
            balanced_sources.contains("cve"),
            "Balanced query must include cve (was starved in naive query)"
        );
        assert!(
            balanced_sources.contains("arxiv"),
            "Balanced query must include arxiv (was starved in naive query)"
        );
        assert!(
            balanced_sources.contains("npm_registry"),
            "Balanced query must include npm_registry (was starved in naive query)"
        );

        // Balanced must be at least as source-diverse as naive
        assert!(
            balanced_sources.len() >= naive_sources.len(),
            "Balanced query must never be LESS diverse than naive query"
        );

        // Per-source cap must be respected
        let reddit_count = balanced
            .iter()
            .filter(|i| i.source_type == "reddit")
            .count();
        assert!(
            reddit_count <= 10,
            "Reddit count {reddit_count} exceeded per-source cap of 10"
        );
        let lobsters_count = balanced
            .iter()
            .filter(|i| i.source_type == "lobsters")
            .count();
        assert!(
            lobsters_count <= 10,
            "Lobsters count {lobsters_count} exceeded per-source cap of 10"
        );

        // Overall limit must be respected
        assert!(balanced.len() <= 50, "Overall limit of 50 exceeded");
    }

    #[test]
    fn test_balanced_query_handles_single_source() {
        let db = test_db();

        for i in 0..20 {
            insert_test_item(
                &db,
                "hackernews",
                &format!("h_{i}"),
                &format!("HN item {i}"),
                "content",
            );
        }

        let balanced = db.get_items_balanced_by_source(168, 5, 50).unwrap();
        // Single source should still respect per_source_cap
        assert_eq!(balanced.len(), 5);
        assert!(balanced.iter().all(|i| i.source_type == "hackernews"));
    }

    #[test]
    fn test_balanced_query_empty_db_returns_empty() {
        let db = test_db();
        let balanced = db.get_items_balanced_by_source(168, 10, 50).unwrap();
        assert!(balanced.is_empty());
    }

    #[test]
    fn test_get_items_tiered_now_returns_diverse_sources() {
        let db = test_db();

        // High volume community source
        for i in 0..500 {
            insert_test_item(
                &db,
                "reddit",
                &format!("r_{i}"),
                &format!("Reddit item {i}"),
                "content",
            );
        }
        // Low volume security source
        for i in 0..3 {
            insert_test_item(
                &db,
                "cve",
                &format!("cve_{i}"),
                &format!("CVE-2026-{i:04}"),
                "content",
            );
        }

        // get_items_tiered now routes through balanced
        let items = db.get_items_tiered(168, 1000).unwrap();
        let sources: std::collections::HashSet<&str> =
            items.iter().map(|i| i.source_type.as_str()).collect();

        assert!(
            sources.contains("cve"),
            "get_items_tiered must include CVE items even when Reddit has 500 items"
        );
    }

    // ========================================================================
    // Per-Feed Health Tests
    // ========================================================================

    #[test]
    fn test_feed_health_success_resets_failures() {
        let db = test_db();
        db.record_feed_failure("https://example.com/feed", "rss", "timeout")
            .unwrap();
        db.record_feed_failure("https://example.com/feed", "rss", "timeout")
            .unwrap();

        let health = db
            .get_feed_health("https://example.com/feed", "rss")
            .unwrap();
        assert_eq!(health.consecutive_failures, 2);

        db.record_feed_success("https://example.com/feed", "rss")
            .unwrap();

        let health = db
            .get_feed_health("https://example.com/feed", "rss")
            .unwrap();
        assert_eq!(health.consecutive_failures, 0);
        assert!(!health.circuit_open);
        assert_eq!(health.total_successes, 1);
        assert_eq!(health.total_failures, 2);
    }

    #[test]
    fn test_feed_circuit_opens_after_5_failures() {
        let db = test_db();
        for _ in 0..5 {
            db.record_feed_failure("https://bad.com/feed", "rss", "connection refused")
                .unwrap();
        }

        let health = db.get_feed_health("https://bad.com/feed", "rss").unwrap();
        assert!(health.circuit_open);
        assert_eq!(health.consecutive_failures, 5);
        assert!(db.is_feed_circuit_open("https://bad.com/feed", "rss"));
    }

    #[test]
    fn test_feed_health_independent_per_feed() {
        let db = test_db();
        db.record_feed_failure("https://bad.com/feed", "rss", "error")
            .unwrap();
        db.record_feed_success("https://good.com/feed", "rss")
            .unwrap();

        let bad = db.get_feed_health("https://bad.com/feed", "rss").unwrap();
        assert_eq!(bad.consecutive_failures, 1);

        let good = db.get_feed_health("https://good.com/feed", "rss").unwrap();
        assert_eq!(good.consecutive_failures, 0);
    }

    #[test]
    fn test_get_all_feed_health_filters_by_type() {
        let db = test_db();
        db.record_feed_success("https://feed1.com", "rss").unwrap();
        db.record_feed_success("UCtest123", "youtube").unwrap();

        let rss_health = db.get_all_feed_health("rss");
        assert_eq!(rss_health.len(), 1);
        assert_eq!(rss_health[0].feed_origin, "https://feed1.com");

        let yt_health = db.get_all_feed_health("youtube");
        assert_eq!(yt_health.len(), 1);
    }

    #[test]
    fn test_reset_feed_health() {
        let db = test_db();
        for _ in 0..5 {
            db.record_feed_failure("https://bad.com/feed", "rss", "error")
                .unwrap();
        }
        assert!(db.is_feed_circuit_open("https://bad.com/feed", "rss"));

        db.reset_feed_health("https://bad.com/feed", "rss").unwrap();
        assert!(!db.is_feed_circuit_open("https://bad.com/feed", "rss"));
    }
}
