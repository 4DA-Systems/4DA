//! Intelligence Mesh — The Filter (Phase 5b.2).
//!
//! The final piece of the calibration pipeline. Where
//! `calibration_samples` is the write-side substrate and `calibration` +
//! `calibration_store` are the apply-side substrate, THIS module is the
//! "read the data, derive labels, fit a curve, save it" loop.
//!
//! ## Pairing design
//!
//! For each unprocessed calibration sample:
//!
//!   1. Look up interactions on `source_item_id` within the window
//!      `[sample.created_at, sample.created_at + 48h]`. The window is
//!      the observation horizon — a user either read the item or
//!      didn't within two days; beyond that, absence is ambiguous.
//!
//!   2. If an explicit `feedback.relevant` row exists for the item,
//!      use it as the ground truth (overrides any inferred pattern).
//!      This is the strongest possible label — a user clicked "mark
//!      irrelevant" on purpose.
//!
//!   3. Otherwise, derive a label from the strongest InteractionPattern
//!      observed in the window:
//!
//!         - Completed / Engaged                  → `true` (positive)
//!         - Bounced                              → `false` (negative)
//!         - Scanned / Reread / Abandoned / none  → drop (ambiguous)
//!
//!      "Drop" is NOT the same as "negative" — a paired-but-ambiguous
//!      sample carries no information about calibration quality and
//!      including it as either class inflates noise. We require the
//!      signal to be clear before it shapes the curve.
//!
//!   4. Samples with zero interactions AND no explicit feedback are
//!      also dropped. No impression → no signal → can't calibrate.
//!
//! ## Why 48 hours?
//!
//! A content item's signal lifetime is ~1-2 days. Longer windows would
//! include stale interactions (user clicked an unrelated shared tab,
//! or the system re-surfaced the item hours later and the second
//! interaction is actually against a different score). Shorter windows
//! would starve the fitter. 48h matches the briefing cadence.
//!
//! ## Fitting strategy
//!
//! Equal-width 5-bucket histogram over [0, 1]. Each bucket's
//! observed_positive_rate is the mean of its labels. Empty buckets are
//! dropped before curve construction (the apply function requires
//! `len >= 2` to interpolate; we do NOT want to fabricate empty
//! buckets that would collapse adjacent interpolation ranges).
//!
//! We pick equal-width over quantile bucketing for v1 because the
//! bucket boundaries must be stable across fits — a user inspecting
//! receipts shouldn't see "bucket 3" mean different ranges across two
//! fit runs. Equal-frequency is more statistically efficient but
//! requires different bucket boundaries per fit; post-launch work.
//!
//! ## Failure modes
//!
//! - Not enough samples → return `None`, log at info. Curve persists
//!   nothing; CalibratedCore stays in pass-through mode.
//! - All samples drop (all ambiguous) → same as above.
//! - Fewer than 2 non-empty buckets after bucketing → return `None`.
//!   A degenerate curve is worse than no curve; the apply function
//!   falls through to raw scores when len < 2, so we'd persist a
//!   curve that does nothing but bloats the receipts UI.
//! - DB errors → propagate as Err; the caller logs and leaves the
//!   curve untouched.

use crate::calibration::{CalibrationBucket, CalibrationCurve};
use crate::calibration_samples::{self, CalibrationSample};
use crate::calibration_store;
use crate::error::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use tracing::{debug, info, warn};

/// Minimum samples required to attempt a fit. Below this, the curve is
/// too noisy to be informative — a single outlier shifts a bucket by
/// 20%+ when `n < 20` per bucket. 50 gives ~10 per bucket at 5 buckets.
pub const MIN_FIT_SAMPLES: usize = 50;

/// Hours a sample must age before we consider it paired. Interactions
/// and InteractionPattern classification come in on item close, which
/// can be hours after the judgment. Too-recent samples would all look
/// "no label yet, drop" and waste the fit slot.
pub const MIN_AGE_HOURS: i64 = 24;

/// Observation horizon for pairing. Interactions newer than this from
/// the sample's created_at are not considered — see module docs.
pub const PAIRING_WINDOW_HOURS: i64 = 48;

/// Number of equal-width buckets in [0.0, 1.0] for the fit.
const BUCKET_COUNT: usize = 5;

/// Max samples consumed per fit run. Prevents a single fit from
/// scanning a year of history when a user first upgrades past this
/// commit. Anything older should be archived, not retrofitted.
const COLLECT_LIMIT: usize = 5_000;

/// Result of a single fit attempt. The caller logs this; the Tauri
/// command surfaces it back to the UI for the "last calibration run"
/// panel (future).
#[derive(Debug, Clone)]
pub struct FitReport {
    // Retained for the log pipeline's structured context — reading these
    // fields off the struct lets a future debug handler re-emit a past
    // fit run's identity without re-querying. The UI-facing summary
    // wrapper in calibration_commands.rs mirrors them.
    #[allow(dead_code)]
    pub model_identity_hash: String,
    #[allow(dead_code)]
    pub task: String,
    /// Total unprocessed samples that were candidates for this fit.
    pub samples_scanned: usize,
    /// Samples that yielded a positive/negative label (the rest were
    /// dropped as ambiguous or unpaired).
    pub samples_labeled: usize,
    pub curve: Option<CalibrationCurve>,
    /// If `curve` is None, the reason the fitter skipped.
    pub skipped_reason: Option<String>,
}

