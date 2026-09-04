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
//!
//! ## The in-version hole (Phase 108)
//!
//! Version-scoping alone left a second immortality: WITHIN a version, a
//! score-sourced verdict never re-entered the working set no matter how far
//! same-version churn dragged the live score afterwards. Measured live
//! 2026-08-23 (adversarial scoring audit): 106 of 532 feed members sat below
//! 0.45 with `feed_verdict_source = 'score'`, some as low as 0.17 against a
//! 0.40 threshold. [`Database::demote_sunk_verdicts`] closes it with a pure-SQL,
//! demote-only sweep on the reconciliation cadence, and Phase 108's
//! `feed_verdict_reason` column records why each repaired verdict flipped
//! ([`VerdictReason`]).

use rusqlite::{params, OptionalExtension, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, Database, StoredSourceItem};

/// Parse the direction of a `feed_verdict_pending` marker (`"1@<rfc3339>"` /
/// `"0@<rfc3339>"`, Phase 109; the drain lane appends `@<attempts>` — the
/// direction field is unaffected). `None` for NULL/garbled markers — a corrupt
/// marker is treated as "no flip pending" and rewritten, never trusted.
fn pending_flip_direction(marker: Option<&str>) -> Option<bool> {
    match marker?.split_once('@')?.0 {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// A parsed `feed_verdict_pending` marker.
///
/// Wire format: `direction@first_seen[@attempts]`. The first two fields are
/// Phase 109's deferred-flip record; the pending-verdict drain (2026-08-31
/// live audit) added the third — how many drain visits have looked at this
/// flip without resolving it. Attempts absent = 1: every marker the persist
/// boundary writes is by definition on its first attempt, so every pre-drain
/// marker parses without rewrite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingMarker {
    /// The deferred flip's target value for `feed_relevant`.
    pub direction: bool,
    /// When the flip was first deferred.
    pub first_seen: chrono::DateTime<chrono::Utc>,
    /// Drain visits that ended unresolved (>= 1).
    pub attempts: u32,
}

impl PendingMarker {
    /// Parse a stored marker. `None` = corrupt; per the Phase-109 doctrine a
    /// corrupt marker is never trusted — callers clear it.
    pub fn parse(marker: &str) -> Option<Self> {
        let (dir_str, rest) = marker.split_once('@')?;
        let direction = match dir_str {
            "1" => true,
            "0" => false,
            _ => return None,
        };
        let (ts_str, attempts) = match rest.rsplit_once('@') {
            Some((ts, att)) => (ts, att.parse::<u32>().ok()?.max(1)),
            None => (rest, 1),
        };
        let first_seen = chrono::DateTime::parse_from_rfc3339(ts_str)
            .ok()?
            .with_timezone(&chrono::Utc);
        Some(Self {
            direction,
            first_seen,
            attempts,
        })
    }

    /// Stored form. `attempts == 1` writes the legacy two-field shape so a
    /// marker that has never been drained is byte-identical to what the
    /// persist boundary writes.
    pub fn to_marker_string(self) -> String {
        let dir = i64::from(self.direction);
        let ts = self.first_seen.to_rfc3339();
        if self.attempts <= 1 {
            format!("{dir}@{ts}")
        } else {
            format!("{dir}@{ts}@{}", self.attempts)
        }
    }
}

/// One row of the pending-flip backlog, oldest items first.
#[derive(Debug)]
pub struct PendingVerdictRow {
    pub id: i64,
    /// `None` = the stored marker is corrupt (cleared by the drain, never
    /// trusted).
    pub marker: Option<PendingMarker>,
}

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
    /// The pending-verdict drain resolved a starved deferred flip terminally
    /// after every retry lane was exhausted (2026-08-31 live audit: 175
    /// markers pending, the oldest since 08-12, none ever revisited). Always
    /// paired with `feed_relevant = 0` — a terminal resolution NEVER promotes
    /// without a real verdict; it only records that no real verdict could be
    /// obtained and stops the item from squatting in the retry queue forever.
    FallbackExhausted,
}

