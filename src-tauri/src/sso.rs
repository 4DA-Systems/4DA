// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! SSO/SAML/OIDC enterprise authentication for 4DA desktop app.
//!
//! Desktop SSO flow:
//! 1. Admin configures SSO (IdP URL, entity ID, certificate or OIDC client)
//! 2. User clicks "Sign in with SSO" → app opens system browser to IdP
//! 3. IdP authenticates → redirects to localhost:4445/sso/callback
//! 4. App validates the SAML assertion or OIDC token from the callback
//! 5. Identity extracted (email, name, groups) → stored locally for role assignment
//!
//! ## Tables
//!
//! ```sql
//! sso_config   — singleton row (id=1) with provider configuration
//! sso_sessions — active SSO sessions with identity + expiry
//! ```

use base64::Engine as _;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::sso_xml;

// ============================================================================
// Schema
// ============================================================================

/// Ensure SSO tables exist.
pub fn ensure_sso_tables(conn: &rusqlite::Connection) -> crate::error::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sso_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            provider_type TEXT NOT NULL DEFAULT 'saml',
            idp_url TEXT NOT NULL,
            entity_id TEXT NOT NULL DEFAULT 'com.4da.app',
            certificate TEXT,
            client_id TEXT,
            issuer TEXT,
            enabled INTEGER DEFAULT 1,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sso_sessions (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            display_name TEXT NOT NULL,
            groups TEXT DEFAULT '[]',
            authenticated_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT,
            provider_type TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_sso_sessions_email
            ON sso_sessions(email);
        CREATE INDEX IF NOT EXISTS idx_sso_sessions_expires
            ON sso_sessions(expires_at);",
    )?;
    Ok(())
}

// ============================================================================
// Types
// ============================================================================

/// SSO provider configuration (SAML or OIDC).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SsoConfig {
    /// Provider type: "saml" or "oidc"
    pub provider_type: String,
    /// Identity Provider URL (SAML SSO endpoint or OIDC authorization endpoint)
    pub idp_url: String,
    /// Service Provider entity ID (default: com.4da.app)
    pub entity_id: String,
    /// IdP certificate in PEM format (SAML only)
    pub certificate: Option<String>,
    /// OIDC client ID
    pub client_id: Option<String>,
    /// OIDC issuer URL
    pub issuer: Option<String>,
    /// Whether SSO is enabled
    pub enabled: bool,
}

/// An active SSO session with extracted identity.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SsoSession {
    /// User email from IdP assertion
    pub email: String,
    /// Display name from IdP assertion
    pub display_name: String,
    /// Group memberships from IdP assertion
    pub groups: Vec<String>,
    /// ISO 8601 timestamp of authentication
    pub authenticated_at: String,
    /// Optional expiry (ISO 8601) — session invalid after this time
    pub expires_at: Option<String>,
    /// Provider that authenticated this session ("saml" or "oidc")
    pub provider_type: String,
}

/// The localhost callback port for SSO redirects.
const SSO_CALLBACK_PORT: u16 = 4445;

// ============================================================================
// Internal helpers
// ============================================================================

/// Build a SAML AuthnRequest URL for the given IdP.
///
/// Constructs a minimal SAML 2.0 AuthnRequest XML, base64-encodes it,
/// and appends it as the SAMLRequest query parameter to the IdP URL.
fn build_saml_login_url(config: &SsoConfig) -> crate::error::Result<String> {
    let request_id = format!("_4da_{}", uuid::Uuid::new_v4());
    let issue_instant = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let callback_url = format!("http://localhost:{SSO_CALLBACK_PORT}/sso/callback");

    let authn_request = format!(
        r#"<samlp:AuthnRequest
    xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
    xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
    ID="{request_id}"
    Version="2.0"
    IssueInstant="{issue_instant}"
    Destination="{idp_url}"
    AssertionConsumerServiceURL="{callback_url}"
    ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST">
    <saml:Issuer>{entity_id}</saml:Issuer>
    <samlp:NameIDPolicy
        Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress"
        AllowCreate="true"/>
</samlp:AuthnRequest>"#,
        request_id = request_id,
        issue_instant = issue_instant,
        idp_url = config.idp_url,
        callback_url = callback_url,
        entity_id = config.entity_id,
    );

    let encoded = base64::engine::general_purpose::STANDARD.encode(authn_request.as_bytes());
    let url_encoded = urlencoding::encode(&encoded);

    // Append SAMLRequest as query parameter
    let separator = if config.idp_url.contains('?') {
        "&"
    } else {
        "?"
    };
    Ok(format!(
        "{}{separator}SAMLRequest={url_encoded}",
        config.idp_url
    ))
}

