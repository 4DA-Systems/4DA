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

/// Below this batch size, the thread-spawn overhead outweighs the win — score
/// sequentially. The scheduled trickle (500/run) and the bulk drain (2000/chunk)
/// both clear this comfortably; only tiny probe/tail batches fall through.
const PARALLEL_SCORE_MIN_ITEMS: usize = 64;

/// Reduce ONE freshly-scored item to the tuple `persist_analysis_scores` writes,
/// applying THE canonical score-shaping boundary (`scoring::finalize_scores`)
/// first.
///
/// `finalize_scores` documents itself as "call at the end of EVERY analysis path
/// (cached, fresh, deep-scan, backfill, headless)", but neither backfill cycle
/// called it — `backfill_unscored_cycle` and `drain_stale_version_cycle` both
/// went straight from `score_chunk` to `db.persist_analysis_scores`. Only
/// `analysis_deep_scan.rs` and `scoring/analyzer.rs` honoured the contract.
///
/// The gap is LATENT, not live: `score_item` already applies both
/// `apply_final_soft_ceiling` and the categorical `score_ceiling` internally,
/// and this path deliberately runs no post-pipeline reranker, so nothing here
/// currently exceeds the invariant. It was unguarded, though — the two paths
/// that DO call `finalize_scores` acquired their rerankers and cluster boosts
/// after their scores were computed, and the first boost added to the backfill
/// path would have persisted unbounded. Funnelling every persisted tuple
/// through this one function makes the boundary impossible to skip by
/// construction: `score_chunk` has no other way to build the tuple.
fn persistable(
    item_id: i64,
    mut r: crate::SourceRelevance,
) -> Option<(i64, f32, Option<String>, Option<String>)> {
    scoring::finalize_scores(std::slice::from_mut(&mut r));
    // persist_analysis_scores only writes top_score > 0; the caller returns the
    // id unconditionally so mark_items_scored_version stamps even re-scored noise.
    (r.top_score > 0.0).then_some((item_id, r.top_score, r.signal_type, r.signal_priority))
}

