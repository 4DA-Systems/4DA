// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM Judgment storage — CRUD operations for Tier 2 intelligence judgments.
//!
//! After ingestion, items scoring above a threshold are evaluated by the user's
//! configured LLM. Results are stored in `llm_judgments` and read by the
//! preemption/blind_spots feeds.

use rusqlite::{params, Result as SqliteResult};

use super::Database;

// ============================================================================
// Types
// ============================================================================

/// A stored LLM judgment for a source item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredJudgment {
    pub id: i64,
    pub source_item_id: i64,
    pub relevance_score: f64,
    pub explanation: String,
    /// JSON array of suggested actions (e.g. `["review_security", "investigate"]`).
    pub actions: Option<String>,
    pub confidence: f64,
    pub model: String,
    pub prompt_version: String,
    pub judged_at: String,
}

/// SQL predicate over a `source_items` alias: the row is a REGISTRY release of
/// a package the user's dependency graph matched (`{a}` = the alias).
///
/// An LLM relevance verdict never removes such a row. The judge sees a thin
/// profile (ten tech names, no dependency list) and is instructed to reject
/// what it "cannot confirm is in their stack", so it reads a release of the
/// user's own dependency as off-stack. Measured on the live corpus 2026-09-25:
/// 22 registry releases of real dependencies (rusqlite, sha2, better-sqlite3,
/// ed25519-dalek, sqlite-vec, …) were demoted `llm_reject` by Haiku — 9 by the
/// ingest gate, 5 by the verdict drain, 8 by both — and a literal local judge
/// (gemma4:26b) demoted `jsonwebtoken v11.1.0` for "not listed in your
/// dependencies" on its first engine cycle. Registry rows are grounded by their
/// SUBJECT package (`dep_linker::is_registry_source`), so `matched_deps` on one
/// is a deterministic dependency fact, not a title heuristic; editorial rows
/// that merely name a dependency stay demotable.
pub(crate) fn dependency_release_sql(alias: &str) -> String {
    let registries = crate::dep_linker::REGISTRY_SOURCE_TYPES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "({alias}.content_type = 'release_notes'
          AND {alias}.source_type IN ({registries})
          AND EXISTS (SELECT 1 FROM scoring_explanations dep_se
                      WHERE dep_se.source_item_id = {alias}.id
                        AND json_extract(dep_se.breakdown, '$.breakdown.matched_deps[0]') IS NOT NULL))"
    )
}

/// A curated feed item whose fresh judgment argues for demotion
/// (see `llm_judgments::apply_judgment_demotions`).
#[derive(Debug, Clone)]
pub struct LlmRejectCandidate {
    pub item_id: i64,
    pub title: String,
    pub judged_relevance: f64,
    pub confidence: f64,
}

// ============================================================================
// Database Operations
// ============================================================================