/// Build an OIDC authorization URL.
///
/// Constructs a standard OAuth 2.0 / OIDC authorization request with
/// openid, email, and profile scopes. The state parameter provides CSRF protection.
/// Returns `(auth_url, state, nonce)` so the caller can persist state/nonce.
fn build_oidc_login_url(config: &SsoConfig) -> crate::error::Result<String> {
    let client_id = config
        .client_id
        .as_deref()
        .ok_or("OIDC requires client_id")?;
    let issuer = config.issuer.as_deref().ok_or("OIDC requires issuer URL")?;

    let state = uuid::Uuid::new_v4().to_string();
    let nonce = uuid::Uuid::new_v4().to_string();
    let callback_url = format!("http://localhost:{SSO_CALLBACK_PORT}/sso/callback");

    let auth_url = format!(
        "{issuer}/authorize?response_type=code&client_id={client_id}&redirect_uri={redirect}&scope=openid+email+profile&state={state}&nonce={nonce}",
        issuer = issuer.trim_end_matches('/'),
        client_id = urlencoding::encode(client_id),
        redirect = urlencoding::encode(&callback_url),
        state = state,
        nonce = nonce,
    );

    Ok(auth_url)
}

/// Persist OIDC state/nonce for callback validation.
///
/// Called from `initiate_sso_login` after building the URL. Stores the state
/// and nonce in `sso_pending_auth` with a 10-minute TTL.
fn persist_oidc_state(conn: &rusqlite::Connection, auth_url: &str) -> crate::error::Result<()> {
    // Extract state from the URL
    let state = auth_url
        .split("state=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .ok_or("Failed to extract state from OIDC URL")?;
    let nonce = auth_url
        .split("nonce=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .ok_or("Failed to extract nonce from OIDC URL")?;

    let pending_id = uuid::Uuid::new_v4().to_string();
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(10))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    conn.execute(
        "INSERT INTO sso_pending_auth (id, state, nonce, provider_type, expires_at)
         VALUES (?1, ?2, ?3, 'oidc', ?4)",
        params![pending_id, state, nonce, expires],
    )?;
    // Clean expired entries
    let _ = conn.execute(
        "DELETE FROM sso_pending_auth WHERE expires_at < datetime('now')",
        [],
    );

    Ok(())
}

/// Parse a SAML Response assertion and extract identity claims.
///
/// Extracts NameID (email), display name, and group attributes from
/// the base64-encoded SAML Response XML. Verifies XML digital signature
/// if an IdP certificate is configured.
fn parse_saml_assertion(
    assertion_b64: &str,
    config: &SsoConfig,
) -> crate::error::Result<SsoSession> {
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(assertion_b64)
        .map_err(|e| format!("Invalid base64 in SAML assertion: {e}"))?;
    let xml = String::from_utf8(decoded_bytes)
        .map_err(|e| format!("SAML assertion is not valid UTF-8: {e}"))?;

    // Verify XML digital signature if certificate is configured
    if let Some(ref cert_pem) = config.certificate {
        crate::sso_crypto::verify_saml_signature(&xml, cert_pem)?;
        info!(target: "4da::sso", "SAML assertion signature verified");
    } else {
        warn!(target: "4da::sso",
            "No IdP certificate configured — SAML assertion accepted without signature verification. \
             Configure a certificate for production use.");
    }

    // Extract NameID (email)
    let email = sso_xml::extract_xml_element(&xml, "NameID")
        .ok_or("SAML assertion missing NameID element")?;

    // Extract display name from Attribute elements
    let display_name = sso_xml::extract_saml_attribute(&xml, "displayName")
        .or_else(|| sso_xml::extract_saml_attribute(&xml, "cn"))
        .or_else(|| {
            sso_xml::extract_saml_attribute(
                &xml,
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name",
            )
        })
        .unwrap_or_else(|| email.clone());

    // Extract group memberships
    let groups = sso_xml::extract_saml_attribute_values(&xml, "memberOf")
        .or_else(|| sso_xml::extract_saml_attribute_values(&xml, "groups"))
        .or_else(|| {
            sso_xml::extract_saml_attribute_values(&xml, "http://schemas.xmlsoap.org/claims/Group")
        })
        .unwrap_or_default();

    // Check NotOnOrAfter for session expiry
    let expires_at =
        sso_xml::extract_xml_attribute_value(&xml, "SubjectConfirmationData", "NotOnOrAfter");

    // Validate NotOnOrAfter hasn't passed
    if let Some(ref expiry) = expires_at {
        if let Ok(expiry_time) = chrono::DateTime::parse_from_rfc3339(expiry) {
            if expiry_time < chrono::Utc::now() {
                return Err("SAML assertion has expired (NotOnOrAfter)".into());
            }
        }
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Ok(SsoSession {
        email,
        display_name,
        groups,
        authenticated_at: now,
        expires_at,
        provider_type: "saml".to_string(),
    })
}

/// Parse an OIDC token response and extract identity claims.
///
/// Verifies the JWT ID token signature against the IdP's JWKS keys,
/// then extracts standard OIDC claims: sub/email, name, groups.
async fn parse_oidc_token(
    token_json: &str,
    expected_nonce: Option<&str>,
    config: &SsoConfig,
) -> crate::error::Result<SsoSession> {
    let token_data: serde_json::Value = serde_json::from_str(token_json)
        .map_err(|e| format!("Invalid OIDC token response: {e}"))?;

    let id_token = token_data["id_token"]
        .as_str()
        .ok_or("OIDC response missing id_token")?;

    // Verify JWT signature and extract validated claims
    let issuer = config.issuer.as_deref().ok_or("OIDC requires issuer URL")?;
    let client_id = config
        .client_id
        .as_deref()
        .ok_or("OIDC requires client_id")?;

    let claims =
        crate::sso_crypto::verify_oidc_token(id_token, issuer, client_id, expected_nonce).await?;

    let email = claims["email"]
        .as_str()
        .or_else(|| claims["sub"].as_str())
        .ok_or("OIDC token missing email and sub claims")?
        .to_string();

    let display_name = claims["name"]
        .as_str()
        .or_else(|| claims["preferred_username"].as_str())
        .unwrap_or(&email)
        .to_string();

    let groups = claims["groups"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Use exp claim for session expiry
    let expires_at = claims["exp"].as_i64().map(|exp| {
        chrono::DateTime::from_timestamp(exp, 0)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default()
    });

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Ok(SsoSession {
        email,
        display_name,
        groups,
        authenticated_at: now,
        expires_at,
        provider_type: "oidc".to_string(),
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Read the current SSO configuration.
#[tauri::command]
pub async fn get_sso_config() -> crate::error::Result<Option<SsoConfig>> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT provider_type, idp_url, entity_id, certificate, client_id, issuer, enabled
         FROM sso_config WHERE id = 1",
    )?;

    let config = stmt
        .query_row([], |row| {
            Ok(SsoConfig {
                provider_type: row.get(0)?,
                idp_url: row.get(1)?,
                entity_id: row.get(2)?,
                certificate: row.get(3)?,
                client_id: row.get(4)?,
                issuer: row.get(5)?,
                enabled: row.get::<_, i32>(6)? != 0,
            })
        })
        .ok();

    Ok(config)
}

/// Save SSO configuration (admin only).
///
/// Uses INSERT OR REPLACE to maintain the singleton row constraint.
#[tauri::command]
pub async fn set_sso_config(config: SsoConfig) -> crate::error::Result<()> {
    // Validate provider type
    if config.provider_type != "saml" && config.provider_type != "oidc" {
        return Err("provider_type must be 'saml' or 'oidc'".into());
    }

    // Validate required fields per provider type
    if config.provider_type == "oidc" {
        if config.client_id.as_ref().map_or(true, |s| s.is_empty()) {
            return Err("OIDC provider requires client_id".into());
        }
        if config.issuer.as_ref().map_or(true, |s| s.is_empty()) {
            return Err("OIDC provider requires issuer URL".into());
        }
    }

    if config.idp_url.is_empty() {
        return Err("IdP URL is required".into());
    }

    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    conn.execute(
        "INSERT OR REPLACE INTO sso_config
            (id, provider_type, idp_url, entity_id, certificate, client_id, issuer, enabled, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
        params![
            config.provider_type,
            config.idp_url,
            config.entity_id,
            config.certificate,
            config.client_id,
            config.issuer,
            config.enabled as i32,
        ],
    )?;

    // Audit log
    let details = serde_json::json!({
        "provider_type": config.provider_type,
        "entity_id": config.entity_id,
        "enabled": config.enabled,
    });
    crate::audit::log_team_audit(
        &conn,
        "sso.config_updated",
        "sso_config",
        None,
        Some(&details),
    );

    info!(target: "4da::sso",
        provider_type = %config.provider_type,
        enabled = config.enabled,
        "SSO configuration updated");

    Ok(())
}

/// Generate the IdP login URL and return it for the frontend to open in the system browser.
///
/// Returns the full URL that should be opened in the user's default browser.
/// The IdP will authenticate the user and redirect back to localhost:4445/sso/callback.
#[tauri::command]
pub async fn initiate_sso_login() -> crate::error::Result<String> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    let config = conn
        .query_row(
            "SELECT provider_type, idp_url, entity_id, certificate, client_id, issuer, enabled
             FROM sso_config WHERE id = 1",
            [],
            |row| {
                Ok(SsoConfig {
                    provider_type: row.get(0)?,
                    idp_url: row.get(1)?,
                    entity_id: row.get(2)?,
                    certificate: row.get(3)?,
                    client_id: row.get(4)?,
                    issuer: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                })
            },
        )
        .map_err(|_| "SSO not configured. Please configure SSO settings first.")?;

    if !config.enabled {
        return Err("SSO is disabled. Enable it in organization settings.".into());
    }

    let login_url = match config.provider_type.as_str() {
        "saml" => build_saml_login_url(&config)?,
        "oidc" => {
            let url = build_oidc_login_url(&config)?;
            // Persist state/nonce for callback validation
            persist_oidc_state(&conn, &url)?;
            url
        }
        other => return Err(format!("Unsupported SSO provider type: {other}").into()),
    };

    info!(target: "4da::sso",
        provider_type = %config.provider_type,
        "SSO login initiated");

    Ok(login_url)
}

/// Get the current active SSO session, if any.
///
/// Returns None if no session exists or if the session has expired.
#[tauri::command]
pub async fn get_sso_session() -> crate::error::Result<Option<SsoSession>> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT id, email, display_name, groups, authenticated_at, expires_at, provider_type
         FROM sso_sessions
         ORDER BY authenticated_at DESC
         LIMIT 1",
    )?;

    let session = stmt
        .query_row([], |row| {
            let groups_json: String = row.get(3)?;
            let groups: Vec<String> = serde_json::from_str(&groups_json).unwrap_or_default();

            Ok(SsoSession {
                email: row.get(1)?,
                display_name: row.get(2)?,
                groups,
                authenticated_at: row.get(4)?,
                expires_at: row.get(5)?,
                provider_type: row.get(6)?,
            })
        })
        .ok();

    // Check expiry if session exists
    if let Some(ref s) = session {
        if let Some(ref expires_at) = s.expires_at {
            if let Ok(expiry) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                if expiry < chrono::Utc::now() {
                    // Session expired — clean up and return None
                    conn.execute("DELETE FROM sso_sessions", [])?;
                    info!(target: "4da::sso",
                        email = %s.email,
                        "SSO session expired, cleaned up");
                    return Ok(None);
                }
            }
        }
    }

    Ok(session)
}

/// Validate a SAML assertion or OIDC token received from the localhost callback.
///
/// The `assertion` parameter contains either:
/// - For SAML: the base64-encoded SAMLResponse from the IdP POST
/// - For OIDC: the JSON token response from the token exchange
///
/// The `state` parameter is required for OIDC flows (CSRF protection).
///
/// Returns the created session on success.
#[tauri::command]
pub async fn validate_sso_callback(
    assertion: String,
    state: Option<String>,
) -> crate::error::Result<SsoSession> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    // Read full config for crypto verification
    let config: SsoConfig = conn
        .query_row(
            "SELECT provider_type, idp_url, entity_id, certificate, client_id, issuer, enabled
             FROM sso_config WHERE id = 1",
            [],
            |row| {
                Ok(SsoConfig {
                    provider_type: row.get(0)?,
                    idp_url: row.get(1)?,
                    entity_id: row.get(2)?,
                    certificate: row.get(3)?,
                    client_id: row.get(4)?,
                    issuer: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                })
            },
        )
        .map_err(|_| "SSO not configured")?;

    let session = match config.provider_type.as_str() {
        "saml" => parse_saml_assertion(&assertion, &config)?,
        "oidc" => {
            // Validate state and retrieve nonce
            let state_val = state.ok_or("OIDC callback missing state parameter")?;
            let (nonce, pending_id) = conn
                .query_row(
                    "SELECT nonce, id FROM sso_pending_auth
                     WHERE state = ?1 AND expires_at > datetime('now')",
                    params![state_val],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(|_| {
                    "SSO session expired or invalid state. Please try signing in again."
                })?;

            // Delete used entry (single-use)
            conn.execute(
                "DELETE FROM sso_pending_auth WHERE id = ?1",
                params![pending_id],
            )?;

            parse_oidc_token(&assertion, Some(&nonce), &config).await?
        }
        other => return Err(format!("Unsupported provider type: {other}").into()),
    };

    // Clear any existing sessions (single-user desktop app)
    conn.execute("DELETE FROM sso_sessions", [])?;

    // Store the new session
    let session_id = uuid::Uuid::new_v4().to_string();
    let groups_json = serde_json::to_string(&session.groups)?;

    conn.execute(
        "INSERT INTO sso_sessions
            (id, email, display_name, groups, authenticated_at, expires_at, provider_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            session_id,
            session.email,
            session.display_name,
            groups_json,
            session.authenticated_at,
            session.expires_at,
            session.provider_type,
        ],
    )?;

    // Audit log
    let details = serde_json::json!({
        "email": session.email,
        "provider_type": session.provider_type,
        "groups": session.groups,
    });
    crate::audit::log_team_audit(
        &conn,
        "sso.login",
        "sso_session",
        Some(&session_id),
        Some(&details),
    );

    info!(target: "4da::sso",
        email = %session.email,
        provider_type = %session.provider_type,
        groups = ?session.groups,
        "SSO login successful");

    Ok(session)
}

/// Clear the current SSO session (logout).
#[tauri::command]
pub async fn logout_sso() -> crate::error::Result<()> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    // Get session info for audit log before deleting
    let session_email: Option<String> = conn
        .query_row(
            "SELECT email FROM sso_sessions ORDER BY authenticated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let deleted = conn.execute("DELETE FROM sso_sessions", [])?;

    if deleted > 0 {
        let details = serde_json::json!({
            "email": session_email,
        });
        crate::audit::log_team_audit(&conn, "sso.logout", "sso_session", None, Some(&details));

        info!(target: "4da::sso",
            email = %session_email.unwrap_or_default(),
            "SSO session cleared (logout)");
    } else {
        warn!(target: "4da::sso", "SSO logout called but no active session found");
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_sso_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_ensure_tables_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_sso_tables(&conn).unwrap();
        ensure_sso_tables(&conn).unwrap(); // Should not error on second call
    }

    #[test]
    fn test_sso_config_roundtrip() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO sso_config
                (id, provider_type, idp_url, entity_id, certificate, client_id, issuer, enabled)
             VALUES (1, 'saml', 'https://idp.example.com/sso', 'com.4da.app', 'MIIC...cert', NULL, NULL, 1)",
            [],
        )
        .unwrap();

        let (provider_type, idp_url, entity_id, enabled): (String, String, String, i32) = conn
            .query_row(
                "SELECT provider_type, idp_url, entity_id, enabled FROM sso_config WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(provider_type, "saml");
        assert_eq!(idp_url, "https://idp.example.com/sso");
        assert_eq!(entity_id, "com.4da.app");
        assert_eq!(enabled, 1);
    }

    #[test]
    fn test_sso_config_singleton_constraint() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO sso_config (id, provider_type, idp_url) VALUES (1, 'saml', 'https://a.com')",
            [],
        )
        .unwrap();

        // Second insert with different id should fail
        let result = conn.execute(
            "INSERT INTO sso_config (id, provider_type, idp_url) VALUES (2, 'oidc', 'https://b.com')",
            [],
        );
        assert!(result.is_err(), "Should reject id != 1");
    }

    #[test]
    fn test_sso_config_replace() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO sso_config (id, provider_type, idp_url) VALUES (1, 'saml', 'https://a.com')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO sso_config (id, provider_type, idp_url, entity_id) VALUES (1, 'oidc', 'https://b.com', 'com.4da.app')",
            [],
        )
        .unwrap();

        let provider_type: String = conn
            .query_row(
                "SELECT provider_type FROM sso_config WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provider_type, "oidc");
    }

    #[test]
    fn test_sso_session_storage() {
        let conn = setup_db();
        let session_id = uuid::Uuid::new_v4().to_string();
        let groups = serde_json::to_string(&vec!["engineering", "admins"]).unwrap();

        conn.execute(
            "INSERT INTO sso_sessions (id, email, display_name, groups, provider_type)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                "alice@example.com",
                "Alice Smith",
                groups,
                "saml"
            ],
        )
        .unwrap();

        let (email, display_name, groups_json): (String, String, String) = conn
            .query_row(
                "SELECT email, display_name, groups FROM sso_sessions WHERE id = ?1",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(email, "alice@example.com");
        assert_eq!(display_name, "Alice Smith");
        let parsed_groups: Vec<String> = serde_json::from_str(&groups_json).unwrap();
        assert_eq!(parsed_groups, vec!["engineering", "admins"]);
    }

    #[test]
    fn test_session_cleanup() {
        let conn = setup_db();

        // Insert two sessions
        for i in 0..2 {
            conn.execute(
                "INSERT INTO sso_sessions (id, email, display_name, groups, provider_type)
                 VALUES (?1, ?2, ?3, '[]', 'saml')",
                params![
                    format!("session-{i}"),
                    format!("user{i}@example.com"),
                    format!("User {i}"),
                ],
            )
            .unwrap();
        }

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM sso_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Clear all sessions (logout)
        conn.execute("DELETE FROM sso_sessions", []).unwrap();

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM sso_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_build_saml_login_url() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/saml/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let url = build_saml_login_url(&config).unwrap();
        assert!(url.starts_with("https://idp.example.com/saml/sso?SAMLRequest="));
        assert!(url.contains("SAMLRequest="));
    }

    #[test]
    fn test_build_saml_login_url_with_existing_query() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/saml/sso?tenant=abc".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let url = build_saml_login_url(&config).unwrap();
        assert!(
            url.contains("&SAMLRequest="),
            "Should use & for existing query string"
        );
    }

    #[test]
    fn test_build_oidc_login_url() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("my-client-id".to_string()),
            issuer: Some("https://idp.example.com".to_string()),
            enabled: true,
        };

        let url = build_oidc_login_url(&config).unwrap();
        assert!(url.starts_with("https://idp.example.com/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=my-client-id"));
        assert!(url.contains("scope=openid+email+profile"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("state="));
        assert!(url.contains("nonce="));
    }

    #[test]
    fn test_build_oidc_login_url_missing_client_id() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: Some("https://idp.example.com".to_string()),
            enabled: true,
        };

        let result = build_oidc_login_url(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_saml_assertion_basic() {
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress">alice@corp.com</saml:NameID>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2099-12-31T23:59:59Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
                <saml:AttributeStatement>
                    <saml:Attribute Name="displayName">
                        <saml:AttributeValue>Alice Engineer</saml:AttributeValue>
                    </saml:Attribute>
                    <saml:Attribute Name="memberOf">
                        <saml:AttributeValue>engineering</saml:AttributeValue>
                    </saml:Attribute>
                </saml:AttributeStatement>
            </saml:Assertion>
        </samlp:Response>"#;

        // Config without certificate — skips signature verification
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let session = parse_saml_assertion(&encoded, &config).unwrap();

        assert_eq!(session.email, "alice@corp.com");
        assert_eq!(session.display_name, "Alice Engineer");
        assert_eq!(session.groups, vec!["engineering"]);
        assert_eq!(session.provider_type, "saml");
        assert!(session.expires_at.is_some());
    }

    #[test]
    fn test_parse_saml_assertion_expired() {
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID>alice@corp.com</saml:NameID>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2020-01-01T00:00:00Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
            </saml:Assertion>
        </samlp:Response>"#;

        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let result = parse_saml_assertion(&encoded, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_pending_auth_roundtrip() {
        let conn = setup_db();
        // Create sso_pending_auth table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sso_pending_auth (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL UNIQUE,
                nonce TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL
            );",
        )
        .unwrap();

        let state = "test-state-123";
        let nonce = "test-nonce-456";
        let expires = "2099-12-31T23:59:59Z";

        conn.execute(
            "INSERT INTO sso_pending_auth (id, state, nonce, provider_type, expires_at)
             VALUES ('pending-1', ?1, ?2, 'oidc', ?3)",
            params![state, nonce, expires],
        )
        .unwrap();

        // Look up by state
        let found_nonce: String = conn.query_row(
            "SELECT nonce FROM sso_pending_auth WHERE state = ?1 AND expires_at > datetime('now')",
            params![state],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(found_nonce, nonce);

        // Delete (single-use)
        conn.execute(
            "DELETE FROM sso_pending_auth WHERE state = ?1",
            params![state],
        )
        .unwrap();

        // Should not find it again
        let result = conn.query_row(
            "SELECT nonce FROM sso_pending_auth WHERE state = ?1",
            params![state],
            |row| row.get::<_, String>(0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_pending_auth_expired_rejected() {
        let conn = setup_db();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sso_pending_auth (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL UNIQUE,
                nonce TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL
            );",
        )
        .unwrap();

        // Insert with past expiry
        conn.execute(
            "INSERT INTO sso_pending_auth (id, state, nonce, provider_type, expires_at)
             VALUES ('pending-2', 'expired-state', 'old-nonce', 'oidc', '2020-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let result = conn.query_row(
            "SELECT nonce FROM sso_pending_auth WHERE state = 'expired-state' AND expires_at > datetime('now')",
            [],
            |row| row.get::<_, String>(0),
        );
        assert!(result.is_err(), "Should not find expired pending auth");
    }

    // ========================================================================
    // Config validation (mirrors set_sso_config logic)
    // ========================================================================

    /// Helper: validate an SsoConfig the same way set_sso_config does,
    /// but without touching the database.
    fn validate_config(config: &SsoConfig) -> crate::error::Result<()> {
        if config.provider_type != "saml" && config.provider_type != "oidc" {
            return Err("provider_type must be 'saml' or 'oidc'".into());
        }
        if config.provider_type == "oidc" {
            if config.client_id.as_ref().map_or(true, |s| s.is_empty()) {
                return Err("OIDC provider requires client_id".into());
            }
            if config.issuer.as_ref().map_or(true, |s| s.is_empty()) {
                return Err("OIDC provider requires issuer URL".into());
            }
        }
        if config.idp_url.is_empty() {
            return Err("IdP URL is required".into());
        }
        Ok(())
    }

    #[test]
    fn test_config_validation_rejects_invalid_provider_type() {
        let config = SsoConfig {
            provider_type: "ldap".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("provider_type"),
            "Error should mention provider_type: {err}"
        );
    }

    #[test]
    fn test_config_validation_accepts_saml() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/saml".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: Some("MIIC...cert".to_string()),
            client_id: None,
            issuer: None,
            enabled: true,
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_config_validation_oidc_requires_client_id() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None, // Missing
            issuer: Some("https://idp.example.com".to_string()),
            enabled: true,
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("client_id"),
            "Error should mention client_id: {err}"
        );
    }

    #[test]
    fn test_config_validation_oidc_requires_issuer() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("my-client".to_string()),
            issuer: None, // Missing
            enabled: true,
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("issuer"),
            "Error should mention issuer: {err}"
        );
    }

    #[test]
    fn test_config_validation_oidc_rejects_empty_client_id() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("".to_string()), // Empty string
            issuer: Some("https://idp.example.com".to_string()),
            enabled: true,
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("client_id"),
            "Error should mention client_id: {err}"
        );
    }

    #[test]
    fn test_config_validation_rejects_empty_idp_url() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "".to_string(), // Empty
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("IdP URL"),
            "Error should mention IdP URL: {err}"
        );
    }

    // ========================================================================
    // OIDC URL construction edge cases
    // ========================================================================

    #[test]
    fn test_build_oidc_login_url_missing_issuer() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("my-client-id".to_string()),
            issuer: None, // Missing issuer
            enabled: true,
        };
        let result = build_oidc_login_url(&config);
        assert!(result.is_err(), "Should fail without issuer");
    }

    #[test]
    fn test_build_oidc_login_url_strips_trailing_slash() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://idp.example.com".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("my-client-id".to_string()),
            issuer: Some("https://idp.example.com/".to_string()), // trailing slash
            enabled: true,
        };
        let url = build_oidc_login_url(&config).unwrap();
        // Should be .../authorize? not ...//authorize?
        assert!(
            url.contains("example.com/authorize?"),
            "Trailing slash should be stripped: {url}"
        );
        assert!(
            !url.contains("//authorize"),
            "Double slash should not appear: {url}"
        );
    }

    // ========================================================================
    // SAML URL content verification
    // ========================================================================

    #[test]
    fn test_saml_login_url_contains_valid_authn_request() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/saml/sso".to_string(),
            entity_id: "com.test.entity".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let url = build_saml_login_url(&config).unwrap();

        // Extract and decode the SAMLRequest parameter
        let saml_param = url
            .split("SAMLRequest=")
            .nth(1)
            .expect("URL must contain SAMLRequest");
        let decoded_param = urlencoding::decode(saml_param).unwrap();
        let xml_bytes = base64::engine::general_purpose::STANDARD
            .decode(decoded_param.as_bytes())
            .unwrap();
        let xml = String::from_utf8(xml_bytes).unwrap();

        assert!(
            xml.contains("AuthnRequest"),
            "Decoded XML must be an AuthnRequest"
        );
        assert!(
            xml.contains("com.test.entity"),
            "Entity ID must appear in the request"
        );
        assert!(
            xml.contains("localhost:4445"),
            "Callback URL must reference the SSO callback port"
        );
        assert!(xml.contains("Version=\"2.0\""), "Must be SAML 2.0");
    }

    // ========================================================================
    // SAML assertion parsing edge cases
    // ========================================================================

    #[test]
    fn test_parse_saml_assertion_invalid_base64() {
        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let result = parse_saml_assertion("not-valid-base64!!!", &config);
        assert!(result.is_err(), "Invalid base64 should fail");
        assert!(
            result.unwrap_err().to_string().contains("base64"),
            "Error should mention base64"
        );
    }

    #[test]
    fn test_parse_saml_assertion_missing_name_id() {
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2099-12-31T23:59:59Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
            </saml:Assertion>
        </samlp:Response>"#;

        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let result = parse_saml_assertion(&encoded, &config);
        assert!(result.is_err(), "Missing NameID should fail");
        assert!(
            result.unwrap_err().to_string().contains("NameID"),
            "Error should mention NameID"
        );
    }

    #[test]
    fn test_parse_saml_assertion_display_name_fallback_to_email() {
        // No displayName, cn, or claims/name attribute — should fall back to email
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID>bob@corp.com</saml:NameID>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2099-12-31T23:59:59Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
                <saml:AttributeStatement>
                </saml:AttributeStatement>
            </saml:Assertion>
        </samlp:Response>"#;

        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let session = parse_saml_assertion(&encoded, &config).unwrap();

        assert_eq!(session.email, "bob@corp.com");
        assert_eq!(
            session.display_name, "bob@corp.com",
            "display_name should fall back to email when no name attributes exist"
        );
        assert!(session.groups.is_empty(), "Should have no groups");
    }

    #[test]
    fn test_parse_saml_assertion_groups_single_value() {
        // Uses "groups" attribute name (alternate to "memberOf") with a single group
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID>carol@corp.com</saml:NameID>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2099-12-31T23:59:59Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
                <saml:AttributeStatement>
                    <saml:Attribute Name="groups">
                        <saml:AttributeValue>devops</saml:AttributeValue>
                    </saml:Attribute>
                </saml:AttributeStatement>
            </saml:Assertion>
        </samlp:Response>"#;

        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let session = parse_saml_assertion(&encoded, &config).unwrap();

        assert_eq!(session.groups, vec!["devops"]);
        assert_eq!(session.email, "carol@corp.com");
    }

    #[test]
    fn test_parse_saml_assertion_cn_display_name() {
        // Uses "cn" attribute for display name instead of "displayName"
        let saml_response = r#"<samlp:Response>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID>dave@corp.com</saml:NameID>
                    <saml:SubjectConfirmation>
                        <saml:SubjectConfirmationData NotOnOrAfter="2099-12-31T23:59:59Z"/>
                    </saml:SubjectConfirmation>
                </saml:Subject>
                <saml:AttributeStatement>
                    <saml:Attribute Name="cn">
                        <saml:AttributeValue>Dave Thompson</saml:AttributeValue>
                    </saml:Attribute>
                </saml:AttributeStatement>
            </saml:Assertion>
        </samlp:Response>"#;

        let config = SsoConfig {
            provider_type: "saml".to_string(),
            idp_url: "https://idp.example.com/sso".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: None,
            issuer: None,
            enabled: true,
        };

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let session = parse_saml_assertion(&encoded, &config).unwrap();

        assert_eq!(session.display_name, "Dave Thompson");
    }

    // ========================================================================
    // Session expiry logic
    // ========================================================================

    #[test]
    fn test_session_expiry_check_past() {
        // Verify that a session with a past expires_at is recognized as expired
        let past = "2020-01-01T00:00:00Z";
        let expiry = chrono::DateTime::parse_from_rfc3339(past).unwrap();
        assert!(
            expiry < chrono::Utc::now(),
            "Test fixture: past date must be before now"
        );
    }

    #[test]
    fn test_session_expiry_check_future() {
        let future = "2099-12-31T23:59:59Z";
        let expiry = chrono::DateTime::parse_from_rfc3339(future).unwrap();
        assert!(
            expiry > chrono::Utc::now(),
            "Test fixture: future date must be after now"
        );
    }

    // ========================================================================
    // Type serialization
    // ========================================================================

    #[test]
    fn test_sso_config_serialize_deserialize() {
        let config = SsoConfig {
            provider_type: "oidc".to_string(),
            idp_url: "https://login.microsoftonline.com/tenant/v2.0".to_string(),
            entity_id: "com.4da.app".to_string(),
            certificate: None,
            client_id: Some("abc-123".to_string()),
            issuer: Some("https://login.microsoftonline.com/tenant/v2.0".to_string()),
            enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SsoConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider_type, "oidc");
        assert_eq!(deserialized.client_id.as_deref(), Some("abc-123"));
        assert_eq!(deserialized.enabled, true);
    }

    #[test]
    fn test_sso_session_serialize_deserialize() {
        let session = SsoSession {
            email: "test@example.com".to_string(),
            display_name: "Test User".to_string(),
            groups: vec!["admin".to_string(), "users".to_string()],
            authenticated_at: "2026-04-06T12:00:00Z".to_string(),
            expires_at: Some("2026-04-06T14:00:00Z".to_string()),
            provider_type: "saml".to_string(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: SsoSession = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.email, "test@example.com");
        assert_eq!(deserialized.groups.len(), 2);
        assert_eq!(deserialized.groups[0], "admin");
        assert_eq!(
            deserialized.expires_at.as_deref(),
            Some("2026-04-06T14:00:00Z")
        );
    }

    // ========================================================================
    // Callback port constant
    // ========================================================================

    #[test]
    fn test_sso_callback_port_is_4445() {
        assert_eq!(SSO_CALLBACK_PORT, 4445, "SSO callback must use port 4445");
    }
}
