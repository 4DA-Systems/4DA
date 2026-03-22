// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `sso` module when "enterprise" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_sso_config() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn set_sso_config() -> Result<()> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn initiate_sso_login() -> Result<String> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_sso_session() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn validate_sso_callback(
    _assertion: String,
    _state: Option<String>,
) -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn logout_sso() -> Result<()> {
    Err("Enterprise features require --features enterprise".into())
}
