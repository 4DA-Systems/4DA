// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Advisor judgment memo — the rerank judge's working memory.
//!
//! Measured 2026-08-31 on the live corpus: 42% of rerank judgments over 48h
//! (808 provenance rows / 466 distinct items / 17 passes) re-judged items an
//! earlier pass had already judged with the SAME model identity and prompt
//! version — identical inputs, identical verdicts, real API spend. The feed's
//! top band is sticky by construction, so every ~10-minute analysis cycle
//! re-bought judgments it already owned.
//!
//! This module persists each fresh judgment keyed by
//! `(source_item_id, identity_hash, prompt_version)` and hands back
//! fresh-enough rows so `analysis_rerank` can REPLAY them through the
//! reconciler instead of re-billing the model. Two invariants:
//!
//! - **A replay never re-stamps provenance or calibration samples.** A stored
//!   judgment re-entering the calibration set would double-count the sample
//!   and, at the limit, feed the curve its own output (the 2026-08-11
//!   degenerate-curve incident class). Replays adjust scores and explanations
//!   only.
//! - **Only judgments from a pass that survived the uniform-advisor circuit
//!   breaker are memoized.** A discarded pass is discarded everywhere.
//!
//! Freshness window: [`MEMO_FRESHNESS_HOURS`]. The judged content is
//! immutable post-ingest; what drifts is the USER's context (git activity,
//! interests), so the window is just under a day — a stable item is judged
//! once per day per model+prompt instead of ~9 times, and the freed budget
//! rotates to items the judge has never seen.

use std::collections::HashMap;

use tracing::{debug, warn};

/// How long a stored judgment may be replayed before the item is judged
/// afresh. Just under a day: context drift re-judges daily; anything shorter
/// re-buys judgments the context can't have invalidated.
pub(crate) const MEMO_FRESHNESS_HOURS: i64 = 20;

/// A judgment loaded from the memo, ready to replay.
#[derive(Debug, Clone)]
pub(crate) struct MemoizedJudgment {
    pub raw_score: f32,
    pub confidence: f32,
    pub reasoning: String,
}

/// Load fresh-enough memoized judgments for the candidate items under the
/// given model identity + prompt version. Returns a map keyed by
/// source_item_id; items absent from the map need a fresh LLM judgment.
pub(crate) fn load_fresh(
    conn: &rusqlite::Connection,
    item_ids: &[i64],
    identity_hash: &str,
    prompt_version: &str,
) -> HashMap<i64, MemoizedJudgment> {
    let mut out = HashMap::new();
    if item_ids.is_empty() {
        return out;
    }

    let placeholders: String = item_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT source_item_id, raw_score, confidence, reasoning
         FROM advisor_judgments
         WHERE identity_hash = ? AND prompt_version = ?
           AND judged_at >= datetime('now', '-{MEMO_FRESHNESS_HOURS} hours')
           AND source_item_id IN ({placeholders})"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::rerank", error = %e, "advisor memo read failed — judging everything fresh");
            return out;
        }
    };

    let mut params: Vec<&dyn rusqlite::types::ToSql> = vec![
        &identity_hash as &dyn rusqlite::types::ToSql,
        &prompt_version,
    ];
    for id in item_ids {
        params.push(id as &dyn rusqlite::types::ToSql);
    }

    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            MemoizedJudgment {
                raw_score: row.get::<_, f64>(1)? as f32,
                confidence: row.get::<_, f64>(2)? as f32,
                reasoning: row.get::<_, String>(3)?,
            },
        ))
    });

    match rows {
        Ok(iter) => {
            for row in iter.flatten() {
                out.insert(row.0, row.1);
            }
        }
        Err(e) => {
            warn!(target: "4da::rerank", error = %e, "advisor memo query failed — judging everything fresh");
        }
    }
    out
}

