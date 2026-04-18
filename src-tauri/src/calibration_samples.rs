// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intelligence Mesh — Calibration Sample Store (Phase 5b.2 substrate).
//!
//! Persistence for raw advisor signals so the Filter (see
//! `calibration_fitter`) can later pair each signal with the user's
//! downstream InteractionPattern on the same item and derive a binary
//! "relevant" label.
//!
//! ## Why a separate table (not provenance)
//!
//! The `provenance` table records WHICH model/prompt/calibration produced
//! each artifact, but not WHAT score was produced. Fitting a calibration
//! curve needs (predicted_confidence, actual_outcome) pairs — we cannot
//! fit without the predicted value. Extending provenance to carry scores
//! would conflate "audit trail" with "training data"; keeping them
//! separate lets each table evolve for its own purpose (provenance can
//! grow columns for drift detection without bloating every sample row).
//!
//! ## Write path
//!
//! `stamp_signals` is called alongside `provenance::record_batch` in
//! `analysis_rerank.rs` per judged item. Failures are logged but non-fatal:
//! a DB error here degrades future calibration quality but must not fail
//! the rerank pass that already produced valid results.
//!
//! ## Read path
//!
//! The fitter scans `(model_identity_hash, task, processed_at IS NULL)`
//! for unprocessed samples, joins them to `interactions` and `feedback`
//! by `source_item_id`, derives labels, fits, then calls `mark_processed`
//! to prevent refits.
//!
//! ## Invariants
//!
//! - Samples are append-only. Rows are never updated except for the
//!   `processed_at` timestamp going from NULL to a value exactly once.
//! - `mark_processed` is idempotent: re-marking an already-processed row
//!   is a no-op (preserves the original timestamp). This matters because
//!   the fitter may retry on partial failure.

use crate::error::Result;
use crate::types::AdvisorSignal;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use tracing::{debug, warn};

