// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Speculative embedding — pre-embeds content the user is likely to engage with.
//!
//! When the user clicks or saves an item, we find related un-embedded items from
//! the same source and embed them in the background. This makes subsequent searches
//! instant because the embedding work was already done.

use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

static WORKER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Trigger speculative embedding after a user engagement event.
/// Finds un-embedded or stale items related to the engaged item's source
/// and embeds them in the background.
pub(crate) async fn on_engagement(item_id: i64) {
    if WORKER_ACTIVE.swap(true, Ordering::SeqCst) {
        return; // Another speculative embed is running
    }

    let result = tokio::task::spawn(async move {
        if let Err(e) = run_speculative_batch(item_id).await {
            warn!(target: "4da::speculative", error = %e, "Speculative embedding batch failed");
        }
        WORKER_ACTIVE.store(false, Ordering::Relaxed);
    });

    // Fire and forget — don't block the engagement handler
    drop(result);
}

async fn run_speculative_batch(trigger_item_id: i64) -> crate::error::Result<()> {
    let db = crate::get_database()?;

    // Find items from the same source type that lack embeddings (all-zero vectors)
    let candidates: Vec<(i64, String, String)> = {
        let conn = db.read_conn();
        let mut stmt = conn
            .prepare(
                "SELECT si.id, si.title, COALESCE(si.content, '')
             FROM source_items si
             JOIN source_items trigger_item ON trigger_item.id = ?1
             WHERE si.source_type = trigger_item.source_type
               AND si.id != ?1
               AND (si.embedding IS NULL OR LENGTH(si.embedding) = 0
                    OR si.embedding = ZEROBLOB(1536))
             ORDER BY si.created_at DESC
             LIMIT 16",
            )
            .map_err(|e| crate::error::FourDaError::Internal(format!("speculative query: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![trigger_item_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1).unwrap_or_default(),
                    row.get::<_, String>(2).unwrap_or_default(),
                ))
            })
            .map_err(|e| crate::error::FourDaError::Internal(format!("speculative query: {e}")))?
            .flatten()
            .collect::<Vec<_>>();
        rows
    };

    if candidates.is_empty() {
        debug!(target: "4da::speculative", "No un-embedded items to speculate on");
        return Ok(());
    }

    info!(
        target: "4da::speculative",
        count = candidates.len(),
        trigger = trigger_item_id,
        "Speculative embedding: pre-embedding related items"
    );

    // Embed in batches
    let texts: Vec<String> = candidates
        .iter()
        .map(|(_, title, content)| {
            if content.is_empty() {
                title.clone()
            } else {
                format!("{} {}", title, &content[..content.floor_char_boundary(500)])
            }
        })
        .collect();

    let embeddings = crate::embed_texts(&texts).await?;

    // Store embeddings back
    let conn = db.conn.lock();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| crate::error::FourDaError::Internal(format!("speculative tx: {e}")))?;

    for ((id, _, _), embedding) in candidates.iter().zip(embeddings.iter()) {
        let blob = crate::db::embedding_to_blob(embedding);
        let _ = tx.execute(
            "UPDATE source_items SET embedding = ?1 WHERE id = ?2",
            rusqlite::params![blob, id],
        );
        let _ = tx.execute(
            "INSERT OR REPLACE INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![id, blob],
        );
    }
    tx.commit()
        .map_err(|e| crate::error::FourDaError::Internal(format!("speculative commit: {e}")))?;

    debug!(
        target: "4da::speculative",
        embedded = candidates.len(),
        "Speculative embedding batch complete"
    );

    Ok(())
}
