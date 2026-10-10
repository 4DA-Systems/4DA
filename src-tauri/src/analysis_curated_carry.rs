// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The cold full-window candidate set carries the curated corpus.
//!
//! Measured live 2026-10-04 13:30 UTC: after an app restart the display set
//! has no merge base, so the analysis takes the full path and scores what
//! `get_items_tiered(168, 1000)` selects — the newest 1000 rows under a
//! per-source cap. On that corpus the newest 1000 reached back only eight
//! hours, while 744 rows were curated (`feed_relevant = 1`), 50 of them
//! registry releases and advisories. The "Your stack" lane, fed almost
//! entirely by registry rows, showed 2 of them after the restart against 30
//! crates.io rows before it. The differential cycles that follow merge only
//! rows that CHANGED since the watermark, so the older curated rows never
//! came back until a `PIPELINE_VERSION` bump forced a full re-score.
//!
//! ## Why the rows are re-scored rather than hydrated
//!
//! A display row is a `SourceRelevance`: matches, explanation, signal action
//! and triggers, advisory id, applicability, `is_critical_alert`, the full
//! score breakdown. The durable columns hold the evidence score, the signal
//! type and priority, and a breakdown truncated to eight array items and 400
//! bytes per string (`scoring_explanations`). The rest exists only in the
//! scorer's output, and the analyzer's invariant is that every row reaching
//! the user passes through `score_item` (see `scoring::analyzer`). So the
//! curated rows join the cycle's scoring batch (union, dedup by id) and are
//! judged exactly like the rows around them: same pipeline version, same
//! batch layer, then the same demote-only display convergence and #821 news
//! window in `analyze_cached_content_inner`. Their evidence persists like any
//! re-score; their rank and verdict do not (see [`FullWindow::cycle`]).
//!
//! ## What is carried
//!
//! - Only `feed_relevant = 1`. A negative or missing verdict is never brought
//!   back.
//! - Only sources the feed admits (`feed_admission::admits_to_feed`).
//! - Not news the #821 window would take straight back out: a news row older
//!   than its source's display window is skipped before it costs a scoring
//!   slot, using the same effective date and the same `aged_out_of_feed` rule
//!   as `analysis_display_window`. Rows carrying CVE ids or classified as a
//!   security advisory are kept regardless, because the window exempts
//!   backend-confirmed security truth and only the scorer can confirm it.
//! - The same tier history gate as every other backlog selection: free tier
//!   reads the last `FREE_HISTORY_LIMIT_HOURS` by first ingest, Signal has
//!   no recency bound.
//! - At most [`CURATED_CARRY_CAP`] rows, rows that never age out (registries,
//!   advisories, research) first, then newest first.

use std::collections::HashSet;

use rusqlite::params;
use tracing::{info, warn};

use crate::analysis::analysis_cycle::{CycleResults, ScoredBatch};
use crate::db::{item_embeddings, Database, StoredSourceItem, FREE_HISTORY_LIMIT_HOURS};
use crate::sources::feed_admission::{admits_to_feed, aged_out_of_feed, news_display_window_days};

/// Window and size of the full-analysis selection (unchanged by this module).
pub(crate) const FULL_WINDOW_HOURS: i64 = 168;
pub(crate) const FULL_WINDOW_LIMIT: usize = 1000;

/// Upper bound on curated rows added to one full-window batch. The live
/// corpus carried 372 (Signal) / 347 (free) after the age filter on
/// 2026-10-04; the cap bounds cold-start cost if the curated corpus grows.
pub(crate) const CURATED_CARRY_CAP: usize = 600;

/// One curated row as the selection sees it, before its body is loaded.
struct CarryRow {
    id: i64,
    source_type: String,
    age_days: Option<f64>,
    security: bool,
}

impl CarryRow {
    /// Would the display set keep this row once scored as relevant?
    fn displayable(&self) -> bool {
        admits_to_feed(&self.source_type)
            && (self.security || !aged_out_of_feed(&self.source_type, self.age_days))
    }
}

/// The full-analysis candidate set and which of its rows were carried.
pub(crate) struct FullWindow {
    /// The newest-N window followed by the carried curated rows.
    pub items: Vec<StoredSourceItem>,
    /// Ids of the carried rows (not in the newest-N window).
    pub carried: HashSet<u64>,
}

impl FullWindow {
    /// The cycle result for a full pass over this window.
    ///
    /// Carried rows are DISPLAY carry-over, not this cycle's judgment of new
    /// work: their evidence and version stamp persist from `evaluated` (as on
    /// any re-score), but rank and verdict are written only for the window
    /// rows, and they are not "new this cycle" for receipts or notifications.
    /// Measured 2026-10-04 on a live snapshot, 134 of 356 carried rows score
    /// below the line once their freshness has decayed; persisting that as a
    /// verdict would flip a curated row out of the corpus because time passed,
    /// which `stale_news` (#789) and the #821 display window both decline to
    /// do. The display still follows the scorer (and the demote-only durable
    /// convergence), so such a row is simply not shown.
    pub(crate) fn cycle(&self, batch: ScoredBatch) -> CycleResults {
        let mut cycle = CycleResults::full_from_batch(batch);
        if !self.carried.is_empty() {
            let scored: HashSet<u64> = cycle
                .results
                .iter()
                .map(|r| r.id)
                .filter(|id| !self.carried.contains(id))
                .collect();
            cycle.scored_ids = Some(scored);
        }
        cycle
    }
}

