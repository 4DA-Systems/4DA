// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
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

/// Parse a SAML Response assertion and extract identity claims.
///
/// Extracts NameID (email), display name, and group attributes from
/// the base64-encoded SAML Response XML.
///
/// NOTE: This performs structural validation only. Cryptographic signature
/// verification of the SAML assertion requires an XML-DSig library and is
/// marked with TODO below.
fn parse_saml_assertion(assertion_b64: &str) -> crate::error::Result<SsoSession> {
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(assertion_b64)
        .map_err(|e| format!("Invalid base64 in SAML assertion: {e}"))?;
    let xml = String::from_utf8(decoded_bytes)
        .map_err(|e| format!("SAML assertion is not valid UTF-8: {e}"))?;

    // TODO: Verify XML digital signature against the IdP certificate.
    // This requires an XML-DSig verification library (e.g., xmlsec1 bindings
    // or a pure-Rust implementation). For now, we extract claims structurally.
    // Production deployments MUST enable signature verification.

    // Extract NameID (email)
    let email =
        extract_xml_element(&xml, "NameID").ok_or("SAML assertion missing NameID element")?;

    // Extract display name from Attribute elements
    let display_name = extract_saml_attribute(&xml, "displayName")
        .or_else(|| extract_saml_attribute(&xml, "cn"))
        .or_else(|| {
            extract_saml_attribute(
                &xml,
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name",
            )
        })
        .unwrap_or_else(|| email.clone());

    // Extract group memberships
    let groups = extract_saml_attribute_values(&xml, "memberOf")
        .or_else(|| extract_saml_attribute_values(&xml, "groups"))
        .or_else(|| extract_saml_attribute_values(&xml, "http://schemas.xmlsoap.org/claims/Group"))
        .unwrap_or_default();

    // Check NotOnOrAfter for session expiry
    let expires_at = extract_xml_attribute_value(&xml, "SubjectConfirmationData", "NotOnOrAfter");

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
/// Decodes the JWT ID token (without signature verification — see TODO)
/// and extracts standard OIDC claims: sub/email, name, groups.
///
/// NOTE: Full JWT signature verification requires the IdP's JWKS endpoint.
/// Marked with TODO below for production hardening.
fn parse_oidc_token(token_json: &str) -> crate::error::Result<SsoSession> {
    let token_data: serde_json::Value = serde_json::from_str(token_json)
        .map_err(|e| format!("Invalid OIDC token response: {e}"))?;

    let id_token = token_data["id_token"]
        .as_str()
        .ok_or("OIDC response missing id_token")?;

    // Decode JWT payload (middle segment)
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format: expected 3 dot-separated segments".into());
    }

    // TODO: Verify JWT signature against IdP's JWKS keys.
    // This requires fetching {issuer}/.well-known/openid-configuration,
    // extracting the jwks_uri, and verifying the RS256/ES256 signature.
    // Production deployments MUST enable signature verification.

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("Invalid base64 in JWT payload: {e}"))?;
    let claims: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("Invalid JSON in JWT payload: {e}"))?;

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

    // Validate expiry
    if let Some(exp) = claims["exp"].as_i64() {
        let now = chrono::Utc::now().timestamp();
        if exp < now {
            return Err("OIDC token has expired".into());
        }
    }

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
// XML parsing helpers (lightweight, no external XML crate dependency)
// ============================================================================

