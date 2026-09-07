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
use crate::types::SourceRelevance;

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
    /// v32: never-judged items whose current-version score clears the line —
    /// given their first verdict this batch.
    pub promoted: usize,
    /// v32: items a superseded pipeline rejected whose current-version score
    /// clears the line — deferred to the judge drain as an unreasoned flip.
    pub deferred_promotions: usize,
    /// v32: later copies of a story the feed already held, demoted
    /// `duplicate_curated` (both from the standing feed and among the risen).
    pub twin_demoted: usize,
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

/// Risen items promoted per cycle (v32). The post-v31 backlog was 915 rows
/// live; at this budget it clears in five cycles, and each deferred flip
/// still has to pass the judge drain, which adjudicates a handful per cycle —
/// the promotion lane must not outrun the second opinion it relies on.
const RISEN_PROMOTE_BUDGET: usize = 200;

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

    // v32: the lane in the OTHER direction. Every pass in this file could only
    // remove; the drain persists scores only; the cycle re-verdicts only what
    // it selects. So a row the current brain scores above the line with no
    // verdict — or a rejection a superseded brain wrote — stayed out of the
    // feed forever (915 such rows live after the v31 drain). Twins first, so a
    // story the feed already holds is not promoted a second time under a new
    // id; then the risen set, through the persist boundary (first verdicts
    // apply, flips against a standing rejection defer to the judge drain).
    let twins = db
        .demote_curated_twins(scoring::PIPELINE_VERSION)
        .map_err(|e| format!("Failed to demote curated twins: {e}"))?;
    // A twin verdict whose curated original has since left the feed is a
    // claim about nothing; withdraw it so the risen sweep below can judge the
    // row on its own score (TWIR 666 class, 2026-09-07).
    let orphaned = db
        .withdraw_orphaned_duplicate_verdicts()
        .map_err(|e| format!("Failed to withdraw orphaned duplicate verdicts: {e}"))?;
    if orphaned > 0 {
        info!(
            target: "4da::verdicts",
            withdrawn = orphaned,
            "Orphaned duplicate verdicts withdrawn — their curated twin is no longer in the feed"
        );
    }
    let risen = db
        .promote_risen_verdicts(
            scoring::PIPELINE_VERSION,
            crate::get_relevance_threshold(),
            RISEN_PROMOTE_BUDGET,
        )
        .map_err(|e| format!("Failed to promote risen verdicts: {e}"))?;
    if twins > 0 || risen.candidates > 0 {
        info!(
            target: "4da::verdicts",
            twins_demoted = twins + risen.twins,
            candidates = risen.candidates,
            promoted = risen.promoted,
            deferred = risen.deferred,
            version = scoring::PIPELINE_VERSION,
            "Risen sweep: current-version scores above the line re-enter the verdict lane"
        );
    }
    let base = VerdictReconciliation {
        sunk_demoted: sunk,
        promoted: risen.promoted,
        deferred_promotions: risen.deferred,
        twin_demoted: twins + risen.twins,
        ..VerdictReconciliation::default()
    };

    // Cheap indexed probe next: this runs on EVERY analysis cycle forever, so
    // the idle path must cost ~0 and must not build a scoring context.
    let stale = db
        .count_stale_verdicts(scoring::PIPELINE_VERSION)
        .map_err(|e| format!("Failed to probe stale verdicts: {e}"))?;
    if stale == 0 {
        return Ok(base);
    }

    let items = db
        .get_stale_verdict_items(scoring::PIPELINE_VERSION, budget)
        .map_err(|e| format!("Failed to load stale-verdict items: {e}"))?;
    if items.is_empty() {
        return Ok(base);
    }

    let ctx = scoring::build_scoring_context_with_timeout(db, "reconcile_verdicts").await?;
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
        ..base
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

/// `excluded_by` prefix a durable-verdict demotion writes on a display row.
/// Namespaced like `brief:` so it can be expired by this pass alone — user
/// and anti-topic exclusions are never touched.
const VERDICT_EXCLUSION_PREFIX: &str = "verdict:";

/// `excluded_by` prefix of a Brief rejection (`brief_rejections`). A Brief
/// demotion is display ORDER, not curation (AD-035): the row keeps
/// `relevant = true` and every list that reads `relevant` alone still shows
/// it. It therefore cannot shield a row from a durable rejection.
const BRIEF_EXCLUSION_PREFIX: &str = "brief:";

fn is_brief_exclusion(r: &SourceRelevance) -> bool {
    r.excluded
        && r.excluded_by
            .as_deref()
            .is_some_and(|e| e.starts_with(BRIEF_EXCLUSION_PREFIX))
}

