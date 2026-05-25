// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Delivery status helpers, retry scheduling, and circuit breaker.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use tracing::{info, warn};
use uuid::Uuid;

use super::types::{CIRCUIT_BREAKER_THRESHOLD, RETRY_BACKOFF_SECS};

/// Record a webhook delivery attempt in the database.
///
/// Creates a row in `webhook_deliveries` and returns the generated delivery ID.
/// The `status` should be one of: `"pending"`, `"delivered"`, `"failed"`, `"exhausted"`.
pub(super) fn record_delivery(
    conn: &Connection,
    webhook_id: &str,
    event_type: &str,
    payload: &str,
    status: &str,
    http_status: Option<i32>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO webhook_deliveries (id, webhook_id, event_type, payload, status, http_status, attempt_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        params![id, webhook_id, event_type, payload, status, http_status],
    )
    .context("Record webhook delivery")?;
    Ok(id)
}

/// Calculate the next retry timestamp for a given attempt number (1-indexed).
pub fn next_retry_at(attempt: i32) -> String {
    let idx = ((attempt - 1) as usize).min(RETRY_BACKOFF_SECS.len() - 1);
    let delay = chrono::Duration::seconds(RETRY_BACKOFF_SECS[idx]);
    (chrono::Utc::now() + delay)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

pub(super) fn mark_delivered(conn: &Connection, delivery_id: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhook_deliveries SET status = 'delivered', delivered_at = ?1,
                attempt_count = attempt_count + 1 WHERE id = ?2",
        params![now, delivery_id],
    )?;
    Ok(())
}

pub(super) fn mark_failed(
    conn: &Connection,
    delivery_id: &str,
    attempt: i32,
    http_status: Option<i32>,
) -> Result<()> {
    let retry_at = next_retry_at(attempt);
    conn.execute(
        "UPDATE webhook_deliveries SET status = 'failed', attempt_count = ?1,
                http_status = ?2, next_retry_at = ?3 WHERE id = ?4",
        params![attempt, http_status, retry_at, delivery_id],
    )?;
    Ok(())
}

pub(super) fn record_success(conn: &Connection, webhook_id: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhooks SET failure_count = 0, last_fired_at = ?1, last_status_code = 200 WHERE id = ?2",
        params![now, webhook_id],
    )?;
    Ok(())
}

pub(super) fn record_failure(
    conn: &Connection,
    webhook_id: &str,
    http_status: Option<i32>,
) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhooks SET failure_count = failure_count + 1, last_fired_at = ?1,
                last_status_code = ?2 WHERE id = ?3",
        params![now, http_status, webhook_id],
    )?;
    Ok(())
}

/// Check if circuit breaker has tripped. Auto-disables webhook at threshold.
pub fn check_circuit_breaker(conn: &Connection, webhook_id: &str) -> Result<bool> {
    let failure_count: i32 = conn
        .query_row(
            "SELECT failure_count FROM webhooks WHERE id = ?1",
            params![webhook_id],
            |row| row.get(0),
        )
        .context("Read failure_count for circuit breaker")?;

    if failure_count >= CIRCUIT_BREAKER_THRESHOLD {
        conn.execute(
            "UPDATE webhooks SET active = 0 WHERE id = ?1",
            params![webhook_id],
        )?;
        warn!(target: "4da::webhooks", webhook_id, failure_count, "Circuit breaker tripped");
        return Ok(true);
    }
    Ok(false)
}

/// Reset circuit breaker: clear failure count and re-enable.
pub fn reset_circuit_breaker(conn: &Connection, webhook_id: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE webhooks SET failure_count = 0, active = 1 WHERE id = ?1",
        params![webhook_id],
    )?;
    if changed == 0 {
        anyhow::bail!("Webhook not found: {}", webhook_id);
    }
    info!(target: "4da::webhooks", webhook_id, "Circuit breaker reset");
    Ok(())
}
