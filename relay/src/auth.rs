//! JWT authentication for relay API.

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::RelayError;

/// Read the JWT secret from the environment.
/// Returns an error instead of panicking so callers can produce proper HTTP responses.
fn jwt_secret() -> Result<String, RelayError> {
    std::env::var("JWT_SECRET")
        .map_err(|_| RelayError::Internal("JWT_SECRET environment variable not set".to_string()))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamClaims {
    pub team_id: String,
    pub client_id: String,
    pub role: String,
    pub exp: usize,
}

/// Extract authenticated team claims from the Authorization header.
pub struct AuthTeam(pub TeamClaims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthTeam
where
    S: Send + Sync,
{
    type Rejection = RelayError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| RelayError::Auth("Missing Authorization header".to_string()))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| RelayError::Auth("Invalid Authorization format".to_string()))?;

        let jwt_secret = jwt_secret()?;

        let token_data = decode::<TeamClaims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| RelayError::Auth(format!("Invalid token: {e}")))?;

        Ok(AuthTeam(token_data.claims))
    }
}

/// Verify that the authenticated client is still a member of their claimed team
/// and that their role matches the current DB state.
/// Call this in any handler that needs fresh membership verification.
pub async fn verify_membership(
    pool: &sqlx::SqlitePool,
    claims: &TeamClaims,
) -> Result<TeamClaims, RelayError> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT role FROM team_clients WHERE team_id = $1 AND client_id = $2",
    )
    .bind(&claims.team_id)
    .bind(&claims.client_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((current_role,)) => Ok(TeamClaims {
            team_id: claims.team_id.clone(),
            client_id: claims.client_id.clone(),
            role: current_role,
            exp: claims.exp,
        }),
        None => Err(RelayError::Auth("Member no longer in team".to_string())),
    }
}

/// Issue a JWT for a team member.
pub fn issue_token(team_id: &str, client_id: &str, role: &str) -> Result<String, RelayError> {
    let jwt_secret = jwt_secret()?;

    let claims = TeamClaims {
        team_id: team_id.to_string(),
        client_id: client_id.to_string(),
        role: role.to_string(),
        exp: (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )?;

    Ok(token)
}
