// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scoring-funnel DB queries — the read/requeue layer for the relevance funnel.
//!
//! Extracted from `cache.rs` (which exceeded the file-size limit) to keep the funnel's
//! query surface in one coherent place: the recall-audit rows (Phase 0), the
//! never-scored backlog drain (Phase 2), and the dependency-change re-examination
//! candidates + requeue (Phase 3). The stale-VERSION drain and the score-persist
//! boundary (`persist_analysis_scores` + churn instrumentation, `mark_items_scored_version`,
//! `get_stale_scored_items`) moved here 2026-08-23 when `cache.rs` crossed the limit again.

use rusqlite::{params, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};

/// Persist-boundary score hysteresis (2026-08-23 audit, item 10): a same-item
/// re-score moving less than this keeps the OLD durable score (the version
/// stamp still advances). The audit measured ~300 items jittering 0.37–0.43
/// across consecutive 30-minute runs — sub-0.05 wobble is measurement noise,
/// not information, and re-writing it churned every ranking surface daily.
/// Deliberately BELOW the 0.10 churn-telemetry threshold: everything the
/// damper eats was already invisible to the churn counters.
pub const SCORE_WRITE_HYSTERESIS: f64 = 0.05;

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
                    feed_origin, tags, published_at
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
                published_at: crate::db::parse_datetime_opt(
                    row.get::<_, Option<String>>(13).ok().flatten(),
                ),
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

    /// Persist relevance scores from in-memory analysis back to the database.
    /// Called after scoring completes so the DB fallback path has real scores.
    ///
    /// `path` labels the persisting pipeline (`"analysis"` / `"backfill"` /
    /// `"drain"`) in the `scoring_churn` summary row this writes alongside the
    /// scores. Same-version re-scores can move items materially (2026-08-22
    /// live: 0.94 → 0.50 at the same PIPELINE_VERSION), and until this row
    /// existed the only way to see that churn was an ad-hoc snapshot diff of
    /// the whole table. The old score is read inside the same transaction, so
    /// the delta is exact, not racy.
    ///
    /// **Persist hysteresis (2026-08-23 audit, item 10):** a re-score whose
    /// move is smaller than [`SCORE_WRITE_HYSTERESIS`] keeps the OLD durable
    /// score — sub-noise jitter (the audit measured ~300 items wobbling
    /// 0.37–0.43 every 30 minutes) stops rewriting the feed's ranking substrate.
    /// The row is STILL stamped (`scored_pipeline_version` + signal columns):
    /// skipping the stamp would trap damped items in the version-drain set
    /// forever. First-ever scores (old NULL) always write. Churn statistics
    /// are computed over the RAW deltas (what the scorer produced), with
    /// `suppressed_writes` recording how many the damper kept at the old value.
    pub fn persist_analysis_scores(
        &self,
        scores: &[(i64, f32, Option<String>, Option<String>)],
        path: &str,
    ) -> SqliteResult<usize> {
        /// A move this large is churn worth counting (matches the forensic
        /// threshold the 2026-08-22/23 audits used).
        const CHURN_DELTA: f64 = 0.10;
        /// Raw moves below this floor are invisible at every consumer and are
        /// excluded from the `top_offenders` forensic list.
        const OFFENDER_FLOOR: f64 = 0.01;
        /// Largest-|Δ| movers recorded per persist batch.
        const TOP_OFFENDERS_CAP: usize = 10;

        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        let mut rescored: i64 = 0;
        let mut moved_up: i64 = 0;
        let mut moved_down: i64 = 0;
        let mut max_up: f64 = 0.0;
        let mut max_down: f64 = 0.0;
        let mut sum_abs: f64 = 0.0;
        let mut suppressed: i64 = 0;
        // (id, old, new) raw movers — sorted/truncated into `top_offenders`.
        let mut movers: Vec<(i64, f64, f64)> = Vec::new();
        {
            let mut read_stmt =
                tx.prepare_cached("SELECT relevance_score FROM source_items WHERE id = ?1")?;
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items SET relevance_score = ?1, scored_pipeline_version = ?2, signal_type = ?3, signal_priority = ?4 WHERE id = ?5",
            )?;
            for (id, score, signal_type, signal_priority) in scores {
                // Old score first (same txn): NULL = first-ever score, not churn.
                let old: Option<f64> = read_stmt
                    .query_row(params![id], |r| r.get::<_, Option<f64>>(0))
                    .unwrap_or(None);
                let mut write_score = f64::from(*score);
                if let Some(old) = old {
                    let delta = write_score - old;
                    rescored += 1;
                    sum_abs += delta.abs();
                    if delta > CHURN_DELTA {
                        moved_up += 1;
                    } else if delta < -CHURN_DELTA {
                        moved_down += 1;
                    }
                    max_up = max_up.max(delta);
                    max_down = max_down.max(-delta);
                    if delta.abs() >= OFFENDER_FLOOR {
                        movers.push((*id, old, write_score));
                    }
                    if delta.abs() < SCORE_WRITE_HYSTERESIS {
                        // Keep the durable score; the stamp below still writes.
                        suppressed += 1;
                        write_score = old;
                    }
                }
                stmt.execute(params![
                    write_score,
                    crate::scoring::PIPELINE_VERSION,
                    signal_type,
                    signal_priority,
                    id
                ])?;
                count += 1;
            }
        }
        if count > 0 {
            let mean_abs = if rescored > 0 {
                sum_abs / rescored as f64
            } else {
                0.0
            };
            movers.sort_by(|a, b| {
                (b.2 - b.1)
                    .abs()
                    .partial_cmp(&(a.2 - a.1).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            movers.truncate(TOP_OFFENDERS_CAP);
            // One grep instead of a forensic hunt: name the biggest movers.
            if !movers.is_empty() {
                let top3 = movers
                    .iter()
                    .take(3)
                    .map(|(id, old, new)| format!("id={id} {old:.3}->{new:.3}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                tracing::info!(
                    target: "4da::churn",
                    path,
                    rescored,
                    suppressed_writes = suppressed,
                    top_movers = %top3,
                    "Score churn summary"
                );
            }
            let top_offenders: Option<String> = if movers.is_empty() {
                None
            } else {
                let arr: Vec<serde_json::Value> = movers
                    .iter()
                    .map(|(id, old, new)| {
                        serde_json::json!({
                            "id": id,
                            "old": (old * 1000.0).round() / 1000.0,
                            "new": (new * 1000.0).round() / 1000.0,
                        })
                    })
                    .collect();
                serde_json::to_string(&arr).ok()
            };
            tx.execute(
                "INSERT INTO scoring_churn (path, pipeline_version, items_written, rescored,
                    moved_up_gt_010, moved_down_gt_010, max_up, max_down, mean_abs_delta,
                    suppressed_writes, top_offenders)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    path,
                    crate::scoring::PIPELINE_VERSION,
                    count,
                    rescored,
                    moved_up,
                    moved_down,
                    max_up,
                    max_down,
                    mean_abs,
                    suppressed,
                    top_offenders
                ],
            )?;
        }
        tx.commit()?;
        Ok(count)
    }

    /// How many already-scored items are stamped below `current_version` — the
    /// pending stale-version drain, using the SAME predicate family (incl. the
    /// tier window) as [`Database::get_stale_scored_items`]. The differential
    /// gate reads this: a backlog bigger than one drain batch means a pipeline
    /// bump just landed and the run should take the full window instead.
    pub fn count_stale_scored_items(&self, current_version: i32) -> SqliteResult<i64> {
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
            "SELECT COUNT(*) FROM source_items
             WHERE scored_pipeline_version < ?1
               AND relevance_score IS NOT NULL{time_clause}"
        );
        conn.query_row(&sql, params![current_version], |r| r.get(0))
    }

    /// Among `ids`, the ones whose durable curation is FRESH: a persisted
    /// `relevance_score` AND a `feed_verdict_at` within `max_age_days`.
    ///
    /// This is the working set of the degraded-input persist guard (2026-08-23
    /// audit, item 11): a scoring run whose inputs systemically collapsed must
    /// not overwrite these rows. `feed_verdict_at` stands in for "when the
    /// durable score was last written" — every persisting cycle stamps it in
    /// the same breath as the score, and no dedicated score timestamp exists.
    /// An item older than the window (or never curated) is deliberately NOT
    /// returned: accepting the degraded write there beats freezing forever.
    pub fn ids_with_fresh_durable_scores(
        &self,
        ids: &[i64],
        max_age_days: u32,
    ) -> SqliteResult<std::collections::HashSet<i64>> {
        let mut fresh = std::collections::HashSet::new();
        if ids.is_empty() {
            return Ok(fresh);
        }
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT 1 FROM source_items
             WHERE id = ?1
               AND relevance_score IS NOT NULL
               AND COALESCE(feed_verdict_at, '1970-01-01') > datetime('now', ?2)",
        )?;
        let age_modifier = format!("-{max_age_days} days");
        for id in ids {
            if stmt.exists(params![id, age_modifier])? {
                fresh.insert(*id);
            }
        }
        Ok(fresh)
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
    /// The pending-drain COUNT lives in [`Database::count_stale_scored_items`],
    /// which must keep the same predicate.
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
    ///
    /// RECENCY-FIRST (Phase 1, scoring-drain elimination): the outermost sort key
    /// is now "is this item inside the ≤30-day visible window?" Every user-facing
    /// surface (Signal graph, Blind Spots, MCP, briefings) reads a recency-bounded
    /// window; ordering the drain by score alone meant a fresh 2-day item waited
    /// behind high-scoring 3-week-old items, so a version bump did NOT converge the
    /// windows the user actually looks at until deep into the drain. Front-loading
    /// the recent window means the visible surfaces re-score in the FIRST chunks
    /// (correct within minutes) while the invisible cold tail — consumed only by
    /// the drain itself, autophagy (now current-version-gated), and diagnostics —
    /// re-stamps unwatched afterward. This preserves the stack-update intent: a
    /// genuine stack update is a *recent* release, so it is in the recent window
    /// AND a release_notes row, i.e. tier-0 on both keys — it still drains first.
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
                    feed_origin, tags, published_at
             FROM source_items
             WHERE scored_pipeline_version < ?1
               AND relevance_score IS NOT NULL{time_clause}
             ORDER BY
                 CASE WHEN created_at >= datetime('now', '-30 days') THEN 0 ELSE 1 END,
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
                published_at: crate::db::parse_datetime_opt(
                    row.get::<_, Option<String>>(13).ok().flatten(),
                ),
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod stability_tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    fn score_and_version(db: &Database, id: i64) -> (Option<f64>, i64) {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT relevance_score, scored_pipeline_version FROM source_items WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    /// Item 10 (score side): a sub-hysteresis re-score keeps the durable score
    /// but STILL advances the version stamp — skipping the stamp would trap the
    /// item in the version-drain set forever. A move at/above the hysteresis
    /// writes through, and a first-ever score (old NULL) always writes.
    #[test]
    fn hysteresis_keeps_score_but_stamps_version() {
        let db = test_db();
        let wobble = insert_test_item(&db, "hackernews", "hy1", "Wobbler", "x");
        let mover = insert_test_item(&db, "hackernews", "hy2", "Mover", "x");
        let fresh = insert_test_item(&db, "hackernews", "hy3", "First score", "x");

        // Seed wobble+mover with a prior score at an OLD pipeline version, so
        // the version stamp is observable.
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.50, scored_pipeline_version = 1
                 WHERE id IN (?1, ?2)",
                params![wobble, mover],
            )
            .unwrap();
        }

        db.persist_analysis_scores(
            &[
                (wobble, 0.52, None, None), // |Δ| = 0.02 < 0.05 → damped
                (mover, 0.60, None, None),  // |Δ| = 0.10 ≥ 0.05 → written
                (fresh, 0.03, None, None),  // old NULL → always written
            ],
            "analysis",
        )
        .unwrap();

        let (s_wobble, v_wobble) = score_and_version(&db, wobble);
        assert_eq!(
            s_wobble,
            Some(0.50),
            "sub-hysteresis move keeps the old score"
        );
        assert_eq!(
            v_wobble,
            i64::from(crate::scoring::PIPELINE_VERSION),
            "damped write must still stamp the pipeline version (drain safety)"
        );
        let (s_mover, _) = score_and_version(&db, mover);
        assert!(
            (s_mover.unwrap() - 0.60).abs() < 1e-6,
            "real move writes through"
        );
        let (s_fresh, v_fresh) = score_and_version(&db, fresh);
        assert!(s_fresh.is_some(), "first-ever score always writes");
        assert_eq!(v_fresh, i64::from(crate::scoring::PIPELINE_VERSION));
    }

    /// Item 13: the churn row names WHICH items moved (top_offenders, raw
    /// deltas, largest first) and how many writes the damper suppressed.
    #[test]
    fn churn_row_carries_offender_list_and_suppressed_count() {
        let db = test_db();
        let a = insert_test_item(&db, "hackernews", "off_a", "big mover", "x");
        let b = insert_test_item(&db, "hackernews", "off_b", "small mover", "x");
        let c = insert_test_item(&db, "hackernews", "off_c", "damped", "x");
        db.persist_analysis_scores(
            &[
                (a, 0.90, None, None),
                (b, 0.40, None, None),
                (c, 0.50, None, None),
            ],
            "analysis",
        )
        .unwrap();
        db.persist_analysis_scores(
            &[
                (a, 0.50, None, None), // Δ = −0.40 → offender #1
                (b, 0.48, None, None), // Δ = +0.08 → offender #2 (written)
                (c, 0.52, None, None), // Δ = +0.02 → damped, below offender floor? 0.02 ≥ 0.01 → listed
            ],
            "analysis",
        )
        .unwrap();

        let conn = db.conn.lock();
        let (suppressed, offenders_json): (Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT suppressed_writes, top_offenders FROM scoring_churn ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(suppressed, Some(1), "exactly c's write was damped");
        let offenders: Vec<serde_json::Value> =
            serde_json::from_str(&offenders_json.expect("offender list present")).unwrap();
        assert_eq!(offenders.len(), 3, "all rescored movers ≥0.01 are listed");
        assert_eq!(
            offenders[0]["id"],
            serde_json::json!(a),
            "largest |Δ| first"
        );
        assert!((offenders[0]["old"].as_f64().unwrap() - 0.90).abs() < 1e-6);
        assert!((offenders[0]["new"].as_f64().unwrap() - 0.50).abs() < 1e-6);

        // First-ever batch (all old NULL) records no offenders and 0 suppressed.
        let (first_suppressed, first_offenders): (Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT suppressed_writes, top_offenders FROM scoring_churn ORDER BY id ASC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(first_suppressed, Some(0));
        assert!(
            first_offenders.is_none(),
            "first-ever scores are not movers"
        );
    }

    /// The differential gate's pending-drain count mirrors the drain predicate:
    /// stale = scored below the current version AND has a persisted score.
    #[test]
    fn count_stale_scored_items_mirrors_drain_predicate() {
        let db = test_db();
        let stale = insert_test_item(&db, "hackernews", "st1", "stale", "x");
        let current = insert_test_item(&db, "hackernews", "st2", "current", "x");
        let never = insert_test_item(&db, "hackernews", "st3", "never scored", "x");
        let v = crate::scoring::PIPELINE_VERSION;
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.5, scored_pipeline_version = ?1 WHERE id = ?2",
                params![v - 1, stale],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.5, scored_pipeline_version = ?1 WHERE id = ?2",
                params![v, current],
            )
            .unwrap();
            // `never` keeps relevance_score NULL / version 0 — that is the
            // NEVER-scored backlog (backfill's job), not the stale drain's.
            let _ = never;
        }
        assert_eq!(db.count_stale_scored_items(v).unwrap(), 1);
        assert_eq!(db.count_stale_scored_items(v - 1).unwrap(), 0);
    }

    /// Item 11's working-set probe: fresh durable curation = persisted score +
    /// recent verdict stamp. Old or never-curated rows fall out of protection
    /// (the escape hatch that prevents a permanently-degraded run from
    /// freezing the corpus).
    #[test]
    fn fresh_durable_score_probe_honors_the_age_escape_hatch() {
        let db = test_db();
        let fresh = insert_test_item(&db, "hackernews", "fd1", "fresh", "x");
        let old = insert_test_item(&db, "hackernews", "fd2", "old verdict", "x");
        let unscored = insert_test_item(&db, "hackernews", "fd3", "no score", "x");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.6, feed_verdict_at = datetime('now') WHERE id = ?1",
                params![fresh],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.6, feed_verdict_at = datetime('now', '-8 days') WHERE id = ?1",
                params![old],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_items SET feed_verdict_at = datetime('now') WHERE id = ?1",
                params![unscored],
            )
            .unwrap();
        }
        let protected = db
            .ids_with_fresh_durable_scores(&[fresh, old, unscored], 7)
            .unwrap();
        assert!(
            protected.contains(&fresh),
            "fresh durable score is protected"
        );
        assert!(
            !protected.contains(&old),
            ">7-day-old curation accepts the write"
        );
        assert!(
            !protected.contains(&unscored),
            "no durable score → nothing to protect"
        );
        assert!(db.ids_with_fresh_durable_scores(&[], 7).unwrap().is_empty());
    }
}
