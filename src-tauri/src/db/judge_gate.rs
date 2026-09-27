// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Repair sweep for judge-gated admission (`crate::judge_gate`).

use rusqlite::{params, Result as SqliteResult};

use super::{Database, VerdictReason, VerdictSource};

/// Outcome of one [`Database::reconcile_judge_gate`] pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct JudgeGateSweep {
    /// Curated or waiting items the card-aware judge put below the bar.
    pub rejected: usize,
    /// Waiting items the judge cleared (or, with no judge, all of them):
    /// verdict withdrawn so the risen sweep admits them by score.
    pub released: usize,
    /// Items out of the feed (score verdict, zero-engagement cap, or a
    /// thin-context `llm_reject`) that a newer card-aware judgment cleared:
    /// verdict withdrawn and queued for one re-score.
    pub rescued: usize,
}

impl Database {
    /// The latest card-aware judge relevance for `id` (`crate::judge_gate`).
    /// Read by the scorer for judge-gated sources only.
    pub fn card_judgment_of(&self, id: i64) -> Option<f64> {
        super::verdicts::card_judgment(&self.read_conn(), id)
            .ok()
            .flatten()
    }

    /// Rescue: gated items out of the feed that a card-aware judgment at the
    /// bar cleared AFTER their last score. Measured 2026-09-27 on 241 blind
    /// items: the judge picks the useful ones among zero-engagement posts at
    /// AUC 0.89 and among thin-context rejections at 0.95, at the feed's own
    /// precision. The verdict is withdrawn (`llm_reject` too: a newer,
    /// better-informed judge disagrees) and the row is queued for one
    /// re-score, whose verdict goes back through the persist boundary. The
    /// guard `judged_at > scored_at` makes it one-shot: the re-score stamps
    /// `scored_at` past the judgment.
    fn rescue_judge_cleared(&self) -> SqliteResult<usize> {
        let [ingest, drain] = crate::judge_gate::CARD_PROMPT_VERSIONS;
        let conn = self.conn.lock();
        conn.execute(
            &format!(
                "UPDATE source_items
                 SET feed_relevant = NULL, feed_verdict_at = NULL, feed_verdict_version = NULL,
                     feed_verdict_source = NULL, feed_verdict_reason = NULL,
                     feed_verdict_pending = NULL, scored_pipeline_version = 0
                 WHERE source_type IN ({gated})
                   AND created_at >= datetime('now', '-14 days')
                   AND COALESCE(feed_relevant, 0) = 0
                   AND COALESCE(feed_verdict_source, 'score') = 'score'
                   AND COALESCE(feed_verdict_reason, '')
                       IN ('', 'stale_version', 'score_sunk_in_version', 'llm_reject')
                   AND COALESCE(scored_pipeline_version, 0) <> 0
                   AND EXISTS (
                       SELECT 1 FROM llm_judgments lj
                       WHERE lj.source_item_id = source_items.id
                         AND lj.prompt_version IN (?1, ?2)
                         AND lj.relevance_score >= ?3
                         AND lj.judged_at > COALESCE(source_items.scored_at, '')
                         AND lj.id = (SELECT lj2.id FROM llm_judgments lj2
                                      WHERE lj2.source_item_id = source_items.id
                                        AND lj2.prompt_version IN (?1, ?2)
                                        ORDER BY lj2.judged_at DESC, lj2.id DESC LIMIT 1))",
                gated = crate::judge_gate::gated_sources_sql()
            ),
            params![ingest, drain, crate::judge_gate::GATE_RELEVANCE],
        )
    }

    /// Bring standing verdicts in line with the gate.
    /// - A curated item from a gated source whose latest card-aware judgment
    ///   is below the bar is demoted (`llm_reject`).
    /// - A waiting item (`awaiting_judge`) is released once judged at the
    ///   bar, and rejected once judged below it.
    /// - With `gate_active` false (no judge can run), every waiting item is
    ///   released, so a missing judge never strands a source.
    ///
    /// Serendipity picks are left alone. Released rows get their first
    /// verdict from the risen sweep, back through the persist boundary.
    pub fn reconcile_judge_gate(
        &self,
        gate_active: bool,
        version: i32,
    ) -> SqliteResult<JudgeGateSweep> {
        let gated = crate::judge_gate::gated_sources_sql();
        let rows: Vec<(i64, bool, Option<f64>)> = {
            let conn = self.read_conn();
            let mut stmt = conn.prepare(&format!(
                "SELECT id, feed_relevant = 1 FROM source_items
                 WHERE source_type IN ({gated})
                   AND ((feed_relevant = 1 AND COALESCE(feed_verdict_source, 'score') = 'score')
                        OR feed_verdict_reason = 'awaiting_judge')"
            ))?;
            let ids = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)))?
                .collect::<SqliteResult<Vec<_>>>()?;
            let mut out = Vec::with_capacity(ids.len());
            for (id, curated) in ids {
                out.push((id, curated, super::verdicts::card_judgment(&conn, id)?));
            }
            out
        };
        let mut reject = Vec::new();
        let mut release = Vec::new();
        for (id, curated, judgment) in rows {
            match (gate_active, curated, judgment) {
                (false, false, _) => release.push(id),
                (false, true, _) => {}
                (true, _, Some(r)) if r < crate::judge_gate::GATE_RELEVANCE => reject.push((
                    id,
                    false,
                    VerdictSource::Score,
                    Some(VerdictReason::LlmReject),
                )),
                (true, false, Some(_)) => release.push(id),
                _ => {}
            }
        }
        let rejected = if reject.is_empty() {
            0
        } else {
            self.persist_feed_verdicts_with_reasons(&reject, version)?
        };
        let mut released = 0usize;
        if !release.is_empty() {
            let conn = self.conn.lock();
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare_cached(
                    "UPDATE source_items
                     SET feed_relevant = NULL, feed_verdict_at = NULL, feed_verdict_version = NULL,
                         feed_verdict_source = NULL, feed_verdict_reason = NULL,
                         feed_verdict_pending = NULL
                     WHERE id = ?1 AND feed_verdict_reason = 'awaiting_judge'",
                )?;
                for id in &release {
                    released += stmt.execute(params![id])?;
                }
            }
            tx.commit()?;
        }
        let rescued = if gate_active {
            self.rescue_judge_cleared()?
        } else {
            0
        };
        Ok(JudgeGateSweep {
            rejected,
            released,
            rescued,
        })
    }
}

#[cfg(test)]
#[path = "judge_gate_tests.rs"]
mod tests;
