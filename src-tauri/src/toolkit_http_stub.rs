// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `toolkit_http` module when "experimental" feature is disabled.

use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHistoryEntry {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub created_at: String,
}

#[tauri::command]
pub async fn toolkit_http_request(_request: HttpProbeRequest) -> Result<HttpProbeResponse> {
    Err(crate::error::FourDaError::Config(
        "HTTP toolkit is an experimental feature".into(),
    ))
}

#[tauri::command]
pub async fn toolkit_get_http_history(_limit: Option<u32>) -> Result<Vec<HttpHistoryEntry>> {
    Ok(vec![])
}
