// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 0 of the scoring relevance funnel: measure the cheap relevance-triage gate
//! against the LIVE corpus BEFORE wiring it into the pipeline.
//!
//! The critical output is `false_negative_rate` — the fraction of currently-relevant
//! items (stored `relevance_score >= relevant_threshold`) that the gate would DROP.
//! A relevance gate is only safe to deploy if that number is ~0; everything else here
//! (coverage, overall keep rate, reason histogram) is context for tuning.
//!
//! Read-only: builds the real scoring context and runs the gate in memory. It does not
//! change any stored score or pipeline state.

use serde::Serialize;

use crate::error::Result;
use crate::get_database;
use crate::scoring::{self, triage_item, TriageReason, TriageThresholds};

#[derive(Serialize)]
pub(crate) struct TriageAuditReport {
    // ── Coverage (whole corpus) ──
    pub total_items: i64,
    pub scored_items: i64,
    pub unscored_items: i64,
    pub unscored_over_7d: i64,
    pub on_current_version: i64,
    pub current_pipeline_version: i32,

    // ── Gate over a random sample of the whole corpus ──
    pub sample_size: usize,
    pub sample_kept: usize,
    pub sample_deferred: usize,
    pub sample_keep_rate: f32,
    pub keep_reason_histogram: Vec<(String, usize)>,

    // ── RECALL validation against the currently-relevant set ──
    pub relevant_threshold: f64,
    pub relevant_set_size: usize,
    pub relevant_kept: usize,
    pub relevant_dropped: usize,
    pub false_negative_rate: f32,
    /// Any relevant items the gate would drop — inspect these to confirm they are
    /// genuinely fine to defer (or to tune thresholds). Capped at 25.
    pub dropped_relevant_samples: Vec<DroppedSample>,

    pub taste_min: f32,
    pub topic_min: f32,
    pub has_taste_embedding: bool,
    pub topic_embedding_count: usize,
}

#[derive(Serialize)]
pub(crate) struct DroppedSample {
    pub id: i64,
    pub title: String,
    pub relevance_score: f64,
    pub similarity: f32,
}

/// Per-developer calibration snapshot (Phase 5 observability). Computes, from THIS
/// developer's own feedback, whether scoring is calibrated for them: precision/recall
/// miss rates + discrimination. Cold-start-silent (insufficient_feedback until enough
/// engagement). The scheduler logs this every 6h; this command surfaces it on demand
/// (for a calibration-health UI or a live check). Read-only.
#[tauri::command]
pub(crate) async fn get_calibration_snapshot(
    threshold: Option<f32>,
) -> Result<crate::scoring::CalibrationSnapshot> {
    let db = get_database()?;
    let t = threshold.unwrap_or_else(crate::get_relevance_threshold);
    let snap = crate::scoring::compute_calibration_snapshot(db, t)
        .map_err(|e| format!("Failed to compute calibration snapshot: {e}"))?;
    Ok(snap)
}

/// Lightweight scoring-coverage snapshot (Phase 1 observability). Cheap COUNT queries
/// only — no scoring-context build — so it can be called frequently / by the UI as the
/// safety net that makes silent coverage collapse (the class of bug behind the i64::MAX
/// stale-drain) visible.
#[derive(Serialize)]
pub(crate) struct ScoringCoverage {
    pub total: i64,
    pub scored: i64,
    pub unscored: i64,
    pub unscored_over_7d: i64,
    pub on_current_version: i64,
    pub current_pipeline_version: i32,
    /// % of the whole corpus scored under the CURRENT pipeline — the number that was
    /// ~2.6% when the stale-drain was silently empty.
    pub current_version_coverage_pct: f32,
    pub version_histogram: Vec<(i32, i64)>,
}

#[tauri::command]
pub(crate) async fn get_scoring_coverage() -> Result<ScoringCoverage> {
    let db = get_database()?;
    let conn = db.conn.lock();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM source_items", [], |r| r.get(0))?;
    let scored: i64 = conn.query_row(
        "SELECT COUNT(*) FROM source_items WHERE relevance_score IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    let unscored_over_7d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM source_items WHERE relevance_score IS NULL \
         AND created_at < datetime('now','-7 days')",
        [],
        |r| r.get(0),
    )?;
    let on_current_version: i64 = conn.query_row(
        "SELECT COUNT(*) FROM source_items WHERE scored_pipeline_version = ?1",
        [scoring::PIPELINE_VERSION],
        |r| r.get(0),
    )?;
    let mut histogram: Vec<(i32, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT scored_pipeline_version, COUNT(*) FROM source_items \
             WHERE relevance_score IS NOT NULL GROUP BY scored_pipeline_version ORDER BY 1",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        for row in rows {
            histogram.push(row?);
        }
    }
    let current_version_coverage_pct = if total > 0 {
        on_current_version as f32 / total as f32 * 100.0
    } else {
        0.0
    };
    Ok(ScoringCoverage {
        total,
        scored,
        unscored: total - scored,
        unscored_over_7d,
        on_current_version,
        current_pipeline_version: scoring::PIPELINE_VERSION,
        current_version_coverage_pct,
        version_histogram: histogram,
    })
}

