// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stubs for team notification commands when "team-sync" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_team_notifications() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_notification_summary() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn mark_notification_read() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn mark_all_notifications_read() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn dismiss_notification() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