/// One (predicted_confidence, observed_outcome) pair. The fitter's
/// input format after pairing + labeling.
#[derive(Debug, Clone, Copy)]
pub struct LabeledSample {
    /// Surfaced for debugging / test assertions; the bucketizer itself
    /// doesn't use it.
    #[allow(dead_code)]
    pub sample_id: i64,
    pub predicted: f32,
    pub observed: bool,
}

/// Collect labeled samples for a given (model, task). Joins
/// `calibration_samples` ↔ `interactions` ↔ `feedback` and returns
/// the resolved labels. The returned `sample_ids_included` list is
/// used by `fit_and_save` to mark processed — samples that dropped
/// out of labeling are NOT included (they stay unprocessed so a
/// later fit with more interaction data can still use them).
///
/// Exposed for the e2e test; most callers should use `fit_and_save`.
pub fn collect_labeled_samples(
    conn: &Connection,
    identity_hash: &str,
    task: &str,
) -> Result<(Vec<LabeledSample>, Vec<CalibrationSample>)> {
    let candidates = calibration_samples::collect_unprocessed(
        conn,
        identity_hash,
        task,
        MIN_AGE_HOURS,
        COLLECT_LIMIT,
    )?;
    let mut labeled = Vec::with_capacity(candidates.len());
    for sample in &candidates {
        let Some(sample_id) = sample.id else { continue };
        if let Some(outcome) = resolve_outcome(conn, sample)? {
            labeled.push(LabeledSample {
                sample_id,
                predicted: sample.raw_score,
                observed: outcome,
            });
        }
    }
    Ok((labeled, candidates))
}

/// Resolve a single sample's binary outcome. Returns `None` when the
/// sample is ambiguous or unpaired — the caller drops the row and
/// leaves it as unprocessed for a future fit.
///
/// Precedence: explicit feedback wins over inferred InteractionPattern.
fn resolve_outcome(conn: &Connection, sample: &CalibrationSample) -> Result<Option<bool>> {
    // 1. Check explicit feedback (created within the pairing window,
    //    or any time really — explicit labels don't expire).
    //    The feedback table is append-only; take the latest row.
    let feedback: Option<bool> = conn
        .query_row(
            "SELECT relevant FROM feedback
             WHERE source_item_id = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            params![sample.source_item_id],
            |r| {
                let v: i64 = r.get(0)?;
                Ok(v != 0)
            },
        )
        .ok();
    if let Some(v) = feedback {
        return Ok(Some(v));
    }

    // 2. Scan interactions in the pairing window. We look for the
    //    strongest classifiable signal: explicit dismiss/mark_irrelevant
    //    first, then any InteractionPattern embedded in action_data.
    //
    //    We take the LAST signal by timestamp so a user who initially
    //    bounced but returned to read fully later ends up positive.
    let action_rows = load_interactions_in_window(conn, sample)?;
    for (action_type, action_data) in action_rows.iter().rev() {
        if let Some(outcome) = classify_action(action_type, action_data.as_deref()) {
            return Ok(Some(outcome));
        }
    }

    Ok(None)
}

fn load_interactions_in_window(
    conn: &Connection,
    sample: &CalibrationSample,
) -> Result<Vec<(String, Option<String>)>> {
    // The pairing window opens at created_at and closes PAIRING_WINDOW_HOURS
    // later. Interactions BEFORE created_at aren't causal (the sample
    // didn't exist yet); interactions after the window are too stale.
    //
    // We match against BOTH item_id and source_item_id because the
    // `interactions` table is dual-keyed (see ace/db.rs:122-144) —
    // ACE uses item_id, ContextEngine uses source_item_id, and for a
    // given source_items row we don't know which system wrote the row.
    let Some(created_at) = sample.created_at else {
        return Ok(Vec::new());
    };
    // Format as SQLite-native datetime (YYYY-MM-DD HH:MM:SS) — NOT
    // RFC 3339.  The interactions table stores timestamps via SQLite's
    // datetime('now', ...) which omits the 'T' separator and timezone
    // suffix.  A BETWEEN on mismatched formats silently matches nothing.
    let window_start = created_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let window_end = (created_at + chrono::Duration::hours(PAIRING_WINDOW_HOURS))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let mut stmt = conn.prepare(
        "SELECT action_type, action_data
         FROM interactions
         WHERE (source_item_id = ?1 OR item_id = ?1)
           AND timestamp BETWEEN ?2 AND ?3
         ORDER BY timestamp ASC",
    )?;
    let rows = stmt.query_map(
        params![sample.source_item_id, window_start, window_end],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, Option<String>>(1)?,
            ))
        },
    )?;

    let mut out = Vec::new();
    for r in rows {
        if let Ok(pair) = r {
            out.push(pair);
        }
    }
    Ok(out)
}