fn reason_label(r: TriageReason) -> &'static str {
    match r {
        TriageReason::HighStakes => "high_stakes",
        TriageReason::DepMatch => "dep_match",
        TriageReason::TasteSimilar => "taste_similar",
        TriageReason::TopicSimilar => "topic_similar",
        TriageReason::NoEmbedding => "no_embedding",
        TriageReason::Deferred => "deferred",
    }
}

/// Measure the triage gate against the live corpus.
///
/// * `relevant_threshold` — stored relevance_score at/above which an item is treated
///   as "currently relevant" (the recall ground-truth set). 0.4 is a good default.
/// * `sample_limit` — size of the random whole-corpus sample for the keep-rate.
#[tauri::command]
pub(crate) async fn measure_triage_recall(
    relevant_threshold: f64,
    sample_limit: i64,
    taste_min: Option<f32>,
    topic_min: Option<f32>,
) -> Result<TriageAuditReport> {
    let db = get_database()?;

    // Build the same scoring context the real pipeline uses, so taste/topic/dep
    // signals are identical to production.
    let ctx = scoring::build_scoring_context(db)
        .await
        .map_err(|e| format!("Failed to build scoring context: {e}"))?;
    let defaults = TriageThresholds::default();
    let th = TriageThresholds {
        taste_min: taste_min.unwrap_or(defaults.taste_min),
        topic_min: topic_min.unwrap_or(defaults.topic_min),
    };

    // ── Coverage counts ──
    let (total_items, scored_items, unscored_items, unscored_over_7d, on_current_version) = {
        let conn = db.conn.lock();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM source_items", [], |r| r.get(0))?;
        let scored: i64 = conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE relevance_score IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let unscored_old: i64 = conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE relevance_score IS NULL \
             AND created_at < datetime('now','-7 days')",
            [],
            |r| r.get(0),
        )?;
        let on_current: i64 = conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE scored_pipeline_version = ?1",
            [scoring::PIPELINE_VERSION],
            |r| r.get(0),
        )?;
        (total, scored, total - scored, unscored_old, on_current)
    };

    // ── RECALL: run the gate over the currently-relevant set ──
    let relevant_rows = db.get_triage_audit_rows(Some(relevant_threshold), 10_000)?;
    let relevant_set_size = relevant_rows.len();
    let mut relevant_kept = 0usize;
    let mut dropped_relevant_samples = Vec::new();
    for row in &relevant_rows {
        let v = triage_item(
            &row.embedding,
            &row.title,
            &row.content,
            row.content_type.as_deref(),
            row.cve_ids.as_deref(),
            &ctx,
            &th,
        );
        if v.keep {
            relevant_kept += 1;
        } else if dropped_relevant_samples.len() < 25 {
            dropped_relevant_samples.push(DroppedSample {
                id: row.id,
                title: row.title.clone(),
                relevance_score: row.relevance_score.unwrap_or(0.0),
                similarity: v.similarity,
            });
        }
    }
    let relevant_dropped = relevant_set_size - relevant_kept;
    let false_negative_rate = if relevant_set_size > 0 {
        relevant_dropped as f32 / relevant_set_size as f32
    } else {
        0.0
    };

    // ── Keep-rate over a random whole-corpus sample ──
    let sample_rows = db.get_triage_audit_rows(None, sample_limit.max(0) as usize)?;
    let sample_size = sample_rows.len();
    let mut sample_kept = 0usize;
    let mut reason_counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for row in &sample_rows {
        let v = triage_item(
            &row.embedding,
            &row.title,
            &row.content,
            row.content_type.as_deref(),
            row.cve_ids.as_deref(),
            &ctx,
            &th,
        );
        *reason_counts.entry(reason_label(v.reason)).or_insert(0) += 1;
        if v.keep {
            sample_kept += 1;
        }
    }
    let sample_deferred = sample_size - sample_kept;
    let sample_keep_rate = if sample_size > 0 {
        sample_kept as f32 / sample_size as f32
    } else {
        0.0
    };
    let mut keep_reason_histogram: Vec<(String, usize)> = reason_counts
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    keep_reason_histogram.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(TriageAuditReport {
        total_items,
        scored_items,
        unscored_items,
        unscored_over_7d,
        on_current_version,
        current_pipeline_version: scoring::PIPELINE_VERSION,
        sample_size,
        sample_kept,
        sample_deferred,
        sample_keep_rate,
        keep_reason_histogram,
        relevant_threshold,
        relevant_set_size,
        relevant_kept,
        relevant_dropped,
        false_negative_rate,
        dropped_relevant_samples,
        taste_min: th.taste_min,
        topic_min: th.topic_min,
        has_taste_embedding: ctx.taste_embedding.is_some(),
        topic_embedding_count: ctx.topic_embeddings.len(),
    })
}
