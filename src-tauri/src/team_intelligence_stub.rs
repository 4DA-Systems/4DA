// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stubs for team intelligence commands when "team-sync" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_team_profile_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_team_blind_spots_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_bus_factor_report_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
#[tauri::command]
pub async fn get_team_signal_summary_cmd() -> Result<serde_json::Value> {
    Err("Team sync requires --features team-sync".into())
}
