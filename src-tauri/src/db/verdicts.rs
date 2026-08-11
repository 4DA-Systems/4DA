// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Curation-verdict persistence and its epoch guard.
//!
//! `feed_relevant` is the verdict that decides what the user actually SEES —
//! the content graph and the MCP curation guard both select on it. Phase 95
//! made it durable; it shipped WITHOUT the epoch machinery that
//! `relevance_score` has (`scored_pipeline_version` + a drain + scoped
//! epochs), so every `PIPELINE_VERSION` bump silently invalidated the whole
//! curated corpus and nothing converged it back.
//!
//! Measured live 2026-07-26 on the 202,539-item corpus, AFTER the v18 drain
//! had re-scored the corpus to 100% current: **399 of 426 curated items still
//! held a verdict decided by a pre-v18 brain, and 181 of those scored below
//! the relevance threshold under v18** — 156 of them `crates_io` look-alike
//! releases, precisely the class v18 declared *categorically* never
//! feed-relevant (#375). Scores converged; verdicts did not. They cannot: the
//! analysis cycle re-verdicts only what `get_items_tiered(168, …)` selects, so
//! an item that ages out of the 7-day window has its verdict frozen forever
//! (the frozen set is visible in the data as a hard floor at the timestamp of
//! the last corpus-wide pass).
//!
//! Phase 101 adds the two columns this needs:
//!
//! - `feed_verdict_version` — the `PIPELINE_VERSION` that decided the verdict.
//!   NULL = unstamped, i.e. written before this module existed: unknown, treated
//!   as stale. No backfill — honest by construction.
//! - `feed_verdict_source` — WHERE the verdict came from, because not every
//!   verdict is score-derived (see [`VerdictSource`]). Without it, the
//!   reconciliation pass cannot tell a stale verdict from a deliberate
//!   anti-bubble pick, and would silently delete the serendipity feature.
//!
//! ## What the stamp means
//!
//! `feed_verdict_version = N` means **"the pipeline at version N did not reject
//! this item"** — NOT "a full curation run re-selected it". That is exactly the
//! property consumers need (never show something the current brain would
//! reject) and is all a per-item pass can honestly claim.

use rusqlite::{params, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};

/// Provenance of a persisted `feed_relevant` verdict.
///
/// The reconciliation pass re-runs the scorer and demotes verdicts the current
/// pipeline rejects. That is only sound for verdicts the scorer PRODUCED. Two
/// live paths set `relevant = true` without asking `score_item`, both of them
/// deliberate anti-bubble features:
///
/// 1. `scoring::dedup::compute_serendipity_candidates` — takes items
///    `score_item` scored but marked `relevant = false`, and flips the flag so
///    they surface (budget: 1–5 per cycle, `serendipity.budget_percent`).
/// 2. `analysis_deep_scan`'s concept-graph injection — constructs a
///    `SourceRelevance` outright for a 2–3-hop conceptual neighbour, never
///    scoring it at all.
///
/// Both set `SourceRelevance::serendipity`, so provenance is read from that
/// flag rather than inferred: an inferred signature (e.g. "`top_score` is
/// exactly 0.45") only ever caught path 2 and silently mis-classified every
/// pick from path 1, which keeps its original score and is therefore
/// indistinguishable from a stale verdict by score alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictSource {
    /// The scoring pipeline decided this verdict. Reconcilable: re-running the
    /// scorer is a like-for-like comparison.
    Score,
    /// An anti-bubble injection decided it, bypassing or overriding the scorer.
    /// **Not demoted by reconciliation while fresh** — the current pipeline
    /// rejecting it is the normal case, not evidence of staleness. It DOES
    /// expire: after `SERENDIPITY_VERDICT_TTL_DAYS` the verdict re-enters the
    /// reconciliation working set, so anti-bubble picks rotate instead of
    /// squatting in the curated set forever (measured live 2026-08-11:
    /// immune-forever picks had accumulated to 17.6% of the curated feed).
    Serendipity,
}

