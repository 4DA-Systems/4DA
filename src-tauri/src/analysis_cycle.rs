// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Analysis-cycle plumbing shared by the cache-first paths: the cycle result
//! carrier, the single curation-persistence site, the degraded-input persist
//! guard, and the rolling freshness-refresh merger.
//!
//! Factored out of `analysis_status.rs` (2026-08-24, stability wave) when the
//! DB-watermark differential work pushed that file past the size limit. The
//! orchestration (gate decision, scoring loops, state restore) stays there;
//! what lives here is the pure "what a cycle produced and how it lands in the
//! database" layer.

use tracing::warn;

use crate::SourceRelevance;

// ============================================================================
// Rank provenance (audit 2026-08-23 §3.5, items 12+26)
// ============================================================================

/// A `top_score` move smaller than this did not "fire" — float noise from
/// multiplying by 1.0-ish factors is not provenance.
const RANK_FACTOR_FIRED_EPSILON: f32 = 5e-4;

/// Records which batch-relative stages actually moved each item's rank value
/// (`top_score`) away from its evidence score this run, by diffing `top_score`
/// snapshots around each stage. `finish` serializes the fired factors into
/// [`SourceRelevance::rank_factors`] as compact JSON
/// (e.g. `{"ce":-0.12,"percentile":0.03}`), which [`persist_cycle_results`]
/// stamps into `source_items.rank_factors` next to `rank_score`.
///
/// Factor names in use: `"ce"` (cross-encoder blend), `"corroboration"`
/// (dedup/cluster boosts), `"diversity"` (domain + source-topic decay),
/// `"percentile"` (per-source normalization), `"llm"` (LLM advisor delta),
/// `"cap"` (final ceiling reassertion).
///
/// Items removed between snapshots (dedup, clustering) simply drop out; items
/// that appear between snapshots (serendipity swaps re-insert clones of ids
/// already tracked, so in practice none) are adopted without a delta.
pub(crate) struct RankProvenance {
    /// id → top_score as of the previous snapshot.
    last: std::collections::HashMap<u64, f32>,
    /// id → (factor name, delta) for every factor that fired.
    deltas: std::collections::HashMap<u64, Vec<(&'static str, f32)>>,
}

impl RankProvenance {
    /// Snapshot the pre-batch-layer scores (call right after the scoring loop).
    pub(crate) fn begin(results: &[SourceRelevance]) -> Self {
        Self {
            last: results.iter().map(|r| (r.id, r.top_score)).collect(),
            deltas: std::collections::HashMap::new(),
        }
    }

    /// Diff current `top_score`s against the previous snapshot, attributing any
    /// move to `factor`, then advance the snapshot.
    pub(crate) fn record(&mut self, results: &[SourceRelevance], factor: &'static str) {
        for r in results {
            if let Some(prev) = self.last.get(&r.id) {
                let delta = r.top_score - prev;
                if delta.abs() >= RANK_FACTOR_FIRED_EPSILON {
                    self.deltas.entry(r.id).or_default().push((factor, delta));
                }
            }
            self.last.insert(r.id, r.top_score);
        }
    }

