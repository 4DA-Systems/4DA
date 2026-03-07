//! Source item CRUD, feedback, source registry, and health tracking.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use std::sync::{LazyLock, Mutex};
use tracing::info;

use super::StoredSourceItem;
use super::{
    blob_to_embedding, embedding_to_blob, hash_content_parts, parse_datetime, Database,
    ScoringStatsAggregate,
};

// Cache for feedback topic summary — invalidated on feedback writes.
static FEEDBACK_TOPIC_CACHE: LazyLock<Mutex<Option<Vec<FeedbackTopicSummary>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Invalidate the feedback topic summary cache.
/// Must be called after any feedback write (record_feedback, etc.).
pub fn invalidate_feedback_topic_cache() {
    let mut cache = FEEDBACK_TOPIC_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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
    /// Store or update a source item (also updates vec0 index)
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
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embedding = ?5, last_seen = datetime('now') WHERE id = ?6",
                params![url, title, content, content_hash, embedding_blob, id],
            )?;
            tx.execute(
                "UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            tx.commit()?;
            Ok(id)
        } else {
            tx.execute(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![source_type, source_id, url, title, content, content_hash, embedding_blob],
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

    /// Batch upsert source items in a transaction (much faster than individual calls)
    #[allow(clippy::type_complexity)]
    pub fn batch_upsert_source_items(
        &self,
        items: &[(String, String, Option<String>, String, String, Vec<f32>)],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut check_stmt = tx.prepare_cached(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            )?;
            let mut update_stmt = tx.prepare_cached(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embedding = ?5, last_seen = datetime('now') WHERE id = ?6",
            )?;
            let mut update_vec_stmt =
                tx.prepare_cached("UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2")?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            )?;
            let mut insert_vec_stmt =
                tx.prepare_cached("INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)")?;

            for (source_type, source_id, url, title, content, embedding) in items {
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
                        embedding_blob
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
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
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

    /// Get source items by type
    pub fn get_source_items(
        &self,
        source_type: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
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
            })
        })?;

        rows.collect()
    }

    /// Get recent source items within a time window (hours)
    pub fn get_items_since_hours(
        &self,
        hours: i64,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
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
            })
        })?;

        rows.collect()
    }

    /// Get items added since a specific ISO timestamp (for differential analysis)
    pub fn get_items_since_timestamp(
        &self,
        since: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
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
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
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
        .map(|v| v.flatten())
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
        let conn = self.conn.lock();

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
                    let _ = conn.execute(
                        "UPDATE source_health SET consecutive_failures = 0, status = 'error' WHERE source_type = ?1",
                        params![source_type],
                    );
                    tracing::info!(target: "4da::health", source = source_type, "Circuit breaker auto-reset after cooldown");
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Get feedback summary aggregated by topic for scoring boost.
    /// Results are cached until invalidated by a feedback write.
    pub fn get_feedback_topic_summary(&self) -> SqliteResult<Vec<FeedbackTopicSummary>> {
        // Check cache first
        {
            let cache = FEEDBACK_TOPIC_CACHE
                .lock()
                .unwrap_or_else(|e| e.into_inner());
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
            .filter_map(|r| r.ok())
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
                .map(|s| s.to_string())
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
            let mut cache = FEEDBACK_TOPIC_CACHE
                .lock()
                .unwrap_or_else(|e| e.into_inner());
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
}
