// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scoring-funnel DB queries — the read/requeue layer for the relevance funnel.
//!
//! Extracted from `cache.rs` (which exceeded the file-size limit) to keep the funnel's
//! query surface in one coherent place: the recall-audit rows (Phase 0), the
//! never-scored backlog drain (Phase 2), and the dependency-change re-examination
//! candidates + requeue (Phase 3). The stale-VERSION drain (`get_stale_scored_items`,
//! `mark_items_scored_version`) remains in `cache.rs` alongside `persist_analysis_scores`.

use rusqlite::{params, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};

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

impl Database {
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

    /// Count items that have NEVER been through a scoring run, respecting the tier
    /// history window (Signal = unlimited, Free = 30 days). This is the backlog the
    /// Phase-2 backfill worker drains.
    ///
    /// The predicate is `scored_pipeline_version = 0` (the column's default before any
    /// scoring run), NOT `relevance_score IS NULL`. This matters: a noise item that
    /// scores exactly 0.0 gets version-stamped but no relevance_score written (the
    /// version stamp is the canonical "has been scored" marker — same as the analysis
    /// path). Keying on the version stamp guarantees scored items leave the backlog and
    /// can never be re-picked forever. Distinct from the stale-VERSION drain, which
    /// handles already-scored items at versions 1..<current.
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
        let sql = format!(
            "SELECT COUNT(*) FROM source_items WHERE scored_pipeline_version = 0{time_clause}"
        );
        conn.query_row(&sql, [], |r| r.get(0))
    }

    /// A chunk of NEVER-scored items for the backfill worker, in PRIORITY order:
    /// high-stakes first (security/breaking/CVE — error-cost asymmetry), then stack
    /// releases, then most-recent. This realises the "prioritize, don't discard"
    /// design: everything is scored eventually, the highest-value items first.
    ///
    /// "Never scored" = `scored_pipeline_version = 0` (the default before any scoring
    /// run), NOT `relevance_score IS NULL` — so an item that scores 0.0 (relevance left
    /// unwritten, version stamped) leaves the backlog instead of being re-picked forever.
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
             WHERE scored_pipeline_version = 0{time_clause}
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

    /// Candidates for dependency-change re-examination (Phase 3): items scored as noise
    /// (< `threshold`) whose content_type is one a dependency match would FLIP —
    /// releases and security/breaking advisories. Casual mentions (discussions) are
    /// excluded because a dep match doesn't change their verdict, so re-scoring them
    /// would be wasted work. Returns (id, title, content) for the canonical dep matcher.
    pub fn get_reexaminable_candidates(
        &self,
        threshold: f32,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, title, content
             FROM source_items
             WHERE relevance_score IS NOT NULL
               AND relevance_score < ?1
               AND scored_pipeline_version >= 1
               AND content_type IN
                   ('release_notes', 'platform_update', 'security_advisory', 'breaking_change')
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![threshold as f64, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect()
    }

    /// Reset `scored_pipeline_version` to 0 for the given items so the backfill worker
    /// re-scores them (prioritized) against the current profile. Used by Phase-3
    /// re-examination. Batched in one transaction; returns the number reset.
    pub fn requeue_items_by_ids(&self, ids: &[i64]) -> SqliteResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items SET scored_pipeline_version = 0 WHERE id = ?1",
            )?;
            for id in ids {
                count += stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Scored high-stakes items (security/breaking/CVE) for the calibration monitor's
    /// dep-scoped recall check (Phase 5b). Returns (id, title, content, relevance_score)
    /// so the caller can run the canonical dep matcher and find advisories that affect
    /// the user's stack yet scored as noise — a concrete recall bug. Read-only.
    pub fn get_scored_high_stakes_items(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, f64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, title, content, relevance_score
             FROM source_items
             WHERE relevance_score IS NOT NULL
               AND (cve_ids IS NOT NULL
                    OR content_type IN ('security_advisory', 'breaking_change'))
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect()
    }
}
