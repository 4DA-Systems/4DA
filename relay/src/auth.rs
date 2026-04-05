//! JWT authentication for relay API.

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::RelayError;

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

        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("FATAL: JWT_SECRET environment variable must be set");

        let token_data = decode::<TeamClaims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| RelayError::Auth(format!("Invalid token: {e}")))?;

        Ok(AuthTeam(token_data.claims))
    }
}

/// Issue a JWT for a team member.
pub fn issue_token(team_id: &str, client_id: &str, role: &str) -> Result<String, RelayError> {
    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("FATAL: JWT_SECRET environment variable must be set");

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
