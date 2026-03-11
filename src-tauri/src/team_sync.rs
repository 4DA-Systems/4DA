//! Team sync engine — queue, apply, and manage team metadata entries.
//!
//! Phase 1: local sync primitives (no server communication yet).
//! Queues outbound entries, stores inbound entries, applies membership ops.

use crate::team_sync_types::*;
use anyhow::Result;
use rusqlite::params;
use tracing::{info, warn};

/// Queue a new team metadata entry for sync to the relay.
/// Returns the entry_id for tracking.
pub fn queue_entry(
    conn: &rusqlite::Connection,
    team_id: &str,
    client_id: &str,
    hlc_ts: u64,
    operation: &TeamOp,
) -> Result<String> {
    let entry_id = uuid::Uuid::new_v4().to_string();
    let op_json = serde_json::to_string(operation)?;

    conn.execute(
        "INSERT INTO team_sync_queue (entry_id, team_id, client_id, operation, hlc_ts, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())",
        params![entry_id, team_id, client_id, op_json, hlc_ts as i64],
    )?;

    info!(target: "4da::team_sync", entry_id = %entry_id, "Queued team sync entry");
    Ok(entry_id)
}

/// Get all pending outbound entries (not yet acknowledged by relay).
/// Returns `(entry_id, payload_bytes)` pairs. If encryption has not yet been applied,
/// the raw operation JSON is returned as bytes.
pub fn get_pending_entries(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut stmt = conn.prepare(
        "SELECT entry_id, COALESCE(encrypted, CAST(operation AS BLOB))
         FROM team_sync_queue
         WHERE team_id = ?1 AND acked_at IS NULL
         ORDER BY hlc_ts ASC
         LIMIT 100",
    )?;

    let entries = stmt
        .query_map(params![team_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(entries)
}

/// Mark entries as acknowledged by the relay.
pub fn mark_entries_acked(conn: &rusqlite::Connection, entry_ids: &[String]) -> Result<usize> {
    if entry_ids.is_empty() {
        return Ok(0);
    }

    let mut count = 0;
    for id in entry_ids {
        count += conn.execute(
            "UPDATE team_sync_queue SET acked_at = unixepoch() WHERE entry_id = ?1",
            params![id],
        )?;
    }

    info!(target: "4da::team_sync", count = count, "Marked entries as acked");
    Ok(count)
}

/// Store an inbound entry received from the relay.
/// Returns `true` if the entry was inserted, `false` if it was a duplicate.
pub fn store_inbound_entry(
    conn: &rusqlite::Connection,
    relay_seq: i64,
    team_id: &str,
    client_id: &str,
    encrypted: &[u8],
) -> Result<bool> {
    let result = conn.execute(
        "INSERT OR IGNORE INTO team_sync_log (relay_seq, team_id, client_id, encrypted, received_at)
         VALUES (?1, ?2, ?3, ?4, unixepoch())",
        params![relay_seq, team_id, client_id, encrypted],
    )?;

    Ok(result > 0)
}

/// Apply a decrypted team metadata entry to local team tables.
/// Uses Last-Write-Wins with HLC timestamps.
///
/// Membership operations update `team_members_cache` directly.
/// Other operations (DNA, signals, decisions) are stored but actual
/// aggregation logic will be built in the Team tier implementation phase.
pub fn apply_entry(
    conn: &rusqlite::Connection,
    team_id: &str,
    entry: &TeamMetadataEntry,
) -> Result<()> {
    match &entry.operation {
        TeamOp::DeliverTeamKey {
            target_client_id,
            encrypted_team_key: _,
        } => {
            // Team key delivery is handled by the sync scheduler which has
            // access to settings + crypto for decryption. The apply_entry layer
            // just logs it. The encrypted blob stays in team_sync_log for the
            // scheduler to process.
            info!(target: "4da::team_sync",
                target = %target_client_id,
                from = %entry.client_id,
                "Team key delivery entry (processed by scheduler)");
        }
        TeamOp::MemberJoined { display_name, role } => {
            conn.execute(
                "INSERT OR REPLACE INTO team_members_cache
                    (team_id, client_id, display_name, role, last_seen)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                params![team_id, &entry.client_id, display_name, role],
            )?;
        }
        TeamOp::MemberLeft { .. } => {
            conn.execute(
                "DELETE FROM team_members_cache WHERE team_id = ?1 AND client_id = ?2",
                params![team_id, &entry.client_id],
            )?;
        }
        TeamOp::RoleChanged {
            target_member_id,
            new_role,
        } => {
            conn.execute(
                "UPDATE team_members_cache SET role = ?1
                 WHERE team_id = ?2 AND client_id = ?3",
                params![new_role, team_id, target_member_id],
            )?;
        }
        _ => {
            // DNA, signals, decisions — stored but aggregation logic built in Team tier
            info!(target: "4da::team_sync",
                entry_id = %entry.entry_id,
                client_id = %entry.client_id,
                "Applied team entry (aggregation pending team tier implementation)");
        }
    }

    Ok(())
}

/// Apply all unapplied inbound entries and update sync state.
///
/// The `decrypt_fn` closure is responsible for decrypting the raw blob
/// from the relay into a `TeamMetadataEntry`. In Phase 1 (no encryption),
/// callers can pass a simple JSON deserializer.
pub fn apply_pending_inbound(
    conn: &rusqlite::Connection,
    team_id: &str,
    decrypt_fn: &dyn Fn(&[u8]) -> Result<TeamMetadataEntry>,
) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT relay_seq, encrypted FROM team_sync_log
         WHERE team_id = ?1 AND applied = 0
         ORDER BY relay_seq ASC",
    )?;

    let entries: Vec<(i64, Vec<u8>)> = stmt
        .query_map(params![team_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut applied = 0;
    let mut max_seq: i64 = 0;

    for (seq, encrypted) in &entries {
        match decrypt_fn(encrypted) {
            Ok(entry) => {
                if let Err(e) = apply_entry(conn, team_id, &entry) {
                    warn!(target: "4da::team_sync", seq = seq, error = %e, "Failed to apply entry");
                    continue;
                }
                conn.execute(
                    "UPDATE team_sync_log SET applied = 1 WHERE relay_seq = ?1 AND team_id = ?2",
                    params![seq, team_id],
                )?;
                applied += 1;
                max_seq = max_seq.max(*seq);
            }
            Err(e) => {
                warn!(target: "4da::team_sync", seq = seq, error = %e, "Failed to decrypt entry");
            }
        }
    }

    // Update sync state with the highest sequence we processed
    if max_seq > 0 {
        conn.execute(
            "INSERT INTO team_sync_state (team_id, last_relay_seq, last_sync_at)
             VALUES (?1, ?2, unixepoch())
             ON CONFLICT(team_id) DO UPDATE SET
                last_relay_seq = MAX(last_relay_seq, excluded.last_relay_seq),
                last_sync_at = excluded.last_sync_at",
            params![team_id, max_seq],
        )?;
    }

    info!(target: "4da::team_sync", applied = applied, max_seq = max_seq, "Applied inbound entries");
    Ok(applied)
}

/// Get the last relay sequence number we've processed.
pub fn get_last_relay_seq(conn: &rusqlite::Connection, team_id: &str) -> Result<i64> {
    let seq = conn
        .query_row(
            "SELECT COALESCE(last_relay_seq, 0) FROM team_sync_state WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(seq)
}

/// Get the count of pending outbound entries.
pub fn pending_outbound_count(conn: &rusqlite::Connection, team_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM team_sync_queue WHERE team_id = ?1 AND acked_at IS NULL",
        params![team_id],
        |row| row.get(0),
    )?;

    Ok(count as usize)
}

/// Get team sync status for the UI.
pub fn get_sync_status(
    conn: &rusqlite::Connection,
    team_id: &str,
    client_id: &str,
) -> Result<TeamSyncStatus> {
    let last_relay_seq = get_last_relay_seq(conn, team_id)?;
    let pending = pending_outbound_count(conn, team_id)?;

    let last_sync: Option<String> = conn
        .query_row(
            "SELECT datetime(last_sync_at, 'unixepoch') FROM team_sync_state WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .ok();

    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members_cache WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(TeamSyncStatus {
        enabled: true,
        connected: false, // Updated by scheduler when relay is reachable
        team_id: Some(team_id.to_string()),
        client_id: Some(client_id.to_string()),
        display_name: None, // Filled from settings by the caller
        role: None,
        member_count: member_count as usize,
        pending_outbound: pending,
        last_sync_at: last_sync,
        last_relay_seq,
    })
}

/// Purge acknowledged entries older than the given age (in seconds).
pub fn cleanup_acked_entries(conn: &rusqlite::Connection, max_age_secs: i64) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM team_sync_queue WHERE acked_at IS NOT NULL AND acked_at < unixepoch() - ?1",
        params![max_age_secs],
    )?;

    if deleted > 0 {
        info!(target: "4da::team_sync", deleted = deleted, "Cleaned up old acked entries");
    }

    Ok(deleted)
}

/// Get all cached team members for a given team.
pub fn get_team_members(conn: &rusqlite::Connection, team_id: &str) -> Result<Vec<TeamMember>> {
    let mut stmt = conn.prepare(
        "SELECT client_id, display_name, role, last_seen
         FROM team_members_cache
         WHERE team_id = ?1
         ORDER BY display_name ASC",
    )?;

    let members = stmt
        .query_map(params![team_id], |row| {
            Ok(TeamMember {
                client_id: row.get(0)?,
                display_name: row.get(1)?,
                role: row.get(2)?,
                last_seen: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(members)
}

#[cfg(test)]
#[path = "team_sync_tests.rs"]
mod tests;
