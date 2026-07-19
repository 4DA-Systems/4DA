// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Data loading pipeline for the content graph.

use rusqlite::params;
use tracing::debug;

use crate::db::blob_to_embedding;
use crate::error::Result;

use super::types::RawItem;

pub(super) fn load_scored_items(
    conn: &rusqlite::Connection,
    days: u32,
    max_nodes: usize,
) -> Result<Vec<RawItem>> {
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
    // Excluded: judged-and-rejected items (feed_relevant = 0) and old
    // never-judged items — the latter carry stale-epoch scores no current
    // run stands behind (live 2026-07-19: war-news at a stale 0.94 while the
    // current pipeline scores fresh war news 0.07-0.40, relevant = false).
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.embedding, si.signal_type, si.signal_priority,
                (SELECT sid.package_name FROM source_item_dependencies sid
                 WHERE sid.source_item_id = si.id
                 ORDER BY sid.confidence DESC, sid.id LIMIT 1) AS matched_package,
                (si.feed_relevant IS 1) AS curated
         FROM source_items si
         WHERE si.relevance_score IS NOT NULL
           AND si.created_at >= datetime('now', ?1)
           AND si.embedding_status = 'complete'
           AND (si.feed_relevant IS 1
                OR (si.feed_relevant IS NULL
                    AND si.created_at >= datetime('now', '-2 days')))
         ORDER BY (si.feed_relevant IS 1) DESC, si.relevance_score DESC
         LIMIT ?2",
    )?;

    let days_param = format!("-{days} days");
    let rows = stmt.query_map(params![days_param, max_nodes as i64], |row| {
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

    let mut items = Vec::new();
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
            embedding,
        });
    }

    debug!(
        target: "4da::content_graph",
        loaded = items.len(),
        days,
        "Loaded scored items for graph"
    );

    Ok(items)
}