/// Score a batch of items through the cheap PASIFA pipeline, in parallel across
/// one OS thread per pooled reader. Both backfill (never-scored) and the
/// stale-version drain run this identical per-item loop; extracting it keeps
/// them in lock-step and lets the drain use every core.
///
/// SAFETY / correctness: `score_item` is a pure function of the item plus
/// read-only `ctx`/`db` access — it performs no DB writes and mutates no shared
/// state, so scoring items concurrently changes only wall-clock, never the
/// result. Each thread borrows its OWN pooled read connection for the per-item
/// KNN (`read_conn` hands out a distinct reader via non-blocking try-lock), so
/// threads don't serialize on a single reader; the cap at
/// [`crate::db::Database::read_pool_len`] keeps thread count matched to
/// available readers (extras would fall back to the writer lock and serialize).
/// The pool is sized from the host since 2026-08-27 (`db::read_pool_size`), so
/// this runs 8 threads on an 8-core box where it used to run a fixed 3 —
/// measured 3.97x vs 2.46x. Results are keyed by item id and merged
/// order-independently. Rust's `Send`/`Sync` bounds on `thread::scope` prove the
/// absence of data races at compile time. Returns (persistable scores, all
/// scored ids) — the second is EVERY id (incl. re-scored-to-noise) so the caller
/// can version-stamp them and the drain converges.
fn score_chunk(
    items: &[crate::db::StoredSourceItem],
    ctx: &scoring::ScoringContext,
    db: &crate::db::Database,
    options: &ScoringOptions,
    classifier: Option<&crate::signals::SignalClassifier>,
) -> (Vec<(i64, f32, Option<String>, Option<String>)>, Vec<i64>) {
    type Scored = (Option<(i64, f32, Option<String>, Option<String>)>, i64);
    let score_one = |item: &crate::db::StoredSourceItem| -> Scored {
        // Path parity: the analyzer path parses topic tags into source_tags;
        // this path used to pass &[] — one of the "two score families" the
        // 2026-08-23 audit traced (§3.5). Same parse, same signal.
        let parsed_tags = scoring::parse_tags_topics(item.tags.as_deref());
        let r = scoring::score_item(
            &ScoringInput {
                id: item.id as u64,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: &item.embedding,
                // Effective publication date: honest freshness (falls back to first-seen)
                created_at: Some(item.published_at.as_ref().unwrap_or(&item.created_at)),
                detected_lang: &item.detected_lang,
                source_tags: &parsed_tags,
                tags_json: item.tags.as_deref(),
                feed_origin: item.feed_origin.as_deref(),
                source_id: Some(&item.source_id),
            },
            ctx,
            db,
            options,
            classifier,
        );
        (persistable(item.id, r), item.id)
    };

    // One thread per pooled reader, never more: a thread past the pool falls
    // through read_conn() to the writer lock and serialises against every other
    // writer. `read_pool_len()` is 0 for the in-memory test database, which has
    // no pool — that reads as "sequential", which is correct there.
    let threads = db
        .read_pool_len()
        .min(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));

    let collected: Vec<Scored> = if threads <= 1 || items.len() < PARALLEL_SCORE_MIN_ITEMS {
        items.iter().map(score_one).collect()
    } else {
        let chunk_size = items.len().div_ceil(threads);
        let score_one_ref = &score_one; // Sync fn shared read-only across threads
        std::thread::scope(|s| {
            let handles: Vec<_> = items
                .chunks(chunk_size)
                .map(|chunk| {
                    s.spawn(move || chunk.iter().map(score_one_ref).collect::<Vec<Scored>>())
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        })
    };

    let mut score_data = Vec::new();
    let mut scored_ids = Vec::with_capacity(collected.len());
    for (scored, id) in collected {
        if let Some(s) = scored {
            score_data.push(s);
        }
        scored_ids.push(id);
    }
    (score_data, scored_ids)
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

    // persist_analysis_scores only writes top_score > 0; mark_items_scored_version
    // stamps EVERY scored id (including noise) so the item leaves the unscored
    // backlog and never gets re-picked — same invariant as the analysis path.
    let (score_data, scored_ids) =
        score_chunk(&items, &ctx, db, &options, Some(signal_classifier()));

    let relevant_this_cycle = score_data.len();
    if !score_data.is_empty() {
        if let Err(e) = db.persist_analysis_scores(&score_data, "backfill") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::ScoringContext;

    /// Minimal `SourceRelevance` via serde defaults, with an optional
    /// categorical `score_ceiling` in the breakdown.
    fn relevance(top_score: f32, score_ceiling: Option<f32>) -> crate::SourceRelevance {
        let mut r: crate::SourceRelevance = serde_json::from_value(serde_json::json!({
            "id": 7,
            "title": "item-7",
            "url": null,
            "top_score": top_score,
            "matches": [],
            "relevant": true,
            "source_type": "test",
        }))
        .expect("SourceRelevance from JSON");
        if let Some(ceiling) = score_ceiling {
            r.score_breakdown = Some(
                serde_json::from_value(serde_json::json!({
                    "context_score": 0.0,
                    "interest_score": 0.0,
                    "ace_boost": 0.0,
                    "affinity_mult": 1.0,
                    "anti_penalty": 0.0,
                    "confidence_by_signal": {},
                    "score_ceiling": ceiling,
                }))
                .expect("ScoreBreakdown from JSON"),
            );
        }
        r
    }

    /// The backfill persist path must honour the categorical `score_ceiling`.
    /// Without `finalize_scores` a 0.99 overwrite on a 0.60-capped item is
    /// persisted verbatim.
    #[test]
    fn persistable_reasserts_the_categorical_score_ceiling() {
        let (id, score, _, _) =
            persistable(7, relevance(0.99, Some(0.60))).expect("positive score is persistable");
        assert_eq!(id, 7);
        assert!(
            score <= 0.60 + f32::EPSILON,
            "capped item must not be persisted above its ceiling, got {score}"
        );
    }

    /// …and the absolute-max boundary, which is the invariant
    /// `final_ceiling.absolute_max` in `scoring/pipeline.scoring`.
    #[test]
    fn persistable_holds_the_absolute_max_boundary() {
        let (_, score, _, _) =
            persistable(7, relevance(0.99, None)).expect("positive score is persistable");
        assert!(
            score < 0.99,
            "a score above the boundary knee must be compressed, got {score}"
        );
        assert!(
            score <= crate::scoring_config::FINAL_CEILING_ABSOLUTE_MAX,
            "persisted score must honour final_ceiling.absolute_max, got {score}"
        );
    }

    /// Noise (score 0) is never persisted — the caller still returns the id so
    /// `mark_items_scored_version` stamps it and the drain converges.
    #[test]
    fn persistable_drops_zero_scores() {
        assert!(persistable(7, relevance(0.0, None)).is_none());
    }

    fn item(id: i64) -> crate::db::StoredSourceItem {
        crate::db::StoredSourceItem {
            id,
            source_type: "hackernews".to_string(),
            source_id: format!("hn-{id}"),
            url: Some(format!("https://example.com/{id}")),
            title: format!("Rust tauri async runtime item {id}"),
            content: "A post about rust, tokio, and tauri IPC command handlers.".to_string(),
            content_hash: format!("hash-{id}"),
            // Non-zero embedding so the KNN path actually executes (concurrently).
            embedding: crate::test_utils::seed_embedding(&format!("item-{id}")),
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
            detected_lang: "en".to_string(),
            feed_origin: None,
            tags: None,
            published_at: None,
        }
    }

    /// The parallel path (items >= PARALLEL_SCORE_MIN_ITEMS) must re-score EVERY
    /// item exactly once — deadlock-free and complete — with each thread driving a
    /// concurrent per-item KNN against the shared context store. Proves the
    /// extracted `score_chunk` behaves identically to the old sequential loop on
    /// its output contract (all ids stamped), just across cores.
    #[test]
    fn score_chunk_parallel_covers_every_item() {
        let db = crate::test_utils::test_db();
        // Seed a context chunk so cached_context_count > 0 and the KNN has rows to
        // scan concurrently from multiple threads.
        let ctx_vec = crate::test_utils::seed_embedding("rust tauri ipc context");
        db.upsert_context(
            "src/main.rs",
            "rust tauri ipc command handler registering invoke handlers",
            &ctx_vec,
        )
        .expect("seed context");

        let scoring_ctx = ScoringContext::builder().cached_context_count(1).build();
        let options = ScoringOptions {
            apply_freshness: true,
            apply_signals: false,
            trend_topics: vec![],
        };

        // Comfortably above PARALLEL_SCORE_MIN_ITEMS so the threaded branch runs.
        let items: Vec<_> = (1..=200).map(item).collect();
        let (score_data, scored_ids) = score_chunk(
            &items,
            &scoring_ctx,
            &db,
            &options,
            Some(signal_classifier()),
        );

        assert_eq!(
            scored_ids.len(),
            200,
            "every item must be stamped exactly once"
        );
        let unique: std::collections::HashSet<_> = scored_ids.iter().collect();
        assert_eq!(
            unique.len(),
            200,
            "no id duplicated or dropped across threads"
        );
        assert!(
            score_data.len() <= scored_ids.len(),
            "scored (>0) is a subset of all stamped ids"
        );
        // Every persisted score corresponds to a real item id.
        let id_set: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
        for (id, score, _, _) in &score_data {
            assert!(id_set.contains(id), "score for a phantom id");
            assert!(*score > 0.0, "persisted scores are strictly positive");
        }
    }

    /// The small-batch branch (< PARALLEL_SCORE_MIN_ITEMS) takes the sequential
    /// path and must produce the same complete coverage.
    #[test]
    fn score_chunk_small_batch_sequential_covers_every_item() {
        let db = crate::test_utils::test_db();
        let scoring_ctx = ScoringContext::builder().build();
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };
        let items: Vec<_> = (1..=10).map(item).collect();
        let (_score_data, scored_ids) = score_chunk(
            &items,
            &scoring_ctx,
            &db,
            &options,
            Some(signal_classifier()),
        );
        assert_eq!(scored_ids.len(), 10, "sequential path stamps every item");
    }
}

/// Re-score one chunk of items stamped at an OLDER `PIPELINE_VERSION`, persist the
/// new scores, and re-stamp at the current version. This is the bulk-drain twin of
/// [`backfill_unscored_cycle`]: that one drains NEVER-scored items (`version = 0`),
/// this one drains the stale-VERSION backlog (`0 < version < current`) that a
/// scoring-logic change creates.
///
/// Same cheap, LLM-free PASIFA pipeline as the live `merge_stale_drain_batch` path —
/// it just isn't throttled to 500/analysis-run, so a `--engine-drain` loop converges
/// the whole corpus in minutes instead of the ~2.8 days the 500-per-30-min scheduler
/// trickle takes. Convergent + resumable: progress is the version stamp in the DB.
pub(crate) async fn drain_stale_scores_cycle(chunk_size: usize) -> Result<BackfillProgress> {
    let db = get_database()?;

    // Scoped-epoch promotion first: items the current version's registered
    // predicate proves unaffected are re-stamped without re-scoring, so the
    // drain below only works the slice the change could actually touch.
    // ~0ms no-op when the registry is empty or the corpus is current;
    // fail-open (a promotion error just means a full drain).
    scoring::epochs::promote_unaffected_stale_logged(db);

    let items = db
        .get_stale_scored_items(scoring::PIPELINE_VERSION, chunk_size)
        .map_err(|e| format!("Failed to load stale-version backlog: {e}"))?;
    if items.is_empty() {
        return Ok(BackfillProgress {
            scored_this_cycle: 0,
            relevant_this_cycle: 0,
            remaining_unscored: 0,
            done: true,
        });
    }

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

    let (score_data, scored_ids) =
        score_chunk(&items, &ctx, db, &options, Some(signal_classifier()));

    let relevant_this_cycle = score_data.len();
    if !score_data.is_empty() {
        if let Err(e) = db.persist_analysis_scores(&score_data, "drain") {
            warn!(target: "4da::backfill", error = %e, "Failed to persist re-scored values");
        }
    }
    // Stamp EVERY re-scored item (including any that fell to noise) so it leaves the
    // stale pool and the drain converges — same invariant as the analysis path.
    if let Err(e) = db.mark_items_scored_version(&scored_ids, scoring::PIPELINE_VERSION) {
        warn!(target: "4da::backfill", error = %e, "Failed to stamp re-scored pipeline version");
    }

    // Remaining stale items are those still below the current version. Reuse the
    // same query with a probe-sized window to learn whether more work remains.
    let remaining = db
        .get_stale_scored_items(scoring::PIPELINE_VERSION, 1)
        .map(|v| v.len() as i64)
        .unwrap_or(0);
    info!(
        target: "4da::backfill",
        rescored = scored_ids.len(),
        relevant = relevant_this_cycle,
        more_remaining = remaining > 0,
        "Stale-version drain cycle complete"
    );

    Ok(BackfillProgress {
        scored_this_cycle: scored_ids.len(),
        relevant_this_cycle,
        remaining_unscored: remaining,
        done: remaining == 0,
    })
}

/// Drain stale SCORES and stale VERDICTS together, one bounded chunk of each.
///
/// The bulk `--engine-drain` entry point. Verdicts run FIRST and unconditionally:
/// once the score drain finishes, every item is score-current and the score pass
/// returns `done` immediately — so a reconciliation placed after that point would
/// never run, in exactly the state (scores converged, verdicts not) that
/// motivates it.
pub(crate) async fn drain_stale_version_cycle(chunk_size: usize) -> Result<BackfillProgress> {
    let verdicts = reconcile_stale_verdicts_logged().await;
    let mut progress = drain_stale_scores_cycle(chunk_size).await?;
    // Not done until BOTH backlogs are converged, so a bulk drain keeps cycling.
    progress.done = progress.done && verdicts.remaining == 0;
    Ok(progress)
}

/// What one budgeted in-cycle drain actually achieved.
///
/// `converted` is the number that matters and the number that used to be
/// missing: the old in-cycle drain logged `stale=500` (what it *merged*) every
/// eleven minutes for days while converting five items a cycle. A repair loop
/// that reports only its attempts is indistinguishable from one that works —
/// the same failure class as the 90-day re-embed outage in `.ai/FAILURE_MODES.md`.
#[derive(Debug, Clone, Default)]
pub(crate) struct DrainOutcome {
    pub rescored: usize,
    pub converted: i64,
    pub remaining: i64,
}

/// Wall-clock budget one background cycle may spend draining stale scores.
///
/// A budget rather than an item count, because the per-item cost is not a
/// constant the caller can know: it is ~55 ms today and drops by ~96% once the
/// context-match cache is warm. Fifteen seconds converges whatever the machine
/// can converge in fifteen seconds, on any host, at any per-item cost — and the
/// drain-to-completion trigger handles anything larger.
pub(crate) const CYCLE_DRAIN_BUDGET: std::time::Duration = std::time::Duration::from_secs(15);

/// Rows per chunk inside the budgeted drain. Small enough that the budget is
/// honoured with reasonable granularity, large enough to clear
/// `PARALLEL_SCORE_MIN_ITEMS` so every chunk runs threaded.
const CYCLE_DRAIN_CHUNK: usize = 500;

/// Wall-clock budget for warming the context-match cache each background cycle.
///
/// Runs BEFORE the drain and before the cycle's own scoring, because
/// `score_item` only READS that cache: against a cold one every item pays the
/// full 52 ms KNN, and against a warm one it pays ~2.7 ms. Warming first is the
/// difference between a 22-minute corpus re-score and a one-minute one.
pub(crate) const CYCLE_CACHE_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// Backlog above which a per-cycle budget cannot converge in reasonable time,
/// so the engine drains to COMPLETION instead of trickling.
///
/// This is the trigger that was missing entirely. `--engine-drain` has existed
/// since PIPELINE_VERSION 7 and converts 100% of what it scores, but nothing in
/// the app, the scheduler, the updater or the migration path ever called it — it
/// was reachable only by a human typing a flag, which no shipped user can do.
/// The measured consequence: 46,997 stale items draining at 88/hour, 22 days,
/// against the 22 minutes the same machine took once the flag was typed.
///
/// 5,000 rather than 1: a handful of stale rows is ordinary churn that the
/// budgeted drain absorbs inside one cycle. A five-figure backlog is a version
/// bump, and a version bump means the user is looking at a feed ranked by two
/// different brains until it converges.
pub(crate) const DRAIN_TO_COMPLETION_THRESHOLD: i64 = 5_000;

/// Chunk size for the run-to-completion drain. Matches `--engine-drain`.
const BULK_DRAIN_CHUNK: usize = 2_000;

/// Runaway guard: 1M items at `BULK_DRAIN_CHUNK`, far above any real corpus.
const BULK_DRAIN_MAX_CYCLES: usize = 500;

/// Warm the per-item context-match cache, then drain the stale-score backlog
/// with the policy the backlog size calls for.
///
/// Small backlog -> the wall-clock-budgeted drain, so a background cycle stays a
/// background cycle. Large backlog -> run to completion, because that state means
/// a scoring change has landed and every surface is currently ranking items
/// judged by two different pipeline versions against each other. Only
/// `blind_spots` filters reads on `scored_pipeline_version`; the Signal feed, the
/// content graph and the briefings do not. Converging fast is an accuracy
/// requirement, not a tidiness one.
pub(crate) async fn maintain_scoring_epoch() -> DrainOutcome {
    let Ok(db) = get_database() else {
        return DrainOutcome::default();
    };
    crate::scoring::context_cache::refresh_context_cache(db, CYCLE_CACHE_BUDGET);

    let pending = db
        .count_stale_scored_items(scoring::PIPELINE_VERSION)
        .unwrap_or(0);
    if pending <= DRAIN_TO_COMPLETION_THRESHOLD {
        return drain_stale_scores_budgeted(CYCLE_DRAIN_BUDGET).await;
    }

    info!(
        target: "4da::backfill",
        pending,
        threshold = DRAIN_TO_COMPLETION_THRESHOLD,
        "Stale-score backlog past the trickle threshold — draining to completion"
    );
    let started = std::time::Instant::now();
    let mut rescored = 0usize;
    for cycle in 0..BULK_DRAIN_MAX_CYCLES {
        // Re-warm periodically: a long drain outlives one cache pass on a cold
        // corpus, and every warmed item makes the rest of the drain cheaper.
        if cycle > 0 && cycle % 10 == 0 {
            crate::scoring::context_cache::refresh_context_cache(db, CYCLE_CACHE_BUDGET);
        }
        match drain_stale_scores_cycle(BULK_DRAIN_CHUNK).await {
            Ok(p) => {
                rescored += p.scored_this_cycle;
                if p.done || p.scored_this_cycle == 0 {
                    break;
                }
            }
            Err(e) => {
                warn!(target: "4da::backfill", error = %e, rescored, "Bulk drain cycle failed");
                break;
            }
        }
    }
    let remaining = db
        .count_stale_scored_items(scoring::PIPELINE_VERSION)
        .unwrap_or(0);
    let outcome = DrainOutcome {
        rescored,
        converted: (pending - remaining).max(0),
        remaining,
    };
    info!(
        target: "4da::backfill",
        rescored = outcome.rescored,
        converted = outcome.converted,
        remaining = outcome.remaining,
        elapsed_ms = started.elapsed().as_millis(),
        "Drain-to-completion finished"
    );
    outcome
}

/// Drain the stale-score backlog beside an analysis cycle, bounded by `budget`.
///
/// ## Why this is not merged into the cycle's batch
///
/// It used to be: `merge_stale_drain_batch` appended 500 stale items to the
/// items the cycle was about to score. They were scored — and then cross-source
/// dedup, fuzzy-title dedup, topic dedup and temporal clustering DELETED 831 of
/// the 1,458 results before the version stamp was written, and the stale items
/// lost that contest systematically because they are older and lower-scoring
/// than the fresh window they were merged into. Measured 2026-08-27 by capturing
/// the exact 500 ids the drain query returned and re-checking after persist:
/// **495 were still stale**. Net corpus drain 88 items/hour, 22 days remaining,
/// 1.1% of the compute doing useful work.
///
/// Draining beside the cycle instead of inside it means the drain never enters
/// the display pipeline, never pays for the cross-encoder / diversity passes /
/// LLM rerank — none of which change the value it is trying to write — and
/// stamps 100% of what it scores.
pub(crate) async fn drain_stale_scores_budgeted(budget: std::time::Duration) -> DrainOutcome {
    let Ok(db) = get_database() else {
        return DrainOutcome::default();
    };
    let before = db
        .count_stale_scored_items(scoring::PIPELINE_VERSION)
        .unwrap_or(0);
    if before == 0 {
        return DrainOutcome::default();
    }

    let started = std::time::Instant::now();
    let mut rescored = 0usize;
    let mut done = false;
    while started.elapsed() < budget {
        match drain_stale_scores_cycle(CYCLE_DRAIN_CHUNK).await {
            Ok(p) => {
                rescored += p.scored_this_cycle;
                if p.done || p.scored_this_cycle == 0 {
                    done = true;
                    break;
                }
            }
            Err(e) => {
                warn!(target: "4da::backfill", error = %e, "In-cycle drain chunk failed — next cycle retries");
                break;
            }
        }
    }

    let remaining = db
        .count_stale_scored_items(scoring::PIPELINE_VERSION)
        .unwrap_or(before);
    let outcome = DrainOutcome {
        rescored,
        converted: (before - remaining).max(0),
        remaining,
    };
    warn_if_split_across_epochs(db, remaining);
    info!(
        target: "4da::backfill",
        rescored = outcome.rescored,
        converted = outcome.converted,
        remaining = outcome.remaining,
        done,
        elapsed_ms = started.elapsed().as_millis(),
        "In-cycle stale-score drain"
    );
    // CONVERSION, not attempts: rescoring without converting is the treadmill
    // this function exists to end, so say so loudly if it ever comes back.
    if outcome.rescored > 0 && outcome.converted == 0 {
        warn!(
            target: "4da::backfill",
            rescored = outcome.rescored,
            "Drain re-scored items but converted NONE — stale set is not shrinking"
        );
    }
    outcome
}

/// Say out loud when the corpus is being ranked by two brains at once.
///
/// Only `blind_spots` filters reads on `scored_pipeline_version`. The Signal
/// feed, the content graph, the briefings and the MCP surface all order by
/// `relevance_score` without asking which pipeline version produced it — so
/// while a drain is outstanding, items judged by the superseded brain are
/// ranked directly against items judged by the current one.
///
/// That is not a cosmetic backlog. On 2026-08-27 it ran for days: v25's own
/// commit message describes the brain that had scored 89% of the corpus as
/// re-admitting every dependency the git-recency scope filter existed to
/// exclude. Principle 5 is "never show intelligence the system can't stand
/// behind"; a long drain is that principle failing quietly, so it gets a log
/// line that names the consequence rather than a number that names the backlog.
fn warn_if_split_across_epochs(db: &crate::db::Database, remaining: i64) {
    if remaining <= 0 {
        return;
    }
    let total = db.count_embedded_source_items().unwrap_or(0);
    if total <= 0 {
        return;
    }
    let pct = remaining as f64 * 100.0 / total as f64;
    // Below a few percent this is ordinary churn at the edge of the window,
    // not a split corpus.
    if pct < 5.0 {
        return;
    }
    warn!(
        target: "4da::backfill",
        stale = remaining,
        corpus = total,
        pct = format!("{pct:.1}"),
        version = scoring::PIPELINE_VERSION,
        "Corpus split across scoring epochs — surfaces are ranking items judged by two pipeline versions against each other until this converges"
    );
}

/// Outcome of one verdict-reconciliation batch.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct VerdictReconciliation {
    /// Curated items the current pipeline rejects — un-curated this batch.
    pub demoted: usize,
    /// Curated items the current pipeline still accepts — stamped, flag kept.
    pub confirmed: usize,
    /// Stale, score-derived verdicts still outstanding after this batch.
    pub remaining: i64,
    /// In-version verdicts demoted because their live score sank below the
    /// demote line (`threshold − SCORE_SUNK_EPSILON`) — the 2026-08-23 audit's
    /// "immortal within a version" class. Reason: `score_sunk_in_version`.
    pub sunk_demoted: usize,
}