impl VerdictSource {
    /// Stored form. Kept short and lowercase — it is a persisted enum, so the
    /// strings are part of the schema contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Score => "score",
            Self::Serendipity => "serendipity",
        }
    }

    /// Provenance of a verdict as an analysis cycle produced it.
    pub fn from_serendipity(serendipity: bool) -> Self {
        if serendipity {
            Self::Serendipity
        } else {
            Self::Score
        }
    }
}

/// SQL fragment selecting curated items whose verdict is stale AND
/// score-derived, i.e. the reconciliation pass's exact working set.
///
/// - `feed_relevant = 1` — only the curated set. A `0` verdict is never
///   promoted here (that needs a full run's dedup/diversity/rerank context)
///   and `NULL` means *never judged*, which is not the same as "rejected".
/// - `COALESCE(feed_verdict_version, 0) < ?` — NULL (pre-Phase-101) counts as
///   stale, which is the honest reading: provenance was never recorded.
/// - `COALESCE(feed_verdict_source, 'score') = 'score'` — NULL legacy rows are
///   treated as score-derived. Their provenance is genuinely unrecoverable
///   (nothing persisted it before Phase 101), and the alternative — skipping
///   every unstamped row — would mean the pre-existing stale set NEVER
///   converges, which is the entire defect. The cost is bounded and
///   self-healing: a legacy serendipity pick is un-curated, not deleted, and
///   the engine re-injects on the very next cycle. Every verdict written from
///   Phase 101 onward carries exact provenance, so this fallback applies once.
const STALE_VERDICT_WHERE: &str = "feed_relevant = 1
       AND (
            (COALESCE(feed_verdict_version, 0) < ?1
             AND COALESCE(feed_verdict_source, 'score') = 'score')
         OR (feed_verdict_source = 'serendipity'
             AND COALESCE(feed_verdict_at, '1970-01-01') <= datetime('now', '-14 days'))
       )";

/// How long an anti-bubble (serendipity) verdict stays immune to
/// reconciliation. After this window it re-enters the working set and is
/// re-judged like any score verdict — the scorer rejecting it then demotes
/// it, and the next cycle's injection rotates a FRESH pick in. Mirrors the
/// `-14 days` literal inside `STALE_VERDICT_WHERE` (SQL cannot interpolate
/// a const; the TTL regression test pins the two together).
pub const SERENDIPITY_VERDICT_TTL_DAYS: u32 = 14;

impl Database {
    /// Persist the per-run feed curation VERDICT (Phase 95, W4-5 corpus
    /// parity), stamped with the pipeline version and provenance that produced
    /// it (Phase 101). That verdict is the curated corpus, and it used to
    /// evaporate with the run. Persisting it lets every surface (the content
    /// graph first) select "what the current brain actually stands behind"
    /// instead of re-deriving a corpus from raw cross-epoch scores.
    ///
    /// What the verdict actually is: `analysis_status::persist_cycle_results`
    /// writes `SourceRelevance::relevant` DIRECTLY. Two things can shape that
    /// value before it lands here — `analysis_rerank` may demote it to false,
    /// and the two serendipity paths may set it true for an item `score_item`
    /// rejected or never scored. There is no dedup/diversity stage in this
    /// path. (Wording before 2026-07-26 claimed the verdict came "after dedup,
    /// diversity, reranking, and brief-rejection demotions"; an audit traced
    /// the code and found that overstated. Corrected because anyone reasoning
    /// about verdict staleness has to trust this comment.)
    pub fn persist_feed_verdicts(
        &self,
        verdicts: &[(i64, bool, VerdictSource)],
        version: i32,
    ) -> SqliteResult<usize> {
        if verdicts.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = ?1,
                     feed_verdict_at = datetime('now'),
                     feed_verdict_version = ?2,
                     feed_verdict_source = ?3
                 WHERE id = ?4",
            )?;
            for (id, relevant, source) in verdicts {
                stmt.execute(params![i64::from(*relevant), version, source.as_str(), id])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// How many curated items hold a stale, score-derived verdict.
    ///
    /// Deliberately a COUNT and not a `LIMIT 1` existence probe: the caller
    /// logs it, and a *rising* count across cycles is the signal that
    /// reconciliation is losing ground to ingest. Backed by the Phase-101
    /// partial index, so it reads only the curated set (hundreds of rows), not
    /// the corpus.
    pub fn count_stale_verdicts(&self, current_version: i32) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row(
            &format!("SELECT COUNT(*) FROM source_items WHERE {STALE_VERDICT_WHERE}"),
            params![current_version],
            |r| r.get(0),
        )
    }