/// One row of the `calibration_samples` table. Constructed from an
/// AdvisorSignal + the source_item_id that was judged.
///
/// Most call sites don't need to build a `CalibrationSample` directly;
/// `stamp_signals` accepts AdvisorSignal slices and does the conversion.
/// The struct is exposed so the fitter's SELECT can decode rows into it.
#[allow(dead_code)]
// Several fields are decoded but not read by the fitter yet;
// the fitter uses raw_score + source_item_id + created_at +
// id. The remaining fields are reserved for the upcoming
// multi-task fitter that will re-group by (hash, task) +
// drift-detection by prompt_version.
#[derive(Debug, Clone)]
pub struct CalibrationSample {
    pub id: Option<i64>,
    pub source_item_id: i64,
    pub model_identity_hash: String,
    pub task: String,
    pub prompt_version: String,
    /// Raw confidence reported by the advisor (the value we want to
    /// calibrate against observed outcomes).
    pub raw_score: f32,
    /// Separately-reported confidence in the signal itself. Kept for
    /// completeness; the fitter uses `raw_score` as the predicted value.
    pub confidence: f32,
    pub created_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Persist a batch of advisor signals against a single source_item.
///
/// Groups all inserts into one transaction — avoids per-row fsync cost
/// when a rerank batch stamps several signals for the same item (e.g.
/// after multi-advisor arrives). Signals missing a `prompt_version`
/// are dropped (can't key a curve without it); this only happens for
/// test stubs and pre-mesh paths that haven't set the field.
pub fn stamp_signals(
    conn: &Connection,
    source_item_id: i64,
    identity_hash: &str,
    signals: &[AdvisorSignal],
) -> Result<usize> {
    if signals.is_empty() {
        return Ok(0);
    }

    let tx = conn.unchecked_transaction()?;
    let mut stmt = tx.prepare(
        "INSERT INTO calibration_samples
            (source_item_id, model_identity_hash, task, prompt_version, raw_score, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    let mut inserted = 0usize;
    for s in signals {
        // A signal without prompt_version cannot be keyed to a curve that
        // invalidates on prompt drift, so we skip it. In practice every
        // production path sets this via CalibratedCore/LlmJudgeCore.
        let Some(prompt_version) = s.prompt_version.as_deref() else {
            debug!(
                target: "4da::calibration_samples",
                source_item_id,
                "Skipping signal without prompt_version (cannot key curve)"
            );
            continue;
        };

        stmt.execute(params![
            source_item_id,
            identity_hash,
            s.task,
            prompt_version,
            s.raw_score,
            s.confidence,
        ])?;
        inserted += 1;
    }

    drop(stmt);
    tx.commit()?;

    if inserted > 0 {
        debug!(
            target: "4da::calibration_samples",
            source_item_id,
            inserted,
            "Stamped advisor signals for calibration"
        );
    }
    Ok(inserted)
}

/// Count unprocessed samples for a given (model, task). Exposed for the
/// UI's "pending samples" indicator; the fitter uses `collect_unprocessed`
/// directly because it needs the rows, not just a count.
#[allow(dead_code)] // Consumed by the future receipts UI panel for pending samples.
pub fn count_unprocessed(
    conn: &Connection,
    identity_hash: &str,
    task: &str,
    min_age_hours: i64,
) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM calibration_samples
         WHERE model_identity_hash = ?1
           AND task = ?2
           AND processed_at IS NULL
           AND created_at <= datetime('now', ?3)",
        params![identity_hash, task, format!("-{} hours", min_age_hours),],
        |r| r.get(0),
    )?;
    Ok(count.max(0) as u32)
}

/// Fetch unprocessed samples older than `min_age_hours` for a given
/// (model, task). The age floor matters because InteractionPattern
/// classification needs dwell/scroll telemetry that only arrives on
/// item close — scanning too recent samples produces many "no label
/// yet, drop" outcomes and wastes a fit slot.
pub fn collect_unprocessed(
    conn: &Connection,
    identity_hash: &str,
    task: &str,
    min_age_hours: i64,
    limit: usize,
) -> Result<Vec<CalibrationSample>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_item_id, model_identity_hash, task,
                prompt_version, raw_score, confidence, created_at, processed_at
         FROM calibration_samples
         WHERE model_identity_hash = ?1
           AND task = ?2
           AND processed_at IS NULL
           AND created_at <= datetime('now', ?3)
         ORDER BY created_at ASC
         LIMIT ?4",
    )?;

    let rows = stmt.query_map(
        params![
            identity_hash,
            task,
            format!("-{} hours", min_age_hours),
            limit as i64,
        ],
        row_to_sample,
    )?;

    let mut out = Vec::new();
    for r in rows {
        match r {
            Ok(sample) => out.push(sample),
            Err(e) => warn!(
                target: "4da::calibration_samples",
                error = %e,
                "Failed to decode calibration_samples row — skipping"
            ),
        }
    }
    Ok(out)
}

/// Mark a batch of samples as processed. Idempotent: rows with a non-NULL
/// `processed_at` are left alone (WHERE clause filters them).
pub fn mark_processed(conn: &Connection, sample_ids: &[i64]) -> Result<usize> {
    if sample_ids.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    let mut marked = 0usize;
    {
        let mut stmt = tx.prepare(
            "UPDATE calibration_samples
             SET processed_at = datetime('now')
             WHERE id = ?1 AND processed_at IS NULL",
        )?;
        for id in sample_ids {
            marked += stmt.execute(params![id])?;
        }
    }
    tx.commit()?;
    Ok(marked)
}