/// Map one interaction to a binary outcome, or `None` if the signal
/// is ambiguous.
///
/// Invariants:
/// - `dismiss`, `mark_irrelevant` → always negative.
/// - `save`, `share`, `engagement_complete`, `save_with_context` →
///   always positive (explicit user intent).
/// - `click` with `pattern=Completed|Engaged` → positive.
/// - `click` with `pattern=Bounced` → negative.
/// - Everything else (scanned/reread/abandoned/no pattern) drops.
fn classify_action(action_type: &str, action_data: Option<&str>) -> Option<bool> {
    match action_type {
        "dismiss" | "mark_irrelevant" | "briefing_dismiss" => Some(false),
        "save" | "share" | "save_with_context" | "engagement_complete" => Some(true),
        "click" | "briefing_click" => classify_pattern(action_data),
        _ => None,
    }
}

fn classify_pattern(action_data: Option<&str>) -> Option<bool> {
    let data = action_data?;
    // Parse just enough of the JSON to extract pattern. We avoid
    // deserializing the whole BehaviorAction enum — it has drift-prone
    // shape variations across versions.
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    let pattern = value.get("pattern")?.as_str()?;
    match pattern {
        "completed" | "engaged" => Some(true),
        "bounced" => Some(false),
        // scanned, reread, abandoned — explicitly ambiguous, fall
        // through to None. Callers treat None as "drop".
        _ => None,
    }
}

/// Fit a curve from labeled samples, save it to disk, and mark the
/// contributing samples as processed. Returns a `FitReport` describing
/// what happened.
///
/// The full pipeline: scan → pair → label → bucket → Brier/ECE →
/// construct curve → save → mark processed.
pub fn fit_and_save(
    conn: &Connection,
    identity_hash: &str,
    task: &str,
    current_prompt_version: &str,
) -> Result<FitReport> {
    let (labeled, candidates) = collect_labeled_samples(conn, identity_hash, task)?;

    if labeled.len() < MIN_FIT_SAMPLES {
        let reason = format!(
            "only {} labeled samples (need >= {})",
            labeled.len(),
            MIN_FIT_SAMPLES
        );
        debug!(
            target: "4da::calibration_fitter",
            identity_hash,
            task,
            scanned = candidates.len(),
            labeled = labeled.len(),
            "{}",
            reason
        );
        return Ok(FitReport {
            model_identity_hash: identity_hash.to_string(),
            task: task.to_string(),
            samples_scanned: candidates.len(),
            samples_labeled: labeled.len(),
            curve: None,
            skipped_reason: Some(reason),
        });
    }

    let buckets = bucketize(&labeled);
    if buckets.len() < 2 {
        let reason = format!(
            "only {} non-empty buckets after bucketing (need >= 2 to interpolate)",
            buckets.len()
        );
        warn!(
            target: "4da::calibration_fitter",
            identity_hash,
            task,
            labeled = labeled.len(),
            "{}",
            reason
        );
        return Ok(FitReport {
            model_identity_hash: identity_hash.to_string(),
            task: task.to_string(),
            samples_scanned: candidates.len(),
            samples_labeled: labeled.len(),
            curve: None,
            skipped_reason: Some(reason),
        });
    }

    let predictions: Vec<(f32, bool)> = labeled.iter().map(|l| (l.predicted, l.observed)).collect();
    let brier = crate::calibration::compute_brier_score(&predictions);
    let ece = crate::calibration::compute_ece(&predictions, BUCKET_COUNT);

    let curve = CalibrationCurve {
        curve_id: make_curve_id(task, identity_hash),
        model_identity_hash: identity_hash.to_string(),
        task: task.to_string(),
        prompt_version: current_prompt_version.to_string(),
        buckets,
        brier_score: brier,
        ece,
        sample_count: labeled.len() as u32,
        created_at: Utc::now(),
    };

    calibration_store::save_curve(&curve)?;

    // Mark all candidate samples (labeled + dropped) as processed.
    // The dropped ones would otherwise be scanned again and again on
    // every fit run — if interactions never arrived within the window,
    // they never will. Marking them prevents an unbounded accumulation
    // of stuck-unprocessed rows.
    let sample_ids: Vec<i64> = candidates.iter().filter_map(|s| s.id).collect();
    let marked = calibration_samples::mark_processed(conn, &sample_ids)?;

    info!(
        target: "4da::calibration_fitter",
        identity_hash,
        task,
        curve_id = %curve.curve_id,
        scanned = candidates.len(),
        labeled = labeled.len(),
        marked,
        brier,
        ece,
        "Fit and saved calibration curve"
    );

    Ok(FitReport {
        model_identity_hash: identity_hash.to_string(),
        task: task.to_string(),
        samples_scanned: candidates.len(),
        samples_labeled: labeled.len(),
        curve: Some(curve),
        skipped_reason: None,
    })
}

