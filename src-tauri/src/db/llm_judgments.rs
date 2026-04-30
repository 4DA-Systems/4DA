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
    pub fn get_unjudged_item_ids(&self, min_score: f64, limit: usize) -> SqliteResult<Vec<i64>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT si.id FROM source_items si
             LEFT JOIN llm_judgments lj ON si.id = lj.source_item_id
             WHERE lj.id IS NULL
               AND si.relevance_score >= ?1
               AND si.created_at >= datetime('now', '-7 days')
             ORDER BY si.relevance_score DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![min_score, limit as i64], |row| row.get(0))?;
        rows.collect()
    }

    /// Get total number of stored judgments.
    pub fn get_judgment_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM llm_judgments", [], |row| row.get(0))
    }

    /// Get dismiss category patterns over the last N days.
    /// Returns `(category, count)` pairs sorted by count descending.
    pub fn get_dismiss_patterns(&self, days: i64) -> SqliteResult<Vec<(String, i64)>> {
        let conn = self.conn.lock();
        let days_param = format!("-{days} days");
        let mut stmt = conn.prepare(
            "SELECT dismiss_category, COUNT(*) as cnt
             FROM interactions
             WHERE action_type = 'dismiss'
               AND dismiss_category IS NOT NULL
               AND timestamp >= datetime('now', ?1)
             GROUP BY dismiss_category
             ORDER BY cnt DESC",
        )?;
        let rows = stmt.query_map(params![days_param], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect()
    }

    /// Get per-source dismiss rate over the last N days.
    /// Only includes sources with >= 5 interactions. Returns `(source_type, dismiss_rate)`.
    pub fn get_source_dismiss_rate(&self, days: i64) -> SqliteResult<Vec<(String, f64)>> {
        let conn = self.conn.lock();
        let days_param = format!("-{days} days");
        let mut stmt = conn.prepare(
            "SELECT si.source_type,
                    CAST(SUM(CASE WHEN i.action_type = 'dismiss' THEN 1 ELSE 0 END) AS REAL)
                    / MAX(COUNT(*), 1) as dismiss_rate
             FROM interactions i
             JOIN source_items si ON i.source_item_id = si.id
             WHERE i.timestamp >= datetime('now', ?1)
             GROUP BY si.source_type
             HAVING COUNT(*) >= 5
             ORDER BY dismiss_rate DESC",
        )?;
        let rows = stmt.query_map(params![days_param], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        rows.collect()
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
}
