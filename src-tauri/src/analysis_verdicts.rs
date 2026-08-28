// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Curation-verdict reconciliation — the `feed_relevant` twin of the stale-score
//! drain.
//!
//! An item's SCORE and its VERDICT go stale independently. The drain selects on
//! `scored_pipeline_version`, so once it finishes every item is score-current and
//! therefore invisible to it — while its verdict may still be whatever a
//! superseded brain decided. That is not hypothetical; it is the live state this
//! pass was written for (2026-07-26: corpus 100% v18, yet 399 of 426 curated
//! items held a pre-v18 verdict, 181 of them scoring below the threshold).
//!
//! Split out of `analysis_backfill.rs` on 2026-08-28: the drain arc pushed that
//! file to 998 of its 1,000-line ceiling, and two lines of headroom is a trap for
//! whoever edits it next. The two halves were already independent — one owns
//! `relevance_score`, this one owns `feed_relevant`.

use tracing::{info, warn};

use crate::analysis::signal_classifier;
use crate::error::Result;
use crate::get_database;
use crate::scoring::{self, ScoringInput, ScoringOptions};

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
