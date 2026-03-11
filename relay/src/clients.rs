//! Team client management — registration, listing, invites.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::{issue_token, AuthTeam};
use crate::error::RelayError;

#[derive(Serialize)]
pub struct ClientInfo {
    pub client_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub public_key: Option<Vec<u8>>,
    pub last_seen: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterClient {
    pub client_id: String,
    pub display_name: String,
    pub public_key: Vec<u8>,
}

#[derive(Deserialize)]
pub struct CreateTeamRequest {
    pub team_id: String,
    pub client_id: String,
    pub display_name: String,
    pub public_key: Vec<u8>,
    pub license_key_hash: String,
}

#[derive(Serialize)]
pub struct CreateTeamResponse {
    pub token: String,
    pub team_id: String,
}

#[derive(Deserialize)]
pub struct CreateInviteRequest {
    pub email: Option<String>,
    pub role: String,
}

#[derive(Serialize)]
pub struct InviteResponse {
    pub code: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct JoinTeamRequest {
    pub invite_code: String,
    pub client_id: String,
    pub display_name: String,
    pub public_key: Vec<u8>,
}

#[derive(Serialize)]
pub struct JoinTeamResponse {
    pub token: String,
    pub team_id: String,
    pub role: String,
    pub admin_public_key: Vec<u8>,
}

/// POST /teams -- create a new team (admin).
pub async fn create_team(
    State(pool): State<SqlitePool>,
    Json(body): Json<CreateTeamRequest>,
) -> Result<Json<CreateTeamResponse>, RelayError> {
    // Register the admin as the first client
    sqlx::query(
        "INSERT INTO team_clients (team_id, client_id, public_key, display_name, role)
         VALUES ($1, $2, $3, $4, 'admin')",
    )
    .bind(&body.team_id)
    .bind(&body.client_id)
    .bind(&body.public_key)
    .bind(&body.display_name)
    .execute(&pool)
    .await?;

    let token = issue_token(&body.team_id, &body.client_id, "admin")?;

    tracing::info!(target: "relay::clients", team_id = %body.team_id, "Team created");

    Ok(Json(CreateTeamResponse {
        token,
        team_id: body.team_id,
    }))
}

/// GET /teams/{team_id}/clients -- list team members.
pub async fn list_clients(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<ClientInfo>>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }

    let clients = sqlx::query_as::<_, (String, Option<String>, String, Option<Vec<u8>>, Option<String>)>(
        "SELECT client_id, display_name, role, public_key, last_seen
         FROM team_clients
         WHERE team_id = $1
         ORDER BY role DESC, display_name ASC",
    )
    .bind(&team_id)
    .fetch_all(&pool)
    .await?;

    let clients: Vec<ClientInfo> = clients
        .into_iter()
        .map(
            |(client_id, display_name, role, public_key, last_seen)| ClientInfo {
                client_id,
                display_name,
                role,
                public_key,
                last_seen,
            },
        )
        .collect();

    Ok(Json(clients))
}

/// POST /teams/{team_id}/clients -- register a new client (via invite).
pub async fn register_client(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
    Json(body): Json<RegisterClient>,
) -> Result<Json<serde_json::Value>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }

    sqlx::query(
        "INSERT OR REPLACE INTO team_clients (team_id, client_id, public_key, display_name, role)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&team_id)
    .bind(&body.client_id)
    .bind(&body.public_key)
    .bind(&body.display_name)
    .bind(&claims.role)
    .execute(&pool)
    .await?;

    tracing::info!(target: "relay::clients", team_id = %team_id, client_id = %body.client_id, "Client registered");

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /teams/{team_id}/invites -- generate an invite code (admin only).
pub async fn create_invite(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
    Json(body): Json<CreateInviteRequest>,
) -> Result<Json<InviteResponse>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }

    if claims.role != "admin" {
        return Err(RelayError::Auth(
            "Only admins can create invites".to_string(),
        ));
    }

    let code = format!(
        "4DA-TEAM-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .get(..16)
            .unwrap_or("unknown")
    );
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(72))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    sqlx::query(
        "INSERT INTO team_invites (code, team_id, email, role, created_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&code)
    .bind(&team_id)
    .bind(&body.email)
    .bind(&body.role)
    .bind(&claims.client_id)
    .bind(&expires_at)
    .execute(&pool)
    .await?;

    tracing::info!(target: "relay::clients", team_id = %team_id, "Invite created");

    Ok(Json(InviteResponse { code, expires_at }))
}

/// POST /auth/invite -- join a team via invite code.
pub async fn join_via_invite(
    State(pool): State<SqlitePool>,
    Json(body): Json<JoinTeamRequest>,
) -> Result<Json<JoinTeamResponse>, RelayError> {
    // Validate invite
    let invite = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT team_id, role, used_at FROM team_invites
         WHERE code = $1 AND expires_at > datetime('now')",
    )
    .bind(&body.invite_code)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| RelayError::NotFound("Invalid or expired invite code".to_string()))?;

    let (team_id, role, used_at) = invite;

    if used_at.is_some() {
        return Err(RelayError::BadRequest(
            "Invite code already used".to_string(),
        ));
    }

    // Mark invite as used
    sqlx::query("UPDATE team_invites SET used_at = datetime('now'), used_by = $1 WHERE code = $2")
        .bind(&body.client_id)
        .bind(&body.invite_code)
        .execute(&pool)
        .await?;

    // Register the new member
    sqlx::query(
        "INSERT INTO team_clients (team_id, client_id, public_key, display_name, role)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&team_id)
    .bind(&body.client_id)
    .bind(&body.public_key)
    .bind(&body.display_name)
    .bind(&role)
    .execute(&pool)
    .await?;

    // Get admin's public key for key exchange
    let admin_pk = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT public_key FROM team_clients WHERE team_id = $1 AND role = 'admin' LIMIT 1",
    )
    .bind(&team_id)
    .fetch_one(&pool)
    .await?;

    // Issue JWT
    let token = issue_token(&team_id, &body.client_id, &role)?;

    tracing::info!(target: "relay::clients", team_id = %team_id, client_id = %body.client_id, "Member joined via invite");

    Ok(Json(JoinTeamResponse {
        token,
        team_id,
        role,
        admin_public_key: admin_pk,
    }))
}
