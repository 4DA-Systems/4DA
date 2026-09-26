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
}

impl Database {
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
        Ok(JudgeGateSweep { rejected, released })
    }
}

#[cfg(test)]
#[path = "judge_gate_tests.rs"]
mod tests;