impl Database {
    /// Upsert an LLM judgment for a source item.
    /// Key: (source_item_id, prompt_version).
    pub fn upsert_llm_judgment(
        &self,
        source_item_id: i64,
        relevance_score: f64,
        explanation: &str,
        actions: Option<&str>,
        confidence: f64,
        model: &str,
        prompt_version: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO llm_judgments (source_item_id, relevance_score, explanation, actions, confidence, model, prompt_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_item_id, prompt_version) DO UPDATE SET
                 relevance_score = excluded.relevance_score,
                 explanation = excluded.explanation,
                 actions = excluded.actions,
                 confidence = excluded.confidence,
                 model = excluded.model,
                 judged_at = datetime('now')",
            params![source_item_id, relevance_score, explanation, actions, confidence, model, prompt_version],
        )?;
        Ok(())
    }

    /// Get the most recent LLM judgment for a source item (any prompt version).
    pub fn get_llm_judgment(&self, source_item_id: i64) -> SqliteResult<Option<StoredJudgment>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_item_id, relevance_score, explanation, actions, confidence, model, prompt_version, judged_at
             FROM llm_judgments WHERE source_item_id = ?1 ORDER BY judged_at DESC, id DESC LIMIT 1",
        )?;
        let result = stmt.query_row(params![source_item_id], map_judgment_row);
        match result {
            Ok(j) => Ok(Some(j)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get judgments above a relevance threshold, ordered by relevance.
    pub fn get_relevant_judgments(
        &self,
        min_relevance: f64,
        limit: usize,
    ) -> SqliteResult<Vec<StoredJudgment>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_item_id, relevance_score, explanation, actions, confidence, model, prompt_version, judged_at
             FROM llm_judgments
             WHERE relevance_score >= ?1
             ORDER BY relevance_score DESC, judged_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![min_relevance, limit as i64], map_judgment_row)?;
        rows.collect()
    }

    /// Get source item IDs that have no judgment yet and scored above a threshold.
    /// Only considers items from the last 7 days.
    ///
    /// Ranked read (audit items 12+26): the top-band SELECTION threshold stays
    /// on relevance_score (evidence decides membership); which of the band's
    /// members get judged first follows the shared rank-then-evidence order.
    pub fn get_unjudged_item_ids(&self, min_score: f64, limit: usize) -> SqliteResult<Vec<i64>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT si.id FROM source_items si
             LEFT JOIN llm_judgments lj ON si.id = lj.source_item_id
             WHERE lj.id IS NULL
               AND si.relevance_score >= ?1
               AND si.created_at >= datetime('now', '-7 days')
             ORDER BY {ranked}
             LIMIT ?2",
            ranked = super::ranked_order_expr("si")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![min_score, limit as i64], |row| row.get(0))?;
        rows.collect()
    }

    /// Get total number of stored judgments.
    pub fn get_judgment_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM llm_judgments", [], |row| row.get(0))
    }

    /// Curated (`feed_relevant = 1`), score-sourced items whose judgment under
    /// `prompt_version` is BOTH clearly irrelevant (`relevance_score <
    /// relevance_below`) AND confident (`confidence >= confidence_min`) AND
    /// fresh (judged within `window_days`).
    ///
    /// Serendipity-sourced verdicts are deliberately excluded: anti-bubble
    /// picks are SUPPOSED to look irrelevant to a relevance judge, and this
    /// query feeds a demote-only pass — including them would silently delete
    /// the serendipity feature (the exact mis-classification `VerdictSource`
    /// exists to prevent; see `db/verdicts.rs`).
    ///
    /// Ordered newest-judgment-first and bounded by `limit` — the caller's
    /// per-run demotion cap.
    pub fn get_llm_reject_candidates(
        &self,
        prompt_version: &str,
        relevance_below: f64,
        confidence_min: f64,
        window_days: u32,
        limit: usize,
    ) -> SqliteResult<Vec<LlmRejectCandidate>> {
        let conn = self.conn.lock();
        // Dependency releases are excluded IN the query, not filtered after
        // it: a post-filter would let immune rows occupy the LIMIT slots on
        // every pass and starve the demotions that should happen.
        let sql = format!(
            "SELECT si.id, si.title, lj.relevance_score, lj.confidence
             FROM source_items si
             JOIN llm_judgments lj
               ON lj.source_item_id = si.id
              AND lj.prompt_version = ?1
             WHERE si.feed_relevant = 1
               AND COALESCE(si.feed_verdict_source, 'score') = 'score'
               AND lj.relevance_score < ?2
               AND lj.confidence >= ?3
               AND lj.judged_at >= datetime('now', '-' || ?4 || ' days')
               AND NOT {}
             ORDER BY lj.judged_at DESC, lj.id DESC
             LIMIT ?5",
            dependency_release_sql("si")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                prompt_version,
                relevance_below,
                confidence_min,
                window_days,
                limit as i64
            ],
            |row| {
                Ok(LlmRejectCandidate {
                    item_id: row.get(0)?,
                    title: row.get(1)?,
                    judged_relevance: row.get(2)?,
                    confidence: row.get(3)?,
                })
            },
        )?;
        rows.collect()
    }

    /// The subset of `ids` that are registry releases of a matched dependency
    /// (see [`dependency_release_sql`]) — the verdict drain's guard.
    pub fn dependency_release_ids(
        &self,
        ids: &[i64],
    ) -> SqliteResult<std::collections::HashSet<i64>> {
        if ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = format!(
            "SELECT si.id FROM source_items si WHERE si.id IN ({placeholders}) AND {}",
            dependency_release_sql("si")
        );
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| r.get(0))?;
        rows.collect()
    }

    /// Withdraw `llm_reject` verdicts that sit on a dependency release — the
    /// rows the judge was never entitled to remove (see
    /// [`dependency_release_sql`]). Cleared outright, never flipped, exactly
    /// like an orphaned duplicate: the risen sweep then gives the row a first
    /// verdict from its own current score. Convergent — a cleared row carries
    /// no `llm_reject`, and neither demotion lane may write one again.
    pub fn withdraw_llm_rejects_on_dependency_releases(&self) -> SqliteResult<usize> {
        let sql = format!(
            "UPDATE source_items
             SET feed_relevant = NULL, feed_verdict_at = NULL, feed_verdict_version = NULL,
                 feed_verdict_source = NULL, feed_verdict_reason = NULL,
                 feed_verdict_pending = NULL
             WHERE feed_relevant = 0
               AND feed_verdict_reason = 'llm_reject'
               AND {}",
            dependency_release_sql("source_items")
        );
        let conn = self.conn.lock();
        conn.execute(&sql, [])
    }
}

