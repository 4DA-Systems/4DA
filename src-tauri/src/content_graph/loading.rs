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
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.embedding
         FROM source_items si
         WHERE si.relevance_score IS NOT NULL
           AND si.created_at >= datetime('now', ?1)
           AND si.embedding_status = 'complete'
         ORDER BY si.relevance_score DESC
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
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, title, url, source_type, score, created_at, embedding_blob) = row?;
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
            created_at,
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