/// The full-analysis candidate set: the newest-N window `get_items_tiered`
/// selects, plus the curated rows it no longer reaches.
pub(crate) fn full_window_candidates(db: &Database) -> rusqlite::Result<FullWindow> {
    full_window_candidates_with(
        db,
        crate::settings::is_signal(),
        FULL_WINDOW_LIMIT,
        CURATED_CARRY_CAP,
    )
}

/// [`full_window_candidates`] with the tier and both bounds explicit (tests).
pub(crate) fn full_window_candidates_with(
    db: &Database,
    signal: bool,
    limit: usize,
    carry_cap: usize,
) -> rusqlite::Result<FullWindow> {
    // Same tier window and per-source cap as `Database::get_items_tiered`.
    let hours = if signal {
        FULL_WINDOW_HOURS
    } else {
        FULL_WINDOW_HOURS.min(FREE_HISTORY_LIMIT_HOURS)
    };
    let mut items = db.get_items_balanced_by_source(hours, (limit / 5).max(50), limit)?;
    // An empty window means "the cache is stale, fetch" to every caller; the
    // carry must not turn that signal off.
    let carried = if items.is_empty() {
        HashSet::new()
    } else {
        merge_curated_carry(db, &mut items, signal, carry_cap)
    };
    Ok(FullWindow { items, carried })
}

/// Append the carried curated rows missing from `items`, returning their ids.
/// Fail-open: a failed probe leaves the window exactly as `get_items_tiered`
/// selected it.
fn merge_curated_carry(
    db: &Database,
    items: &mut Vec<StoredSourceItem>,
    signal: bool,
    cap: usize,
) -> HashSet<u64> {
    let present: HashSet<i64> = items.iter().map(|i| i.id).collect();
    let carried = curated_carry_ids(db, signal, cap).and_then(|ids| {
        let missing: Vec<i64> = ids.into_iter().filter(|id| !present.contains(id)).collect();
        load_items(db, &missing)
    });
    match carried {
        Ok(rows) => {
            if !rows.is_empty() {
                info!(
                    target: "4da::analysis",
                    window = items.len(),
                    carried = rows.len(),
                    "Curated rows outside the newest-N window carried into the full pass"
                );
            }
            let ids = rows.iter().map(|r| r.id as u64).collect();
            items.extend(rows);
            ids
        }
        Err(e) => {
            warn!(
                target: "4da::analysis",
                error = %e,
                "Curated carry probe failed — full pass scores the newest-N window only"
            );
            HashSet::new()
        }
    }
}

/// Ids of the curated rows to carry, in carry priority, at most `cap`.
fn curated_carry_ids(db: &Database, signal: bool, cap: usize) -> rusqlite::Result<Vec<i64>> {
    let tier_clause = if signal {
        String::new()
    } else {
        format!(" AND created_at >= datetime('now', '-{FREE_HISTORY_LIMIT_HOURS} hours')")
    };
    let sql = format!(
        "SELECT id, source_type,
                julianday('now') - julianday(COALESCE(published_at, created_at)),
                (cve_ids IS NOT NULL OR content_type = 'security_advisory')
         FROM source_items
         WHERE feed_relevant = 1{tier_clause} AND {enabled}
         ORDER BY COALESCE(published_at, created_at) DESC, id DESC",
        // A source the user turned off carries nothing (AD-054).
        enabled = crate::sources::source_class::enabled_source_sql("source_type"),
    );
    let rows: Vec<CarryRow> = {
        let conn = db.read_conn();
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map([], |r| {
            Ok(CarryRow {
                id: r.get(0)?,
                source_type: r.get(1)?,
                age_days: r.get(2)?,
                security: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
            })
        })?;
        mapped.collect::<rusqlite::Result<_>>()?
    };
    let mut kept: Vec<CarryRow> = rows.into_iter().filter(CarryRow::displayable).collect();
    // Stable: newest-first is kept within each group.
    kept.sort_by_key(|r| news_display_window_days(&r.source_type).is_some());
    Ok(kept.into_iter().take(cap).map(|r| r.id).collect())
}

/// Load full `StoredSourceItem`s for `ids`, in `ids` order (missing ids are
/// skipped).
fn load_items(db: &Database, ids: &[i64]) -> rusqlite::Result<Vec<StoredSourceItem>> {
    let conn = db.read_conn();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {} FROM source_items WHERE id = ?1",
        item_embeddings::stored_item_columns("")
    ))?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match stmt.query_row(params![id], item_embeddings::stored_item_from_row) {
            Ok(item) => out.push(item),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e),
        }
    }
    item_embeddings::attach_embeddings(&conn, &mut out)?;
    Ok(out)
}

#[cfg(test)]
#[path = "analysis_curated_carry_tests.rs"]
mod tests;
