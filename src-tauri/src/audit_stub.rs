// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `audit` module when "enterprise" feature is disabled.

use crate::error::Result;

/// Bundled audit logging parameters (used by team-sync without enterprise).
#[cfg(feature = "team-sync")]
pub struct AuditLogParams<'a> {
    pub conn: &'a rusqlite::Connection,
    pub team_id: &'a str,
    pub actor_id: &'a str,
    pub actor_display_name: &'a str,
    pub action: &'a str,
    pub resource_type: &'a str,
    pub resource_id: Option<&'a str>,
    pub details: Option<&'a serde_json::Value>,
}

/// No-op audit logging when enterprise feature is disabled.
#[allow(unused_variables)]
pub fn log_team_audit(
    conn: &rusqlite::Connection,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    details: Option<&serde_json::Value>,
) {
    // Enterprise audit logging disabled -- no-op
}

/// No-op direct audit logging when enterprise feature is disabled.
#[cfg(feature = "team-sync")]
#[allow(unused_variables)]
pub fn log_audit(_params: &AuditLogParams<'_>) {
    // Enterprise audit logging disabled -- no-op
}

#[tauri::command]
pub async fn get_audit_log() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_audit_summary_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn export_audit_csv_cmd() -> Result<String> {
    Err("Enterprise features require --features enterprise".into())
}
