// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Persistence for derived signal-chain snapshots.

use rusqlite::params;

use crate::error::Result;

use super::{predict_chain_lifecycle, signal_type_rank, SignalChain, SIGNAL_CHAIN_WINDOW_DAYS};

pub(super) fn record_signal_chain_events(
    conn: &rusqlite::Connection,
    chains: &[SignalChain],
) -> Result<usize> {
    ensure_signal_chain_event_table(conn)?;

    // Signal chains are derived current-state snapshots. Rewriting this event
    // type keeps MCP/export readers from seeing stale chains after a topic fades
    // or after stricter grounding removes a false positive.
    conn.execute(
        "DELETE FROM temporal_events WHERE event_type = 'signal_chain'",
        [],
    )?;

    let expires_at =
        (chrono::Utc::now() + chrono::Duration::days(SIGNAL_CHAIN_WINDOW_DAYS)).to_rfc3339();
    let mut persisted = 0_usize;
    for chain in chains {
        let prediction = predict_chain_lifecycle(chain);
        let source_item_ids: Vec<i64> =
            chain.links.iter().map(|link| link.source_item_id).collect();
        let data = serde_json::json!({
            "schema_version": 1,
            "chain": chain,
            "prediction": prediction,
            "source_item_ids": source_item_ids,
        });
        let data_str = serde_json::to_string(&data)?;
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
             VALUES ('signal_chain', ?1, ?2, ?3, ?4)",
            params![
                chain.chain_name,
                data_str,
                primary_source_item_id(chain),
                expires_at
            ],
        )?;
        persisted += 1;
    }

    Ok(persisted)
}

fn ensure_signal_chain_event_table(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS temporal_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data JSON NOT NULL,
            embedding BLOB,
            source_item_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_temporal_type_time ON temporal_events(event_type, created_at);
        CREATE INDEX IF NOT EXISTS idx_temporal_subject ON temporal_events(subject);
        CREATE INDEX IF NOT EXISTS idx_temporal_expires ON temporal_events(expires_at);",
    )?;
    Ok(())
}

fn primary_source_item_id(chain: &SignalChain) -> Option<i64> {
    chain
        .links
        .iter()
        .min_by(|a, b| {
            signal_type_rank(&a.signal_type)
                .cmp(&signal_type_rank(&b.signal_type))
                .then(a.timestamp.cmp(&b.timestamp))
                .then(a.source_item_id.cmp(&b.source_item_id))
        })
        .map(|link| link.source_item_id)
}
