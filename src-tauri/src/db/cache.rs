// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Caching, projection data, LLM content retrieval, briefings, and digest operations.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};
use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoringEvent {
    pub cycle_ts: String,
    pub total_scored: i64,
    pub total_relevant: i64,
    pub avg_score: f64,
    pub max_score: f64,
    pub gate_rejections: i64,
    pub commodity_caps: i64,
    pub briefing_items: i64,
}

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
    pub content_type: Option<String>,
}

/// Row for the relevance-triage recall audit (Phase 0 of the scoring funnel).
/// Carries exactly what the cheap gate reads plus the stored relevance_score.
#[derive(Debug, Clone)]
pub struct TriageAuditRow {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub content_type: Option<String>,
    pub cve_ids: Option<String>,
    pub relevance_score: Option<f64>,
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
            "SELECT id, title, url, source_type, created_at, content, relevance_score, content_type
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
                    content_type: row.get(7)?,
                })
            },
        )?;

        rows.collect()
    }

    /// Persist relevance scores from in-memory analysis back to the database.
    /// Called after scoring completes so the DB fallback path has real scores.
    pub fn persist_analysis_scores(
        &self,
        scores: &[(i64, f32, Option<String>, Option<String>)],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items SET relevance_score = ?1, scored_pipeline_version = ?2, signal_type = ?3, signal_priority = ?4 WHERE id = ?5",
            )?;
            for (id, score, signal_type, signal_priority) in scores {
                stmt.execute(params![
                    *score as f64,
                    crate::scoring::PIPELINE_VERSION,
                    signal_type,
                    signal_priority,
                    id
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Stamp `scored_pipeline_version` for every item that was scored this run,
    /// regardless of its relevance. This is load-bearing for the stale-drain:
    /// `persist_analysis_scores` only writes items with `top_score > 0`, so items
    /// that re-score to 0 (noise) would never be stamped, stay "stale" forever, and
    /// the relevance-ordered drain would re-pick the same zero-scorers every run —
    /// the backlog could never fully drain past a band of zero-scoring items. An
    /// item we scored IS scored at the current version even if the verdict is "noise".
    pub fn mark_items_scored_version(&self, ids: &[i64], version: i32) -> SqliteResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items SET scored_pipeline_version = ?1 WHERE id = ?2",
            )?;
            for id in ids {
                stmt.execute(params![version, id])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Get items whose scores were computed under an older pipeline version.
    /// These need re-scoring to reflect current pipeline logic.
    ///
    /// Ordering is deliberately NOT pure `relevance_score DESC`. A pipeline-version
    /// bump happens precisely because scoring CHANGED, and the change that matters
    /// most — the necessity stack-update path (try_stack_update_path) — RESCUES items
    /// that the old pipeline buried as noise: a release of your own dependency
    /// (`crates.io: axum v0.8.9`) used to be recency-decayed to a near-zero score.
    /// Ordering by old relevance DESC therefore drains the already-relevant items
    /// first and the buried releases LAST, so a dev's own stack updates only surface
    /// after the entire backlog drains (many scheduler cycles). We front-load
    /// `release_notes` / `platform_update` items (the stack-update candidates, an
    /// indexed `content_type`) so they re-score in the first drain batches and
    /// EcosystemShift items surface promptly; everything else keeps relevance DESC.
    pub fn get_stale_scored_items(
        &self,
        current_version: i32,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        // Signal users have unlimited history, so the recency bound is dropped ENTIRELY
        // for them. A "very large hours" sentinel does NOT work: passing i64::MAX (the
        // previous behaviour) to SQLite's datetime() overflows to NULL, and
        // `created_at >= NULL` is never true — so the drain silently returned ZERO stale
        // items for every Signal user. That was the real reason the version-bump drain
        // never reached the deep backlog (and stack releases never surfaced) on the
        // live, Signal-tier app: it wasn't slow, it was empty. Free users keep the
        // 30-day recency bound (their history is gated anyway). The constant is embedded
        // directly (it is a compile-time i64, never user input — no injection risk).
        let time_clause = if crate::settings::is_signal() {
            String::new()
        } else {
            format!(
                " AND created_at >= datetime('now', '-{} hours')",
                super::sources::FREE_HISTORY_LIMIT_HOURS
            )
        };
        let sql = format!(
            "SELECT id, source_type, source_id, url, title, content, content_hash,
                    embedding, created_at, last_seen, COALESCE(detected_lang, 'en'),
                    feed_origin, tags
             FROM source_items
             WHERE scored_pipeline_version < ?1
               AND relevance_score IS NOT NULL{time_clause}
             ORDER BY
                 CASE WHEN content_type IN ('release_notes', 'platform_update') THEN 0 ELSE 1 END,
                 relevance_score DESC
             LIMIT ?2"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![current_version, limit as i64], |row| {
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
                tags: row.get(12).ok().flatten(),
            })
        })?;
        rows.collect()
    }

    /// Rows for the relevance-triage recall audit (Phase 0 of the scoring funnel).
    /// Returns the exact fields the cheap gate reads (embedding + title/content +
    /// content_type + cve_ids) PLUS the stored `relevance_score`, so the audit can
    /// define the "currently relevant" set and measure whether the gate would ever
    /// drop one of them (a false negative). Read-only.
    ///
    /// `min_relevance = Some(t)`: only items with `relevance_score >= t`, ordered by
    /// score DESC (the relevant set). `None`: a uniform RANDOM sample of the whole
    /// corpus (for the overall keep-rate). Only items with a non-NULL embedding blob
    /// of the expected size are returned.
    pub fn get_triage_audit_rows(
        &self,
        min_relevance: Option<f64>,
        limit: usize,
    ) -> SqliteResult<Vec<TriageAuditRow>> {
        let conn = self.conn.lock();
        let sql = if min_relevance.is_some() {
            "SELECT id, title, content, embedding, content_type, cve_ids, relevance_score
             FROM source_items
             WHERE embedding IS NOT NULL AND relevance_score >= ?1
             ORDER BY relevance_score DESC
             LIMIT ?2"
        } else {
            "SELECT id, title, content, embedding, content_type, cve_ids, relevance_score
             FROM source_items
             WHERE embedding IS NOT NULL
             ORDER BY RANDOM()
             LIMIT ?2"
        };
        let mut stmt = conn.prepare(sql)?;
        // Bind ?1 even in the random branch (ignored) so the param set is uniform.
        let min = min_relevance.unwrap_or(0.0);
        let rows = stmt.query_map(params![min, limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(3)?;
            Ok(TriageAuditRow {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                embedding: blob_to_embedding(&embedding_blob),
                content_type: row.get(4).ok().flatten(),
                cve_ids: row.get(5).ok().flatten(),
                relevance_score: row.get(6).ok().flatten(),
            })
        })?;
        rows.collect()
    }

    /// Count items that have NEVER been scored (`relevance_score IS NULL`), respecting
    /// the tier history window (Signal = unlimited, Free = 30 days). This is the
    /// backlog the Phase-2 backfill worker drains — distinct from the stale-VERSION
    /// drain (which only touches already-scored items).
    pub fn count_unscored_backlog(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let time_clause = if crate::settings::is_signal() {
            String::new()
        } else {
            format!(
                " AND created_at >= datetime('now', '-{} hours')",
                super::sources::FREE_HISTORY_LIMIT_HOURS
            )
        };
        let sql =
            format!("SELECT COUNT(*) FROM source_items WHERE relevance_score IS NULL{time_clause}");
        conn.query_row(&sql, [], |r| r.get(0))
    }

    /// A chunk of NEVER-scored items for the backfill worker, in PRIORITY order:
    /// high-stakes first (security/breaking/CVE — error-cost asymmetry), then stack
    /// releases, then most-recent. This realises the "prioritize, don't discard"
    /// design: everything is scored eventually, the highest-value items first.
    /// Mirrors the tier window logic of `get_stale_scored_items` (Signal drops the
    /// recency bound entirely — never an i64::MAX overflow).
    pub fn get_unscored_backlog_chunk(&self, limit: usize) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let time_clause = if crate::settings::is_signal() {
            String::new()
        } else {
            format!(
                " AND created_at >= datetime('now', '-{} hours')",
                super::sources::FREE_HISTORY_LIMIT_HOURS
            )
        };
        let sql = format!(
            "SELECT id, source_type, source_id, url, title, content, content_hash,
                    embedding, created_at, last_seen, COALESCE(detected_lang, 'en'),
                    feed_origin, tags
             FROM source_items
             WHERE relevance_score IS NULL{time_clause}
             ORDER BY
                 CASE
                     WHEN cve_ids IS NOT NULL
                          OR content_type IN ('security_advisory', 'breaking_change') THEN 0
                     WHEN content_type IN ('release_notes', 'platform_update') THEN 1
                     ELSE 2
                 END,
                 created_at DESC
             LIMIT ?1"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| {
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
                tags: row.get(12).ok().flatten(),
            })
        })?;
        rows.collect()
    }

    /// Record a scoring cycle event for audit trail and recalibration backtesting.
    pub fn record_scoring_event(
        &self,
        total_scored: usize,
        total_relevant: usize,
        avg_score: f32,
        max_score: f32,
        gate_rejections: usize,
        commodity_caps: usize,
        briefing_items: usize,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO scoring_events (total_scored, total_relevant, avg_score, max_score, gate_rejections, commodity_caps, briefing_items)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                total_scored as i64,
                total_relevant as i64,
                avg_score as f64,
                max_score as f64,
                gate_rejections as i64,
                commodity_caps as i64,
                briefing_items as i64,
            ],
        )?;
        Ok(())
    }

    pub fn get_recent_scoring_events(&self, limit: usize) -> SqliteResult<Vec<ScoringEvent>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT cycle_ts, total_scored, total_relevant, avg_score, max_score,
                    gate_rejections, commodity_caps, briefing_items
             FROM scoring_events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ScoringEvent {
                cycle_ts: row.get(0)?,
                total_scored: row.get(1)?,
                total_relevant: row.get(2)?,
                avg_score: row.get(3)?,
                max_score: row.get(4)?,
                gate_rejections: row.get(5)?,
                commodity_caps: row.get(6)?,
                briefing_items: row.get(7)?,
            })
        })?;
        rows.collect()
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
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen, COALESCE(detected_lang, 'en'), feed_origin, tags
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
                    feed_origin: row.get(11).ok().flatten(),
                    tags: row.get(12).ok().flatten(),
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

    /// The stale-drain must surface stack-release items FIRST, not last. A pipeline
    /// bump rescues releases the old pipeline buried as near-zero noise; ordering the
    /// drain purely by old relevance would re-score them only after the whole backlog
    /// drained. Releases (`release_notes`/`platform_update`) jump the queue ahead of
    /// higher-relevance non-release items so EcosystemShift items surface promptly.
    #[test]
    fn test_stale_drain_prioritizes_stack_releases() {
        use crate::scoring::PIPELINE_VERSION;
        let db = test_db();

        // A high-relevance non-release, plus two LOW-relevance stack releases — all
        // stale (version 1, well below current PIPELINE_VERSION).
        let high_discussion = insert_test_item(&db, "hackernews", "hn1", "Hot thread", "x");
        let low_release_a = insert_test_item(&db, "crates_io", "axum", "axum 0.8.9", "release");
        let low_release_b = insert_test_item(&db, "npm", "vite", "vite 7.0.0", "release");

        {
            let conn = db.conn.lock();
            // created_at set to now so the items fall inside the drain's recency window.
            conn.execute(
                "UPDATE source_items SET relevance_score=0.92, content_type='discussion', scored_pipeline_version=1, created_at=datetime('now') WHERE id=?1",
                rusqlite::params![high_discussion],
            ).unwrap();
            conn.execute(
                "UPDATE source_items SET relevance_score=0.05, content_type='release_notes', scored_pipeline_version=1, created_at=datetime('now') WHERE id=?1",
                rusqlite::params![low_release_a],
            ).unwrap();
            conn.execute(
                "UPDATE source_items SET relevance_score=0.03, content_type='release_notes', scored_pipeline_version=1, created_at=datetime('now') WHERE id=?1",
                rusqlite::params![low_release_b],
            ).unwrap();
        }

        let stale = db.get_stale_scored_items(PIPELINE_VERSION, 10).unwrap();
        assert_eq!(stale.len(), 3, "all three stale items are returned");
        // Releases come first despite far lower relevance; relevance DESC breaks ties
        // within the release group, then non-release items follow.
        assert_eq!(stale[0].id, low_release_a, "higher-relevance release first");
        assert_eq!(
            stale[1].id, low_release_b,
            "second release still before the discussion"
        );
        assert_eq!(
            stale[2].id, high_discussion,
            "high-relevance non-release drains only after the releases"
        );
    }

    /// Regression guard: a Signal user's stale-drain must reach items older than the
    /// free-tier window. The old code passed `i64::MAX` hours to `datetime('now', ...)`,
    /// which SQLite overflows to NULL — so `created_at >= NULL` was never true and the
    /// drain silently returned ZERO items for every Signal user, meaning the version-bump
    /// backlog (and stack releases) never drained on the live, Signal-tier app.
    #[test]
    fn test_stale_drain_no_overflow_for_unlimited_history() {
        use crate::scoring::PIPELINE_VERSION;
        let db = test_db();

        let old = insert_test_item(
            &db,
            "crates_io",
            "old_axum",
            "axum 0.1.0",
            "ancient release",
        );
        {
            let conn = db.conn.lock();
            // 400 days old — well beyond the 30-day free-tier window.
            conn.execute(
                "UPDATE source_items SET relevance_score=0.2, content_type='release_notes', scored_pipeline_version=1, created_at=datetime('now','-400 days') WHERE id=?1",
                rusqlite::params![old],
            ).unwrap();
        }

        let stale = db.get_stale_scored_items(PIPELINE_VERSION, 10).unwrap();
        if crate::settings::is_signal() {
            // Unlimited history: the recency bound is dropped entirely. If this returns
            // 0 again, the i64::MAX -> NULL overflow has regressed.
            assert_eq!(
                stale.len(),
                1,
                "Signal drain must reach items older than the free window"
            );
            assert_eq!(stale[0].id, old);
        } else {
            // Free tier: the 30-day recency bound correctly excludes a 400-day-old item.
            assert!(
                stale.is_empty(),
                "free-tier drain stays bounded to recent history"
            );
        }
    }

    /// The backfill worker must pull NEVER-scored items in PRIORITY order: high-stakes
    /// (security/breaking/CVE) first, then stack releases, then most-recent. Confirms the
    /// "prioritize, don't discard" design at the query layer.
    #[test]
    fn test_unscored_backlog_priority_ordering() {
        let db = test_db();

        // All start unscored (insert_test_item leaves relevance_score NULL).
        let high = insert_test_item(&db, "cve", "cve1", "advisory", "x");
        let release = insert_test_item(&db, "crates_io", "rel1", "tokio 1.5", "x");
        let recent_plain = insert_test_item(&db, "hackernews", "hn_new", "newest chatter", "x");
        let old_plain = insert_test_item(&db, "reddit", "rd_old", "old chatter", "x");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET content_type='security_advisory', created_at=datetime('now','-5 days') WHERE id=?1",
                rusqlite::params![high],
            ).unwrap();
            conn.execute(
                "UPDATE source_items SET content_type='release_notes', created_at=datetime('now','-3 days') WHERE id=?1",
                rusqlite::params![release],
            ).unwrap();
            conn.execute(
                "UPDATE source_items SET content_type='discussion', created_at=datetime('now') WHERE id=?1",
                rusqlite::params![recent_plain],
            ).unwrap();
            conn.execute(
                "UPDATE source_items SET content_type='discussion', created_at=datetime('now','-10 days') WHERE id=?1",
                rusqlite::params![old_plain],
            ).unwrap();
        }

        assert_eq!(db.count_unscored_backlog().unwrap(), 4);

        let chunk = db.get_unscored_backlog_chunk(10).unwrap();
        let ids: Vec<i64> = chunk.iter().map(|i| i.id).collect();
        // high-stakes first, then release, then plain by created_at DESC (recent before old).
        assert_eq!(
            ids,
            vec![high, release, recent_plain, old_plain],
            "backfill must prioritize high-stakes > releases > recency"
        );
    }
}