/// Persist fresh judgments after a pass that survived the circuit breaker.
/// Upsert: the newest judgment for a key wins. Best-effort — a memo write
/// failure costs a future re-judgment, never the pass.
pub(crate) fn store(
    conn: &rusqlite::Connection,
    identity_hash: &str,
    prompt_version: &str,
    rows: &[(i64, f32, f32, &str)], // (item_id, raw_score, confidence, reasoning)
) -> usize {
    let mut stored = 0;
    for (item_id, raw_score, confidence, reasoning) in rows {
        let result = conn.execute(
            "INSERT INTO advisor_judgments
                (source_item_id, identity_hash, prompt_version, raw_score, confidence, reasoning, judged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(source_item_id, identity_hash, prompt_version) DO UPDATE SET
                raw_score = excluded.raw_score,
                confidence = excluded.confidence,
                reasoning = excluded.reasoning,
                judged_at = excluded.judged_at",
            rusqlite::params![
                item_id,
                identity_hash,
                prompt_version,
                f64::from(*raw_score),
                f64::from(*confidence),
                reasoning
            ],
        );
        match result {
            Ok(_) => stored += 1,
            Err(e) => {
                debug!(target: "4da::rerank", error = %e, item_id, "advisor memo write failed (non-fatal)")
            }
        }
    }
    stored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memo_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE advisor_judgments (
                source_item_id INTEGER NOT NULL,
                identity_hash TEXT NOT NULL,
                prompt_version TEXT NOT NULL,
                raw_score REAL NOT NULL,
                confidence REAL NOT NULL,
                reasoning TEXT NOT NULL DEFAULT '',
                judged_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (source_item_id, identity_hash, prompt_version)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn stores_and_loads_fresh_judgments() {
        let conn = memo_conn();
        let n = store(
            &conn,
            "hashA",
            "judge-v1",
            &[(1, 0.8, 0.8, "solid"), (2, 0.2, 0.2, "noise")],
        );
        assert_eq!(n, 2);

        let loaded = load_fresh(&conn, &[1, 2, 3], "hashA", "judge-v1");
        assert_eq!(loaded.len(), 2, "item 3 was never judged");
        assert!((loaded[&1].raw_score - 0.8).abs() < 1e-6);
        assert_eq!(loaded[&2].reasoning, "noise");
    }

    #[test]
    fn different_identity_or_prompt_never_replays() {
        // A swapped model or a bumped prompt is a DIFFERENT judge — its
        // stored opinions must not masquerade as the current judge's.
        let conn = memo_conn();
        store(&conn, "hashA", "judge-v1", &[(1, 0.8, 0.8, "r")]);

        assert!(load_fresh(&conn, &[1], "hashB", "judge-v1").is_empty());
        assert!(load_fresh(&conn, &[1], "hashA", "judge-v2").is_empty());
    }

    #[test]
    fn stale_judgments_age_out() {
        let conn = memo_conn();
        store(&conn, "hashA", "judge-v1", &[(1, 0.8, 0.8, "r")]);
        conn.execute(
            "UPDATE advisor_judgments SET judged_at = datetime('now', '-25 hours')",
            [],
        )
        .unwrap();
        assert!(
            load_fresh(&conn, &[1], "hashA", "judge-v1").is_empty(),
            "a {MEMO_FRESHNESS_HOURS}h-window memo must not serve a 25h-old judgment"
        );
    }

    #[test]
    fn upsert_newest_wins() {
        let conn = memo_conn();
        store(&conn, "hashA", "judge-v1", &[(1, 0.2, 0.2, "first read")]);
        store(&conn, "hashA", "judge-v1", &[(1, 0.9, 0.9, "re-judged")]);
        let loaded = load_fresh(&conn, &[1], "hashA", "judge-v1");
        assert!((loaded[&1].confidence - 0.9).abs() < 1e-6);
        assert_eq!(loaded[&1].reasoning, "re-judged");
    }

    #[test]
    fn empty_candidate_list_is_a_noop() {
        let conn = memo_conn();
        assert!(load_fresh(&conn, &[], "hashA", "judge-v1").is_empty());
    }
}
