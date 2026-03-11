// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Enterprise webhook dispatch system.
//!
//! Provides webhook registration, signed HTTP delivery, exponential-backoff
//! retries, and a circuit breaker that auto-disables failing endpoints.
//!
//! ## Database Schema
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS webhooks (
//!     id TEXT PRIMARY KEY, team_id TEXT NOT NULL, name TEXT NOT NULL,
//!     url TEXT NOT NULL, events TEXT NOT NULL, secret TEXT NOT NULL,
//!     active INTEGER DEFAULT 1, failure_count INTEGER DEFAULT 0,
//!     last_fired_at TEXT, last_status_code INTEGER,
//!     created_at TEXT DEFAULT (datetime('now')), created_by TEXT
//! );
//! CREATE TABLE IF NOT EXISTS webhook_deliveries (
//!     id TEXT PRIMARY KEY, webhook_id TEXT NOT NULL, event_type TEXT NOT NULL,
//!     payload TEXT NOT NULL, status TEXT DEFAULT 'pending',
//!     http_status INTEGER, attempt_count INTEGER DEFAULT 0,
//!     next_retry_at TEXT, created_at TEXT DEFAULT (datetime('now')),
//!     delivered_at TEXT,
//!     FOREIGN KEY (webhook_id) REFERENCES webhooks(id)
//! );
//! ```

use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{info, warn};
use ts_rs::TS;
use uuid::Uuid;

/// Maximum retries before a delivery is marked 'exhausted'.
const MAX_RETRY_ATTEMPTS: i32 = 5;
/// Backoff schedule in seconds: 1min, 5min, 30min, 2hr, 12hr.
const RETRY_BACKOFF_SECS: [i64; 5] = [60, 300, 1800, 7200, 43200];
/// Consecutive failures before the circuit breaker trips.
const CIRCUIT_BREAKER_THRESHOLD: i32 = 10;

// ============================================================================
// Core Structs
// ============================================================================

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
struct WebhookPayload {
    event: String,
    timestamp: String,
    data: serde_json::Value,
}

// ============================================================================
// Schema
// ============================================================================

/// Create the webhook tables if they don't exist.
pub fn ensure_webhook_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS webhooks (
            id TEXT PRIMARY KEY, team_id TEXT NOT NULL, name TEXT NOT NULL,
            url TEXT NOT NULL, events TEXT NOT NULL, secret TEXT NOT NULL,
            active INTEGER DEFAULT 1, failure_count INTEGER DEFAULT 0,
            last_fired_at TEXT, last_status_code INTEGER,
            created_at TEXT DEFAULT (datetime('now')), created_by TEXT
        );
        CREATE TABLE IF NOT EXISTS webhook_deliveries (
            id TEXT PRIMARY KEY, webhook_id TEXT NOT NULL,
            event_type TEXT NOT NULL, payload TEXT NOT NULL,
            status TEXT DEFAULT 'pending', http_status INTEGER,
            attempt_count INTEGER DEFAULT 0, next_retry_at TEXT,
            created_at TEXT DEFAULT (datetime('now')), delivered_at TEXT,
            FOREIGN KEY (webhook_id) REFERENCES webhooks(id)
        );",
    )
    .context("Failed to create webhook tables")?;
    Ok(())
}

// ============================================================================
// Signing
// ============================================================================