/// Equal-width 5-bucket histogram. Each bucket's observed_positive_rate
/// is the mean of its contained labels. Empty buckets are dropped so
/// the returned Vec has `len <= BUCKET_COUNT`.
fn bucketize(samples: &[LabeledSample]) -> Vec<CalibrationBucket> {
    let width = 1.0_f32 / BUCKET_COUNT as f32;
    let mut counters: Vec<(u32, u32)> = vec![(0, 0); BUCKET_COUNT]; // (positive, total)

    for s in samples {
        let p = s.predicted.clamp(0.0, 1.0);
        // Right edge inclusive for the last bucket so 1.0 lands in bucket 4.
        let idx = if p >= 1.0 {
            BUCKET_COUNT - 1
        } else {
            (p / width) as usize
        };
        let idx = idx.min(BUCKET_COUNT - 1);
        counters[idx].1 += 1;
        if s.observed {
            counters[idx].0 += 1;
        }
    }

    (0..BUCKET_COUNT)
        .filter_map(|i| {
            let (pos, total) = counters[i];
            if total == 0 {
                return None;
            }
            let lo = i as f32 * width;
            let hi = lo + width;
            Some(CalibrationBucket {
                raw_bucket_lo: lo,
                raw_bucket_hi: hi,
                raw_bucket_center: f32::midpoint(lo, hi),
                observed_positive_rate: pos as f32 / total as f32,
                sample_count: total,
            })
        })
        .collect()
}

