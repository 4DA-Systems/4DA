// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Analysis-cycle plumbing shared by the cache-first paths: the cycle result
//! carrier, the single curation-persistence site, the degraded-input persist
//! guard, and the stale-version drain merger.
//!
//! Factored out of `analysis_status.rs` (2026-08-24, stability wave) when the
//! DB-watermark differential work pushed that file past the size limit. The
//! orchestration (gate decision, scoring loops, state restore) stays there;
//! what lives here is the pure "what a cycle produced and how it lands in the
//! database" layer.

use tracing::warn;

use crate::scoring;
use crate::SourceRelevance;

/// What one analysis cycle produced (internal to the analysis boundary).
pub(crate) struct CycleResults {
    /// The cycle's result set. On a differential run with in-memory previous
    /// results this is the MERGED display corpus; otherwise what was scored.
    pub results: Vec<SourceRelevance>,
    /// IDs actually scored THIS run. `None` = full pass (everything in
    /// `results` was scored). Persistence writes ONLY this subset: re-writing
    /// carried-over previous results would stamp stale in-memory state over
    /// newer DB truth (backfill / drain / repair writes land between cycles).
    pub scored_ids: Option<std::collections::HashSet<u64>>,
    /// Whether `results` is a COMPLETE display corpus, safe to replace shared
    /// state with. False exactly on a differential run without in-memory
    /// previous results (headless / first scheduled run of a GUI process):
    /// that partial set reaches the UI only via the frontend's merging
    /// `background-results` path, never by replacement.
    pub full_display: bool,
}

impl CycleResults {
    /// A full pass: everything in `results` was scored this run.
    pub(crate) fn full(results: Vec<SourceRelevance>) -> Self {
        Self {
            results,
            scored_ids: None,
            full_display: true,
        }
    }
    /// Items actually scored this run — the honest `engine_runs.items_scored`.
    /// The audit's smoking gun was receipts reading "scored 651-674, new 44-62"
    /// every 30 minutes; a differential receipt now reports the real work.
    pub(crate) fn scored_count(&self) -> usize {
        self.scored_ids
            .as_ref()
            .map_or(self.results.len(), std::collections::HashSet::len)
    }
    /// The results actually scored this run (all of them on a full pass) —
    /// what "NEW this cycle" means for notifications and receipts.
    pub(crate) fn scored_results(&self) -> impl Iterator<Item = &SourceRelevance> {
        self.results.iter().filter(|r| match &self.scored_ids {
            None => true,
            Some(ids) => ids.contains(&r.id),
        })
    }
    /// Relevant among the scored subset — "new relevant" for notifications.
    pub(crate) fn scored_relevant(&self) -> usize {
        self.scored_results().filter(|r| r.relevant).count()
    }
}

/// Merge a batch of items still scored under an older `PIPELINE_VERSION` into
/// `items` (skipping any already present), so a version bump drains the backlog
/// a bounded batch per analysis run. Returns the number of items added.
///
/// Runs on BOTH the differential and full-analysis paths. Gating it behind
/// differential mode (the previous behaviour) meant a version bump only drained
/// via the scheduler's differential runs and never on first-run-after-restart or
/// manual `run_cached_analysis` invokes, so the backlog re-scored far slower than
/// intended. The 500-item cap and the stack-release-first ordering both live in
/// `get_stale_scored_items`.
pub(crate) fn merge_stale_drain_batch(
    db: &crate::db::Database,
    items: &mut Vec<crate::db::StoredSourceItem>,
) -> usize {
    // Scoped-epoch promotion before pulling the batch: provably-unaffected
    // items are re-stamped instead of re-scored, so the 500-item budget is
    // spent only on the slice the version bump could actually change.
    // ~0ms no-op when nothing is registered or the corpus is current.
    scoring::epochs::promote_unaffected_stale_logged(db);
    let stale = db
        .get_stale_scored_items(scoring::PIPELINE_VERSION, 500)
        .unwrap_or_default();
    if stale.is_empty() {
        return 0;
    }
    let existing: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
    let added: Vec<_> = stale
        .into_iter()
        .filter(|s| !existing.contains(&s.id))
        .collect();
    let count = added.len();
    items.extend(added);
    count
}