/// Sign a payload using HMAC-SHA256 (RFC 2104).
///
/// Returns the hex-encoded MAC. Used to populate the `X-4DA-Signature-256`
/// header in format `sha256=<hex>`.
pub fn sign_payload(secret: &str, body: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

// ============================================================================
// Webhook Management
// ============================================================================

/// Register a new webhook for a team.
pub fn register_webhook(
    conn: &Connection,
    team_id: &str,
    name: &str,
    url: &str,
    events: &[String],
    created_by: Option<&str>,
) -> Result<Webhook> {
    let id = Uuid::new_v4().to_string();
    let secret = Uuid::new_v4().to_string();
    let events_json = serde_json::to_string(&events).context("Serialize events")?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    conn.execute(
        "INSERT INTO webhooks (id, team_id, name, url, events, secret, active, failure_count, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, ?8)",
        params![id, team_id, name, url, events_json, secret, now, created_by],
    ).context("Insert webhook")?;

    info!(target: "4da::webhooks", webhook_id = %id, name = %name, "Webhook registered");
    Ok(Webhook {
        id,
        team_id: team_id.to_string(),
        name: name.to_string(),
        url: url.to_string(),
        events: events.to_vec(),
        active: true,
        failure_count: 0,
        last_fired_at: None,
        last_status_code: None,
        created_at: now,
    })
}

/// List all webhooks for a team.
pub fn list_webhooks(conn: &Connection, team_id: &str) -> Result<Vec<Webhook>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, team_id, name, url, events, active, failure_count,
                last_fired_at, last_status_code, created_at
         FROM webhooks WHERE team_id = ?1 ORDER BY created_at DESC",
        )
        .context("Prepare list_webhooks")?;

    let rows = stmt
        .query_map(params![team_id], |row| {
            let events_json: String = row.get(4)?;
            let events: Vec<String> = serde_json::from_str(&events_json).unwrap_or_default();
            Ok(Webhook {
                id: row.get(0)?,
                team_id: row.get(1)?,
                name: row.get(2)?,
                url: row.get(3)?,
                events,
                active: row.get::<_, i32>(5)? != 0,
                failure_count: row.get(6)?,
                last_fired_at: row.get(7)?,
                last_status_code: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .context("Query webhooks")?;

    let mut webhooks = Vec::new();
    for row in rows {
        webhooks.push(row.context("Read webhook row")?);
    }
    Ok(webhooks)
}

/// Delete a webhook and its deliveries.
pub fn delete_webhook(conn: &Connection, webhook_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM webhook_deliveries WHERE webhook_id = ?1",
        params![webhook_id],
    )
    .context("Delete webhook deliveries")?;
    let changed = conn
        .execute("DELETE FROM webhooks WHERE id = ?1", params![webhook_id])
        .context("Delete webhook")?;
    if changed == 0 {
        anyhow::bail!("Webhook not found: {}", webhook_id);
    }
    info!(target: "4da::webhooks", webhook_id = %webhook_id, "Webhook deleted");
    Ok(())
}

/// Enable or disable a webhook.
pub fn set_webhook_active(conn: &Connection, webhook_id: &str, active: bool) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE webhooks SET active = ?1 WHERE id = ?2",
            params![i32::from(active), webhook_id],
        )
        .context("Update webhook active state")?;
    if changed == 0 {
        anyhow::bail!("Webhook not found: {}", webhook_id);
    }
    info!(target: "4da::webhooks", webhook_id = %webhook_id, active, "Webhook active state updated");
    Ok(())
}

/// Get recent deliveries for a webhook.
pub fn get_webhook_deliveries(
    conn: &Connection,
    webhook_id: &str,
    limit: i64,
) -> Result<Vec<WebhookDelivery>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, webhook_id, event_type, status, http_status,
                attempt_count, created_at, delivered_at
         FROM webhook_deliveries WHERE webhook_id = ?1
         ORDER BY created_at DESC LIMIT ?2",
        )
        .context("Prepare deliveries query")?;

    let rows = stmt
        .query_map(params![webhook_id, limit], |row| {
            Ok(WebhookDelivery {
                id: row.get(0)?,
                webhook_id: row.get(1)?,
                event_type: row.get(2)?,
                status: row.get(3)?,
                http_status: row.get(4)?,
                attempt_count: row.get(5)?,
                created_at: row.get(6)?,
                delivered_at: row.get(7)?,
            })
        })
        .context("Query deliveries")?;

    let mut deliveries = Vec::new();
    for row in rows {
        deliveries.push(row.context("Read delivery row")?);
    }
    Ok(deliveries)
}

// ============================================================================
// Dispatch Engine
// ============================================================================

