//! Sync entry endpoints — push and pull encrypted blobs.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::AuthTeam;
use crate::error::RelayError;

#[derive(Deserialize)]
pub struct PushEntry {
    #[allow(dead_code)] // Deserialized but identity comes from JWT claims
    pub client_id: String,
    pub payload: Vec<u8>,
}

#[derive(Serialize)]
pub struct PushResponse {
    pub relay_seq: i64,
}

#[derive(Deserialize)]
pub struct PullQuery {
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct SyncEntry {
    pub relay_seq: i64,
    pub client_id: String,
    pub payload: Vec<u8>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct PullResponse {
    pub entries: Vec<SyncEntry>,
    pub has_more: bool,
}

/// POST /teams/{team_id}/entries -- push an encrypted sync entry.
pub async fn push_entry(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
    Json(body): Json<PushEntry>,
) -> Result<Json<PushResponse>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }
    let claims = crate::auth::verify_membership(&pool, &claims).await?;

    if body.payload.is_empty() {
        return Err(RelayError::BadRequest("Empty payload".to_string()));
    }

    // Max payload size: 64KB (metadata should be tiny)
    if body.payload.len() > 65536 {
        return Err(RelayError::BadRequest(
            "Payload too large (max 64KB)".to_string(),
        ));
    }

    let result = sqlx::query_scalar::<_, i64>(
        "INSERT INTO sync_entries (team_id, client_id, payload)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(&team_id)
    .bind(&claims.client_id)
    .bind(&body.payload)
    .fetch_one(&pool)
    .await?;

    // Update client last_seen
    sqlx::query(
        "UPDATE team_clients SET last_seen = datetime('now')
         WHERE team_id = $1 AND client_id = $2",
    )
    .bind(&team_id)
    .bind(&claims.client_id)
    .execute(&pool)
    .await
    .ok(); // Non-critical -- don't fail the push

    tracing::info!(target: "relay::entries", team_id = %team_id, seq = result, "Entry stored");

    Ok(Json(PushResponse { relay_seq: result }))
}

/// GET /teams/{team_id}/entries?since=N&limit=N -- pull entries since a sequence.
pub async fn pull_entries(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
    Query(query): Query<PullQuery>,
) -> Result<Json<PullResponse>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }
    let _claims = crate::auth::verify_membership(&pool, &claims).await?;

    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(200).min(1000);

    let entries = sqlx::query_as::<_, (i64, String, Vec<u8>, String)>(
        "SELECT id, client_id, payload, created_at
         FROM sync_entries
         WHERE team_id = $1 AND id > $2
         ORDER BY id ASC
         LIMIT $3",
    )
    .bind(&team_id)
    .bind(since)
    .bind(limit + 1) // Fetch one extra to detect has_more
    .fetch_all(&pool)
    .await?;

    let has_more = entries.len() as i64 > limit;
    let entries: Vec<SyncEntry> = entries
        .into_iter()
        .take(limit as usize)
        .map(|(id, client_id, payload, created_at)| SyncEntry {
            relay_seq: id,
            client_id,
            payload,
            created_at,
        })
        .collect();

    Ok(Json(PullResponse { entries, has_more }))
}
