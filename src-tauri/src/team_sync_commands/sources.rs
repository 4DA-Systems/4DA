// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 6 — Shared source management commands.

use super::get_team_config;
use crate::audit::log_team_audit;
use crate::error::Result;
use crate::team_sync_types::*;
use rusqlite::params;
use tracing::info;

/// Share a content source with the team.
///
/// Inserts into shared_resources table and queues a TeamOp::ShareSource for sync.
/// The `visible_to` JSON field stores upvoter IDs (initially empty).
#[tauri::command]
pub async fn share_source_with_team(
    source_type: String,
    config_summary: String,
    recommendation: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    // Clone for audit + DB insert before move into op
    let audit_source_type = source_type.clone();
    let db_source_type = source_type.clone();
    let db_config_summary = config_summary.clone();
    let db_recommendation = recommendation.clone();
    let db_client_id = client_id.clone();
    let db_team_id = team_id.clone();

    let op = TeamOp::ShareSource {
        source_type,
        config_summary,
        recommendation,
    };

    let entry_id = super::queue_team_op(&team_id, &client_id, &op)?;

    // Also insert directly into shared_resources for immediate local visibility
    let resource_id = uuid::Uuid::new_v4().to_string();
    let resource_data = serde_json::json!({
        "source_type": db_source_type,
        "config_summary": db_config_summary,
        "recommendation": db_recommendation,
    });

    let conn = crate::state::open_db_connection()?;
    conn.execute(
        "INSERT INTO shared_resources
            (id, team_id, resource_type, resource_data, shared_by, visibility, visible_to, created_at, expires_at)
         VALUES (?1, ?2, 'source', ?3, ?4, 'team', '[]', datetime('now'), NULL)",
        params![resource_id, db_team_id, resource_data.to_string(), db_client_id],
    )
    .map_err(|e| format!("Failed to insert shared source: {e}"))?;

    // Audit: source shared
    log_team_audit(
        &conn,
        "source.shared",
        "source",
        Some(&resource_id),
        Some(&serde_json::json!({ "source_type": audit_source_type })),
    );

    info!(target: "4da::team_sync",
        entry_id = %entry_id,
        resource_id = %resource_id,
        source_type = %audit_source_type,
        "Source shared with team"
    );

    Ok(resource_id)
}

/// Get all sources shared within the team.
///
/// Queries shared_resources where resource_type = 'source', parses the
/// resource_data JSON, and counts upvotes from the visible_to array.
#[tauri::command]
pub async fn get_team_sources() -> Result<Vec<SharedSource>> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let mut stmt = conn
        .prepare(
            "SELECT id, team_id, resource_data, shared_by, visible_to, created_at
             FROM shared_resources
             WHERE team_id = ?1 AND resource_type = 'source'
             ORDER BY created_at DESC",
        )
        .map_err(|e| format!("Failed to prepare team sources query: {e}"))?;

    let sources = stmt
        .query_map(params![team_id], |row| {
            let id: String = row.get(0)?;
            let team_id: String = row.get(1)?;
            let resource_data_str: String = row.get(2)?;
            let shared_by: String = row.get(3)?;
            let visible_to_str: String = row.get(4)?;
            let created_at: String = row.get(5)?;

            // Parse resource_data JSON
            let resource_data: serde_json::Value =
                serde_json::from_str(&resource_data_str).unwrap_or_default();

            let source_type = resource_data["source_type"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let config_summary = resource_data["config_summary"]
                .as_str()
                .unwrap_or("{}")
                .to_string();
            let recommendation = resource_data["recommendation"]
                .as_str()
                .unwrap_or("")
                .to_string();

            // Count upvotes from visible_to array
            let upvotes: u32 = serde_json::from_str::<Vec<String>>(&visible_to_str)
                .map(|v| v.len() as u32)
                .unwrap_or(0);

            Ok(SharedSource {
                id,
                team_id,
                source_type,
                config_summary,
                recommendation,
                shared_by,
                upvotes,
                created_at,
            })
        })
        .map_err(|e| format!("Failed to query team sources: {e}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read team sources: {e}"))?;

    Ok(sources)
}

/// Upvote a shared source. Adds current client_id to the visible_to array
/// if not already present (lightweight voting via JSON array, no extra table).
#[tauri::command]
pub async fn upvote_team_source(source_id: String) -> Result<()> {
    let (team_id, client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    // Get current visible_to JSON array
    let visible_to_str: String = conn
        .query_row(
            "SELECT visible_to FROM shared_resources WHERE id = ?1 AND team_id = ?2",
            params![source_id, team_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Source not found: {e}"))?;

    let mut voters: Vec<String> = serde_json::from_str(&visible_to_str).unwrap_or_default();

    // Add client_id if not already present
    if voters.contains(&client_id) {
        return Ok(()); // Already upvoted, idempotent
    }
    voters.push(client_id.clone());

    let updated_json =
        serde_json::to_string(&voters).map_err(|e| format!("Failed to serialize voters: {e}"))?;

    conn.execute(
        "UPDATE shared_resources SET visible_to = ?1 WHERE id = ?2 AND team_id = ?3",
        params![updated_json, source_id, team_id],
    )
    .map_err(|e| format!("Failed to update upvote: {e}"))?;

    // Audit: source upvoted
    log_team_audit(&conn, "source.upvoted", "source", Some(&source_id), None);

    info!(target: "4da::team_sync", source_id = %source_id, "Source upvoted");
    Ok(())
}

/// Remove a shared source from the team.
#[tauri::command]
pub async fn remove_team_source(source_id: String) -> Result<()> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let deleted = conn
        .execute(
            "DELETE FROM shared_resources WHERE id = ?1 AND team_id = ?2",
            params![source_id, team_id],
        )
        .map_err(|e| format!("Failed to remove shared source: {e}"))?;

    if deleted == 0 {
        return Err("Source not found or not in this team".into());
    }

    // Audit: source removed
    log_team_audit(&conn, "source.removed", "source", Some(&source_id), None);

    info!(target: "4da::team_sync", source_id = %source_id, "Shared source removed");
    Ok(())
}
