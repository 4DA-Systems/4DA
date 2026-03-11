//! Relay error types.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum RelayError {
    Auth(String),
    NotFound(String),
    Database(String),
    BadRequest(String),
    #[allow(dead_code)]
    Internal(String),
}

impl std::fmt::Display for RelayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(msg) => write!(f, "auth: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Database(msg) => write!(f, "database: {msg}"),
            Self::BadRequest(msg) => write!(f, "bad request: {msg}"),
            Self::Internal(msg) => write!(f, "internal: {msg}"),
        }
    }
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            Self::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let json = serde_json::json!({ "error": body });
        (status, axum::Json(json)).into_response()
    }
}

impl From<sqlx::Error> for RelayError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for RelayError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        Self::Auth(e.to_string())
    }
}
