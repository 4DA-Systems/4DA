// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `organization` module when "enterprise" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_organization_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_org_teams_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_retention_policies_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn set_retention_policy_cmd() -> Result<()> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_cross_team_signals_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
