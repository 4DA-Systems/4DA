// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Risen-verdict promotion — the `feed_relevant` lane in the OTHER direction
//! (v32, 2026-09-06 live audit).
//!
//! Every repair pass in `verdicts.rs` is demote-only, and the drain persists
//! SCORES only. So an item whose CURRENT-version score clears the line had no
//! way back into the feed unless the recency-bounded analysis cycle happened
//! to re-select it. Measured live 2026-09-06 after the v31 drain: 915 rows
//! scored >= 0.40 with no relevant verdict — 178 never judged, 331 demoted
//! `stale_version` by an older pipeline, 137 `score_sunk_in_version` under an
//! older pipeline, 269 unreasoned rejections from an older version.
//! "Announcing Rust 1.98.0" sat at 0.90 (sunk at v27); four grounded tokio
//! advisories at 0.88–0.90 had never been judged. "When the engine improves,
//! it re-judges everything it already holds" held for the score and not for
//! the verdict the user actually sees.
//!
//! Split from `verdicts.rs` at birth: that file sat at 980 of its 1,000-line
//! ceiling with this section inside it.

use rusqlite::{params, OptionalExtension, Result as SqliteResult};

use super::{Database, VerdictReason, VerdictSource};

/// One row of the promotion working set
/// ([`Database::get_risen_verdict_candidates`]).
#[derive(Debug, Clone)]
pub struct RisenCandidate {
    pub id: i64,
    pub url: Option<String>,
    pub title: String,
    /// `feed_relevant IS NULL` — never judged, so the boundary applies the
    /// verdict immediately instead of deferring a flip.
    pub first_verdict: bool,
}

/// Outcome of one [`Database::promote_risen_verdicts`] pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RisenPromotion {
    /// Rows the working set held (bounded by the caller's limit).
    pub candidates: usize,
    /// First verdicts applied immediately.
    pub promoted: usize,
    /// Flips against a standing rejection, deferred to the judge drain.
    pub deferred: usize,
    /// Twins of an already-curated story, written `duplicate_curated`.
    pub twins: usize,
}

