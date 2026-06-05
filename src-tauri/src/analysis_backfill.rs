// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Backfill / reconcile worker — Phase 2 of the scoring relevance funnel.
//!
//! The live corpus accumulates a large NEVER-scored backlog (~88% of items): the
//! analysis path only ever scores a recent window (≤1000 / ≤500-since), so items that
//! arrive faster than they're scored, or during downtime, age out of that window and
//! are never evaluated. This worker closes that gap.
//!
//! ## Design — prioritize, don't discard (Phase 0 finding)
//! Phase 0 measured that a cheap semantic gate cannot safely *filter* (no threshold
//! separates relevant from noise without dropping relevant items). So this worker does
//! NOT skip anything — it FULL-SCORES the entire unscored backlog, just in PRIORITY
//! order (high-stakes → stack releases → most-recent, via `get_unscored_backlog_chunk`).
//! Full recall is preserved; compute is simply spent best-first. The expensive LLM
//! rerank is NOT part of this path — only the cheap local PASIFA pipeline runs here, so
//! backfilling the whole corpus is affordable as a throttled background job.
//!
//! Convergent + resumable: progress lives in the DB (`scored_pipeline_version` /
//! `relevance_score`), so a crash or restart simply continues from where it left off.
//! Side-effect free w.r.t. the UI: it persists scores to the DB but does NOT touch the
//! in-memory analysis results the frontend is currently showing.

use tracing::{info, warn};

use crate::analysis::signal_classifier;
use crate::error::Result;
use crate::get_database;
use crate::scoring::{self, ScoringInput, ScoringOptions};

/// Per-cycle progress, returned to the scheduler and the dev command.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct BackfillProgress {
    pub scored_this_cycle: usize,
    pub relevant_this_cycle: usize,
    pub remaining_unscored: i64,
    pub done: bool,
}

/// Score one chunk of the never-scored backlog (highest-priority items first),
/// persist the results, and stamp the current pipeline version. Bounded by
/// `chunk_size`; call repeatedly (scheduler or loop) to converge.
pub(crate) async fn backfill_unscored_cycle(chunk_size: usize) -> Result<BackfillProgress> {
    let db = get_database()?;

    let items = db
        .get_unscored_backlog_chunk(chunk_size)
        .map_err(|e| format!("Failed to load unscored backlog: {e}"))?;
    if items.is_empty() {
        return Ok(BackfillProgress {
            scored_this_cycle: 0,
            relevant_this_cycle: 0,
            remaining_unscored: 0,
            done: true,
        });
    }

    // Same scoring context + options as the real pipeline (minus LLM rerank).
    let ctx = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        scoring::build_scoring_context(db),
    )
    .await
    .map_err(|_| String::from("Scoring context build timed out after 10s"))?
    .map_err(|e| format!("Failed to build scoring context: {e}"))?;
    let trend_topics = crate::detect_trend_topics(
        items
            .iter()
            .map(|item| (item.title.as_str(), item.content.as_str())),
    );
    let options = ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics,
    };

    let mut score_data: Vec<(i64, f32, Option<String>, Option<String>)> = Vec::new();
    let mut scored_ids: Vec<i64> = Vec::with_capacity(items.len());
    for item in &items {
        let r = scoring::score_item(
            &ScoringInput {
                id: item.id as u64,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: &item.embedding,
                created_at: Some(&item.created_at),
                detected_lang: &item.detected_lang,
                source_tags: &[],
                tags_json: item.tags.as_deref(),
                feed_origin: item.feed_origin.as_deref(),
            },
            &ctx,
            db,
            &options,
            Some(signal_classifier()),
        );
        // persist_analysis_scores only writes top_score > 0; mark_items_scored_version
        // stamps EVERY scored item (including noise) so the item leaves the unscored
        // backlog and never gets re-picked — same invariant as the analysis path.
        if r.top_score > 0.0 {
            score_data.push((
                item.id,
                r.top_score,
                r.signal_type.clone(),
                r.signal_priority.clone(),
            ));
        }
        scored_ids.push(item.id);
    }

    let relevant_this_cycle = score_data.len();
    if !score_data.is_empty() {
        if let Err(e) = db.persist_analysis_scores(&score_data) {
            warn!(target: "4da::backfill", error = %e, "Failed to persist backfill scores");
        }
    }
    if let Err(e) = db.mark_items_scored_version(&scored_ids, scoring::PIPELINE_VERSION) {
        warn!(target: "4da::backfill", error = %e, "Failed to stamp backfill pipeline version");
    }

    let remaining = db.count_unscored_backlog().unwrap_or(0);
    info!(
        target: "4da::backfill",
        scored = scored_ids.len(),
        relevant = relevant_this_cycle,
        remaining,
        "Backfill cycle complete"
    );

    Ok(BackfillProgress {
        scored_this_cycle: scored_ids.len(),
        relevant_this_cycle,
        remaining_unscored: remaining,
        done: remaining == 0,
    })
}

/// Dev/ops command: run a single backfill cycle on demand and report progress.
/// The scheduler runs this automatically; this command lets us validate and observe it.
#[tauri::command]
pub(crate) async fn run_backfill_cycle(chunk_size: Option<usize>) -> Result<BackfillProgress> {
    backfill_unscored_cycle(chunk_size.unwrap_or(500)).await
}