/// Extract the text content of the first occurrence of an XML element.
///
/// Simple regex-free parser for well-formed XML. Finds `<tag...>content</tag>`
/// or `<ns:tag...>content</ns:tag>` patterns.
fn extract_xml_element(xml: &str, local_name: &str) -> Option<String> {
    // Match both <localName> and <ns:localName>
    let patterns = [format!("<{local_name}"), format!(":{local_name}")];

    for pattern in &patterns {
        if let Some(start_pos) = xml.find(pattern.as_str()) {
            // Find the end of the opening tag
            let after_tag = &xml[start_pos..];
            let close_bracket = after_tag.find('>')?;
            let content_start = start_pos + close_bracket + 1;

            // Find the next closing tag that contains our element name.
            // We search for "</" first, then check if the tag name matches.
            let remaining = &xml[content_start..];
            let mut search_offset = 0;
            while let Some(close_pos) = remaining[search_offset..].find("</") {
                let abs_close = search_offset + close_pos;
                let tag_rest = &remaining[abs_close + 2..]; // after "</"
                                                            // Check if this closing tag ends with our local_name
                                                            // e.g. "saml:NameID>" or "NameID>"
                if let Some(gt_pos) = tag_rest.find('>') {
                    let tag_name = &tag_rest[..gt_pos];
                    if tag_name == local_name || tag_name.ends_with(&format!(":{local_name}")) {
                        let content = &remaining[..abs_close];
                        return Some(content.trim().to_string());
                    }
                }
                search_offset = abs_close + 2;
            }
        }
    }

    None
}

/// Extract a SAML Attribute value by its Name attribute.
fn extract_saml_attribute(xml: &str, attr_name: &str) -> Option<String> {
    // Look for: <saml:Attribute Name="attr_name"...><saml:AttributeValue>value</...>
    let name_pattern = format!("Name=\"{attr_name}\"");
    let pos = xml.find(&name_pattern)?;
    let after = &xml[pos..];

    // Find the AttributeValue within this Attribute element
    let av_start = after.find("AttributeValue")?;
    let av_content_start = after[av_start..].find('>')? + av_start + 1;
    let av_content_end = after[av_content_start..].find("</")? + av_content_start;

    let value = &after[av_content_start..av_content_end];
    Some(value.trim().to_string())
}