    /// Load a bounded batch of curated items whose verdict a superseded
    /// pipeline version decided, for re-judging.
    ///
    /// Ordered newest-first: a stale verdict on a recent item is the one the
    /// user is most likely to be looking at right now (every visible surface is
    /// recency-bounded), so the visible window converges in the first batch.
    ///
    /// Unlike [`Database::get_stale_scored_items`] there is NO tier-based time
    /// clause. This set is bounded by the curated corpus (hundreds of items,
    /// not the ~200k corpus), and a Free user's gated history is a *display*
    /// bound — leaving a stale positive verdict standing outside that window
    /// would keep feeding the content graph an item the current brain rejects.
    pub fn get_stale_verdict_items(
        &self,
        current_version: i32,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT id, source_type, source_id, url, title, content, content_hash,
                    embedding, created_at, last_seen, COALESCE(detected_lang, 'en'),
                    feed_origin, tags, published_at
             FROM source_items
             WHERE {STALE_VERDICT_WHERE}
             ORDER BY COALESCE(published_at, created_at) DESC
             LIMIT ?2"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![current_version, limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(7)?;
            Ok(StoredSourceItem {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                content_hash: row.get(6)?,
                embedding: blob_to_embedding(&embedding_blob),
                created_at: parse_datetime(row.get::<_, String>(8)?),
                last_seen: parse_datetime(row.get::<_, String>(9)?),
                detected_lang: row
                    .get::<_, String>(10)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(11).ok().flatten(),
                tags: row.get(12).ok().flatten(),
                published_at: crate::db::parse_datetime_opt(
                    row.get::<_, Option<String>>(13).ok().flatten(),
                ),
            })
        })?;
        rows.collect()
    }

    /// Apply one reconciliation batch: `demote` loses its curated flag, `confirm`
    /// keeps it. Both get stamped at `version` so they leave the stale set
    /// either way — that is what makes the pass converge instead of re-picking
    /// the same items forever (the same invariant `mark_items_scored_version`
    /// exists to protect on the score side).
    ///
    /// One transaction, so a crash mid-batch cannot leave a verdict flipped
    /// without its stamp. `feed_verdict_at` is deliberately NOT touched: it
    /// records when a full analysis cycle last JUDGED the item, and a
    /// per-item reconciliation is not that (see the module note on what the
    /// stamp claims).
    pub fn reconcile_feed_verdicts(
        &self,
        demote: &[i64],
        confirm: &[i64],
        version: i32,
    ) -> SqliteResult<usize> {
        if demote.is_empty() && confirm.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        {
            let mut demote_stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = 0,
                     feed_verdict_version = ?1,
                     feed_verdict_source = 'score'
                 WHERE id = ?2 AND feed_relevant = 1",
            )?;
            for id in demote {
                count += demote_stmt.execute(params![version, id])?;
            }
            let mut confirm_stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_verdict_version = ?1,
                     feed_verdict_source = 'score'
                 WHERE id = ?2 AND feed_relevant = 1",
            )?;
            for id in confirm {
                count += confirm_stmt.execute(params![version, id])?;
            }
        }
        tx.commit()?;
        Ok(count)
    }
}

#[cfg(test)]
#[path = "verdicts_tests.rs"]
mod tests;
