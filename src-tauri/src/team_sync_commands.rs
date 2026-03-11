//! Tauri commands for team sync UI integration.
//!
//! Phase 4: Status, members, sharing commands.
//! Phase 5: Team creation, invite flow, key exchange.

use crate::error::Result;
use crate::team_sync;
use crate::team_sync_crypto::TeamCrypto;
use crate::team_sync_types::*;
use rusqlite::params;
use serde::Deserialize;
use tracing::{info, warn};

// ============================================================================
// Phase 4 — Query & Share Commands
// ============================================================================

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

    let op = TeamOp::ShareSignal {
        signal_id,
        chain_name,
        priority,
        tech_topics,
        suggested_action,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;
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

    let op = TeamOp::ProposeDecision {
        decision_id,
        title,
        decision_type,
        rationale,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;
    Ok(entry_id)
}

// ============================================================================
// Phase 5 — Team Creation & Invite Flow
// ============================================================================

/// Relay API response types for Phase 5 commands.
#[derive(Deserialize)]
struct RelayCreateTeamResponse {
    token: String,
    team_id: String,
}

#[derive(Deserialize)]
struct RelayInviteResponse {
    code: String,
    expires_at: String,
}

#[derive(Deserialize)]
struct RelayJoinResponse {
    token: String,
    team_id: String,
    role: String,
    admin_public_key: Vec<u8>,
}

/// Create a new team. Called by the first user (admin).
///
/// Flow:
/// 1. Generate X25519 keypair + team symmetric key
/// 2. POST to relay to create team + register as admin
/// 3. Store keypair + team key in local DB
/// 4. Update settings with team config
/// 5. Queue MemberJoined entry for sync
#[tauri::command]
pub async fn create_team(relay_url: String, display_name: String) -> Result<serde_json::Value> {
    // Generate cryptographic material
    let crypto = TeamCrypto::generate();
    let team_key = TeamCrypto::generate_team_key();
    let client_id = uuid::Uuid::new_v4().to_string();
    let team_id = uuid::Uuid::new_v4().to_string();

    // Call relay to create team
    let http = reqwest::Client::builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = format!("{}/teams", relay_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "team_id": team_id,
        "client_id": client_id,
        "display_name": display_name,
        "public_key": crypto.public_key_bytes().to_vec(),
        "license_key_hash": "", // Validated by Keygen separately
    });

    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach relay: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("Relay returned {status}: {body_text}").into());
    }

    let relay_resp: RelayCreateTeamResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid relay response: {e}"))?;

    // Store keypair + team key in local DB
    let conn = crate::state::open_db_connection()?;
    conn.execute(
        "INSERT OR REPLACE INTO team_crypto
            (team_id, our_public_key, our_private_key_enc, team_symmetric_key_enc)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            relay_resp.team_id,
            crypto.public_key_bytes().to_vec(),
            crypto.private_key_bytes().to_vec(),
            team_key.to_vec(),
        ],
    )
    .map_err(|e| format!("Failed to store crypto: {e}"))?;

    // Update settings with team relay config
    {
        let mut settings = crate::state::get_settings_manager().lock();
        let s = settings.get_mut();
        s.team_relay = Some(TeamRelayConfig {
            enabled: true,
            relay_url: Some(relay_url.clone()),
            auth_token: Some(relay_resp.token),
            team_id: Some(relay_resp.team_id.clone()),
            client_id: Some(client_id.clone()),
            display_name: Some(display_name.clone()),
            role: Some("admin".to_string()),
            sync_interval_secs: Some(30),
        });
        if let Err(e) = settings.save() {
            warn!(target: "4da::team_sync", error = %e, "Failed to save settings");
        }
    }

    // Queue MemberJoined entry so other members see us
    let hlc_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let _ = team_sync::queue_entry(
        &conn,
        &relay_resp.team_id,
        &client_id,
        hlc_ts,
        &TeamOp::MemberJoined {
            display_name: display_name.clone(),
            role: "admin".to_string(),
        },
    );

    // Also add ourselves to the local members cache
    conn.execute(
        "INSERT OR REPLACE INTO team_members_cache
            (team_id, client_id, display_name, role, last_seen)
         VALUES (?1, ?2, ?3, 'admin', datetime('now'))",
        params![relay_resp.team_id, client_id, display_name],
    )
    .map_err(|e| format!("Failed to cache member: {e}"))?;

    info!(target: "4da::team_sync", team_id = %relay_resp.team_id, "Team created successfully");

    Ok(serde_json::json!({
        "team_id": relay_resp.team_id,
        "client_id": client_id,
        "role": "admin",
    }))
}

/// Create an invite code for a new team member (admin only).
///
/// Calls the relay POST /teams/{team_id}/invites endpoint.
#[tauri::command]
pub async fn create_team_invite(
    role: Option<String>,
    email: Option<String>,
) -> Result<serde_json::Value> {
    let (relay_url, auth_token, team_id) = {
        let settings = crate::state::get_settings_manager().lock();
        let config = settings
            .get()
            .team_relay
            .as_ref()
            .ok_or("Team sync not configured")?;

        let r = config.role.as_deref().unwrap_or("member");
        if r != "admin" {
            return Err("Only admins can create invites".into());
        }

        (
            config.relay_url.clone().ok_or("No relay URL")?,
            config.auth_token.clone().ok_or("No auth token")?,
            config.team_id.clone().ok_or("No team ID")?,
        )
    };

    let http = reqwest::Client::builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = format!(
        "{}/teams/{}/invites",
        relay_url.trim_end_matches('/'),
        team_id
    );
    let body = serde_json::json!({
        "role": role.unwrap_or_else(|| "member".to_string()),
        "email": email,
    });

    let resp = http
        .post(&url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach relay: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("Relay returned {status}: {body_text}").into());
    }

    let invite: RelayInviteResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid relay response: {e}"))?;

    info!(target: "4da::team_sync", team_id = %team_id, "Invite code created");

    Ok(serde_json::json!({
        "code": invite.code,
        "expires_at": invite.expires_at,
    }))
}

