// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `enterprise_analytics` module when "enterprise" feature is disabled.

use crate::error::Result;

#[tauri::command]
pub async fn get_org_analytics_cmd() -> Result<serde_json::Value> {
    Err("Enterprise features require --features enterprise".into())
}
#[tauri::command]
pub async fn export_org_analytics_cmd() -> Result<String> {
    Err("Enterprise features require --features enterprise".into())
}