/// Outcome of [`converge_display_on_durable_verdicts`].
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DisplayConvergence {
    /// Display rows the cycle called relevant that the durable verdict rejects.
    pub demoted: usize,
    /// Rows a previous pass demoted whose durable verdict no longer rejects them.
    pub restored: usize,
}

fn is_verdict_exclusion(r: &SourceRelevance) -> bool {
    r.excluded
        && r.excluded_by
            .as_deref()
            .is_some_and(|e| e.starts_with(VERDICT_EXCLUSION_PREFIX))
}

/// Converge the cycle's DISPLAY set on the durable verdict — demote-only.
///
/// `persist_cycle_results` writes the cycle's `relevant` flags through the
/// verdict persist boundary, and that boundary can decline them: an
/// unreasoned flip against a standing rejection is deferred, a cross-cycle
/// twin is written `duplicate_curated`, and the judge drain / reconciliation
/// passes rewrite verdicts between cycles. The in-memory results never heard
/// any of that, so every surface reading them — the Brief's review queue, the
/// free brief, the morning-briefing candidates, the header counts — showed
/// items the feed had already rejected (live 2026-09-07: "Rust has become a
/// spiritual experience", `llm_reject` at v32, second in the review queue at
/// 0.90). The Signal feed reads `feed_relevant` and was right.
///
/// Same doctrine as [`reconcile_stale_verdicts_cycle`]: a durable REJECTION
/// demotes the display row (`relevant = false`, `excluded_by = "verdict:…"`,
/// so `extract_near_misses` and the free brief skip it exactly as they skip a
/// brief rejection); a durable ACCEPT never promotes — promotion is the batch
/// decision the cycle already made — except to lift a demotion THIS pass
/// wrote once the durable verdict stops rejecting the row (a deferred flip
/// the next run confirmed, a judge promotion). Rows with no durable verdict
/// are left alone, and so are rows a user or anti-topic rule excluded (those
/// already carry `relevant = false`).
///
/// A Brief demotion does NOT shield a row: `brief:` keeps `relevant = true`
/// because it is an ordering verdict, and the Brief's own review queue reads
/// `relevant` (live 2026-09-07 11:2x: all five brief-demoted rows in memory
/// were durably `llm_reject`, and "2D Game Development … Rust edition" led
/// the queue at rank 0.93). A durable rejection outranks the Brief's ordering
/// and REPLACES the exclusion, so the brief-expiry pass cannot resurrect the
/// row and only this pass lifts it.
pub(crate) fn converge_display_on_durable_verdicts(
    db: &crate::db::Database,
    results: &mut [SourceRelevance],
) -> DisplayConvergence {
    let candidates: Vec<i64> = results
        .iter()
        .filter(|r| {
            (r.relevant && (!r.excluded || is_brief_exclusion(r))) || is_verdict_exclusion(r)
        })
        .map(|r| r.id as i64)
        .collect();
    if candidates.is_empty() {
        return DisplayConvergence::default();
    }
    let rejected = match db.durable_rejections(&candidates) {
        Ok(map) => map,
        Err(e) => {
            warn!(
                target: "4da::verdicts",
                error = %e,
                "Durable-verdict probe failed — display set left as the cycle scored it"
            );
            return DisplayConvergence::default();
        }
    };
    // `rejected` only ever holds candidate ids, so a hit is either a display
    // row to demote or a held row whose reason is refreshed.
    let mut outcome = DisplayConvergence::default();
    for r in results.iter_mut() {
        let held = is_verdict_exclusion(r);
        match rejected.get(&(r.id as i64)) {
            Some(reason) => {
                r.relevant = false;
                r.excluded = true;
                r.excluded_by = Some(format!(
                    "{VERDICT_EXCLUSION_PREFIX}{}",
                    reason.as_deref().unwrap_or("not_curated")
                ));
                if !held {
                    outcome.demoted += 1;
                }
            }
            None if held => {
                r.relevant = true;
                r.excluded = false;
                r.excluded_by = None;
                outcome.restored += 1;
            }
            None => {}
        }
    }
    outcome
}