fn row_to_sample(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalibrationSample> {
    Ok(CalibrationSample {
        id: Some(row.get(0)?),
        source_item_id: row.get(1)?,
        model_identity_hash: row.get(2)?,
        task: row.get(3)?,
        prompt_version: row.get(4)?,
        raw_score: row.get::<_, f64>(5)? as f32,
        confidence: row.get::<_, f64>(6)? as f32,
        created_at: parse_sqlite_datetime(row.get::<_, Option<String>>(7)?.as_deref()),
        processed_at: parse_sqlite_datetime(row.get::<_, Option<String>>(8)?.as_deref()),
    })
}

/// SQLite's `datetime('now')` returns `YYYY-MM-DD HH:MM:SS` (naive UTC).
/// chrono cannot parse that directly as Utc — we append " +00:00" and use
/// the strict parser. Returns None on any format deviation.
fn parse_sqlite_datetime(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?;
    let with_tz = format!("{s} +00:00");
    chrono::DateTime::parse_from_str(&with_tz, "%Y-%m-%d %H:%M:%S %:z")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Bring up an in-memory DB with only the calibration_samples schema.
    /// We don't pull in the full 57-phase migration stack — this keeps
    /// the unit tests fast and isolated from unrelated schema drift.
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calibration_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                model_identity_hash TEXT NOT NULL,
                task TEXT NOT NULL,
                prompt_version TEXT NOT NULL,
                raw_score REAL NOT NULL,
                confidence REAL NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                processed_at TEXT
            );
            CREATE INDEX idx_cal_samples_item
              ON calibration_samples(source_item_id, created_at);
            CREATE INDEX idx_cal_samples_unfit
              ON calibration_samples(model_identity_hash, task, processed_at);",
        )
        .unwrap();
        conn
    }

    fn signal(task: &str, raw: f32, conf: f32) -> AdvisorSignal {
        AdvisorSignal {
            provider: "ollama".into(),
            model: "llama3.2".into(),
            identity_hash: None,
            task: task.into(),
            raw_score: raw,
            normalized_score: raw,
            confidence: conf,
            reason: None,
            prompt_version: Some("judge-v1-test".into()),
            calibration_id: None,
        }
    }

    #[test]
    fn stamp_inserts_one_row_per_signal() {
        let conn = test_conn();
        let signals = vec![signal("judge", 0.5, 0.9), signal("judge", 0.7, 0.8)];
        let inserted = stamp_signals(&conn, 42, "abc123", &signals).unwrap();
        assert_eq!(inserted, 2);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM calibration_samples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn stamp_empty_slice_is_noop() {
        let conn = test_conn();
        let inserted = stamp_signals(&conn, 1, "hash", &[]).unwrap();
        assert_eq!(inserted, 0);
    }

    #[test]
    fn stamp_drops_signal_without_prompt_version() {
        // Producing a curve keyed on prompt_version from a signal that
        // lacks one is impossible — the drift check in load_current_curve
        // can never validate it. Skip rather than persist a garbage row.
        let conn = test_conn();
        let mut sig = signal("judge", 0.5, 0.9);
        sig.prompt_version = None;
        let inserted = stamp_signals(&conn, 1, "hash", &[sig]).unwrap();
        assert_eq!(inserted, 0);
    }

    #[test]
    fn count_unprocessed_respects_min_age_hours() {
        let conn = test_conn();
        // Insert a fresh sample (created_at = now).
        stamp_signals(&conn, 1, "hash", &[signal("judge", 0.5, 0.9)]).unwrap();

        // min_age=0 sees it; min_age=24 does not.
        assert_eq!(count_unprocessed(&conn, "hash", "judge", 0).unwrap(), 1);
        assert_eq!(count_unprocessed(&conn, "hash", "judge", 24).unwrap(), 0);
    }

    #[test]
    fn count_unprocessed_ignores_processed_rows() {
        let conn = test_conn();
        stamp_signals(&conn, 1, "hash", &[signal("judge", 0.5, 0.9)]).unwrap();
        conn.execute(
            "UPDATE calibration_samples SET processed_at = datetime('now') WHERE id = 1",
            [],
        )
        .unwrap();
        assert_eq!(count_unprocessed(&conn, "hash", "judge", 0).unwrap(), 0);
    }

    #[test]
    fn collect_orders_by_created_at_ascending() {
        let conn = test_conn();
        // Force created_at ordering by explicit insert.
        conn.execute(
            "INSERT INTO calibration_samples
                (source_item_id, model_identity_hash, task, prompt_version,
                 raw_score, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![1, "h", "judge", "p", 0.5, 0.9, "2026-04-10 10:00:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO calibration_samples
                (source_item_id, model_identity_hash, task, prompt_version,
                 raw_score, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![2, "h", "judge", "p", 0.6, 0.8, "2026-04-09 10:00:00"],
        )
        .unwrap();

        let samples = collect_unprocessed(&conn, "h", "judge", 0, 10).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].source_item_id, 2, "oldest sample first");
        assert_eq!(samples[1].source_item_id, 1);
    }

    #[test]
    fn collect_respects_limit() {
        let conn = test_conn();
        for i in 0..5 {
            stamp_signals(&conn, i, "h", &[signal("judge", 0.5, 0.9)]).unwrap();
        }
        let samples = collect_unprocessed(&conn, "h", "judge", 0, 3).unwrap();
        assert_eq!(samples.len(), 3);
    }

    #[test]
    fn collect_isolates_by_model_and_task() {
        let conn = test_conn();
        stamp_signals(&conn, 1, "model-a", &[signal("judge", 0.5, 0.9)]).unwrap();
        stamp_signals(&conn, 2, "model-b", &[signal("judge", 0.5, 0.9)]).unwrap();
        stamp_signals(&conn, 3, "model-a", &[signal("summarize", 0.5, 0.9)]).unwrap();

        let a_judge = collect_unprocessed(&conn, "model-a", "judge", 0, 10).unwrap();
        let b_judge = collect_unprocessed(&conn, "model-b", "judge", 0, 10).unwrap();
        let a_summ = collect_unprocessed(&conn, "model-a", "summarize", 0, 10).unwrap();
        assert_eq!(a_judge.len(), 1);
        assert_eq!(b_judge.len(), 1);
        assert_eq!(a_summ.len(), 1);
        assert_eq!(a_judge[0].source_item_id, 1);
    }

    #[test]
    fn mark_processed_sets_timestamp_and_excludes_from_count() {
        let conn = test_conn();
        stamp_signals(
            &conn,
            1,
            "h",
            &[signal("judge", 0.5, 0.9), signal("judge", 0.6, 0.8)],
        )
        .unwrap();
        let samples = collect_unprocessed(&conn, "h", "judge", 0, 10).unwrap();
        let ids: Vec<i64> = samples.iter().filter_map(|s| s.id).collect();
        assert_eq!(ids.len(), 2);

        let marked = mark_processed(&conn, &ids).unwrap();
        assert_eq!(marked, 2);
        assert_eq!(count_unprocessed(&conn, "h", "judge", 0).unwrap(), 0);
    }

    #[test]
    fn mark_processed_is_idempotent() {
        // Marking the same rows twice should not touch their existing
        // timestamp — the fitter may retry on partial failure and we
        // don't want to silently re-date processed samples.
        let conn = test_conn();
        stamp_signals(&conn, 1, "h", &[signal("judge", 0.5, 0.9)]).unwrap();
        let ids: Vec<i64> = collect_unprocessed(&conn, "h", "judge", 0, 10)
            .unwrap()
            .iter()
            .filter_map(|s| s.id)
            .collect();

        let first = mark_processed(&conn, &ids).unwrap();
        let second = mark_processed(&conn, &ids).unwrap();
        assert_eq!(first, 1);
        assert_eq!(second, 0, "second mark is a no-op under the WHERE filter");
    }

    #[test]
    fn mark_processed_empty_slice_is_noop() {
        let conn = test_conn();
        assert_eq!(mark_processed(&conn, &[]).unwrap(), 0);
    }
}