/// Extract multiple SAML Attribute values (for group memberships).
fn extract_saml_attribute_values(xml: &str, attr_name: &str) -> Option<Vec<String>> {
    let name_pattern = format!("Name=\"{attr_name}\"");
    let pos = xml.find(&name_pattern)?;

    // Find the closing </saml:Attribute> or </Attribute>
    let after = &xml[pos..];
    let attr_end = after.find("</").and_then(|p| {
        after[p..]
            .find("Attribute>")
            .map(|q| p + q + "Attribute>".len())
    })?;

    let attr_block = &after[..attr_end];
    let mut values = Vec::new();
    let mut search_from = 0;

    while let Some(av_start) = attr_block[search_from..].find("AttributeValue") {
        let abs_start = search_from + av_start;
        if let Some(content_start) = attr_block[abs_start..].find('>') {
            let cs = abs_start + content_start + 1;
            if let Some(content_end) = attr_block[cs..].find("</") {
                let value = attr_block[cs..cs + content_end].trim();
                if !value.is_empty() {
                    values.push(value.to_string());
                }
                search_from = cs + content_end;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Extract a named XML attribute from a specific element.
///
/// Finds `<element_name ... attr_name="value" ...>` and returns the value.
fn extract_xml_attribute_value(xml: &str, element_name: &str, attr_name: &str) -> Option<String> {
    let elem_pattern = format!("<{element_name}");
    // Also check namespaced variant
    let patterns = [elem_pattern.clone(), format!(":{element_name}")];

    for pattern in &patterns {
        if let Some(pos) = xml.find(pattern.as_str()) {
            let after = &xml[pos..];
            let tag_end = after.find('>')?;
            let tag = &after[..tag_end];

            let attr_pattern = format!("{attr_name}=\"");
            if let Some(attr_pos) = tag.find(&attr_pattern) {
                let value_start = attr_pos + attr_pattern.len();
                let value_end = tag[value_start..].find('"')? + value_start;
                return Some(tag[value_start..value_end].to_string());
            }
        }
    }

    None
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
        "oidc" => build_oidc_login_url(&config)?,
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
/// Returns the created session on success.
#[tauri::command]
pub async fn validate_sso_callback(assertion: String) -> crate::error::Result<SsoSession> {
    let conn = crate::state::open_db_connection()?;
    ensure_sso_tables(&conn)?;

    // Determine provider type from config
    let provider_type: String = conn
        .query_row(
            "SELECT provider_type FROM sso_config WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "SSO not configured")?;

    let session = match provider_type.as_str() {
        "saml" => parse_saml_assertion(&assertion)?,
        "oidc" => parse_oidc_token(&assertion)?,
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
    fn test_extract_xml_element() {
        let xml = r#"<saml:NameID Format="email">alice@example.com</saml:NameID>"#;
        let result = extract_xml_element(xml, "NameID");
        assert_eq!(result, Some("alice@example.com".to_string()));
    }

    #[test]
    fn test_extract_xml_element_no_namespace() {
        let xml = r#"<NameID>bob@example.com</NameID>"#;
        let result = extract_xml_element(xml, "NameID");
        assert_eq!(result, Some("bob@example.com".to_string()));
    }

    #[test]
    fn test_extract_xml_element_missing() {
        let xml = r#"<Issuer>com.4da.app</Issuer>"#;
        let result = extract_xml_element(xml, "NameID");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_saml_attribute() {
        let xml = r#"
        <saml:Attribute Name="displayName">
            <saml:AttributeValue>Alice Smith</saml:AttributeValue>
        </saml:Attribute>"#;
        let result = extract_saml_attribute(xml, "displayName");
        assert_eq!(result, Some("Alice Smith".to_string()));
    }

    #[test]
    fn test_extract_xml_attribute_value() {
        let xml = r#"<samlp:SubjectConfirmationData NotOnOrAfter="2026-03-14T00:00:00Z" Recipient="http://localhost:4445"/>"#;
        let result = extract_xml_attribute_value(xml, "SubjectConfirmationData", "NotOnOrAfter");
        assert_eq!(result, Some("2026-03-14T00:00:00Z".to_string()));
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

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let session = parse_saml_assertion(&encoded).unwrap();

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

        let encoded = base64::engine::general_purpose::STANDARD.encode(saml_response.as_bytes());
        let result = parse_saml_assertion(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_parse_oidc_token() {
        // Build a minimal JWT: header.payload.signature
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"RS256","typ":"JWT"}"#.as_bytes());
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "sub": "user-123",
                "email": "bob@corp.com",
                "name": "Bob Developer",
                "groups": ["engineering", "team-leads"],
                "exp": 4102444800_i64  // 2099-12-31
            })
            .to_string()
            .as_bytes(),
        );
        let fake_sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"fake-signature");
        let jwt = format!("{header}.{payload}.{fake_sig}");

        let token_response = serde_json::json!({
            "id_token": jwt,
            "access_token": "at-xxx",
            "token_type": "Bearer",
        })
        .to_string();

        let session = parse_oidc_token(&token_response).unwrap();
        assert_eq!(session.email, "bob@corp.com");
        assert_eq!(session.display_name, "Bob Developer");
        assert_eq!(session.groups, vec!["engineering", "team-leads"]);
        assert_eq!(session.provider_type, "oidc");
        assert!(session.expires_at.is_some());
    }

    #[test]
    fn test_parse_oidc_token_expired() {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"RS256"}"#.as_bytes());
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "sub": "user-123",
                "email": "alice@corp.com",
                "exp": 946684800_i64  // 2000-01-01 (expired)
            })
            .to_string()
            .as_bytes(),
        );
        let fake_sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
        let jwt = format!("{header}.{payload}.{fake_sig}");

        let token_response = serde_json::json!({
            "id_token": jwt,
        })
        .to_string();

        let result = parse_oidc_token(&token_response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_parse_oidc_token_invalid_jwt() {
        let token_response = serde_json::json!({
            "id_token": "not.a.valid-jwt.too-many-parts",
        })
        .to_string();

        let result = parse_oidc_token(&token_response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_oidc_token_missing_id_token() {
        let token_response = serde_json::json!({
            "access_token": "at-xxx",
        })
        .to_string();

        let result = parse_oidc_token(&token_response);
        assert!(result.is_err());
    }
}
