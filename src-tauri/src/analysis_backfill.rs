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

/// Score a batch of items through the cheap PASIFA pipeline, in parallel across
/// up to `READ_POOL_SIZE` OS threads. Both backfill (never-scored) and the
/// stale-version drain run this identical per-item loop; extracting it keeps
/// them in lock-step and lets the drain use every core.
///
/// SAFETY / correctness: `score_item` is a pure function of the item plus
/// read-only `ctx`/`db` access — it performs no DB writes and mutates no shared
/// state, so scoring items concurrently changes only wall-clock, never the
/// result. Each thread borrows its OWN pooled read connection for the per-item
/// KNN (`read_conn` hands out a distinct reader via non-blocking try-lock), so
/// threads don't serialize on a single reader; the cap at `READ_POOL_SIZE`
/// keeps thread count matched to available readers (extras would fall back to
/// the writer lock and serialize). Results are keyed by item id and merged
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
                source_tags: &[],
                tags_json: item.tags.as_deref(),
                feed_origin: item.feed_origin.as_deref(),
                source_id: Some(&item.source_id),
            },
            ctx,
            db,
            options,
            classifier,
        );
        // persist_analysis_scores only writes top_score > 0; the id is always
        // returned so mark_items_scored_version stamps even re-scored noise.
        let scored = (r.top_score > 0.0).then(|| {
            (
                item.id,
                r.top_score,
                r.signal_type.clone(),
                r.signal_priority.clone(),
            )
        });
        (scored, item.id)
    };

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .saturating_sub(1) // leave a core for the foreground app / OS
        .clamp(1, crate::db::READ_POOL_SIZE);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::ScoringContext;

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
pub(crate) async fn drain_stale_version_cycle(chunk_size: usize) -> Result<BackfillProgress> {
    let db = get_database()?;

    // Scoped-epoch promotion first: items the current version's registered
    // predicate proves unaffected are re-stamped without re-scoring, so the
    // drain below only works the slice the change could actually touch.
    // ~0ms no-op when the registry is empty or the corpus is current;
    // fail-open (a promotion error just means a full drain).
    scoring::epochs::promote_unaffected_stale_logged(db);

    // Stale VERDICTS are a separate backlog from stale SCORES and must drain
    // here too, BEFORE the early return below: once the score drain finishes,
    // every item is score-current and this function returns `done` immediately
    // — so a reconciliation placed after that point would never run, in exactly
    // the state (scores converged, verdicts not) that motivates it.
    let verdicts = reconcile_stale_verdicts_logged().await;

    let items = db
        .get_stale_scored_items(scoring::PIPELINE_VERSION, chunk_size)
        .map_err(|e| format!("Failed to load stale-version backlog: {e}"))?;
    if items.is_empty() {
        return Ok(BackfillProgress {
            scored_this_cycle: 0,
            relevant_this_cycle: 0,
            remaining_unscored: 0,
            // Not done until the verdict backlog is converged too, so a bulk
            // `--engine-drain` keeps cycling until BOTH epochs are current.
            done: verdicts.remaining == 0,
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
        if let Err(e) = db.persist_analysis_scores(&score_data) {
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

/// Outcome of one verdict-reconciliation batch.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct VerdictReconciliation {
    /// Curated items the current pipeline rejects — un-curated this batch.
    pub demoted: usize,
    /// Curated items the current pipeline still accepts — stamped, flag kept.
    pub confirmed: usize,
    /// Stale, score-derived verdicts still outstanding after this batch.
    pub remaining: i64,
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
pub(crate) async fn reconcile_stale_verdicts_cycle(budget: usize) -> Result<VerdictReconciliation> {
    let db = get_database()?;

    // Cheap indexed probe first: this runs on EVERY analysis cycle forever, so
    // the idle path must cost ~0 and must not build a scoring context.
    let stale = db
        .count_stale_verdicts(scoring::PIPELINE_VERSION)
        .map_err(|e| format!("Failed to probe stale verdicts: {e}"))?;
    if stale == 0 {
        return Ok(VerdictReconciliation::default());
    }

    let items = db
        .get_stale_verdict_items(scoring::PIPELINE_VERSION, budget)
        .map_err(|e| format!("Failed to load stale-verdict items: {e}"))?;
    if items.is_empty() {
        return Ok(VerdictReconciliation::default());
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
                source_tags: &[],
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
