// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Data loading pipeline for the content graph.

use rusqlite::params;
use tracing::debug;

use crate::db::blob_to_embedding;
use crate::error::Result;

use super::types::RawItem;

/// Shared selection predicate: corpus parity (curated at any window age, or
/// young not-yet-judged) minus items under an active snooze. Kept as one
/// fragment so the item query and the candidate count can never disagree.
const SELECTION_WHERE: &str = "si.relevance_score IS NOT NULL
           AND si.created_at >= datetime('now', ?1)
           AND si.embedding_status = 'complete'
           AND (si.feed_relevant IS 1
                OR (si.feed_relevant IS NULL
                    AND si.created_at >= datetime('now', '-2 days')))
           AND si.id NOT IN (SELECT source_item_id FROM snoozed_items
                             WHERE snooze_until > datetime('now'))";

/// Reserved slots for security items (osv/cve sources or a persisted
/// security_alert signal) among the loaded set. The load and every downstream
/// cap are relevance-first, and security advisories chronically score low —
/// live audit 2026-07-19: 130 CVE/OSV items in the window produced ONE
/// security node, leaving the category legend advertising a channel that
/// structurally could not appear. Reserved items are exempt from the story
/// cap and singleton cap (like curated items) but are never faked upward in
/// rank — they are the top-relevance items of their category, honestly low.
const RESERVE_SECURITY: usize = 12;
/// Reserved slots for research items (arxiv / papers_with_code), same logic.
const RESERVE_RESEARCH: usize = 8;

const SECURITY_PRED: &str =
    "(si.source_type IN ('osv','cve') OR si.signal_type = 'security_alert')";
const RESEARCH_PRED: &str = "si.source_type IN ('arxiv','papers_with_code')";

/// Everything the loader learned about the window, in one struct so callers
/// can't accidentally mix counts from different predicates.
pub(super) struct LoadedWindow {
    pub items: Vec<RawItem>,
    /// Total items matching the selection in this window, independent of the
    /// node budget — the UI states real coverage ("top N of M") from this.
    pub window_candidates: usize,
    /// True when a curated verdict exists older than the shortest window —
    /// i.e. the 7/14/30d toggle would actually show different graphs. While
    /// all verdicts are young the toggle is inert and the UI hides it
    /// (cold-start doctrine: no dead controls).
    pub windows_differ: bool,
}

pub(super) fn load_scored_items(
    conn: &rusqlite::Connection,
    days: u32,
    raw_limit: usize,
) -> Result<LoadedWindow> {
    // signal_type/signal_priority are plain columns on source_items (Phase 82).
    // The matched package comes from dep_linker's link table via a correlated
    // pick of the highest-confidence row (indexed on source_item_id).
    //
    // Corpus parity (W4-5, Phase 95): the graph shows what the CURRENT brain
    // stands behind, not a raw score ranking. Selection:
    // - feed_relevant = 1 — items the analysis run curated into the corpus
    //   (any age inside the window), ranked first;
    // - feed_relevant IS NULL AND younger than the re-scoring horizon —
    //   not yet judged, but fresh enough that the current pipeline will judge
    //   them shortly (honest interim; ramps to zero as verdicts accumulate).
    // Excluded: judged-and-rejected items (feed_relevant = 0), old
    // never-judged items — the latter carry stale-epoch scores no current
    // run stands behind (live 2026-07-19: war-news at a stale 0.94 while the
    // current pipeline scores fresh war news 0.07-0.40, relevant = false) —
    // and items under an active snooze (Phase 96: snooze is a real deferral,
    // not a write-only flag).
    let days_param = format!("-{days} days");

    let window_candidates: usize = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM source_items si WHERE {SELECTION_WHERE}"),
            params![days_param],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n.max(0) as usize)?;

    // The 7/14/30d windows can only differ where a curated verdict is older
    // than 7 days (unjudged items are window-independent by construction).
    let windows_differ: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items
             WHERE feed_relevant IS 1
               AND embedding_status = 'complete'
               AND created_at < datetime('now', '-7 days')
               AND created_at >= datetime('now', '-30 days')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)?;

    let select_columns = "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.embedding, si.signal_type, si.signal_priority,
                (SELECT sid.package_name FROM source_item_dependencies sid
                 WHERE sid.source_item_id = si.id
                 ORDER BY sid.confidence DESC, sid.id LIMIT 1) AS matched_package,
                (si.feed_relevant IS 1) AS curated
         FROM source_items si";

    let mut items = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let mut run_query = |sql: &str, limit: usize, items: &mut Vec<RawItem>| -> Result<()> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![days_param, limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(6)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)? as f32,
                row.get::<_, String>(5)?,
                embedding_blob,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, bool>(10)?,
            ))
        })?;
        for row in rows {
            let (
                id,
                title,
                url,
                source_type,
                score,
                created_at,
                embedding_blob,
                signal_type,
                signal_priority,
                matched_package,
                curated,
            ) = row?;
            if !seen.insert(id) {
                continue;
            }
            let embedding = blob_to_embedding(&embedding_blob);
            if embedding.is_empty() || embedding.iter().all(|&v| v == 0.0) {
                continue;
            }
            items.push(RawItem {
                id,
                title,
                url,
                source_type,
                relevance_score: score,
                signal_type,
                signal_priority,
                matched_package,
                created_at,
                curated,
                reserved: false,
                embedding,
            });
        }
        Ok(())
    };

    let main_sql = format!(
        "{select_columns}
         WHERE {SELECTION_WHERE}
         ORDER BY (si.feed_relevant IS 1) DESC, si.relevance_score DESC
         LIMIT ?2"
    );
    run_query(&main_sql, raw_limit, &mut items)?;

    // Stratified top-ups (P2.12): the best security and research items of the
    // window join the load even when relevance-first ordering would exclude
    // them. Dedup against the main set keeps totals honest.
    for (pred, reserve) in [
        (SECURITY_PRED, RESERVE_SECURITY),
        (RESEARCH_PRED, RESERVE_RESEARCH),
    ] {
        let quota_sql = format!(
            "{select_columns}
             WHERE {SELECTION_WHERE} AND {pred}
             ORDER BY si.relevance_score DESC
             LIMIT ?2"
        );
        run_query(&quota_sql, reserve, &mut items)?;
    }

    // Flag the quota AFTER all queries, over the merged set: per category,
    // the top-`reserve` items by relevance carry `reserved` — regardless of
    // which query loaded them — so the flagged count never exceeds intent
    // and always names the category's best items.
    let is_security = |i: &RawItem| {
        matches!(i.source_type.as_str(), "osv" | "cve")
            || i.signal_type.as_deref() == Some("security_alert")
    };
    let is_research = |i: &RawItem| matches!(i.source_type.as_str(), "arxiv" | "papers_with_code");
    for (pred, reserve) in [
        (&is_security as &dyn Fn(&RawItem) -> bool, RESERVE_SECURITY),
        (&is_research as &dyn Fn(&RawItem) -> bool, RESERVE_RESEARCH),
    ] {
        let mut idxs: Vec<usize> = (0..items.len()).filter(|&i| pred(&items[i])).collect();
        idxs.sort_by(|&a, &b| {
            items[b]
                .relevance_score
                .partial_cmp(&items[a].relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(items[a].id.cmp(&items[b].id))
        });
        for &i in idxs.iter().take(reserve) {
            items[i].reserved = true;
        }
    }

    debug!(
        target: "4da::content_graph",
        loaded = items.len(),
        window_candidates,
        windows_differ,
        days,
        "Loaded scored items for graph"
    );

    Ok(LoadedWindow {
        items,
        window_candidates,
        windows_differ,
    })
}