/// Persist everything a completed analysis cycle owes the DB: relevance scores,
/// pipeline-version stamps, the per-item curation VERDICT (`feed_relevant`), and
/// the `scoring_events` telemetry row.
///
/// This is the SINGLE curation-persistence site for every real analysis path.
/// Foreground (`run_cached_analysis`), scheduled (`run_scheduled_analysis`), and
/// headless (`headless::run_cycle`) all reach the scorer through
/// `analyze_cached_content_inner`, so persisting here guarantees all three curate
/// the corpus identically. Previously each caller wrapper hand-copied this block
/// and the scheduled wrapper omitted the verdict + scoring-event writes — so
/// background / tray-resident / headless runs SCORED without ever CURATING:
/// `feed_relevant` froze, and the content graph + calibration telemetry silently
/// went stale (live 2026-07-22: 586 items unjudged across 14 scheduled cycles).
/// Centralising here makes score-without-curate structurally impossible.
pub(crate) fn persist_cycle_results(db: &crate::db::Database, results: &[SourceRelevance]) {
    // ── Degraded-input persist guard (2026-08-23 audit, item 11) ─────────
    // A run whose inputs SYSTEMICALLY collapsed (dependency-intel load failure,
    // context-KNN failure) still produces scores — confidently wrong ones (the
    // dep axis reads as "user has no deps"; the context axis as "no match").
    // Those must not overwrite fresh durable scores or verdicts. The in-memory
    // results stay intact for display; the durable corpus keeps the last
    // healthy run's judgment. Escape hatch: an item whose durable curation is
    // older than DEGRADED_OVERWRITE_MAX_AGE_DAYS (or absent) accepts the
    // degraded write — a permanently-degraded deployment must not freeze.
    // Per-item "embedding_missing" is deliberately NOT guarded: it is an
    // attribute of the item, not a collapse of the run.
    let protected = degraded_protected_ids(db, results);
    if !protected.is_empty() {
        warn!(
            target: "4da::scoring",
            degraded_skipped = protected.len(),
            "Systemically degraded run — existing durable scores/verdicts kept (display unaffected)"
        );
    }
    let persistable = |r: &SourceRelevance| !protected.contains(&(r.id as i64));

    // Relevance scores — only items that scored > 0 (noise is version-stamped below).
    let score_data: Vec<(i64, f32, Option<String>, Option<String>)> = results
        .iter()
        .filter(|r| r.top_score > 0.0 && persistable(r))
        .map(|r| {
            (
                r.id as i64,
                r.top_score,
                r.signal_type.clone(),
                r.signal_priority.clone(),
            )
        })
        .collect();
    if !score_data.is_empty() {
        if let Err(e) = db.persist_analysis_scores(&score_data, "analysis") {
            warn!(target: "4da::scoring", error = %e, "Failed to persist relevance scores");
        }
    }

    // Stamp the pipeline version for EVERY item scored this run — including noise
    // items (top_score == 0) that persist_analysis_scores skips. Without this,
    // zero-scoring stale items never leave the version-bump drain set.
    // Degraded-protected items are NOT stamped: stamping without writing would
    // mark a stale score as current and silently exempt it from the drain.
    let scored_ids: Vec<i64> = results
        .iter()
        .filter(|r| persistable(r))
        .map(|r| r.id as i64)
        .collect();
    if let Err(e) = db.mark_items_scored_version(&scored_ids, crate::scoring::PIPELINE_VERSION) {
        warn!(target: "4da::scoring", error = %e, "Failed to stamp scored pipeline version");
    }

    // Persist the curation VERDICT for every item judged this run (relevant = in
    // the curated corpus). Corpus-parity surfaces (content graph) select on this
    // instead of re-deriving a corpus from raw cross-epoch scores.
    //
    // Stamped with the pipeline version AND the provenance of the decision
    // (Phase 101). Provenance is read from `SourceRelevance::serendipity`, which
    // BOTH anti-bubble paths already set — `compute_serendipity_candidates`
    // (flips a scorer-rejected item to relevant) and the concept-graph injection
    // (never scores the item at all). Reading the flag rather than inferring a
    // signature matters: an inferred one ("top_score == 0.45") catches only the
    // second path, and the first keeps its original score, so it is
    // indistinguishable from a stale verdict by score alone.
    let verdicts: Vec<(i64, bool, crate::db::VerdictSource)> = results
        .iter()
        .filter(|r| persistable(r))
        .map(|r| {
            (
                r.id as i64,
                r.relevant,
                crate::db::VerdictSource::from_serendipity(r.serendipity),
            )
        })
        .collect();
    if let Err(e) = db.persist_feed_verdicts(&verdicts, crate::scoring::PIPELINE_VERSION) {
        warn!(target: "4da::scoring", error = %e, "Failed to persist feed verdicts");
    }

    // Scoring-event telemetry row — drives calibration + recalibration backtesting.
    let total_scored = results.len();
    let relevant_count = results.iter().filter(|r| r.relevant).count();
    let scores: Vec<f32> = results.iter().map(|r| r.top_score).collect();
    let avg_score = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f32>() / scores.len() as f32
    };
    let max_score = scores.iter().copied().fold(0.0f32, f32::max);
    // The three per-cycle counters are NOT measured on this path (gate/cap
    // detail lives in ScoringTelemetry logs; briefing_items only exists when a
    // briefing builds). They used to be hardcoded 0, which read as "gates
    // never fire" in any analysis of the table — absent data is NULL.
    let _ = db.record_scoring_event(
        total_scored,
        relevant_count,
        avg_score,
        max_score,
        None, // gate_rejections — not measured here; see ScoringTelemetry logs
        None, // commodity_caps — not measured here; see ScoringTelemetry logs
        None, // briefing_items — only meaningful on briefing build
    );

    // Necessity persistence (audit item 25): the breakdowns only exist
    // in-memory at this single shared persist site — every real path
    // (foreground/scheduled/headless) funnels through here, so this is the
    // one honest place to make item_necessity durable for the MCP surface.
    crate::scoring::necessity::persist_from_results(db, results);
}