impl VerdictSource {
    /// Stored form. Kept short and lowercase — it is a persisted enum, so the
    /// strings are part of the schema contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Score => "score",
            Self::Serendipity => "serendipity",
            Self::FallbackExhausted => "fallback_exhausted",
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

/// WHY a persisted verdict flipped (Phase 108, `feed_verdict_reason`).
///
/// `feed_verdict_source` says who decided a verdict; it cannot say why a
/// curated item LOST that verdict — and the 2026-08-23 adversarial audit
/// produced two demotion classes that are indistinguishable without it
/// (an epoch demotion vs. an in-version score-churn demotion). A normal
/// score-derived verdict needs no explanation and leaves the column NULL;
/// only the repair passes write a reason.
///
/// A later wave adds verdict-flip hysteresis at the same persist boundary —
/// compose with [`Database::persist_feed_verdicts_with_reasons`], which
/// already carries per-verdict metadata, rather than adding another parallel
/// writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictReason {
    /// The item's LIVE score sank clearly below the admission line while the
    /// verdict's pipeline version was still current (in-version demotion —
    /// the score churned down after the verdict was granted).
    ScoreSunkInVersion,
    /// A superseded pipeline version decided the verdict and the current
    /// pipeline rejects the item (the Phase-101 reconciliation pass).
    StaleVersion,
    /// The LLM judge pass (Tier 2) rated a curated item clearly irrelevant
    /// with high confidence — demote-only, like every repair reason.
    LlmReject,
    /// The pending-verdict drain gave the deferred flip its full retry budget
    /// (attempts + age gates, see `llm_judge::drain`) and no real verdict ever
    /// resolved it. Recorded alongside `VerdictSource::FallbackExhausted`.
    PendingRetriesExhausted,
    /// The item is a cross-cycle twin (same canonical URL or normalized title)
    /// of an item ALREADY curated. Dedup is batch-scoped and differential
    /// cycles score 250–300 items, so the same story re-fetched under a new
    /// `source_items` id in a later cycle earned its own standing verdict —
    /// "Bun 1.4 Rust rewrite is not looking good" was feed-relevant TEN times
    /// across four cycles (2026-09-04 audit, M2). Written at the persist
    /// boundary; the original keeps its verdict.
    DuplicateCurated,
}

