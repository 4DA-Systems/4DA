// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Persisted per-item scoring explanations (schema 115) — the durable "why".
//!
//! Closes the E5010 auditability gap (standing finding, 2026-08-21/23 audits):
//! the scorer's per-item breakdown (which axes fired, which dep matches, which
//! caps applied) lived only in the session's in-memory analysis results, so
//! "why did zustand score 0.092" was unanswerable after a restart. The LLM
//! lane already persisted its explanations (`llm_judgments.explanation`); this
//! module gives the deterministic scorer the same property.
//!
//! Write lane: [`Database::persist_analysis_scores`] upserts one
//! `scoring_explanations` row per persisted score, in the SAME transaction as
//! the score write (newest evaluation wins — `source_item_id` is the primary
//! key). The stored value is a bounded envelope, not the raw struct:
//!
//! ```json
//! {"score": 0.42, "breakdown": { ...ScoreBreakdown... },
//!  "truncated": {"breakdown.matched_deps": 57}}
//! ```
//!
//! - `score` is the raw score the scorer produced for THIS evaluation (the
//!   durable `relevance_score` can lag it by up to the hysteresis band).
//! - `breakdown` is the full [`ScoreBreakdown`] with long arrays and strings
//!   truncated IN PLACE (arrays stay homogeneous, so the object still
//!   deserializes into the typed struct).
//! - `truncated` maps each shortened path to its original length — the
//!   marker demanded by the size bound: truncate loudly, never fail.
//!
//! This is WRITE-ONLY additional data: scores are byte-identical with or
//! without it, so there is deliberately NO `PIPELINE_VERSION` bump.
//!
//! Retention: the table carries `ON DELETE CASCADE` from `source_items`
//! (same precedent as `source_item_dependencies`), so every prune path that
//! deletes an item deletes its explanation in the same statement.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use serde_json::Value;

use super::Database;
use crate::types::ScoreBreakdown;

/// First-pass bounds: generous enough that a normal breakdown is stored
/// verbatim (typical serialized size is 1.5–3 KB).
const MAX_ARRAY_ITEMS: usize = 8;
const MAX_STRING_CHARS: usize = 400;
/// Second-pass bounds when the first pass still exceeds the hard cap
/// (pathological inputs: hundreds of matched deps with long evidence text).
const AGGRESSIVE_ARRAY_ITEMS: usize = 3;
const AGGRESSIVE_STRING_CHARS: usize = 80;
/// The stored envelope never exceeds this many bytes.
pub const EXPLANATION_HARD_CAP_BYTES: usize = 8192;

/// One persisted explanation row, exactly as stored.
#[derive(Debug, Clone)]
pub struct PersistedExplanation {
    /// `scoring::PIPELINE_VERSION` at the time the breakdown was written.
    pub pipeline_version: i64,
    /// The bounded envelope JSON (see module docs for the shape).
    pub breakdown_json: String,
    /// UTC `datetime('now')` stamp of the explanation write.
    pub scored_at: String,
}

/// Serialize a [`ScoreBreakdown`] into the bounded explanation envelope.
///
/// `score` is the raw score this evaluation produced (embedded so the row is
/// self-contained even when the durable score was hysteresis-damped or later
/// re-ranked). Returns `None` only when serde cannot serialize the breakdown
/// at all — the caller treats that as "nothing to persist", never an error.
///
/// Size contract: the returned string is at most
/// [`EXPLANATION_HARD_CAP_BYTES`]. Long arrays/strings are truncated in place
/// and recorded in the envelope's `truncated` map; if even the aggressive
/// pass cannot fit (not reachable with the current struct shape, but the
/// bound must be unconditional) the envelope degrades to `{"score", "elided"}`.
pub fn bounded_breakdown_json(score: f32, breakdown: &ScoreBreakdown) -> Option<String> {
    let full = serde_json::to_value(breakdown).ok()?;
    for (max_items, max_chars) in [
        (MAX_ARRAY_ITEMS, MAX_STRING_CHARS),
        (AGGRESSIVE_ARRAY_ITEMS, AGGRESSIVE_STRING_CHARS),
    ] {
        let mut bounded = full.clone();
        let mut truncated = serde_json::Map::new();
        // Paths are envelope-scoped ("breakdown.matched_deps") so a reader of
        // the stored JSON can resolve them without extra context.
        bound_value(
            &mut bounded,
            "breakdown",
            max_items,
            max_chars,
            &mut truncated,
        );

        let mut envelope = serde_json::Map::new();
        envelope.insert("score".into(), Value::from(f64::from(score)));
        envelope.insert("breakdown".into(), bounded);
        if !truncated.is_empty() {
            envelope.insert("truncated".into(), Value::Object(truncated));
        }
        let json = serde_json::to_string(&Value::Object(envelope)).ok()?;
        if json.len() <= EXPLANATION_HARD_CAP_BYTES {
            return Some(json);
        }
    }
    // Unconditional bound: scalars only, explicitly marked.
    Some(
        serde_json::json!({
            "score": f64::from(score),
            "elided": true,
        })
        .to_string(),
    )
}