#[cfg(test)]
mod display_convergence_tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    fn scored(id: i64, score: f32) -> SourceRelevance {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": format!("item {id}"),
            "url": null,
            "top_score": score,
            "matches": [],
            "relevant": true,
        }))
        .expect("minimal SourceRelevance deserializes")
    }

    fn set_verdict(db: &crate::db::Database, id: i64, relevant: i64, reason: Option<&str>) {
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET feed_relevant = ?1, feed_verdict_reason = ?2 WHERE id = ?3",
                rusqlite::params![relevant, reason, id],
            )
            .unwrap();
    }

    #[test]
    fn durable_rejection_demotes_the_display_row_and_nothing_else() {
        let db = test_db();
        let rejected = insert_test_item(
            &db,
            "reddit",
            "dc1",
            "Rust has become a spiritual experience",
            "opinion",
        );
        let curated = insert_test_item(&db, "rss", "dc2", "Announcing Rust 1.98", "release");
        let unjudged = insert_test_item(&db, "hackernews", "dc3", "fresh item", "new");
        set_verdict(&db, rejected, 0, Some("llm_reject"));
        set_verdict(&db, curated, 1, None);
        let mut results = vec![
            scored(rejected, 0.90),
            scored(curated, 0.80),
            scored(unjudged, 0.70),
        ];

        let outcome = converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(
            outcome,
            DisplayConvergence {
                demoted: 1,
                restored: 0
            }
        );
        assert!(!results[0].relevant && results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("verdict:llm_reject")
        );
        assert!(
            results[1].relevant && !results[1].excluded,
            "a durable accept is left as the cycle scored it"
        );
        assert!(
            results[2].relevant && !results[2].excluded,
            "a never-judged row has nothing durable to converge on"
        );
        let near_misses = crate::types::extract_near_misses(&results).unwrap_or_default();
        assert!(
            near_misses.iter().all(|r| r.id != rejected as u64),
            "a judge rejection is not a near miss"
        );
    }

    #[test]
    fn unreasoned_rejection_reads_as_not_curated_and_lifts_when_the_verdict_flips() {
        let db = test_db();
        let id = insert_test_item(&db, "devto", "dc4", "deferred flip", "body");
        set_verdict(&db, id, 0, None);
        let mut results = vec![scored(id, 0.75)];

        let first = converge_display_on_durable_verdicts(&db, &mut results);
        assert_eq!(first.demoted, 1);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("verdict:not_curated")
        );

        // The next run confirms the flip: the durable verdict accepts the row,
        // so the demotion THIS pass wrote is lifted — and only this pass's.
        set_verdict(&db, id, 1, None);
        let second = converge_display_on_durable_verdicts(&db, &mut results);
        assert_eq!(
            second,
            DisplayConvergence {
                demoted: 0,
                restored: 1
            }
        );
        assert!(results[0].relevant && !results[0].excluded);
        assert!(results[0].excluded_by.is_none());
    }

    #[test]
    fn a_brief_demotion_does_not_shield_a_durable_rejection() {
        let db = test_db();
        let id = insert_test_item(
            &db,
            "reddit",
            "dc6",
            "2D Game Development: From Zero To Hero - Rust edition",
            "tutorial",
        );
        set_verdict(&db, id, 0, Some("llm_reject"));
        let mut results = vec![scored(id, 0.93)];
        // The Brief demoted it first: an ORDERING verdict that keeps
        // `relevant = true`, which is what the review queue reads.
        results[0].excluded = true;
        results[0].excluded_by = Some("brief:game dev tutorial, no stack relevance".to_string());

        let outcome = converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(
            outcome,
            DisplayConvergence {
                demoted: 1,
                restored: 0
            }
        );
        assert!(!results[0].relevant && results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("verdict:llm_reject"),
            "the durable rejection replaces the ordering exclusion so brief expiry cannot resurrect it"
        );
    }

    #[test]
    fn a_brief_demotion_without_a_durable_rejection_is_left_alone() {
        let db = test_db();
        let id = insert_test_item(&db, "devto", "dc7", "brief-demoted but curated", "body");
        set_verdict(&db, id, 1, None);
        let mut results = vec![scored(id, 0.62)];
        results[0].excluded = true;
        results[0].excluded_by = Some("brief:self-promotional release".to_string());

        let outcome = converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(outcome, DisplayConvergence::default());
        assert!(results[0].relevant && results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("brief:self-promotional release"),
            "the Brief's ordering verdict stands when the durable column agrees the row is curated"
        );
    }

    #[test]
    fn other_exclusions_are_never_touched() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "dc5", "user-excluded", "body");
        set_verdict(&db, id, 1, None);
        let mut results = vec![scored(id, 0.75)];
        results[0].relevant = false;
        results[0].excluded = true;
        results[0].excluded_by = Some("anti-topic:crypto".to_string());

        let outcome = converge_display_on_durable_verdicts(&db, &mut results);

        assert_eq!(outcome, DisplayConvergence::default());
        assert!(!results[0].relevant && results[0].excluded);
        assert_eq!(results[0].excluded_by.as_deref(), Some("anti-topic:crypto"));
    }
}
