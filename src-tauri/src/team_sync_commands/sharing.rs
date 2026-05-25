// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 4 — Query & share commands: status, members, DNA, signals, decisions.

use super::{get_team_config, queue_team_op};
use crate::audit::log_team_audit;
use crate::error::Result;
use crate::team_sync;
use crate::team_sync_types::*;
use tracing::info;

/// Get team sync status for the UI.
#[tauri::command]
pub async fn get_team_sync_status() -> Result<TeamSyncStatus> {
    let settings = crate::state::get_settings_manager().lock();
    let relay_config = settings.get().team_relay.as_ref();

    match relay_config {
        Some(config) if config.enabled && config.team_id.is_some() => {
            let team_id = config.team_id.clone().unwrap_or_default();
            let client_id = config.client_id.clone().unwrap_or_default();
            let display_name = config.display_name.clone();
            let role = config.role.clone();
            // Drop settings lock before DB access (lock ordering: SETTINGS < DATABASE)
            drop(settings);

            let conn = crate::state::open_db_connection()?;
            let mut status = team_sync::get_sync_status(&conn, &team_id, &client_id)
                .map_err(|e| format!("Failed to get sync status: {e}"))?;
            status.display_name = display_name;
            status.role = role;
            Ok(status)
        }
        _ => {
            drop(settings);
            Ok(TeamSyncStatus {
                enabled: false,
                connected: false,
                team_id: None,
                client_id: None,
                display_name: None,
                role: None,
                member_count: 0,
                pending_outbound: 0,
                last_sync_at: None,
                last_relay_seq: 0,
            })
        }
    }
}

/// Get list of team members (from local cache).
#[tauri::command]
pub async fn get_team_members() -> Result<Vec<TeamMember>> {
    let team_id = {
        let settings = crate::state::get_settings_manager().lock();
        settings
            .get()
            .team_relay
            .as_ref()
            .and_then(|c| c.team_id.clone())
            .unwrap_or_default()
    };

    if team_id.is_empty() {
        return Ok(vec![]);
    }

    let conn = crate::state::open_db_connection()?;
    team_sync::get_team_members(&conn, &team_id).map_err(|e| e.to_string().into())
}

/// Queue a DNA summary for team sharing.
#[tauri::command]
pub async fn share_dna_with_team(
    primary_stack: Vec<String>,
    interests: Vec<String>,
    blind_spots: Vec<String>,
    identity_summary: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    let op = TeamOp::ShareDnaSummary {
        primary_stack,
        interests,
        blind_spots,
        identity_summary,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

    // Audit: DNA shared with team
    if let Ok(conn) = crate::state::open_db_connection() {
        log_team_audit(&conn, "dna.shared", "dna", None, None);
    }

    info!(target: "4da::team_sync", entry_id = %entry_id, "DNA summary queued for team sync");
    Ok(entry_id)
}

/// Queue a signal chain for team sharing.
#[tauri::command]
pub async fn share_signal_with_team(
    signal_id: String,
    chain_name: String,
    priority: String,
    tech_topics: Vec<String>,
    suggested_action: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    // Clone for audit before move into op
    let audit_signal_id = signal_id.clone();
    let audit_chain_name = chain_name.clone();

    let op = TeamOp::ShareSignal {
        signal_id,
        chain_name,
        priority,
        tech_topics,
        suggested_action,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

    // Audit: signal shared with team
    if let Ok(conn) = crate::state::open_db_connection() {
        log_team_audit(
            &conn,
            "signal.shared",
            "signal",
            None,
            Some(&serde_json::json!({
                "signal_id": audit_signal_id,
                "chain_name": audit_chain_name,
            })),
        );
    }

    Ok(entry_id)
}

/// Queue a decision proposal for team sharing.
#[tauri::command]
pub async fn propose_team_decision(
    decision_id: String,
    title: String,
    decision_type: String,
    rationale: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    // Clone for audit before move into op
    let audit_title = title.clone();

    let op = TeamOp::ProposeDecision {
        decision_id,
        title,
        decision_type,
        rationale,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

    // Audit: decision proposed
    if let Ok(conn) = crate::state::open_db_connection() {
        log_team_audit(
            &conn,
            "decision.proposed",
            "decision",
            None,
            Some(&serde_json::json!({ "title": audit_title })),
        );
    }

    Ok(entry_id)
}