// ============================================================================
// Row Mapper
// ============================================================================

fn map_judgment_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredJudgment> {
    Ok(StoredJudgment {
        id: row.get(0)?,
        source_item_id: row.get(1)?,
        relevance_score: row.get(2)?,
        explanation: row.get(3)?,
        actions: row.get(4)?,
        confidence: row.get(5)?,
        model: row.get(6)?,
        prompt_version: row.get(7)?,
        judged_at: row.get(8)?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;

    #[test]
    fn upsert_and_retrieve_judgment() {
        let db = test_db();

        // Insert a source item first (migration creates the table)
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00')",
                [],
            )
            .unwrap();
        }

        db.upsert_llm_judgment(
            1,
            0.85,
            "Relevant because of X",
            Some("[\"investigate\"]"),
            0.90,
            "claude-sonnet",
            "v1",
        )
        .unwrap();

        let j = db.get_llm_judgment(1).unwrap().unwrap();
        assert_eq!(j.source_item_id, 1);
        assert!((j.relevance_score - 0.85).abs() < 0.01);
        assert_eq!(j.explanation, "Relevant because of X");
        assert_eq!(j.model, "claude-sonnet");
        assert_eq!(j.actions.as_deref(), Some("[\"investigate\"]"));
    }

    #[test]
    fn upsert_updates_existing() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00')",
                [],
            )
            .unwrap();
        }

        db.upsert_llm_judgment(1, 0.50, "First", None, 0.60, "model-a", "v1")
            .unwrap();
        db.upsert_llm_judgment(1, 0.90, "Updated", None, 0.95, "model-b", "v1")
            .unwrap();

        let j = db.get_llm_judgment(1).unwrap().unwrap();
        assert!((j.relevance_score - 0.90).abs() < 0.01);
        assert_eq!(j.explanation, "Updated");
        // Model should be updated too
        assert_eq!(j.model, "model-b");
    }

    #[test]
    fn get_relevant_judgments_filters() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00')",
                [],
            )
            .unwrap();
        }

        db.upsert_llm_judgment(1, 0.30, "Low relevance", None, 0.40, "m", "v1")
            .unwrap();

        let relevant = db.get_relevant_judgments(0.50, 10).unwrap();
        assert!(relevant.is_empty());

        let all = db.get_relevant_judgments(0.20, 10).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn get_unjudged_returns_only_unjudged() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding, relevance_score)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00', 0.5)",
                [],
            )
            .unwrap();
        }

        let unjudged = db.get_unjudged_item_ids(0.3, 10).unwrap();
        assert_eq!(unjudged.len(), 1);

        db.upsert_llm_judgment(1, 0.85, "Judged", None, 0.90, "m", "v1")
            .unwrap();

        let unjudged = db.get_unjudged_item_ids(0.3, 10).unwrap();
        assert!(unjudged.is_empty());
    }

    #[test]
    fn judgment_count() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00')",
                [],
            )
            .unwrap();
        }

        assert_eq!(db.get_judgment_count().unwrap(), 0);
        db.upsert_llm_judgment(1, 0.85, "X", None, 0.90, "m", "v1")
            .unwrap();
        assert_eq!(db.get_judgment_count().unwrap(), 1);
    }

    #[test]
    fn different_prompt_versions_coexist() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('test', 'test-1', 'Test item', '', 'hash1', X'00')",
                [],
            )
            .unwrap();
        }

        db.upsert_llm_judgment(1, 0.50, "V1 judgment", None, 0.60, "m", "v1")
            .unwrap();
        db.upsert_llm_judgment(1, 0.80, "V2 judgment", None, 0.90, "m", "v2")
            .unwrap();

        // Two judgments for the same item (different prompt versions)
        assert_eq!(db.get_judgment_count().unwrap(), 2);

        // get_llm_judgment returns the most recent
        let j = db.get_llm_judgment(1).unwrap().unwrap();
        assert_eq!(j.prompt_version, "v2");
    }

    // ------------------------------------------------------------------
    // Dependency-release guard (llm_reject never removes a registry release
    // of a matched dependency)
    // ------------------------------------------------------------------

    /// A curated item with a fresh, confident v6 reject — a demotion
    /// candidate unless the dependency-release guard exempts it.
    fn rejected_curated(
        db: &Database,
        source_type: &str,
        sid: &str,
        title: &str,
        release_notes: bool,
        matched_dep: Option<&str>,
    ) -> i64 {
        let id = crate::test_utils::insert_test_item(db, source_type, sid, title, "body");
        db.persist_feed_verdicts(
            &[(id, true, crate::db::VerdictSource::Score)],
            crate::scoring::PIPELINE_VERSION,
        )
        .unwrap();
        {
            let conn = db.conn.lock();
            if release_notes {
                conn.execute(
                    "UPDATE source_items SET content_type = 'release_notes' WHERE id = ?1",
                    params![id],
                )
                .unwrap();
            }
            let deps = matched_dep.map_or("[]".to_string(), |d| format!("[\"{d}\"]"));
            conn.execute(
                "INSERT INTO scoring_explanations (source_item_id, pipeline_version, breakdown)
                 VALUES (?1, ?2, ?3)",
                params![
                    id,
                    crate::scoring::PIPELINE_VERSION,
                    format!("{{\"breakdown\":{{\"matched_deps\":{deps}}}}}")
                ],
            )
            .unwrap();
        }
        db.upsert_llm_judgment(id, 0.1, "not in your stack", None, 0.9, "m", "v6")
            .unwrap();
        id
    }

    #[test]
    fn reject_candidates_spare_registry_releases_of_matched_dependencies() {
        let db = test_db();
        let dep_release = rejected_curated(
            &db,
            "crates_io",
            "c1",
            "crates.io: rusqlite v0.40.2",
            true,
            Some("rusqlite"),
        );
        let npm_release = rejected_curated(
            &db,
            "npm_registry",
            "n1",
            "npm: better-sqlite3 v13.0.3",
            true,
            Some("better-sqlite3"),
        );
        // Still demotable: an editorial post that merely names a dependency,
        // a model card name-matched to one, and a registry row that matched
        // no dependency at all.
        let editorial = rejected_curated(
            &db,
            "devto",
            "d1",
            "React 19.3: what changes",
            true,
            Some("react"),
        );
        let hf = rejected_curated(
            &db,
            "huggingface",
            "h1",
            "HF: react-native-executorch-yolo26",
            true,
            Some("react"),
        );
        let unmatched = rejected_curated(
            &db,
            "crates_io",
            "c2",
            "crates.io: some-crate v0.1.0",
            true,
            None,
        );

        let ids: Vec<i64> = db
            .get_llm_reject_candidates("v6", 0.3, 0.7, 7, 100)
            .unwrap()
            .into_iter()
            .map(|c| c.item_id)
            .collect();
        assert!(
            !ids.contains(&dep_release),
            "a release of the user's own dependency must not be LLM-demotable"
        );
        assert!(
            !ids.contains(&npm_release),
            "the guard covers every registry, not only crates.io"
        );
        for id in [editorial, hf, unmatched] {
            assert!(
                ids.contains(&id),
                "item {id} is not a dependency release and must stay demotable"
            );
        }

        let guarded = db
            .dependency_release_ids(&[dep_release, npm_release, editorial, hf, unmatched])
            .unwrap();
        assert_eq!(guarded, [dep_release, npm_release].into_iter().collect());
        assert!(db.dependency_release_ids(&[]).unwrap().is_empty());
    }

    #[test]
    fn withdrawal_clears_only_llm_rejects_on_dependency_releases() {
        let db = test_db();
        let dep_release = rejected_curated(
            &db,
            "crates_io",
            "c1",
            "crates.io: sha2 v0.11.0",
            true,
            Some("sha2"),
        );
        let editorial =
            rejected_curated(&db, "devto", "d1", "Why sha2 is fast", true, Some("sha2"));
        let reject = |id: i64| {
            db.persist_feed_verdicts_with_reasons(
                &[(
                    id,
                    false,
                    crate::db::VerdictSource::Score,
                    Some(crate::db::VerdictReason::LlmReject),
                )],
                crate::scoring::PIPELINE_VERSION,
            )
            .unwrap();
        };
        reject(dep_release);
        reject(editorial);
        let verdict = |id: i64| -> (Option<i64>, Option<String>) {
            db.conn
                .lock()
                .query_row(
                    "SELECT feed_relevant, feed_verdict_reason FROM source_items WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        };
        assert_eq!(
            verdict(dep_release),
            (Some(0), Some("llm_reject".into())),
            "fixture must start rejected"
        );

        assert_eq!(db.withdraw_llm_rejects_on_dependency_releases().unwrap(), 1);
        assert_eq!(
            verdict(dep_release),
            (None, None),
            "cleared outright, never flipped"
        );
        assert_eq!(
            verdict(editorial),
            (Some(0), Some("llm_reject".into())),
            "editorial rejections stand"
        );
        assert_eq!(
            db.withdraw_llm_rejects_on_dependency_releases().unwrap(),
            0,
            "convergent"
        );
    }
}
