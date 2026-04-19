// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `webhooks` module when "enterprise" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn register_webhook_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn list_webhooks_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn delete_webhook_cmd() -> Result<()> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn test_webhook_cmd() -> Result<bool> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn get_webhook_deliveries_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
