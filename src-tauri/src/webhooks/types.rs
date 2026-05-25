// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Core webhook types and constants.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Backoff schedule in seconds: 1min, 5min, 30min, 2hr, 12hr.
pub(crate) const RETRY_BACKOFF_SECS: [i64; 5] = [60, 300, 1800, 7200, 43200];
/// Consecutive failures before the circuit breaker trips.
pub(crate) const CIRCUIT_BREAKER_THRESHOLD: i32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Webhook {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub failure_count: i32,
    pub last_fired_at: Option<String>,
    pub last_status_code: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_id: String,
    pub event_type: String,
    pub status: String,
    pub http_status: Option<i32>,
    pub attempt_count: i32,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct WebhookPayload {
    pub(super) event: String,
    pub(super) timestamp: String,
    pub(super) data: serde_json::Value,
}