/// Check whether an event type matches any of the webhook's event patterns.
/// Supports exact match, `*` (all events), and `prefix.*` wildcards.
fn event_matches(patterns: &[String], event_type: &str) -> bool {
    for pattern in patterns {
        if pattern == "*" || pattern == event_type {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix(".*") {
            if event_type.starts_with(prefix) && event_type[prefix.len()..].starts_with('.') {
                return true;
            }
        }
    }
    false
}

/// Fire an event to all matching active webhooks for a team.
/// Returns the number of webhooks dispatched to.
pub async fn fire_event(team_id: &str, event_type: &str, data: serde_json::Value) -> Result<usize> {
    let conn = crate::state::open_db_connection()
        .map_err(|e| anyhow::anyhow!("DB connection failed: {e}"))?;
    let webhooks = list_webhooks(&conn, team_id)?;
    let active: Vec<&Webhook> = webhooks
        .iter()
        .filter(|w| w.active && event_matches(&w.events, event_type))
        .collect();
    if active.is_empty() {
        return Ok(0);
    }

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let envelope = WebhookPayload {
        event: event_type.to_string(),
        timestamp,
        data,
    };
    let payload_json = serde_json::to_string(&envelope).context("Serialize payload")?;

    let mut dispatched = 0;
    for webhook in &active {
        if check_circuit_breaker(&conn, &webhook.id)? {
            warn!(target: "4da::webhooks", webhook_id = %webhook.id, "Circuit breaker open — skipping");
            continue;
        }
        let delivery_id = create_delivery_record(&conn, &webhook.id, event_type, &payload_json)?;
        let secret: String = conn
            .query_row(
                "SELECT secret FROM webhooks WHERE id = ?1",
                params![webhook.id],
                |row| row.get(0),
            )
            .context("Get webhook secret")?;

        match dispatch_delivery_http(&webhook.url, &secret, &delivery_id, &payload_json).await {
            Ok(true) => {
                mark_delivered(&conn, &delivery_id)?;
                record_success(&conn, &webhook.id)?;
            }
            _ => {
                mark_failed(&conn, &delivery_id, 1, None)?;
                record_failure(&conn, &webhook.id, None)?;
            }
        }
        dispatched += 1;
    }
    info!(target: "4da::webhooks", team_id, event_type, count = dispatched, "Event fired");
    Ok(dispatched)
}

/// Create a delivery record in the database.
fn create_delivery_record(
    conn: &Connection,
    webhook_id: &str,
    event_type: &str,
    payload: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO webhook_deliveries (id, webhook_id, event_type, payload, status, attempt_count)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0)",
        params![id, webhook_id, event_type, payload],
    ).context("Create delivery record")?;
    Ok(id)
}

/// Send HTTP POST to the webhook URL.
/// Returns `Ok(true)` on 2xx, `Ok(false)` on non-2xx, `Err` on network failure.
async fn dispatch_delivery_http(
    url: &str,
    secret: &str,
    delivery_id: &str,
    payload: &str,
) -> Result<bool> {
    let signature = sign_payload(secret, payload);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Build HTTP client")?;

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-4DA-Signature-256", format!("sha256={signature}"))
        .header("X-4DA-Delivery", delivery_id)
        .header("User-Agent", "4DA-Webhooks/1.0")
        .body(payload.to_string())
        .send()
        .await
        .context("Webhook HTTP request failed")?;

    Ok((200..300).contains(&(response.status().as_u16())))
}

// ============================================================================
// Retry Engine
// ============================================================================

/// Process pending/retryable deliveries with exponential backoff.
/// After `MAX_RETRY_ATTEMPTS` failures the delivery is marked `'exhausted'`.
pub async fn process_pending_deliveries() -> Result<usize> {
    let conn = crate::state::open_db_connection()
        .map_err(|e| anyhow::anyhow!("DB connection failed: {e}"))?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut stmt = conn.prepare(
        "SELECT d.id, d.webhook_id, d.payload, d.attempt_count, w.url, w.secret
         FROM webhook_deliveries d JOIN webhooks w ON w.id = d.webhook_id
         WHERE (d.status = 'pending')
            OR (d.status = 'failed' AND d.next_retry_at <= ?1)
         ORDER BY d.created_at ASC",
    )?;
    let pending: Vec<(String, String, String, i32, String, String)> = stmt
        .query_map(params![now], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt); // Release borrow before async loop

    let mut processed = 0;
    for (delivery_id, webhook_id, payload, attempt_count, url, secret) in &pending {
        let attempt = attempt_count + 1;
        match dispatch_delivery_http(url, secret, delivery_id, payload).await {
            Ok(true) => {
                mark_delivered(&conn, delivery_id)?;
                record_success(&conn, webhook_id)?;
            }
            _ => {
                if attempt >= MAX_RETRY_ATTEMPTS {
                    mark_exhausted(&conn, delivery_id, attempt)?;
                } else {
                    mark_failed(&conn, delivery_id, attempt, None)?;
                }
                record_failure(&conn, webhook_id, None)?;
            }
        }
        processed += 1;
    }
    if processed > 0 {
        info!(target: "4da::webhooks", processed, "Processed pending deliveries");
    }
    Ok(processed)
}