impl Database {
    /// Rows whose CURRENT-version score is at or above `promote_at` and whose
    /// verdict is absent or a SUPERSEDED-version score verdict — the promotion
    /// working set, best score first.
    ///
    /// Excluded by construction: any current-version verdict (the current
    /// brain already decided), every reasoned rejection except the two
    /// version-scoped repair reasons (`llm_reject`, `duplicate_curated` and
    /// `pending_retries_exhausted` are judgments in their own right), non-score
    /// provenance, and rows with a pending flip (the boundary owns those).
    pub fn get_risen_verdict_candidates(
        &self,
        current_version: i32,
        promote_at: f32,
        limit: usize,
    ) -> SqliteResult<Vec<RisenCandidate>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id, url, title, feed_relevant IS NULL FROM source_items
             WHERE scored_pipeline_version = ?1
               AND relevance_score >= ?2
               AND feed_verdict_pending IS NULL
               AND (
                    feed_relevant IS NULL
                 OR (feed_relevant = 0
                     AND COALESCE(feed_verdict_source, 'score') = 'score'
                     AND COALESCE(feed_verdict_reason, '')
                         IN ('', 'stale_version', 'score_sunk_in_version')
                     AND COALESCE(feed_verdict_version, 0) < ?1)
               )
             ORDER BY relevance_score DESC, id DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![current_version, f64::from(promote_at), limit as i64],
            |r| {
                Ok(RisenCandidate {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    first_verdict: r.get::<_, i64>(3)? != 0,
                })
            },
        )?;
        rows.collect()
    }

    /// Promote risen items through THE persist boundary
    /// ([`Database::persist_feed_verdicts_with_reasons`]).
    ///
    /// A row with NO verdict gets its first verdict immediately. A row a
    /// superseded pipeline REJECTED is an unreasoned flip against a standing
    /// verdict, so the boundary defers it into `feed_verdict_pending` and the
    /// judge drain adjudicates — the second opinion the demote-only doctrine
    /// asks for, obtained without the batch context this pass lacks. A row
    /// that is a twin of an already-curated story is written
    /// `duplicate_curated` instead (the earliest copy keeps the slot); rows are
    /// applied in id order so a twin promoted in the same batch is seen.
    pub fn promote_risen_verdicts(
        &self,
        current_version: i32,
        promote_at: f32,
        limit: usize,
    ) -> SqliteResult<RisenPromotion> {
        let mut candidates =
            self.get_risen_verdict_candidates(current_version, promote_at, limit)?;
        let mut outcome = RisenPromotion {
            candidates: candidates.len(),
            ..RisenPromotion::default()
        };
        candidates.sort_by_key(|c| c.id);
        for c in &candidates {
            let verdict = if self
                .find_curated_twin(c.id, c.url.as_deref(), &c.title)?
                .is_some()
            {
                outcome.twins += 1;
                (
                    c.id,
                    false,
                    VerdictSource::Score,
                    Some(VerdictReason::DuplicateCurated),
                )
            } else {
                if c.first_verdict {
                    outcome.promoted += 1;
                } else {
                    outcome.deferred += 1;
                }
                (c.id, true, VerdictSource::Score, None)
            };
            self.persist_feed_verdicts_with_reasons(&[verdict], current_version)?;
        }
        Ok(outcome)
    }

    /// [`Database::find_curated_twin`] for an item known only by id — the
    /// judge drain confirms a pending promotion without the row's url/title in
    /// hand. `None` for an unknown id as well as for a story new to the feed.
    pub fn curated_twin_of_item(&self, id: i64) -> SqliteResult<Option<i64>> {
        let row: Option<(Option<String>, String)> = {
            let conn = self.read_conn();
            conn.query_row(
                "SELECT url, title FROM source_items WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
        };
        match row {
            Some((url, title)) => self.find_curated_twin(id, url.as_deref(), &title),
            None => Ok(None),
        }
    }

    /// Demote later copies of a story the curated set already holds — the
    /// cross-cycle twin rule applied to the STANDING feed. The persist boundary
    /// has written `duplicate_curated` since 2026-09-04, but twins curated
    /// before that held the slot together until something looked (live
    /// 2026-09-06: "A 2026 survey of Rust GUI libraries" x3, "Bun 1.4 Rust
    /// rewrite" x2). The earliest copy keeps it; the write is reasoned, so it
    /// applies immediately. Convergent: a demoted copy fails
    /// `feed_relevant = 1` next sweep.
    pub fn demote_curated_twins(&self, version: i32) -> SqliteResult<usize> {
        let curated: Vec<(i64, Option<String>, String)> = {
            let conn = self.read_conn();
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title FROM source_items WHERE feed_relevant = 1 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<SqliteResult<Vec<_>>>()?
        };
        let mut demote = Vec::new();
        for (id, url, title) in &curated {
            if self
                .find_curated_twin(*id, url.as_deref(), title)?
                .is_some()
            {
                demote.push((
                    *id,
                    false,
                    VerdictSource::Score,
                    Some(VerdictReason::DuplicateCurated),
                ));
            }
        }
        if demote.is_empty() {
            return Ok(0);
        }
        self.persist_feed_verdicts_with_reasons(&demote, version)
    }

    /// Withdraw `duplicate_curated` verdicts whose curated twin is gone.
    ///
    /// A twin verdict is a claim about ANOTHER row — "the story is already
    /// in the feed under an earlier id" — and it stays true only while that
    /// earlier copy is curated. Nothing re-checked it: "This Week in Rust
    /// 666" lost both its RSS row (twin of a lemmy mirror) and the lemmy row
    /// (twin of a Mastodon boost) when the boost fell to the UGC gate, and
    /// the issue vanished from a feed that lists 660–665 and 667 (2026-09-07;
    /// 11 such stories live, 4 scored ≥ 0.7).
    ///
    /// The verdict is cleared outright (no verdict, no reason, no pending
    /// marker), never flipped: the row was never judged on its own merits,
    /// so the next risen sweep grants it a FIRST verdict through the persist
    /// boundary — immediate, and twin-checked again against whatever is
    /// curated by then. Convergent: a row whose twin is back is re-written
    /// `duplicate_curated` by that same sweep.
    pub fn withdraw_orphaned_duplicate_verdicts(&self) -> SqliteResult<usize> {
        let duplicates: Vec<(i64, Option<String>, String)> = {
            let conn = self.read_conn();
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title FROM source_items
                 WHERE feed_relevant = 0 AND feed_verdict_reason = 'duplicate_curated'
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<SqliteResult<Vec<_>>>()?
        };
        let mut orphaned: Vec<i64> = Vec::new();
        for (id, url, title) in &duplicates {
            if self
                .find_curated_twin(*id, url.as_deref(), title)?
                .is_none()
            {
                orphaned.push(*id);
            }
        }
        if orphaned.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let mut count = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE source_items
                 SET feed_relevant = NULL, feed_verdict_at = NULL, feed_verdict_version = NULL,
                     feed_verdict_source = NULL, feed_verdict_reason = NULL,
                     feed_verdict_pending = NULL
                 WHERE id = ?1 AND feed_verdict_reason = 'duplicate_curated'",
            )?;
            for id in &orphaned {
                count += stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(count)
    }
}
