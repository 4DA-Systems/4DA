// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 5 — Team creation, invite flow, key exchange.

use crate::audit::{log_audit, log_team_audit, AuditLogParams};
use crate::error::Result;
use crate::team_sync;
use crate::team_sync_crypto::TeamCrypto;
use crate::team_sync_types::*;
use rusqlite::params;
use serde::Deserialize;
use tracing::{info, warn};

// ============================================================================
// Relay API response types
// ============================================================================

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

// ============================================================================
// Commands
// ============================================================================

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
    // Team relays are DELIBERATELY allowed to target private/internal IPs
    // (self-hosted relays on corporate networks are a first-class supported
    // use case). We therefore use the weaker `validate_url_input` (scheme +
    // length + control-char checks) rather than `validate_url_safe_for_request`
    // (which blocks RFC1918). This is a documented tradeoff; the user
    // explicitly opts in by typing a private URL. See SECURITY.md and
    // docs/ADVERSARIAL-AUDIT-2026-04-19.md P2.
    let relay_url = crate::ipc_guard::validate_url_input("relay_url", &relay_url)?;
    let display_name = crate::ipc_guard::validate_length(
        "display_name",
        &display_name,
        crate::ipc_guard::MAX_INPUT_LENGTH,
    )?;
    // Generate cryptographic material
    let crypto = TeamCrypto::generate();
    let team_key = TeamCrypto::generate_team_key();
    let client_id = uuid::Uuid::new_v4().to_string();
    let team_id = uuid::Uuid::new_v4().to_string();

    // Call relay to create team
    let http = crate::http_client::TEAM_CLIENT.clone();

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

    // Store keypair + team key. The X25519 private key and the team symmetric
    // key are both written to the OS keychain first (see Wave 16 in the
    // war-room notes). The DB columns remain as a fallback for hosts without
    // a reliable keychain; on success the DB gets a zero-length BLOB as a
    // migrated sentinel. See the `team_sync_crypto::persist_*` helpers for
    // the write-then-read-back verify that prevents silent loss on keyring
    // backends that lie about the write.
    let conn = crate::state::open_db_connection()?;
    let priv_db = crate::team_sync_crypto::persist_team_private_key(
        &relay_resp.team_id,
        &crypto.private_key_bytes(),
    );
    let team_key_db =
        crate::team_sync_crypto::persist_team_symmetric_key(&relay_resp.team_id, &team_key);
    conn.execute(
        "INSERT OR REPLACE INTO team_crypto
            (team_id, our_public_key, our_private_key_enc, team_symmetric_key_enc)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            relay_resp.team_id,
            crypto.public_key_bytes().to_vec(),
            priv_db,
            team_key_db,
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

    // Audit: team created (use log_audit directly -- settings weren't configured yet during creation)
    log_audit(&AuditLogParams {
        conn: &conn,
        team_id: &relay_resp.team_id,
        actor_id: &client_id,
        actor_display_name: &display_name,
        action: "team.created",
        resource_type: "team",
        resource_id: Some(&relay_resp.team_id),
        details: None,
    });

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

    let http = crate::http_client::TEAM_CLIENT.clone();

    let url = format!(
        "{}/teams/{}/invites",
        relay_url.trim_end_matches('/'),
        team_id
    );
    let invite_role = role.unwrap_or_else(|| "member".to_string());
    let body = serde_json::json!({
        "role": invite_role,
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

    // Audit: member invited
    if let Ok(conn) = crate::state::open_db_connection() {
        log_team_audit(
            &conn,
            "member.invited",
            "invite",
            None,
            Some(&serde_json::json!({ "role": invite_role })),
        );
    }

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
    // Private/internal relay URLs are allowed here for the same reason
    // as `create_team`: self-hosted relays are supported. See the note
    // on `create_team` above.
    let relay_url = crate::ipc_guard::validate_url_input("relay_url", &relay_url)?;
    let invite_code = crate::ipc_guard::validate_length(
        "invite_code",
        &invite_code,
        crate::ipc_guard::MAX_INPUT_LENGTH,
    )?;
    let display_name = crate::ipc_guard::validate_length(
        "display_name",
        &display_name,
        crate::ipc_guard::MAX_INPUT_LENGTH,
    )?;
    // Generate our cryptographic identity
    let crypto = TeamCrypto::generate();
    let client_id = uuid::Uuid::new_v4().to_string();

    // Call relay to join via invite
    let http = crate::http_client::TEAM_CLIENT.clone();

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

    // Store our keypair + admin public key. Team key is not available yet --
    // it arrives later via DeliverTeamKey. The X25519 private key is written
    // to the OS keychain with a write-then-read-back verify; the DB column
    // gets a zero-length BLOB as the migrated sentinel, or the plaintext as
    // a fallback when the keychain is unavailable.
    let conn = crate::state::open_db_connection()?;
    let priv_db = crate::team_sync_crypto::persist_team_private_key(
        &join.team_id,
        &crypto.private_key_bytes(),
    );
    conn.execute(
        "INSERT OR REPLACE INTO team_crypto
            (team_id, our_public_key, our_private_key_enc, team_symmetric_key_enc)
         VALUES (?1, ?2, ?3, NULL)",
        params![join.team_id, crypto.public_key_bytes().to_vec(), priv_db,],
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

    // Audit: member joined via invite
    log_team_audit(
        &conn,
        "member.joined",
        "member",
        None,
        Some(&serde_json::json!({ "role": join.role })),
    );

    info!(target: "4da::team_sync",
        team_id = %join.team_id,
        role = %join.role,
        "Joined team via invite -- awaiting team key delivery"
    );

    Ok(serde_json::json!({
        "team_id": join.team_id,
        "client_id": client_id,
        "role": join.role,
        "awaiting_team_key": true,
    }))
}