/// How old (days) an item's durable curation may be before a systemically
/// degraded run's write is accepted anyway — the item-11 freeze escape hatch.
pub(crate) const DEGRADED_OVERWRITE_MAX_AGE_DAYS: u32 = 7;

/// True when this result was produced under a SYSTEMIC input collapse
/// (`dep_intel_load_failed` / `context_knn_failed` on the breakdown, set by
/// `pipeline_v2`). Per-item `embedding_missing` does not count — see the guard
/// note in [`persist_cycle_results`].
fn has_systemic_degradation(r: &SourceRelevance) -> bool {
    r.score_breakdown.as_ref().is_some_and(|b| {
        b.degraded_inputs
            .iter()
            .any(|m| m == "dep_intel_load_failed" || m == "context_knn_failed")
    })
}

/// The subset of `results` the degraded-input guard protects: systemically
/// degraded AND fresh durable curation exists. Probe failure fails OPEN
/// (persist) — the guard protects data quality, and wedging persistence shut
/// on a broken probe would be the worse failure.
fn degraded_protected_ids(
    db: &crate::db::Database,
    results: &[SourceRelevance],
) -> std::collections::HashSet<i64> {
    let degraded: Vec<i64> = results
        .iter()
        .filter(|r| has_systemic_degradation(r))
        .map(|r| r.id as i64)
        .collect();
    if degraded.is_empty() {
        return std::collections::HashSet::new();
    }
    match db.ids_with_fresh_durable_scores(&degraded, DEGRADED_OVERWRITE_MAX_AGE_DAYS) {
        Ok(set) => set,
        Err(e) => {
            warn!(target: "4da::scoring", error = %e, "Degraded-guard freshness probe failed — persisting (fail-open)");
            std::collections::HashSet::new()
        }
    }
}
