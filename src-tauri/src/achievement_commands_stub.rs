// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `achievement_commands` module when "experimental" feature is disabled.

use crate::error::Result;
use tauri::AppHandle;

#[tauri::command]
pub fn get_game_state() -> Result<serde_json::Value> {
    Ok(serde_json::json!({"counters": [], "achievements": [], "streak": 0, "last_active": null}))
}

#[tauri::command]
pub fn get_achievements() -> Result<serde_json::Value> {
    Ok(serde_json::json!([]))
}

#[tauri::command]
pub fn check_daily_streak(_app: AppHandle) -> Result<serde_json::Value> {
    Ok(serde_json::json!([]))
}