impl VerdictReason {
    /// Stored form. Persisted enum — the strings are part of the schema
    /// contract, same as [`VerdictSource::as_str`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScoreSunkInVersion => "score_sunk_in_version",
            Self::StaleVersion => "stale_version",
            Self::LlmReject => "llm_reject",
            Self::PendingRetriesExhausted => "pending_retries_exhausted",
            Self::DuplicateCurated => "duplicate_curated",
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

/// Margin below the relevance threshold an in-version verdict's LIVE score
/// must sink before [`Database::demote_sunk_verdicts`] pulls it.
///
/// The 2026-08-23 audit measured ~300 items jittering across 0.37–0.43 on
/// same-version re-scores; demoting at the threshold itself would thrash
/// exactly that band (demoted here, re-promoted by the next cycle, demoted
/// again). With the default 0.40 threshold this puts the demote line at 0.37:
/// only clearly-sunk items go, boundary jitter stays untouched. The promote
/// line (the threshold) and the demote line (threshold − epsilon) differing
/// IS the hysteresis this pass needs; the fuller flip-guard lands at the
/// persist boundary in a later wave.
pub const SCORE_SUNK_EPSILON: f32 = 0.03;

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
        // A cycle verdict needs no explanation — reason NULL is the normal
        // case, and writing it (rather than leaving the column alone) clears
        // any repair-pass reason a PREVIOUS flip left behind: a fresh judgment
        // supersedes the old explanation.
        let with_reasons: Vec<(i64, bool, VerdictSource, Option<VerdictReason>)> = verdicts
            .iter()
            .map(|&(id, relevant, source)| (id, relevant, source, None))
            .collect();
        self.persist_feed_verdicts_with_reasons(&with_reasons, version)
    }

    /// [`Database::persist_feed_verdicts`] with an optional per-verdict
    /// [`VerdictReason`]. This is THE persist boundary for feed verdicts —
    /// every cycle writer routes through here.
    ///
    /// ## Verdict-flip damping (Phase 109, 2026-08-23 audit item 10)
    ///
    /// The audit measured daily feed-membership churn driven by unreasoned
    /// verdict flips: the same item entering and leaving the curated set as its
    /// score wobbled around the threshold. The boundary now distinguishes:
    ///
    /// - **Immediate writes** — a reasoned flip (any [`VerdictReason`]), a
    ///   serendipity injection (the anti-bubble rotation is a deliberate
    ///   feature), a FIRST verdict (`feed_relevant` NULL), or a
    ///   re-confirmation (new == standing). These apply exactly as before and
    ///   clear any pending marker — a run that re-confirms the standing
    ///   verdict is a run DISAGREEING with a pending flip.
    /// - **Deferred flips** — an unreasoned, score-sourced flip against a
    ///   standing verdict (1→0, or 0→1 on an item with a prior verdict) is
    ///   recorded in `feed_verdict_pending` ("direction@first-seen") and the
    ///   standing verdict row is left UNTOUCHED. A second consecutive judging
    ///   run wanting the same flip applies it (and clears the marker).
    ///
    /// A deferred flip does not refresh the verdict stamps: the row keeps
    /// describing the run that decided the STANDING verdict, so version-scoped
    /// reconciliation (which is reasoned and therefore immediate) still owns
    /// cross-version convergence.
    pub fn persist_feed_verdicts_with_reasons(
        &self,
        verdicts: &[(i64, bool, VerdictSource, Option<VerdictReason>)],
        version: i32,
    ) -> SqliteResult<usize> {
        if verdicts.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;
        let mut deferred = 0usize;
        let mut confirmed_flips = 0usize;
        {
            let mut read_stmt = tx.prepare_cached(
                "SELECT feed_relevant, feed_verdict_pending FROM source_items WHERE id = ?1",
            )?;
            let mut apply_stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = ?1,
                     feed_verdict_at = datetime('now'),
                     feed_verdict_version = ?2,
                     feed_verdict_source = ?3,
                     feed_verdict_reason = ?4,
                     feed_verdict_pending = NULL
                 WHERE id = ?5",
            )?;
            let mut defer_stmt = tx.prepare_cached(
                "UPDATE source_items SET feed_verdict_pending = ?1 WHERE id = ?2",
            )?;
            for (id, relevant, source, reason) in verdicts {
                // Standing state first, same transaction — exact, not racy.
                let (old_relevant, pending): (Option<i64>, Option<String>) = read_stmt
                    .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?
                    .unwrap_or((None, None));
                let is_flip = matches!(old_relevant, Some(old) if (old != 0) != *relevant);
                let immediate =
                    reason.is_some() || *source == VerdictSource::Serendipity || !is_flip;
                if immediate || pending_flip_direction(pending.as_deref()) == Some(*relevant) {
                    if !immediate {
                        confirmed_flips += 1;
                    }
                    apply_stmt.execute(params![
                        i64::from(*relevant),
                        version,
                        source.as_str(),
                        reason.map(VerdictReason::as_str),
                        id
                    ])?;
                } else {
                    // First sighting of an unreasoned flip: record the pending
                    // direction, keep the standing verdict row untouched.
                    deferred += 1;
                    defer_stmt.execute(params![
                        format!(
                            "{}@{}",
                            i64::from(*relevant),
                            chrono::Utc::now().to_rfc3339()
                        ),
                        id
                    ])?;
                }
                count += 1;
            }
        }
        tx.commit()?;
        if deferred > 0 || confirmed_flips > 0 {
            tracing::debug!(
                target: "4da::verdicts",
                deferred,
                confirmed_flips,
                "Unreasoned verdict flips damped at the persist boundary"
            );
        }
        Ok(count)
    }

    /// The id of an ALREADY-curated item that is the same story as
    /// (`url`, `title`) — same canonical URL (tracking params stripped) or
    /// same normalized title — excluding `id` itself. `None` when the story
    /// is new to the curated set.
    ///
    /// Candidates are pre-filtered in SQL on the raw URL prefix / exact
    /// lowercase title so the query stays cheap, then the exact canonical
    /// comparison happens in Rust with the same normalizers the batch dedup
    /// uses (`scoring::normalize_result_url` / `normalize_result_title`) — one
    /// definition of "same story" for both the batch and the persist boundary.
    ///
    /// Only a twin ingested EARLIER (lower id) can hold the slot. A full
    /// re-drain re-scores every curated twin pair in one cycle, and the
    /// pre-pass reads the corpus before any verdict is written — with a
    /// symmetric rule both twins would find each other and BOTH be demoted,
    /// erasing the story from the feed. Anchoring on ingest order makes the
    /// outcome stable across drains: the first copy keeps the slot, every
    /// later copy yields to it.
    pub fn find_curated_twin(
        &self,
        id: i64,
        url: Option<&str>,
        title: &str,
    ) -> SqliteResult<Option<i64>> {
        let url_key = url
            .map(crate::scoring::normalize_result_url)
            .unwrap_or_default();
        let title_key = crate::scoring::normalize_result_title(title);
        if url_key.is_empty() && title_key.is_empty() {
            return Ok(None);
        }
        let url_prefix = url
            .map(|u| u.split(['?', '#']).next().unwrap_or(u).to_string())
            .unwrap_or_default();
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id, url, title FROM source_items
             WHERE feed_relevant = 1 AND id < ?1
               AND ((?2 <> '' AND url LIKE ?2 || '%') OR LOWER(title) = LOWER(?3))
             ORDER BY id ASC
             LIMIT 25",
        )?;
        let rows = stmt.query_map(params![id, url_prefix, title], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (other_id, other_url, other_title) = row?;
            let same_url = !url_key.is_empty()
                && other_url
                    .as_deref()
                    .is_some_and(|u| crate::scoring::normalize_result_url(u) == url_key);
            let same_title = !title_key.is_empty()
                && crate::scoring::normalize_result_title(&other_title) == title_key;
            if same_url || same_title {
                return Ok(Some(other_id));
            }
        }
        Ok(None)
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
            // Demotions record WHY: a superseded version's verdict, rejected
            // by the current pipeline. Confirmations clear any prior reason —
            // a confirmed verdict is a normal, current score verdict, and a
            // leftover explanation would describe a flip that no longer holds.
            let mut demote_stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = 0,
                     feed_verdict_version = ?1,
                     feed_verdict_source = 'score',
                     feed_verdict_reason = ?2,
                     feed_verdict_pending = NULL
                 WHERE id = ?3 AND feed_relevant = 1",
            )?;
            for id in demote {
                count += demote_stmt.execute(params![
                    version,
                    VerdictReason::StaleVersion.as_str(),
                    id
                ])?;
            }
            let mut confirm_stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_verdict_version = ?1,
                     feed_verdict_source = 'score',
                     feed_verdict_reason = NULL,
                     feed_verdict_pending = NULL
                 WHERE id = ?2 AND feed_relevant = 1",
            )?;
            for id in confirm {
                count += confirm_stmt.execute(params![version, id])?;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Demote curated items whose verdict is CURRENT-version and score-derived
    /// but whose live `relevance_score` has sunk below `demote_below`
    /// (in-version sweep, 2026-08-23 audit).
    ///
    /// The Phase-101 working set above is version-scoped: within a pipeline
    /// version, a score-sourced verdict was immortal no matter how far the
    /// live score fell afterwards. Measured live 2026-08-23: 106 of 532 feed
    /// members sat below 0.45 with `feed_verdict_source = 'score'` — verdicts
    /// granted when the item scored above the line, kept after same-version
    /// churn dragged it as low as 0.17. This sweep closes that hole.
    ///
    /// Pure SQL, no re-score: for an in-version verdict the persisted score IS
    /// the current brain's judgment (the `scored_pipeline_version >= ?1` guard
    /// enforces exactly that — a verdict the reconciliation pass re-stamped
    /// while the score drain is still behind waits for the drain rather than
    /// being judged on a superseded number).
    ///
    /// Demote-only, same doctrine as [`Database::reconcile_feed_verdicts`]:
    /// promotion needs a full run's dedup/diversity/rerank context; removal of
    /// something the current score disowns does not. Convergent by
    /// construction — a demoted row fails `feed_relevant = 1` next sweep — and
    /// the caller passes `demote_below = threshold − SCORE_SUNK_EPSILON`, so
    /// the jitter band between the demote and promote lines never thrashes.
    ///
    /// Verdict provenance (`feed_verdict_version` / `_source` / `_at`) is
    /// deliberately left intact: the row still records which brain granted the
    /// verdict; `feed_verdict_reason` records why it was pulled.
    pub fn demote_sunk_verdicts(
        &self,
        current_version: i32,
        demote_below: f32,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "UPDATE source_items
             SET feed_relevant = 0,
                 feed_verdict_reason = ?3,
                 feed_verdict_pending = NULL
             WHERE feed_relevant = 1
               AND COALESCE(feed_verdict_source, 'score') = 'score'
               AND feed_verdict_version = ?1
               AND scored_pipeline_version >= ?1
               AND relevance_score IS NOT NULL
               AND relevance_score < ?2",
        )?;
        stmt.execute(params![
            current_version,
            f64::from(demote_below),
            VerdictReason::ScoreSunkInVersion.as_str()
        ])
    }

    // ========================================================================
    // Pending-verdict drain (2026-08-31 live audit)
    // ========================================================================
    //
    // The persist boundary defers unreasoned flips into `feed_verdict_pending`
    // and waits for a SECOND judging run to confirm or dispute them. But only
    // items the analysis cycle re-selects ever reach the boundary again, and
    // the cycle's working set is recency-bounded — so a marker on an item that
    // ages out of the window starves forever. Measured live 2026-08-31 on the
    // founder's 59k-item instance: 175 pending markers, 135 pending >2 days,
    // the oldest deferred 2026-08-12 and never revisited once.

    /// The pending-flip backlog, OLDEST items first (`created_at ASC` — the
    /// starvation is precisely that fresh items always won, so the drain's
    /// contract is the reverse order). Corrupt markers come back with
    /// `marker: None` for the caller to clear — never trusted, never parsed
    /// leniently.
    pub fn get_pending_verdict_backlog(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<PendingVerdictRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, feed_verdict_pending FROM source_items
             WHERE feed_verdict_pending IS NOT NULL
             ORDER BY created_at ASC, id ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let id: i64 = row.get(0)?;
            let raw: String = row.get(1)?;
            Ok(PendingVerdictRow {
                id,
                marker: PendingMarker::parse(&raw),
            })
        })?;
        rows.collect()
    }

    /// Record one more unresolved drain visit on each marker: attempts + 1,
    /// direction and first-seen preserved. A row whose marker is corrupt (or
    /// was cleared by a concurrent judging run) is left alone — escalation
    /// only ever rewrites a marker it could fully parse.
    pub fn escalate_pending_attempts(&self, ids: &[i64]) -> SqliteResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut escalated = 0;
        {
            let mut read_stmt =
                tx.prepare_cached("SELECT feed_verdict_pending FROM source_items WHERE id = ?1")?;
            let mut write_stmt = tx.prepare_cached(
                "UPDATE source_items SET feed_verdict_pending = ?1 WHERE id = ?2",
            )?;
            for id in ids {
                let raw: Option<String> = read_stmt
                    .query_row(params![id], |r| r.get(0))
                    .optional()?
                    .flatten();
                let Some(mut marker) = raw.as_deref().and_then(PendingMarker::parse) else {
                    continue;
                };
                marker.attempts = marker.attempts.saturating_add(1);
                escalated += write_stmt.execute(params![marker.to_marker_string(), id])?;
            }
        }
        tx.commit()?;
        Ok(escalated)
    }

    /// Clear pending markers without touching the standing verdict — used when
    /// a drain judgment DISPUTES the deferred flip (the standing verdict is
    /// re-affirmed, so the flip is dead), and for corrupt markers (Phase-109
    /// doctrine: rewritten, never trusted).
    pub fn clear_pending_markers(&self, ids: &[i64]) -> SqliteResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut cleared = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items SET feed_verdict_pending = NULL
                 WHERE id = ?1 AND feed_verdict_pending IS NOT NULL",
            )?;
            for id in ids {
                cleared += stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(cleared)
    }

    /// How many deferred flips are waiting. Logged by the drain so a backlog
    /// that stops shrinking is visible in the run log, cycle over cycle.
    pub fn count_pending_verdicts(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE feed_verdict_pending IS NOT NULL",
            [],
            |r| r.get(0),
        )
    }
}

#[cfg(test)]
#[path = "verdicts_tests.rs"]
mod tests;
