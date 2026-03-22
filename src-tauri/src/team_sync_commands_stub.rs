// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stubs for team sync commands when "team-sync" feature is disabled.
//! Commands register but return errors.

use crate::error::Result;

#[tauri::command]
pub async fn get_team_sync_status() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_team_members() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn share_dna_with_team() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn share_signal_with_team() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn propose_team_decision() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn vote_on_decision() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_team_decisions() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_decision_detail() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn resolve_decision() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn join_team_via_invite() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn create_team() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn create_team_invite() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn share_source_with_team() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_team_sources() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn upvote_team_source() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn remove_team_source() -> Result<()> {
    Err("Team sync requires --features team-sync".into())
}