/// Recursively truncate arrays longer than `max_items` and strings longer
/// than `max_chars` (char-boundary safe), recording `path -> original length`
/// in `truncated`. Arrays are shortened, never replaced with marker strings,
/// so a truncated breakdown still deserializes into [`ScoreBreakdown`].
fn bound_value(
    v: &mut Value,
    path: &str,
    max_items: usize,
    max_chars: usize,
    truncated: &mut serde_json::Map<String, Value>,
) {
    match v {
        Value::Array(arr) => {
            if arr.len() > max_items {
                truncated.insert(path.to_string(), Value::from(arr.len()));
                arr.truncate(max_items);
            }
            for (i, item) in arr.iter_mut().enumerate() {
                bound_value(
                    item,
                    &format!("{path}[{i}]"),
                    max_items,
                    max_chars,
                    truncated,
                );
            }
        }
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                bound_value(val, &child_path, max_items, max_chars, truncated);
            }
        }
        Value::String(s) => {
            let char_count = s.chars().count();
            if char_count > max_chars {
                truncated.insert(path.to_string(), Value::from(char_count));
                *s = s.chars().take(max_chars).collect();
            }
        }
        _ => {}
    }
}

impl Database {
    /// Read the persisted explanation for one item, if any. Read-only; the
    /// autopsy surface falls back to this when the item is not in the current
    /// session's in-memory analysis.
    pub fn get_scoring_explanation(
        &self,
        item_id: i64,
    ) -> SqliteResult<Option<PersistedExplanation>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT pipeline_version, breakdown, scored_at
             FROM scoring_explanations WHERE source_item_id = ?1",
            params![item_id],
            |row| {
                Ok(PersistedExplanation {
                    pipeline_version: row.get(0)?,
                    breakdown_json: row.get(1)?,
                    scored_at: row.get(2)?,
                })
            },
        )
        .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    /// A valid ScoreBreakdown with every defaulted field at its default —
    /// deserialized rather than constructed because the struct has ~50 fields
    /// and no Default impl.
    fn minimal_breakdown() -> ScoreBreakdown {
        serde_json::from_value(serde_json::json!({
            "context_score": 0.31,
            "interest_score": 0.22,
            "ace_boost": 0.05,
            "affinity_mult": 1.0,
            "anti_penalty": 0.0,
            "confidence_by_signal": {"context": 0.8}
        }))
        .expect("minimal breakdown deserializes")
    }

    #[test]
    fn test_explanation_write_then_read_roundtrip() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "expl_1", "roundtrip item", "content");

        let bd_json = bounded_breakdown_json(0.42, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.42, None, None, Some(bd_json))], "analysis")
            .unwrap();

        let row = db
            .get_scoring_explanation(id)
            .unwrap()
            .expect("explanation persisted with the score");
        assert_eq!(
            row.pipeline_version,
            i64::from(crate::scoring::PIPELINE_VERSION),
            "explanation carries the pipeline version that produced it"
        );
        assert!(!row.scored_at.is_empty(), "scored_at stamp is written");

        let envelope: serde_json::Value = serde_json::from_str(&row.breakdown_json).unwrap();
        assert!((envelope["score"].as_f64().unwrap() - 0.42).abs() < 1e-6);
        let parsed: ScoreBreakdown =
            serde_json::from_value(envelope["breakdown"].clone()).expect("typed re-read works");
        assert!((parsed.context_score - 0.31).abs() < 1e-6);

        // Newest wins: a later re-score (beyond the hysteresis band) replaces
        // the row instead of accumulating history.
        let bd2 = bounded_breakdown_json(0.80, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.80, None, None, Some(bd2))], "analysis")
            .unwrap();
        let row2 = db.get_scoring_explanation(id).unwrap().unwrap();
        let envelope2: serde_json::Value = serde_json::from_str(&row2.breakdown_json).unwrap();
        assert!(
            (envelope2["score"].as_f64().unwrap() - 0.80).abs() < 1e-6,
            "one row per item — the newest evaluation replaces the old"
        );
        let count: i64 = db
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM scoring_explanations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            count, 1,
            "PRIMARY KEY upsert keeps exactly one row per item"
        );
    }

    #[test]
    fn test_bounded_breakdown_json_caps_size_and_marks_truncation() {
        let mut bd = minimal_breakdown();
        bd.matched_deps = (0..500).map(|i| format!("package-{i}")).collect();
        bd.confirmed_signals = (0..40).map(|i| format!("signal-{i}")).collect();
        bd.llm_reason = Some("reason ".repeat(2000)); // ~14k chars

        let json = bounded_breakdown_json(0.5, &bd).expect("pathological input still serializes");
        assert!(
            json.len() <= EXPLANATION_HARD_CAP_BYTES,
            "envelope must respect the hard cap; got {} bytes",
            json.len()
        );

        let envelope: serde_json::Value = serde_json::from_str(&json).unwrap();
        let truncated = envelope["truncated"]
            .as_object()
            .expect("truncation must be marked, not silent");
        assert_eq!(
            truncated["breakdown.matched_deps"].as_u64(),
            Some(500),
            "marker records the original array length"
        );
        assert!(
            truncated.contains_key("breakdown.llm_reason"),
            "oversized strings are marked too"
        );

        // Arrays were shortened homogeneously: the typed read still works.
        let parsed: ScoreBreakdown =
            serde_json::from_value(envelope["breakdown"].clone()).expect("typed re-read works");
        assert!(parsed.matched_deps.len() <= 8);
        assert_eq!(parsed.matched_deps[0], "package-0");
    }

    #[test]
    fn test_pruned_item_cascades_explanation() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "expl_prune", "doomed item", "content");
        let bd_json = bounded_breakdown_json(0.3, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.3, None, None, Some(bd_json))], "analysis")
            .unwrap();
        assert!(db.get_scoring_explanation(id).unwrap().is_some());

        // Every prune path issues DELETE FROM source_items; the FK cascade
        // (PRAGMA foreign_keys = ON in Database::new) must take the
        // explanation with it so pruned items leak nothing.
        db.conn
            .lock()
            .execute("DELETE FROM source_items WHERE id = ?1", params![id])
            .unwrap();
        assert!(
            db.get_scoring_explanation(id).unwrap().is_none(),
            "ON DELETE CASCADE must remove the explanation with the item"
        );
    }

    /// A hysteresis-suppressed re-score keeps the OLD durable score, so it
    /// must keep the OLD explanation too — the explanation lane explains the
    /// persisted score, not the wobble the damper discarded.
    #[test]
    fn test_hysteresis_suppressed_write_keeps_old_explanation() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "expl_hyst", "stable item", "content");

        let bd1 = bounded_breakdown_json(0.50, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.50, None, None, Some(bd1))], "analysis")
            .unwrap();

        // 0.52 is inside the SCORE_WRITE_HYSTERESIS band (0.05): the durable
        // score stays 0.50 and the explanation must stay with it.
        let bd2 = bounded_breakdown_json(0.52, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.52, None, None, Some(bd2))], "analysis")
            .unwrap();

        let row = db.get_scoring_explanation(id).unwrap().unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&row.breakdown_json).unwrap();
        assert!(
            (envelope["score"].as_f64().unwrap() - 0.50).abs() < 1e-6,
            "suppressed write must not replace the explanation of the durable score"
        );
    }

    /// Manual timing measurement for the PR write-overhead figure — not a
    /// correctness gate (timing asserts flake on loaded CI).
    /// Run: `cargo test --lib measure_explanation_write_overhead -- --ignored --nocapture`
    #[test]
    #[ignore = "manual timing measurement, not a correctness gate"]
    fn measure_explanation_write_overhead() {
        let db = test_db();
        let n = 1000;
        let ids: Vec<i64> = (0..n)
            .map(|i| {
                insert_test_item(
                    &db,
                    "hackernews",
                    &format!("perf_{i}"),
                    "perf item",
                    "content",
                )
            })
            .collect();
        let bd = minimal_breakdown();

        let without: Vec<crate::db::ScorePersistRow> = ids
            .iter()
            .map(|id| (*id, 0.4_f32, None, None, None))
            .collect();
        let start = std::time::Instant::now();
        db.persist_analysis_scores(&without, "analysis").unwrap();
        let t_without = start.elapsed();

        let ser_start = std::time::Instant::now();
        let with: Vec<crate::db::ScorePersistRow> = ids
            .iter()
            .map(|id| (*id, 0.6_f32, None, None, bounded_breakdown_json(0.6, &bd)))
            .collect();
        let t_serialize = ser_start.elapsed();
        let start = std::time::Instant::now();
        db.persist_analysis_scores(&with, "analysis").unwrap();
        let t_with = start.elapsed();

        println!(
            "persist {n} scores: without explanations {t_without:?}, with {t_with:?} \
             (+serialization {t_serialize:?})"
        );
    }
}
