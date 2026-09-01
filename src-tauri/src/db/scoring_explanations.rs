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
//! Write lane: [`Database::persist_analysis_scores`] writes one
//! `scoring_explanations` row per persisted score, in the SAME transaction as
//! the score write. The invariant it maintains is *a scored item always has
//! an explanation, and a hysteresis-suppressed re-score never replaces a
//! better one*: a score-changing write upserts (newest evaluation wins —
//! `source_item_id` is the primary key), a suppressed write only seeds a
//! missing row. The stored value is a bounded envelope, not the raw struct:
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
//! - `truncated` maps each shortened path to its original length (element
//!   count for arrays, BYTE length for strings) — the marker demanded by the
//!   size bound: truncate loudly, never fail.
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
/// String bounds are measured in BYTES, matching the unit of
/// [`EXPLANATION_HARD_CAP_BYTES`]. They were once measured in characters,
/// which let multi-byte content (CJK, emoji — 4DA ingests Mastodon and Lemmy)
/// pass the string bound untouched at up to 4x its budget and blow the
/// envelope cap, forcing the aggressive pass to sacrifice whole array entries.
const MAX_STRING_BYTES: usize = 400;
/// Second-pass bounds when the first pass still exceeds the hard cap
/// (pathological inputs: hundreds of matched deps with long evidence text).
const AGGRESSIVE_ARRAY_ITEMS: usize = 3;
const AGGRESSIVE_STRING_BYTES: usize = 80;
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
    for (max_items, max_bytes) in [
        (MAX_ARRAY_ITEMS, MAX_STRING_BYTES),
        (AGGRESSIVE_ARRAY_ITEMS, AGGRESSIVE_STRING_BYTES),
    ] {
        let mut bounded = full.clone();
        let mut truncated = serde_json::Map::new();
        // Paths are envelope-scoped ("breakdown.matched_deps") so a reader of
        // the stored JSON can resolve them without extra context.
        bound_value(
            &mut bounded,
            "breakdown",
            max_items,
            max_bytes,
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

/// Largest index `<= max_bytes` that lies on a UTF-8 character boundary.
///
/// `str::floor_char_boundary` is still unstable, so this is the hand-rolled
/// equivalent: UTF-8 continuation bytes are `0b10xxxxxx` and a sequence is at
/// most four bytes, so walking back at most three bytes always lands on a
/// boundary. Truncating at this index can never split a character (which
/// `String::truncate` would panic on).
fn floor_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Recursively truncate arrays longer than `max_items` and strings longer
/// than `max_bytes`, recording `path -> original length` in `truncated`
/// (element count for arrays, BYTE length for strings — the same unit as the
/// bound). Strings are cut at a UTF-8 character boundary, so a multi-byte
/// character is dropped whole rather than split. Arrays are shortened, never
/// replaced with marker strings, so a truncated breakdown still deserializes
/// into [`ScoreBreakdown`].
fn bound_value(
    v: &mut Value,
    path: &str,
    max_items: usize,
    max_bytes: usize,
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
                    max_bytes,
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
                bound_value(val, &child_path, max_items, max_bytes, truncated);
            }
        }
        Value::String(s) if s.len() > max_bytes => {
            truncated.insert(path.to_string(), Value::from(s.len()));
            s.truncate(floor_char_boundary(s, max_bytes));
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

    /// Half of the seed/keep invariant: a hysteresis-suppressed re-score keeps
    /// the OLD durable score, so it must keep the OLD explanation too — the
    /// explanation lane explains the persisted score, not the wobble the
    /// damper discarded. Pairs with
    /// [`test_suppressed_write_seeds_missing_explanation`], which covers the
    /// case where there is no old explanation to keep.
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

    /// The other half of the invariant, and the 81% lockout this repairs
    /// (measured on the live corpus 2026-08-31: 50,788 of 62,822 scored items
    /// had no explanation and could never acquire one). An item that already
    /// carried a score when schema 115 landed has no row, and a STABLE score
    /// is exactly the condition that suppresses the write — so skipping the
    /// suppressed path locked the item out permanently. A suppressed re-score
    /// must SEED the missing row.
    #[test]
    fn test_suppressed_write_seeds_missing_explanation() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "expl_seed", "pre-115 item", "content");

        // Pre-schema-115 state: a durable score with no explanation row.
        db.persist_analysis_scores(&[(id, 0.50, None, None, None)], "analysis")
            .unwrap();
        assert!(
            db.get_scoring_explanation(id).unwrap().is_none(),
            "precondition: the item carries a score but no explanation"
        );

        // 0.52 is inside the 0.05 hysteresis band, so this write is
        // suppressed. Before the fix that skipped the explanation entirely,
        // and every future re-score of a stable item would skip it too.
        let bd = bounded_breakdown_json(0.52, &minimal_breakdown()).expect("serializes");
        db.persist_analysis_scores(&[(id, 0.52, None, None, Some(bd))], "analysis")
            .unwrap();

        let row = db
            .get_scoring_explanation(id)
            .unwrap()
            .expect("a suppressed re-score must seed the missing explanation");
        let envelope: serde_json::Value = serde_json::from_str(&row.breakdown_json).unwrap();
        assert!(
            (envelope["score"].as_f64().unwrap() - 0.52).abs() < 1e-6,
            "the seeded breakdown is the one this evaluation produced"
        );

        // Seeding is a write-path repair, not a change to the hysteresis
        // contract: the durable score is still damped to the old value.
        let durable: f64 = db
            .conn
            .lock()
            .query_row(
                "SELECT relevance_score FROM source_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            (durable - 0.50).abs() < 1e-6,
            "hysteresis still damps the durable score; only the explanation seeds"
        );
    }

    /// The hard cap is measured in BYTES, but the string bound was measured in
    /// CHARACTERS. Multi-byte content (CJK, emoji — 4DA ingests Mastodon and
    /// Lemmy, so this is live-reachable) therefore passed the string bound
    /// untouched at up to 4x its byte budget, overflowed the envelope, and
    /// forced the aggressive second pass to sacrifice whole array entries.
    /// Byte-aware bounding keeps the entries and never splits a character.
    #[test]
    fn test_multibyte_strings_bounded_by_bytes_not_chars() {
        let mut bd = minimal_breakdown();

        // 300 chars but 1200 bytes: under the old 400-CHARACTER bound this
        // string was not truncated at all, and MAX_ARRAY_ITEMS of them
        // (9600 bytes) alone overflow the 8192-byte cap.
        let emoji = "\u{1F680}".repeat(300);
        assert_eq!(emoji.chars().count(), 300, "inside the old 400-char bound");
        assert_eq!(emoji.len(), 1200, "but 4x that in bytes");
        bd.matched_deps = vec![emoji; MAX_ARRAY_ITEMS];
        // 800 chars / 2400 bytes of 3-byte characters, and 400 is NOT a
        // character boundary in it — the truncation must walk back to 399.
        bd.llm_reason = Some("\u{6F22}\u{5B57}".repeat(400));

        let json = bounded_breakdown_json(0.5, &bd).expect("serializes");
        assert!(
            json.len() <= EXPLANATION_HARD_CAP_BYTES,
            "envelope must respect the byte cap; got {} bytes",
            json.len()
        );

        let envelope: serde_json::Value = serde_json::from_str(&json).unwrap();
        let truncated = envelope["truncated"]
            .as_object()
            .expect("truncation must be marked, not silent");

        // The substantive claim first: bounding the STRINGS by bytes keeps the
        // whole array. The old char bound left these strings untouched at 1200
        // bytes each, overflowed the envelope, and the aggressive second pass
        // bought the size back by cutting the array 8 -> 3 — five dependency
        // matches silently lost to a bound measured in the wrong unit.
        let parsed: ScoreBreakdown =
            serde_json::from_value(envelope["breakdown"].clone()).expect("typed re-read works");
        assert_eq!(
            parsed.matched_deps.len(),
            MAX_ARRAY_ITEMS,
            "every dependency match survives the first pass"
        );
        assert!(
            !truncated.contains_key("breakdown.matched_deps"),
            "the array is not truncated at all, so it carries no marker"
        );
        assert_eq!(
            truncated["breakdown.matched_deps[0]"].as_u64(),
            Some(1200),
            "the marker records the original BYTE length, matching the bound's unit"
        );

        for dep in &parsed.matched_deps {
            assert!(
                dep.len() <= MAX_STRING_BYTES,
                "each string is byte-bounded; got {} bytes",
                dep.len()
            );
            assert!(
                dep.chars().all(|c| c == '\u{1F680}'),
                "characters are dropped whole — a split UTF-8 sequence would \
                 not round-trip through JSON as the original character"
            );
        }

        let reason = parsed.llm_reason.expect("llm_reason survives, truncated");
        assert_eq!(
            reason.len(),
            399,
            "truncation walks back from 400 to the nearest character boundary"
        );
        assert!(
            reason.chars().all(|c| c == '\u{6F22}' || c == '\u{5B57}'),
            "no partial character at the cut"
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