    /// Serialize each item's fired factors into `rank_factors` (None when no
    /// factor fired — an honest "the batch layer was an identity here").
    pub(crate) fn finish(self, results: &mut [SourceRelevance]) {
        for r in results.iter_mut() {
            r.rank_factors = self.deltas.get(&r.id).and_then(|fired| {
                let mut map = serde_json::Map::new();
                for (factor, delta) in fired {
                    // 3 decimals: compact, and finer moves are sub-noise.
                    let rounded = (f64::from(*delta) * 1000.0).round() / 1000.0;
                    if let Some(num) = serde_json::Number::from_f64(rounded) {
                        map.insert((*factor).to_string(), serde_json::Value::Number(num));
                    }
                }
                if map.is_empty() {
                    None
                } else {
                    serde_json::to_string(&serde_json::Value::Object(map)).ok()
                }
            });
        }
    }
}

/// One item the scorer actually EVALUATED this run.
///
/// Captured immediately after the per-item scoring loop and before the
/// batch-relative layer, because that layer *deletes entries from the results
/// vector*: cross-source dedup, fuzzy-title dedup, topic dedup and temporal
/// clustering between them removed **831 of 1,458** scored items per cycle on
/// the live corpus (measured 2026-08-27). Stamping the version from the
/// survivors meant those 831 kept an older `scored_pipeline_version`, were
/// re-selected by the next cycle's deterministic drain query, and were scored
/// and discarded again — for ever. The measured conversion of a 500-item drain
/// batch was **5**.
///
/// EVIDENCE is per-item and batch-independent (2026-08-23 evidence/rank
/// separation), so an item losing a *display-ranking* contest says nothing
/// about whether its evidence score is current. Rank and verdict are
/// batch-relative decisions and deliberately stay on the survivors only.
#[derive(Clone)]
pub(crate) struct EvaluatedItem {
    pub id: i64,
    pub evidence_score: f32,
    pub signal_type: Option<String>,
    pub signal_priority: Option<String>,
    /// Mirrors [`has_systemic_degradation`] at capture time, so the degraded
    /// guard applies to dropped items too without needing their full result.
    pub systemically_degraded: bool,
}

impl From<&SourceRelevance> for EvaluatedItem {
    fn from(r: &SourceRelevance) -> Self {
        Self {
            id: r.id as i64,
            evidence_score: r.evidence_score,
            signal_type: r.signal_type.clone(),
            signal_priority: r.signal_priority.clone(),
            systemically_degraded: has_systemic_degradation(r),
        }
    }
}

/// A scoring pass's output: what survived to be displayed, and everything the
/// scorer actually evaluated on the way there.
pub(crate) struct ScoredBatch {
    pub results: Vec<SourceRelevance>,
    pub evaluated: Vec<EvaluatedItem>,
}

impl ScoredBatch {
    /// Combine the FINAL results with a snapshot taken before the batch layer.
    ///
    /// Survivors are taken from `results`, byte-for-byte what persistence wrote
    /// before this change — which matters because the batch layer legitimately
    /// rewrites an item after the snapshot (a serendipity pick flipped to
    /// relevant carries a 0.45 evidence cap, and the concept-graph injector
    /// pushes items that were never in the snapshot at all). The snapshot then
    /// contributes ONLY the ids `results` lost to dedup and clustering.
    ///
    /// So: identical writes for everything that used to be written, plus the
    /// dropped items that used to be silently abandoned.
    pub(crate) fn new(results: Vec<SourceRelevance>, pre_batch: Vec<EvaluatedItem>) -> Self {
        let survived: std::collections::HashSet<i64> =
            results.iter().map(|r| r.id as i64).collect();
        let mut evaluated: Vec<EvaluatedItem> = results.iter().map(EvaluatedItem::from).collect();
        evaluated.extend(pre_batch.into_iter().filter(|e| !survived.contains(&e.id)));
        Self { results, evaluated }
    }
}

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
    /// EVERY item the scorer evaluated this run, including the ones the batch
    /// layer then deleted from `results`. Evidence + version stamp are written
    /// from here; rank and verdict are written from `results`. See
    /// [`EvaluatedItem`].
    pub evaluated: Vec<EvaluatedItem>,
}