/// Calculate the next retry timestamp for a given attempt number (1-indexed).
pub fn next_retry_at(attempt: i32) -> String {
    let idx = ((attempt - 1) as usize).min(RETRY_BACKOFF_SECS.len() - 1);
    let delay = chrono::Duration::seconds(RETRY_BACKOFF_SECS[idx]);
    (chrono::Utc::now() + delay)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

// ============================================================================
// Delivery Status Helpers
// ============================================================================

fn mark_delivered(conn: &Connection, delivery_id: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhook_deliveries SET status = 'delivered', delivered_at = ?1,
                attempt_count = attempt_count + 1 WHERE id = ?2",
        params![now, delivery_id],
    )?;
    Ok(())
}

fn mark_failed(
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

fn mark_exhausted(conn: &Connection, delivery_id: &str, attempt: i32) -> Result<()> {
    conn.execute(
        "UPDATE webhook_deliveries SET status = 'exhausted', attempt_count = ?1 WHERE id = ?2",
        params![attempt, delivery_id],
    )?;
    warn!(target: "4da::webhooks", delivery_id, "Delivery exhausted after {} attempts", attempt);
    Ok(())
}

fn record_success(conn: &Connection, webhook_id: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhooks SET failure_count = 0, last_fired_at = ?1, last_status_code = 200 WHERE id = ?2",
        params![now, webhook_id],
    )?;
    Ok(())
}

fn record_failure(conn: &Connection, webhook_id: &str, http_status: Option<i32>) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    conn.execute(
        "UPDATE webhooks SET failure_count = failure_count + 1, last_fired_at = ?1,
                last_status_code = ?2 WHERE id = ?3",
        params![now, http_status, webhook_id],
    )?;
    Ok(())
}

// ============================================================================
// Circuit Breaker
// ============================================================================

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

// ============================================================================
// Tauri Commands
// ============================================================================

/// Extract team_id from settings for webhook commands.
fn get_webhook_team_id() -> crate::error::Result<String> {
    let settings = crate::state::get_settings_manager().lock();
    let team_id = settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
        .unwrap_or_default();
    drop(settings);
    if team_id.is_empty() {
        return Err("Team not configured — webhooks require a team".into());
    }
    Ok(team_id)
}

#[tauri::command]
pub async fn register_webhook_cmd(
    name: String,
    url: String,
    events: Vec<String>,
) -> crate::error::Result<Webhook> {
    let team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;
    register_webhook(&conn, &team_id, &name, &url, &events, None)
        .map_err(|e| format!("Failed to register webhook: {e}").into())
}

#[tauri::command]
pub async fn list_webhooks_cmd() -> crate::error::Result<Vec<Webhook>> {
    let team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;
    list_webhooks(&conn, &team_id).map_err(|e| format!("Failed to list webhooks: {e}").into())
}

#[tauri::command]
pub async fn delete_webhook_cmd(webhook_id: String) -> crate::error::Result<()> {
    let _team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    delete_webhook(&conn, &webhook_id).map_err(|e| format!("Failed to delete webhook: {e}").into())
}

#[tauri::command]
pub async fn test_webhook_cmd(webhook_id: String) -> crate::error::Result<bool> {
    let _team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;

    let (url, secret): (String, String) = conn
        .query_row(
            "SELECT url, secret FROM webhooks WHERE id = ?1",
            params![webhook_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Webhook not found: {e}"))?;

    let test_payload = serde_json::json!({
        "event": "webhook.test",
        "timestamp": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "data": { "test": true, "webhook_id": webhook_id }
    });
    let payload_str =
        serde_json::to_string(&test_payload).map_err(|e| format!("Serialization failed: {e}"))?;
    let delivery_id = Uuid::new_v4().to_string();
    dispatch_delivery_http(&url, &secret, &delivery_id, &payload_str)
        .await
        .map_err(|e| format!("Test delivery failed: {e}").into())
}

#[tauri::command]
pub async fn get_webhook_deliveries_cmd(
    webhook_id: String,
    limit: Option<i64>,
) -> crate::error::Result<Vec<WebhookDelivery>> {
    let _team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;
    get_webhook_deliveries(&conn, &webhook_id, limit.unwrap_or(50))
        .map_err(|e| format!("Failed to get deliveries: {e}").into())
}

#[cfg(test)]
#[path = "webhooks_tests.rs"]
mod tests;