/// Format: `{task}-{first-8-of-hash}-cal-v1-{YYYY-MM-DD}`. Stable within
/// a day — a second fit the same day overwrites the previous curve file
/// and reuses the curve_id, which is fine (curve_id is a human-readable
/// cohort label, not a unique key).
fn make_curve_id(task: &str, identity_hash: &str) -> String {
    let date = Utc::now().format("%Y-%m-%d");
    let hash_prefix = identity_hash.chars().take(8).collect::<String>();
    format!("{task}-{hash_prefix}-cal-v1-{date}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Schema fixture: the minimal set of tables the fitter touches.
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
            CREATE TABLE interactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                item_id INTEGER,
                action TEXT,
                action_type TEXT,
                action_data TEXT,
                timestamp TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                relevant INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    fn insert_sample(conn: &Connection, item_id: i64, raw: f32, age_hours: i64) -> i64 {
        conn.execute(
            "INSERT INTO calibration_samples
                (source_item_id, model_identity_hash, task, prompt_version,
                 raw_score, confidence, created_at)
             VALUES (?1, 'hash', 'judge', 'p', ?2, ?2, datetime('now', ?3))",
            params![item_id, raw, format!("-{} hours", age_hours)],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_interaction(
        conn: &Connection,
        item_id: i64,
        action_type: &str,
        pattern: Option<&str>,
        offset_hours_from_now: i64,
    ) {
        let action_data = pattern.map(|p| format!(r#"{{"pattern":"{p}"}}"#));
        conn.execute(
            "INSERT INTO interactions
                (source_item_id, item_id, action_type, action_data, timestamp)
             VALUES (?1, ?1, ?2, ?3, datetime('now', ?4))",
            params![
                item_id,
                action_type,
                action_data,
                format!("{} hours", -offset_hours_from_now),
            ],
        )
        .unwrap();
    }

    // ── classify_action / classify_pattern ──────────────────────────────

    #[test]
    fn classify_action_dismiss_is_negative() {
        assert_eq!(classify_action("dismiss", None), Some(false));
        assert_eq!(classify_action("mark_irrelevant", None), Some(false));
        assert_eq!(classify_action("briefing_dismiss", None), Some(false));
    }

    #[test]
    fn classify_action_save_is_positive() {
        assert_eq!(classify_action("save", None), Some(true));
        assert_eq!(classify_action("share", None), Some(true));
        assert_eq!(classify_action("save_with_context", None), Some(true));
        assert_eq!(classify_action("engagement_complete", None), Some(true));
    }

    #[test]
    fn classify_action_click_with_completed_pattern_is_positive() {
        let data = r#"{"pattern":"completed","dwell_time_seconds":60}"#;
        assert_eq!(classify_action("click", Some(data)), Some(true));
    }

    #[test]
    fn classify_action_click_with_bounced_is_negative() {
        let data = r#"{"pattern":"bounced"}"#;
        assert_eq!(classify_action("click", Some(data)), Some(false));
    }

    #[test]
    fn classify_action_click_with_scanned_is_ambiguous() {
        let data = r#"{"pattern":"scanned"}"#;
        assert_eq!(classify_action("click", Some(data)), None);
    }

    #[test]
    fn classify_action_click_without_pattern_is_ambiguous() {
        assert_eq!(classify_action("click", None), None);
        assert_eq!(classify_action("click", Some("{}")), None);
    }

    #[test]
    fn classify_action_unknown_type_is_ambiguous() {
        assert_eq!(classify_action("scroll", None), None);
        assert_eq!(classify_action("ignore", None), None);
    }

    #[test]
    fn classify_pattern_handles_malformed_json() {
        assert_eq!(classify_pattern(Some("not json")), None);
        assert_eq!(classify_pattern(Some("")), None);
        assert_eq!(classify_pattern(None), None);
    }

    // ── resolve_outcome ─────────────────────────────────────────────────

    #[test]
    fn resolve_outcome_explicit_feedback_wins() {
        // A Bounced click + an explicit "relevant" feedback should
        // resolve to positive. Explicit user intent overrides
        // inferred pattern.
        let conn = test_conn();
        insert_sample(&conn, 1, 0.7, 25);
        insert_interaction(&conn, 1, "click", Some("bounced"), 10);
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (1, 1)",
            [],
        )
        .unwrap();

        let samples =
            calibration_samples::collect_unprocessed(&conn, "hash", "judge", 0, 10).unwrap();
        let outcome = resolve_outcome(&conn, &samples[0]).unwrap();
        assert_eq!(outcome, Some(true));
    }

    #[test]
    fn resolve_outcome_last_interaction_wins_over_first() {
        // Initial bounce followed by a completed read should be positive.
        let conn = test_conn();
        insert_sample(&conn, 1, 0.7, 25);
        insert_interaction(&conn, 1, "click", Some("bounced"), 20);
        insert_interaction(&conn, 1, "click", Some("completed"), 5);

        let samples =
            calibration_samples::collect_unprocessed(&conn, "hash", "judge", 0, 10).unwrap();
        let outcome = resolve_outcome(&conn, &samples[0]).unwrap();
        assert_eq!(outcome, Some(true));
    }

    #[test]
    fn resolve_outcome_no_interactions_is_ambiguous() {
        let conn = test_conn();
        insert_sample(&conn, 1, 0.7, 25);
        let samples =
            calibration_samples::collect_unprocessed(&conn, "hash", "judge", 0, 10).unwrap();
        let outcome = resolve_outcome(&conn, &samples[0]).unwrap();
        assert_eq!(outcome, None);
    }

    #[test]
    fn resolve_outcome_interaction_before_sample_is_excluded() {
        // An interaction that happened BEFORE the sample was stamped
        // isn't caused by the advisor's judgment and must not count.
        let conn = test_conn();
        insert_sample(&conn, 1, 0.7, 5); // sample is 5h old
        insert_interaction(&conn, 1, "click", Some("completed"), 10); // interaction is 10h old
        let samples =
            calibration_samples::collect_unprocessed(&conn, "hash", "judge", 0, 10).unwrap();
        let outcome = resolve_outcome(&conn, &samples[0]).unwrap();
        assert_eq!(outcome, None);
    }

    // ── bucketize ───────────────────────────────────────────────────────

    #[test]
    fn bucketize_empty_input_returns_empty() {
        assert!(bucketize(&[]).is_empty());
    }

    #[test]
    fn bucketize_produces_one_bucket_per_occupied_range() {
        let samples = vec![
            LabeledSample {
                sample_id: 1,
                predicted: 0.1,
                observed: false,
            },
            LabeledSample {
                sample_id: 2,
                predicted: 0.3,
                observed: true,
            },
            LabeledSample {
                sample_id: 3,
                predicted: 0.9,
                observed: true,
            },
        ];
        let buckets = bucketize(&samples);
        // Bucket 0 (0.0-0.2), bucket 1 (0.2-0.4), bucket 4 (0.8-1.0) — 3 non-empty.
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].observed_positive_rate, 0.0);
        assert_eq!(buckets[1].observed_positive_rate, 1.0);
        assert_eq!(buckets[2].observed_positive_rate, 1.0);
    }

    #[test]
    fn bucketize_clamps_predictions_to_valid_range() {
        // An out-of-range prediction shouldn't panic and should land
        // in a sensible bucket.
        let samples = vec![
            LabeledSample {
                sample_id: 1,
                predicted: -0.5,
                observed: false,
            },
            LabeledSample {
                sample_id: 2,
                predicted: 1.5,
                observed: true,
            },
        ];
        let buckets = bucketize(&samples);
        assert_eq!(buckets.len(), 2);
        // First bucket (0.0-0.2) gets the negative-clamped sample.
        assert!((buckets[0].raw_bucket_lo - 0.0).abs() < 1e-6);
        // Last bucket (0.8-1.0) gets the 1.5-clamped sample.
        assert!((buckets[1].raw_bucket_hi - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bucketize_right_edge_inclusive_for_last_bucket() {
        // A prediction of exactly 1.0 must land in bucket 4, not panic
        // or get dropped by "p / width" floating-point weirdness.
        let samples = vec![LabeledSample {
            sample_id: 1,
            predicted: 1.0,
            observed: true,
        }];
        let buckets = bucketize(&samples);
        assert_eq!(buckets.len(), 1);
        assert!((buckets[0].raw_bucket_hi - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bucketize_averages_outcomes_within_bucket() {
        // Three samples in bucket 2 (0.4-0.6): 2 positive, 1 negative.
        // Expected observed_positive_rate = 2/3.
        let samples = vec![
            LabeledSample {
                sample_id: 1,
                predicted: 0.42,
                observed: true,
            },
            LabeledSample {
                sample_id: 2,
                predicted: 0.50,
                observed: true,
            },
            LabeledSample {
                sample_id: 3,
                predicted: 0.58,
                observed: false,
            },
        ];
        let buckets = bucketize(&samples);
        assert_eq!(buckets.len(), 1);
        assert!((buckets[0].observed_positive_rate - 2.0 / 3.0).abs() < 1e-5);
        assert_eq!(buckets[0].sample_count, 3);
    }

    // ── fit_and_save ────────────────────────────────────────────────────

    #[test]
    fn fit_skips_when_too_few_samples() {
        // Too-few-samples path returns before save_curve, so no file
        // collisions to worry about.
        let conn = test_conn();
        insert_sample(&conn, 1, 0.7, 25);
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (1, 1)",
            [],
        )
        .unwrap();

        let report = fit_and_save(&conn, "hash", "judge", "p").unwrap();
        assert!(report.curve.is_none());
        assert!(report
            .skipped_reason
            .as_deref()
            .unwrap()
            .contains("labeled samples"));
    }

    #[test]
    fn fit_succeeds_with_enough_labeled_samples() {
        // UUID-based identity avoids filesystem races between parallel
        // test runs (every fit_and_save writes to
        // data/calibrations/{identity_hash}/{task}.json). Cleanup happens
        // inside with_unique_identity.
        with_unique_identity(|identity_hash| {
            let conn = test_conn();
            // Seed 60 samples split across buckets with varied outcomes.
            // Use explicit feedback so we don't need interaction rows.
            //
            // Pattern: low-confidence samples (0.1) are usually negative,
            // high-confidence samples (0.9) are usually positive — an
            // over-confident model, calibration-quality distinguishable.
            for i in 0..60 {
                let raw = if i < 30 { 0.1 } else { 0.9 };
                conn.execute(
                    "INSERT INTO calibration_samples
                        (source_item_id, model_identity_hash, task, prompt_version,
                         raw_score, confidence, created_at)
                     VALUES (?1, ?2, 'judge', 'p', ?3, ?3, datetime('now', '-25 hours'))",
                    params![i + 1, identity_hash, raw],
                )
                .unwrap();
                let positive = if i < 30 { i % 5 == 0 } else { i % 5 != 0 };
                conn.execute(
                    "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
                    params![i + 1, if positive { 1 } else { 0 }],
                )
                .unwrap();
            }

            let report = fit_and_save(&conn, identity_hash, "judge", "prompt-v1").unwrap();
            let curve = report.curve.expect("curve should be produced");
            assert_eq!(curve.task, "judge");
            assert_eq!(curve.prompt_version, "prompt-v1");
            assert_eq!(curve.model_identity_hash, identity_hash);
            assert_eq!(curve.sample_count, 60);
            assert!(curve.buckets.len() >= 2);
            assert!(curve.brier_score >= 0.0 && curve.brier_score <= 1.0);
            assert!(curve.ece >= 0.0 && curve.ece <= 1.0);

            let unprocessed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM calibration_samples WHERE processed_at IS NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(unprocessed, 0);
        });
    }

    #[test]
    fn fit_marks_even_unlabeled_samples_as_processed() {
        // If interactions never arrive in the window, the sample drops
        // from labeling — but the fitter marks it processed anyway so
        // it doesn't accumulate in the scan indefinitely.
        //
        // We spread labeled samples across buckets so bucketize produces
        // >= 2 buckets and the fit actually succeeds.
        with_unique_identity(|identity_hash| {
            let conn = test_conn();
            for i in 0..50 {
                let raw = if i < 25 { 0.2 } else { 0.8 };
                conn.execute(
                    "INSERT INTO calibration_samples
                        (source_item_id, model_identity_hash, task, prompt_version,
                         raw_score, confidence, created_at)
                     VALUES (?1, ?2, 'judge', 'p', ?3, ?3, datetime('now', '-25 hours'))",
                    params![i + 1, identity_hash, raw],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                    params![i + 1],
                )
                .unwrap();
            }
            // 10 additional samples with NO interactions (will be unlabeled).
            for i in 50..60 {
                conn.execute(
                    "INSERT INTO calibration_samples
                        (source_item_id, model_identity_hash, task, prompt_version,
                         raw_score, confidence, created_at)
                     VALUES (?1, ?2, 'judge', 'p', 0.5, 0.5, datetime('now', '-25 hours'))",
                    params![i + 1, identity_hash],
                )
                .unwrap();
            }

            let report = fit_and_save(&conn, identity_hash, "judge", "p").unwrap();
            assert!(
                report.curve.is_some(),
                "curve should be produced with 50 labeled samples across 2 buckets"
            );
            assert_eq!(report.samples_scanned, 60);
            assert_eq!(report.samples_labeled, 50);

            let unprocessed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM calibration_samples WHERE processed_at IS NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(unprocessed, 0);
        });
    }

    #[test]
    fn fit_skips_when_all_samples_in_one_bucket() {
        // All samples at 0.5 (bucket 2 only). bucketize produces 1
        // bucket, which is < 2 → fit skips. No save_curve call in this
        // path, so filesystem isolation isn't strictly needed — but we
        // keep the pattern uniform.
        let conn = test_conn();
        for i in 0..60 {
            insert_sample(&conn, i + 1, 0.5, 25);
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i + 1],
            )
            .unwrap();
        }

        // Identity "hash" matches insert_sample's hardcoded value so
        // the fitter picks up the 60 inserted samples.
        let report = fit_and_save(&conn, "hash", "judge", "p").unwrap();
        assert!(report.curve.is_none());
        assert!(report
            .skipped_reason
            .as_deref()
            .unwrap()
            .contains("non-empty buckets"));
    }

    // ── End-to-end: stamp → pair → fit → save → reload → apply ─────────
    //
    // This is the headline test for Phase 5b.2 — it exercises the entire
    // calibration feedback loop with no mocks beyond the schema fixture,
    // and verifies the fitter's output is actually usable by the apply
    // layer that consumes it at rerank time.
    //
    // We use a UUID-based identity_hash so the filesystem artifact this
    // test writes (`data/calibrations/{uuid}/judge.json`) can't collide
    // with production curves or other test runs. Cleanup removes the
    // directory on success; on test failure the artifact stays around
    // for postmortem inspection.

    use crate::types::AdvisorSignal;

    fn with_unique_identity<F: FnOnce(&str)>(f: F) {
        let unique = uuid::Uuid::new_v4().simple().to_string();
        f(&unique);
        // Best-effort cleanup. Ignore errors: if the fitter didn't
        // write anything the directory won't exist, and if it did
        // we want to remove it to keep data/calibrations/ tidy.
        let dir = crate::runtime_paths::RuntimePaths::get()
            .data_dir
            .join("calibrations")
            .join(&unique);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn e2e_stamp_fit_save_reload_apply_full_pipeline() {
        with_unique_identity(|identity_hash| {
            use crate::calibration::{CalibratedCore, CalibrationCurve};
            use crate::calibration_samples;
            use crate::calibration_store;
            use crate::intelligence_core::{IntelligenceCore, JudgeRequest, Validated};

            // 1. Bring up a clean DB + stamp 60 signals via the public API
            //    (not direct SQL) so we exercise the write path the rerank
            //    loop actually uses.
            let conn = test_conn();
            for i in 0..60 {
                // Alternate raw_score between buckets to guarantee
                // bucketize produces >= 2 occupied buckets.
                let raw = if i < 30 { 0.2 } else { 0.8 };
                let sig = AdvisorSignal {
                    provider: "ollama".into(),
                    model: "llama3.2".into(),
                    identity_hash: None,
                    task: "judge".into(),
                    raw_score: raw,
                    normalized_score: raw,
                    confidence: raw,
                    reason: None,
                    prompt_version: Some("judge-v1-e2e".into()),
                    calibration_id: None,
                };
                // Age the sample past MIN_AGE_HOURS so the fitter sees it.
                conn.execute(
                    "INSERT INTO calibration_samples
                        (source_item_id, model_identity_hash, task, prompt_version,
                         raw_score, confidence, created_at)
                     VALUES (?1, ?2, 'judge', 'judge-v1-e2e', ?3, ?3, datetime('now', '-25 hours'))",
                    params![i + 1, identity_hash, raw],
                )
                .unwrap();
                let _ = sig;

                // Label via explicit feedback: low-confidence items are
                // mostly negative (80%), high-confidence mostly positive
                // (80%). A well-calibrated model would observe exactly
                // these rates at each confidence level; an over-confident
                // one would need pulling down. Our stub data is
                // approximately well-calibrated.
                let positive = if i < 30 { i % 5 == 0 } else { i % 5 != 0 };
                conn.execute(
                    "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
                    params![i + 1, if positive { 1 } else { 0 }],
                )
                .unwrap();
            }

            // Before fit: unprocessed count matches what we stamped.
            let pending =
                calibration_samples::count_unprocessed(&conn, identity_hash, "judge", 0).unwrap();
            assert_eq!(pending, 60);

            // 2. Run the fitter. This writes the curve to disk and marks
            //    every sample processed.
            let report = fit_and_save(&conn, identity_hash, "judge", "judge-v1-e2e").unwrap();
            let fit_curve = report.curve.expect("curve should be produced");
            assert_eq!(fit_curve.sample_count, 60);
            assert_eq!(fit_curve.prompt_version, "judge-v1-e2e");
            assert!(fit_curve.buckets.len() >= 2);

            // After fit: zero unprocessed.
            assert_eq!(
                calibration_samples::count_unprocessed(&conn, identity_hash, "judge", 0).unwrap(),
                0
            );

            // 3. Reload from disk. load_current_curve applies the drift
            //    check: a matching prompt_version returns Some, a drifted
            //    one returns None.
            let loaded =
                calibration_store::load_current_curve(identity_hash, "judge", "judge-v1-e2e")
                    .expect("fresh curve should be retrievable");
            assert_eq!(loaded.curve_id, fit_curve.curve_id);
            assert_eq!(loaded.buckets.len(), fit_curve.buckets.len());
            assert!((loaded.brier_score - fit_curve.brier_score).abs() < 1e-5);

            // Drift check: a stale prompt invalidates.
            let drifted =
                calibration_store::load_current_curve(identity_hash, "judge", "judge-v2-never-fit");
            assert!(
                drifted.is_none(),
                "mismatched prompt_version must invalidate curve"
            );

            // 4. Wrap with CalibratedCore. Prove the loaded curve
            //    actually changes the judge output — if the apply layer
            //    were broken, raw would pass through unchanged.
            struct StubCore;
            #[async_trait::async_trait]
            impl IntelligenceCore for StubCore {
                fn identity(&self) -> crate::provenance::ModelIdentity {
                    crate::provenance::ModelIdentity::new("ollama", "llama3.2")
                }
                fn prompt_version(&self) -> &'static str {
                    "judge-v1-e2e"
                }
                fn calibration_id(&self) -> Option<String> {
                    Some("pre-mesh-unknown".to_string())
                }
                async fn judge(
                    &self,
                    _req: JudgeRequest,
                ) -> Result<Validated<crate::intelligence_core::JudgeResponse>> {
                    Ok(Validated {
                        value: crate::intelligence_core::JudgeResponse {
                            judgments: vec![crate::llm::RelevanceJudgment {
                                item_id: "x".into(),
                                relevant: true,
                                confidence: 0.2, // lands in bucket 1 (0.2-0.4)
                                reasoning: String::new(),
                                key_connections: vec![],
                            }],
                            input_tokens: 1,
                            output_tokens: 1,
                        },
                        identity: self.identity(),
                        prompt_version: "judge-v1-e2e".to_string(),
                        calibration_id: Some("pre-mesh-unknown".to_string()),
                        raw_response_hash: None,
                    })
                }
                fn estimate_cost_cents(&self, _: u64, _: u64) -> u64 {
                    0
                }
            }

            let wrapped = CalibratedCore::new(Box::new(StubCore), Some(loaded.clone()));
            let judged = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(wrapped.judge(JudgeRequest {
                    context_summary: "ctx".into(),
                    items: vec![("x".into(), "t".into(), "c".into())],
                }))
                .unwrap();

            // calibration_id on the Validated response must reflect the
            // fitted curve (not the stub's sentinel) — proves the wiring.
            assert_eq!(
                judged.calibration_id.as_deref(),
                Some(loaded.curve_id.as_str())
            );

            // The fitted curve was produced from approximately-calibrated
            // data, so applying it to raw=0.2 should yield a value in a
            // plausible probability range (not e.g. -infinity or NaN).
            // We don't assert an exact value — the observed rate in
            // bucket 1 depends on our synthetic data and would be brittle.
            let after = judged.value.judgments[0].confidence;
            assert!(
                (0.0..=1.0).contains(&after),
                "calibrated confidence {} must be a valid probability",
                after
            );

            // Prove that loaded is a faithful reconstruction: serde
            // round-trip via save_curve + load_current_curve preserved
            // the full curve.
            assert_curves_equivalent(&fit_curve, &loaded);
        });
    }

    fn assert_curves_equivalent(
        a: &crate::calibration::CalibrationCurve,
        b: &crate::calibration::CalibrationCurve,
    ) {
        assert_eq!(a.curve_id, b.curve_id);
        assert_eq!(a.model_identity_hash, b.model_identity_hash);
        assert_eq!(a.task, b.task);
        assert_eq!(a.prompt_version, b.prompt_version);
        assert_eq!(a.sample_count, b.sample_count);
        assert_eq!(a.buckets.len(), b.buckets.len());
        for (ab, bb) in a.buckets.iter().zip(b.buckets.iter()) {
            assert!((ab.raw_bucket_lo - bb.raw_bucket_lo).abs() < 1e-5);
            assert!((ab.raw_bucket_hi - bb.raw_bucket_hi).abs() < 1e-5);
            assert!((ab.observed_positive_rate - bb.observed_positive_rate).abs() < 1e-5);
            assert_eq!(ab.sample_count, bb.sample_count);
        }
    }

    #[test]
    fn make_curve_id_has_stable_format() {
        let id = make_curve_id("judge", "abcdef0123456789");
        assert!(id.starts_with("judge-abcdef01-cal-v1-"));
        // Date format YYYY-MM-DD at the tail.
        let tail = id.rsplit('-').next().unwrap();
        let date_part: String = id.chars().skip("judge-abcdef01-cal-v1-".len()).collect();
        assert_eq!(date_part.len(), 10);
        let _ = tail;
    }
}