impl VerdictReconciliation {
    /// Whether this batch touched anything — used to report the zero case when
    /// there WAS work to do (see the log at the end of the cycle).
    fn is_empty(&self) -> bool {
        self.demoted == 0 && self.confirmed == 0
    }
}

/// Verdicts to re-judge per cycle. The working set is bounded by the CURATED
/// corpus (426 items live 2026-07-26), not the ~200k-item corpus, so this
/// clears a full post-bump backlog in a single cycle while still capping the
/// transaction if the curated set ever grows.
const VERDICT_RECONCILE_BUDGET: usize = 500;

/// Re-judge curated items whose verdict a superseded `PIPELINE_VERSION`
/// decided, and DEMOTE the ones the current pipeline rejects.
///
/// This is the `feed_relevant` twin of the stale-score drain, and it is a
/// separate pass for a reason that is easy to miss: an item's SCORE and its
/// VERDICT go stale independently. The drain selects on
/// `scored_pipeline_version`, so once it finishes, every item is score-current
/// and therefore INVISIBLE to the drain — while its verdict may still be
/// whatever a superseded brain decided. That is not hypothetical; it is the
/// live state this pass was written for (2026-07-26: corpus 100% v18, yet 399
/// of 426 curated items held a pre-v18 verdict, 181 of them scoring below the
/// relevance threshold under v18).
///
/// ## Demote-only, and why
///
/// A `false` verdict demotes; a `true` verdict only stamps. A `0` verdict is
/// NEVER promoted to `1`, because promotion is not a per-item decision — the
/// real curation run applies dedup, diversity, reranking and brief-rejection
/// across the whole batch, context this pass does not have. Demotion needs no
/// such context: if the current pipeline rejects an item outright, no
/// batch-level stage was going to rescue it. So the pass can only ever REMOVE
/// something the current brain disowns, never inject something it never chose —
/// which is what makes it safe to run unattended on every user's machine.
///
/// Items whose verdict came from an anti-bubble injection are excluded at the
/// query (`feed_verdict_source = 'serendipity'`): the current pipeline
/// rejecting a serendipity pick is that feature working, not staleness.
///
/// Scores are deliberately NOT re-persisted. The stale thing is the verdict;
/// re-writing `relevance_score` from here would silently re-rank every surface
/// as a side effect of a curation repair, and the drain already owns the score.
///
/// Scoring options match the live cycle exactly (`scoring/analyzer.rs`:
/// freshness + signals on, `detect_trend_topics` over the batch). One honest
/// asymmetry remains: trend topics are detected over THIS batch, not the
/// cycle's, so a trend boost can differ at the margin. It is self-correcting in
/// the safe direction — the only failure it can cause is demoting something the
/// full cycle would have kept, and the cycle re-promotes that item the next time
/// it selects it. Oscillation is impossible within a version because a verdict
/// this pass stamps is no longer stale, so it is never re-judged here.
///
/// Consumers are deliberately left unchanged — no surface filters on
/// `feed_verdict_version`. Making the content graph exclude stale verdicts
/// would empty it after every bump (94% of live nodes, measured) until this
/// pass caught up, violating the cold-start doctrine. This pass converging IS
/// the fix.
///
/// The pass also runs the IN-VERSION sunk sweep
/// (`Database::demote_sunk_verdicts`) on the same cadence: a score-sourced
/// verdict whose live score churned clearly below the admission line within
/// the current version is demoted with reason `score_sunk_in_version` — the
/// version-scoped working set alone left that class immortal (2026-08-23
/// audit).
pub(crate) async fn reconcile_stale_verdicts_cycle(budget: usize) -> Result<VerdictReconciliation> {
    let db = get_database()?;

    // In-version sunk sweep FIRST, and before the stale early-return below:
    // once every verdict is version-current the stale probe reads 0 forever,
    // which is precisely the state in which same-version score churn is the
    // ONLY remaining decay path (2026-08-23 audit: 106 of 532 feed members
    // below 0.45 with a score-sourced, current-version verdict). Pure SQL over
    // the curated set — no scoring context, so the idle path stays ~0. Demote
    // line is threshold − epsilon: the ~300-item jitter band the audit
    // measured across 0.37–0.43 must not thrash (see SCORE_SUNK_EPSILON).
    let sunk = db
        .demote_sunk_verdicts(
            scoring::PIPELINE_VERSION,
            crate::get_relevance_threshold() - crate::db::SCORE_SUNK_EPSILON,
        )
        .map_err(|e| format!("Failed to demote sunk in-version verdicts: {e}"))?;
    if sunk > 0 {
        info!(
            target: "4da::verdicts",
            demoted = sunk,
            version = scoring::PIPELINE_VERSION,
            "In-version sweep: curated items whose live score sank below the demote line un-curated"
        );
    }

    // Cheap indexed probe next: this runs on EVERY analysis cycle forever, so
    // the idle path must cost ~0 and must not build a scoring context.
    let stale = db
        .count_stale_verdicts(scoring::PIPELINE_VERSION)
        .map_err(|e| format!("Failed to probe stale verdicts: {e}"))?;
    if stale == 0 {
        return Ok(VerdictReconciliation {
            sunk_demoted: sunk,
            ..VerdictReconciliation::default()
        });
    }

    let items = db
        .get_stale_verdict_items(scoring::PIPELINE_VERSION, budget)
        .map_err(|e| format!("Failed to load stale-verdict items: {e}"))?;
    if items.is_empty() {
        return Ok(VerdictReconciliation {
            sunk_demoted: sunk,
            ..VerdictReconciliation::default()
        });
    }

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
    let classifier = signal_classifier();

    // Sequential by design. The batch is bounded by the curated set (hundreds),
    // not the corpus, so the thread-scope machinery the drain needs for its
    // 2000-item chunks would buy nothing measurable here.
    let mut demote: Vec<i64> = Vec::new();
    let mut confirm: Vec<i64> = Vec::new();
    for item in &items {
        // Path parity with the analyzer path: parse topic tags (§3.5).
        let parsed_tags = scoring::parse_tags_topics(item.tags.as_deref());
        let r = scoring::score_item(
            &ScoringInput {
                id: item.id as u64,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: &item.embedding,
                created_at: Some(item.published_at.as_ref().unwrap_or(&item.created_at)),
                detected_lang: &item.detected_lang,
                source_tags: &parsed_tags,
                tags_json: item.tags.as_deref(),
                feed_origin: item.feed_origin.as_deref(),
                source_id: Some(&item.source_id),
            },
            &ctx,
            db,
            &options,
            Some(classifier),
        );
        if r.relevant {
            confirm.push(item.id);
        } else {
            demote.push(item.id);
        }
    }

    let outcome = VerdictReconciliation {
        demoted: demote.len(),
        confirmed: confirm.len(),
        remaining: (stale - items.len() as i64).max(0),
        sunk_demoted: sunk,
    };
    if let Err(e) = db.reconcile_feed_verdicts(&demote, &confirm, scoring::PIPELINE_VERSION) {
        // Unlike epoch promotion (where failure just means a slower drain), a
        // failed write here leaves the stale verdicts standing — so this must
        // surface as an error, never be logged as a completed reconciliation.
        return Err(format!("Failed to persist verdict reconciliation: {e}").into());
    }

    // Report the ZERO case. A repair loop that only logs its successes is
    // indistinguishable from an idle one — that exact gate hid a 90-day
    // re-embed outage (see `.ai/FAILURE_MODES.md`). Reaching here means there
    // WAS stale work, so "nothing applied" is a defect signal, not silence.
    if outcome.is_empty() {
        warn!(
            target: "4da::verdicts",
            stale,
            loaded = items.len(),
            "Verdict reconciliation applied nothing despite a stale backlog"
        );
    } else {
        info!(
            target: "4da::verdicts",
            demoted = outcome.demoted,
            confirmed = outcome.confirmed,
            remaining = outcome.remaining,
            version = scoring::PIPELINE_VERSION,
            "Verdict reconciliation: stale curation verdicts re-judged"
        );
    }
    Ok(outcome)
}

/// Log-and-continue wrapper for cycle call sites: a reconciliation failure must
/// never fail the analysis cycle that owns the user's results.
pub(crate) async fn reconcile_stale_verdicts_logged() -> VerdictReconciliation {
    match reconcile_stale_verdicts_cycle(VERDICT_RECONCILE_BUDGET).await {
        Ok(outcome) => outcome,
        Err(e) => {
            warn!(
                target: "4da::verdicts",
                error = %e,
                "Verdict reconciliation failed — stale verdicts remain until the next cycle"
            );
            VerdictReconciliation::default()
        }
    }
}
