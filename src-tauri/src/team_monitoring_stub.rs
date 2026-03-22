// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stubs for team monitoring commands when "team-sync" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_team_signals_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn resolve_team_signal_cmd() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_alert_policy_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn set_alert_policy_cmd() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_monitoring_summary_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