impl CycleResults {
    /// A full pass whose batch layer deleted entries: carries the pre-batch
    /// evaluated set through to persistence.
    pub(crate) fn full_from_batch(batch: ScoredBatch) -> Self {
        Self {
            results: batch.results,
            scored_ids: None,
            full_display: true,
            evaluated: batch.evaluated,
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

// `merge_stale_drain_batch` lived here until 2026-08-27. It appended a 500-item
// stale batch to the cycle's scoring set, and the batch layer below then deleted
// most of it before `persist_cycle_results` wrote the version stamp: measured on
// the live corpus, 495 of every 500 were re-scored and thrown away, giving a net
// 88 items/hour against a 47,000-item backlog. The drain now runs BESIDE the
// cycle (`analysis_backfill::drain_stale_scores_budgeted`), where nothing can
// delete its work before it is written. Do not re-merge it.

/// Rolling freshness-refresh budget per background cycle (2026-08-25
/// tightening T1). Now that the differential selection keys on CHANGE
/// (`created_at` / `content_updated_at`) instead of `last_seen` touches, an
/// unchanged item is never re-selected — so without a rotation, an item
/// scored once while young would keep its fresh-boost score forever (the old
/// quiet-cycle full re-score fired only when the differential set was EMPTY,
/// which it almost never is). 100 per cycle guarantees the ~650-item 7-day
/// window re-scores every ~7 cycles ≈ 3.5 h on the 30-minute scheduler,
/// preserving freshness-tier decay cadence at ~1/5th of the old
/// re-score-everything compute.
pub(crate) const FRESHNESS_REFRESH_PER_CYCLE: usize = 100;

/// Window the freshness refresh walks — mirrors the 7-day (168 h) window the
/// full-analysis and quiet-cycle paths score (`get_items_tiered(168, ...)`).
pub(crate) const FRESHNESS_REFRESH_WINDOW_HOURS: i64 = 168;

/// Merge the stalest-scored slice of the recent window into `items` (skipping
/// any already selected), so freshness-tier decay keeps re-scoring the whole
/// window on a bounded per-cycle budget. Returns the number of items added.
///
/// Rotation is guaranteed by the `scored_at` stamp: every scoring path stamps
/// it on evaluation (suppressed and zero-score evaluations included), so the
/// items this batch feeds into a cycle sort to the BACK of the stalest-first
/// ordering ([`crate::db::Database::get_freshness_refresh_batch`]) and the
/// next cycle picks the next-stalest slice instead.
pub(crate) fn merge_freshness_refresh_batch(
    db: &crate::db::Database,
    items: &mut Vec<crate::db::StoredSourceItem>,
) -> usize {
    let batch = db
        .get_freshness_refresh_batch(FRESHNESS_REFRESH_WINDOW_HOURS, FRESHNESS_REFRESH_PER_CYCLE)
        .unwrap_or_default();
    if batch.is_empty() {
        return 0;
    }
    let existing: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
    let added: Vec<_> = batch
        .into_iter()
        .filter(|s| !existing.contains(&s.id))
        .collect();
    let count = added.len();
    items.extend(added);
    count
}

/// Persist everything a completed analysis cycle owes the DB: EVIDENCE scores
/// (`relevance_score` ← `evidence_score`), the batch-relative RANK layer
/// (`rank_score`/`rank_factors`/`rank_scored_at` ← `top_score` + provenance),
/// pipeline-version stamps, the per-item curation VERDICT (`feed_relevant`), and
/// the `scoring_events` telemetry row.
///
/// ## Evidence / rank separation (audit 2026-08-23 §3.5, items 12+26)
///
/// `relevance_score` is the item's EVIDENCE: pure `score_item` output, a fixed
/// point independent of whatever batch the item happened to share a run with.
/// The batch-relative layer (cross-encoder blend, dedup corroboration boosts,
/// diversity decay, per-source percentile, LLM advisor delta, final cap) writes
/// `top_score` only, and lands in its own columns with provenance. The audit's
/// ±0.43 durable-score oscillation was exactly these factors being persisted
/// INTO `relevance_score`: the same item's stored score depended on the rest of
/// its batch and on which path (GUI with cross-encoder vs headless without)
/// last persisted it. Rank churn is expected and honest — it is a ranking with
/// a provenance stamp, no longer poisoning evidence. Ranked read surfaces order
/// by `db::RANKED_ORDER_EXPR` (rank when present, evidence otherwise);
/// membership/threshold filters keep reading evidence.
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
pub(crate) fn persist_cycle_results(
    db: &crate::db::Database,
    results: &[SourceRelevance],
    evaluated: &[EvaluatedItem],
) {
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
    let protected = degraded_protected_ids(db, evaluated);
    if !protected.is_empty() {
        warn!(
            target: "4da::scoring",
            degraded_skipped = protected.len(),
            "Systemically degraded run — existing durable scores/verdicts kept (display unaffected)"
        );
    }
    let persistable_id = |id: i64| !protected.contains(&id);
    let persistable = |r: &SourceRelevance| persistable_id(r.id as i64);

    // EVIDENCE scores — every EVALUATED item whose evidence is > 0 (noise is
    // version-stamped below), NOT just the batch-layer survivors.
    // `evidence_score`, not `top_score`: the write goes through the same
    // hysteresis damper, which now stabilizes evidence.
    //
    // Reading from `evaluated` rather than `results` is the whole of the
    // 2026-08-27 drain fix. Evidence is a per-item, batch-independent value; an
    // item deleted by dedup was still judged by this pipeline version and its
    // evidence is as current as any survivor's. Writing only survivors left the
    // rest carrying an older version stamp AND an older score, and the drain
    // re-selected them for ever.
    let score_data: Vec<(i64, f32, Option<String>, Option<String>)> = evaluated
        .iter()
        .filter(|e| e.evidence_score > 0.0 && persistable_id(e.id))
        .map(|e| {
            (
                e.id,
                e.evidence_score,
                e.signal_type.clone(),
                e.signal_priority.clone(),
            )
        })
        .collect();
    if !score_data.is_empty() {
        if let Err(e) = db.persist_analysis_scores(&score_data, "analysis") {
            warn!(target: "4da::scoring", error = %e, "Failed to persist relevance scores");
        }
    }

    // RANK layer — the WHOLE ranked result set this run (`results` is exactly
    // what the batch layer ranked: the full survivor set on a full pass, the
    // scored subset on a differential run — carried-over display items were
    // not re-ranked and keep the rank of the run that ranked them).
    //
    // Deliberately `results`, not `evaluated`: rank IS the batch-relative
    // value, so an item the batch layer dropped genuinely has no rank from this
    // run and must keep whatever rank last ranked it. Same reasoning holds for
    // the curation verdict below — dedup dropping an item is a statement about
    // the batch, and promoting a duplicate into the feed because it "was
    // scored" would be exactly wrong. Their staleness is owned by
    // `feed_verdict_version` and the demote-only reconciliation pass. Written
    // without hysteresis (rank churn is honest), and `persist_rank_scores`
    // touches neither `scored_pipeline_version` nor the churn telemetry —
    // both track evidence. The degraded guard applies: a systemically
    // degraded run's ranks are as confidently wrong as its scores.
    let rank_data: Vec<(i64, f32, Option<String>)> = results
        .iter()
        .filter(|r| r.top_score > 0.0 && persistable(r))
        .map(|r| (r.id as i64, r.top_score, r.rank_factors.clone()))
        .collect();
    if !rank_data.is_empty() {
        if let Err(e) = db.persist_rank_scores(&rank_data) {
            warn!(target: "4da::scoring", error = %e, "Failed to persist rank scores");
        }
    }

    // Stamp the pipeline version for EVERY item EVALUATED this run — including
    // noise items (evidence == 0) that persist_analysis_scores skips, and
    // including the ones the batch layer deleted from `results`. Without the
    // first, zero-scoring stale items never leave the version-bump drain set;
    // without the second, 57% of every cycle's work was thrown away unstamped
    // (2026-08-27). Degraded-protected items are NOT stamped: stamping without
    // writing would mark a stale score as current and silently exempt it from
    // the drain.
    let scored_ids: Vec<i64> = evaluated
        .iter()
        .filter(|e| persistable_id(e.id))
        .map(|e| e.id)
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
    evaluated: &[EvaluatedItem],
) -> std::collections::HashSet<i64> {
    let degraded: Vec<i64> = evaluated
        .iter()
        .filter(|e| e.systemically_degraded)
        .map(|e| e.id)
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

#[cfg(test)]
mod evidence_rank_tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    /// Minimal result carrying distinct evidence and rank values, as the
    /// analyzer produces after the batch layer: `evidence_score` is the pure
    /// score_item output; `top_score` carries the batch-relative mutations.
    fn make_result(id: u64, evidence: f32, rank: f32, relevant: bool) -> SourceRelevance {
        let mut r: SourceRelevance = serde_json::from_value(serde_json::json!({
            "id": id,
            "title": format!("item {id}"),
            "url": null,
            "top_score": rank,
            "matches": [],
            "relevant": relevant,
            "source_type": "hackernews",
            "evidence_score": evidence,
        }))
        .expect("SourceRelevance from JSON");
        r.rank_factors = None;
        r
    }

    /// Items 12+26 (provenance): only factors that actually moved `top_score`
    /// are recorded, with their deltas; untouched items get `None`, and
    /// sub-epsilon float noise never counts as a fired factor.
    #[test]
    fn rank_provenance_records_only_fired_factors() {
        let mut results = vec![
            make_result(1, 0.50, 0.50, true),
            make_result(2, 0.70, 0.70, true),
            make_result(3, 0.40, 0.40, false),
        ];
        let mut prov = RankProvenance::begin(&results);
        // "ce" moves item 1 up; item 3 wobbles below the fired epsilon.
        results[0].top_score = 0.62;
        results[2].top_score += 1.0e-5;
        prov.record(&results, "ce");
        // "percentile" moves item 2 down.
        results[1].top_score = 0.65;
        prov.record(&results, "percentile");
        prov.finish(&mut results);

        let f1: serde_json::Value =
            serde_json::from_str(results[0].rank_factors.as_deref().expect("item 1 fired"))
                .unwrap();
        assert!(
            (f1["ce"].as_f64().unwrap() - 0.12).abs() < 1e-9,
            "ce delta recorded: {f1}"
        );
        assert!(f1.get("percentile").is_none(), "unfired factor absent");
        let f2: serde_json::Value =
            serde_json::from_str(results[1].rank_factors.as_deref().expect("item 2 fired"))
                .unwrap();
        assert!((f2["percentile"].as_f64().unwrap() + 0.05).abs() < 1e-9);
        assert!(
            results[2].rank_factors.is_none(),
            "sub-epsilon wobble is not provenance"
        );
    }

    /// Items 12+26 (the contract): a batch-layer mutation changes what lands
    /// in `rank_score` but NOT what lands in `relevance_score`. One persist
    /// call writes both layers to their own columns, with provenance and a
    /// timestamp on the rank side and the version stamp on the evidence side.
    #[test]
    fn persist_cycle_separates_evidence_from_rank() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "er1", "separated", "x");
        let mut r = make_result(id as u64, 0.62, 0.87, true);
        r.rank_factors = Some(r#"{"ce":0.25}"#.to_string());

        let evaluated = [EvaluatedItem::from(&r)];
        persist_cycle_results(&db, std::slice::from_ref(&r), &evaluated);

        let conn = db.conn.lock();
        let (evidence, rank, factors, ranked_at, version): (
            Option<f64>,
            Option<f64>,
            Option<String>,
            Option<String>,
            i64,
        ) = conn
            .query_row(
                "SELECT relevance_score, rank_score, rank_factors, rank_scored_at,
                        scored_pipeline_version
                 FROM source_items WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert!(
            (evidence.unwrap() - 0.62).abs() < 1e-6,
            "relevance_score is the EVIDENCE value, not the batch-mutated top_score"
        );
        assert!(
            (rank.unwrap() - 0.87).abs() < 1e-6,
            "rank_score is the batch layer's final top_score"
        );
        assert_eq!(factors.as_deref(), Some(r#"{"ce":0.25}"#));
        assert!(
            ranked_at.is_some(),
            "rank write is provenance-stamped in time"
        );
        assert_eq!(version, i64::from(crate::scoring::PIPELINE_VERSION));
    }

    /// Item 12 (churn side): the evidence write is hysteresis-damped, the rank
    /// write is not — a batch-relative wobble re-ranks freely while the
    /// durable evidence stays put.
    #[test]
    fn evidence_damped_while_rank_rewrites_freely() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "er2", "damped", "x");
        let first = [make_result(id as u64, 0.60, 0.60, true)];
        let first_eval: Vec<EvaluatedItem> = first.iter().map(EvaluatedItem::from).collect();
        persist_cycle_results(&db, &first, &first_eval);
        // Next cycle: evidence wobbles sub-hysteresis, rank swings hard.
        let second = [make_result(id as u64, 0.62, 0.31, true)];
        let second_eval: Vec<EvaluatedItem> = second.iter().map(EvaluatedItem::from).collect();
        persist_cycle_results(&db, &second, &second_eval);

        let conn = db.conn.lock();
        let (evidence, rank): (Option<f64>, Option<f64>) = conn
            .query_row(
                "SELECT relevance_score, rank_score FROM source_items WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(
            (evidence.unwrap() - 0.60).abs() < 1e-6,
            "sub-hysteresis evidence wobble keeps the old durable score"
        );
        assert!(
            (rank.unwrap() - 0.31).abs() < 1e-6,
            "rank rewrites without hysteresis — rank churn is honest"
        );
    }
}