/// Join a team via invite code.
///
/// Flow:
/// 1. Generate X25519 keypair
/// 2. POST to relay /auth/invite with invite code + public key
/// 3. Receive JWT, team_id, role, admin_public_key
/// 4. Store keypair in local DB (team key received later via DeliverTeamKey)
/// 5. Update settings with team config
/// 6. Queue MemberJoined entry
#[tauri::command]
pub async fn join_team_via_invite(
    relay_url: String,
    invite_code: String,
    display_name: String,
) -> Result<serde_json::Value> {
    // Generate our cryptographic identity
    let crypto = TeamCrypto::generate();
    let client_id = uuid::Uuid::new_v4().to_string();

    // Call relay to join via invite
    let http = reqwest::Client::builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = format!("{}/auth/invite", relay_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "invite_code": invite_code,
        "client_id": client_id,
        "display_name": display_name,
        "public_key": crypto.public_key_bytes().to_vec(),
    });

    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach relay: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("Relay returned {status}: {body_text}").into());
    }

    let join: RelayJoinResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid relay response: {e}"))?;

    // Store our keypair + admin public key in local DB.
    // Team key not available yet — it'll arrive via DeliverTeamKey sync entry.
    let conn = crate::state::open_db_connection()?;
    conn.execute(
        "INSERT OR REPLACE INTO team_crypto
            (team_id, our_public_key, our_private_key_enc, team_symmetric_key_enc)
         VALUES (?1, ?2, ?3, NULL)",
        params![
            join.team_id,
            crypto.public_key_bytes().to_vec(),
            crypto.private_key_bytes().to_vec(),
        ],
    )
    .map_err(|e| format!("Failed to store crypto: {e}"))?;

    // Store admin's public key for later team key decryption
    conn.execute(
        "INSERT OR REPLACE INTO team_members_cache
            (team_id, client_id, display_name, role, last_seen)
         VALUES (?1, 'admin_pubkey_ref', ?2, 'key_ref', datetime('now'))",
        params![join.team_id, hex::encode(&join.admin_public_key)],
    )
    .map_err(|e| format!("Failed to cache admin key: {e}"))?;

    // Update settings with team relay config
    {
        let mut settings = crate::state::get_settings_manager().lock();
        let s = settings.get_mut();
        s.team_relay = Some(TeamRelayConfig {
            enabled: true,
            relay_url: Some(relay_url),
            auth_token: Some(join.token),
            team_id: Some(join.team_id.clone()),
            client_id: Some(client_id.clone()),
            display_name: Some(display_name.clone()),
            role: Some(join.role.clone()),
            sync_interval_secs: Some(30),
        });
        if let Err(e) = settings.save() {
            warn!(target: "4da::team_sync", error = %e, "Failed to save settings");
        }
    }

    // Queue MemberJoined so the team sees us
    let hlc_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let _ = team_sync::queue_entry(
        &conn,
        &join.team_id,
        &client_id,
        hlc_ts,
        &TeamOp::MemberJoined {
            display_name: display_name.clone(),
            role: join.role.clone(),
        },
    );

    // Add ourselves to local cache
    conn.execute(
        "INSERT OR REPLACE INTO team_members_cache
            (team_id, client_id, display_name, role, last_seen)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![join.team_id, client_id, display_name, join.role],
    )
    .map_err(|e| format!("Failed to cache member: {e}"))?;

    info!(target: "4da::team_sync",
        team_id = %join.team_id,
        role = %join.role,
        "Joined team via invite — awaiting team key delivery"
    );

    Ok(serde_json::json!({
        "team_id": join.team_id,
        "client_id": client_id,
        "role": join.role,
        "awaiting_team_key": true,
    }))
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract team_id and client_id from settings, validating that team sync is
/// enabled and configured. Returns an error suitable for Tauri command results.
fn get_team_config() -> Result<(String, String)> {
    let settings = crate::state::get_settings_manager().lock();
    let config = settings
        .get()
        .team_relay
        .as_ref()
        .ok_or("Team sync not configured")?;

    if !config.enabled {
        return Err("Team sync is not enabled".into());
    }

    let team_id = config
        .team_id
        .as_ref()
        .ok_or("No team ID configured")?
        .clone();
    let client_id = config
        .client_id
        .as_ref()
        .ok_or("No client ID configured")?
        .clone();

    Ok((team_id, client_id))
}

/// Queue a TeamOp for outbound sync with the current HLC timestamp.
fn queue_team_op(team_id: &str, client_id: &str, op: &TeamOp) -> Result<String> {
    let hlc_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let conn = crate::state::open_db_connection()?;
    let entry_id =
        team_sync::queue_entry(&conn, team_id, client_id, hlc_ts, op).map_err(|e| e.to_string())?;

    Ok(entry_id)
}
