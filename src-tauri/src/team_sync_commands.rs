//! Tauri commands for team sync UI integration.
//!
//! Phase 4: Status, members, sharing commands.
//! Phase 5: Team creation, invite flow, key exchange.

use crate::audit::{log_audit, log_team_audit, AuditLogParams};
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

// ============================================================================
// Phase 4b — Decision Voting & Retrieval
// ============================================================================

/// Vote on an existing team decision. Queues the vote for sync and records
/// it locally for immediate UI display.
#[tauri::command]
pub async fn vote_on_decision(
    decision_id: String,
    stance: String,
    rationale: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    // Clone for audit + local insert before move into op
    let audit_decision_id = decision_id.clone();
    let local_decision_id = decision_id.clone();
    let local_client_id = client_id.clone();
    let local_stance = stance.clone();
    let local_rationale = rationale.clone();

    let op = TeamOp::VoteOnDecision {
        decision_id,
        stance,
        rationale,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

    // Also record the vote locally for immediate visibility
    if let Ok(conn) = crate::state::open_db_connection() {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO decision_votes (decision_id, voter_id, stance, rationale, voted_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![local_decision_id, local_client_id, local_stance, local_rationale],
        );

        // Audit: decision voted
        log_team_audit(
            &conn,
            "decision.voted",
            "decision",
            Some(&audit_decision_id),
            Some(&serde_json::json!({ "stance": local_stance })),
        );
    }

    info!(target: "4da::team_sync", entry_id = %entry_id, "Decision vote queued for team sync");
    Ok(entry_id)
}

/// Get team decisions for the current team, optionally filtered by status.
#[tauri::command]
pub async fn get_team_decisions(status_filter: Option<String>) -> Result<Vec<TeamDecision>> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let (query, use_status_filter) = match &status_filter {
        Some(_) => (
            "SELECT d.id, d.team_id, d.title, d.decision_type, d.rationale,
                    d.proposed_by, d.status, d.created_at, d.resolved_at,
                    (SELECT COUNT(*) FROM decision_votes v WHERE v.decision_id = d.id) as vote_count
             FROM team_decisions d
             WHERE d.team_id = ?1 AND d.status = ?2
             ORDER BY d.created_at DESC",
            true,
        ),
        None => (
            "SELECT d.id, d.team_id, d.title, d.decision_type, d.rationale,
                    d.proposed_by, d.status, d.created_at, d.resolved_at,
                    (SELECT COUNT(*) FROM decision_votes v WHERE v.decision_id = d.id) as vote_count
             FROM team_decisions d
             WHERE d.team_id = ?1
             ORDER BY d.created_at DESC",
            false,
        ),
    };

    let mut stmt = conn
        .prepare(query)
        .map_err(|e| format!("Failed to prepare query: {e}"))?;

    let row_mapper = |row: &rusqlite::Row| {
        Ok(TeamDecision {
            id: row.get(0)?,
            team_id: row.get(1)?,
            title: row.get(2)?,
            decision_type: row.get(3)?,
            rationale: row.get(4)?,
            proposed_by: row.get(5)?,
            status: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8)?,
            vote_count: row.get(9)?,
        })
    };

    let decisions: Vec<TeamDecision> = if use_status_filter {
        let status = status_filter.as_deref().unwrap_or_default();
        stmt.query_map(params![team_id, status], row_mapper)
            .map_err(|e| format!("Failed to query decisions: {e}"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read decisions: {e}"))?
    } else {
        stmt.query_map(params![team_id], row_mapper)
            .map_err(|e| format!("Failed to query decisions: {e}"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read decisions: {e}"))?
    };

    Ok(decisions)
}

/// Get full detail of a single decision including all votes.
#[tauri::command]
pub async fn get_decision_detail(decision_id: String) -> Result<DecisionDetail> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    // Get the decision itself
    let detail = conn
        .query_row(
            "SELECT id, team_id, title, decision_type, rationale, proposed_by, status, created_at, resolved_at
             FROM team_decisions
             WHERE id = ?1 AND team_id = ?2",
            params![decision_id, team_id],
            |row| {
                Ok(DecisionDetail {
                    id: row.get(0)?,
                    team_id: row.get(1)?,
                    title: row.get(2)?,
                    decision_type: row.get(3)?,
                    rationale: row.get(4)?,
                    proposed_by: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    resolved_at: row.get(8)?,
                    votes: vec![], // Populated below
                })
            },
        )
        .map_err(|e| format!("Decision not found: {e}"))?;

    // Get all votes for this decision
    let mut stmt = conn
        .prepare(
            "SELECT voter_id, stance, rationale, voted_at
             FROM decision_votes
             WHERE decision_id = ?1
             ORDER BY voted_at ASC",
        )
        .map_err(|e| format!("Failed to prepare votes query: {e}"))?;

    let votes = stmt
        .query_map(params![decision_id], |row| {
            Ok(DecisionVote {
                voter_id: row.get(0)?,
                stance: row.get(1)?,
                rationale: row.get(2)?,
                voted_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to query votes: {e}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read votes: {e}"))?;

    Ok(DecisionDetail { votes, ..detail })
}

/// Resolve a team decision (accept or reject). Admin/proposer action.
#[tauri::command]
pub async fn resolve_decision(decision_id: String, new_status: String) -> Result<()> {
    if new_status != "accepted" && new_status != "rejected" {
        return Err("Status must be 'accepted' or 'rejected'".into());
    }

    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let updated = conn
        .execute(
            "UPDATE team_decisions SET status = ?1, resolved_at = datetime('now')
             WHERE id = ?2 AND team_id = ?3",
            params![new_status, decision_id, team_id],
        )
        .map_err(|e| format!("Failed to resolve decision: {e}"))?;

    if updated == 0 {
        return Err("Decision not found or not in this team".into());
    }

    // Audit: decision resolved
    log_team_audit(
        &conn,
        "decision.resolved",
        "decision",
        Some(&decision_id),
        Some(&serde_json::json!({ "new_status": new_status })),
    );

    info!(target: "4da::team_sync", decision_id = %decision_id, status = %new_status, "Decision resolved");
    Ok(())
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

    // Audit: team created (use log_audit directly — settings weren't configured yet during creation)
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
// Phase 6 — Shared Source Management
// ============================================================================

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

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

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
