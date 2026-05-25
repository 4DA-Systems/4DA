// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tauri IPC command handlers for webhook management.

use rusqlite::params;
use uuid::Uuid;

use crate::audit::log_team_audit;

use super::dispatch::dispatch_delivery_http;
use super::ensure_webhook_tables;
use super::management::{delete_webhook, list_webhooks, register_webhook};
use super::secrets::read_webhook_secret;
use super::sign_payload;
use super::types::{Webhook, WebhookDelivery};

/// Extract team_id from settings for webhook commands.
pub(super) fn get_webhook_team_id() -> crate::error::Result<String> {
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
    // SSRF defense-in-depth. The frontend also validates the URL, but the
    // webhook dispatcher is a high-risk outbound path (arbitrary user-supplied
    // target + signed request body) so we must enforce at the backend too.
    // Blocks localhost, RFC1918, link-local, IPv6 loopback/ULA, file:// etc.
    // Ref: docs/ADVERSARIAL-AUDIT-2026-04-19.md P1 "SSRF protections exist
    // but are not enforced in webhook registration".
    let url = crate::ipc_guard::validate_url_safe_for_request("url", &url)
        .map_err(|e| format!("Invalid webhook URL: {e}"))?;

    // Name: trim + length cap + reject control chars. Cheap and close to
    // the boundary so downstream formatting/logging cannot be abused.
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 100 {
        return Err("Webhook name must be 1-100 characters".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("Webhook name must not contain control characters".into());
    }

    // Event list: sanity cap and per-string cap. Prevents pathological
    // event arrays from bloating the audit log or the webhooks table.
    if events.is_empty() || events.len() > 50 {
        return Err("Webhook must subscribe to 1-50 events".into());
    }
    for ev in &events {
        if ev.is_empty() || ev.len() > 100 {
            return Err("Each event name must be 1-100 characters".into());
        }
    }

    let team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;
    let webhook = register_webhook(&conn, &team_id, &name, &url, &events, None)
        .map_err(|e| format!("Failed to register webhook: {e}"))?;

    // Audit: webhook created
    log_team_audit(&conn, "webhook.created", "webhook", Some(&webhook.id), None);

    Ok(webhook)
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
    delete_webhook(&conn, &webhook_id).map_err(|e| format!("Failed to delete webhook: {e}"))?;

    // Audit: webhook deleted
    log_team_audit(&conn, "webhook.deleted", "webhook", Some(&webhook_id), None);

    Ok(())
}

#[tauri::command]
pub async fn test_webhook_cmd(webhook_id: String) -> crate::error::Result<bool> {
    let _team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;

    let url: String = conn
        .query_row(
            "SELECT url FROM webhooks WHERE id = ?1",
            params![webhook_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Webhook not found: {e}"))?;
    let secret = read_webhook_secret(&conn, &webhook_id)
        .map_err(|e| format!("Get webhook secret for test: {e}"))?;

    let test_payload = serde_json::json!({
        "event": "webhook.test",
        "timestamp": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "data": { "test": true, "webhook_id": webhook_id }
    });
    let payload_str =
        serde_json::to_string(&test_payload).map_err(|e| format!("Serialization failed: {e}"))?;
    let delivery_id = Uuid::new_v4().to_string();
    let result = dispatch_delivery_http(&url, &secret, &delivery_id, &payload_str)
        .await
        .map_err(|e| format!("Test delivery failed: {e}"))?;

    // Audit: webhook tested
    log_team_audit(&conn, "webhook.tested", "webhook", Some(&webhook_id), None);

    Ok(result)
}

#[tauri::command]
pub async fn get_webhook_deliveries_cmd(
    webhook_id: String,
    limit: Option<i64>,
) -> crate::error::Result<Vec<WebhookDelivery>> {
    let _team_id = get_webhook_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_webhook_tables(&conn).map_err(|e| format!("Schema init failed: {e}"))?;
    super::management::get_webhook_deliveries(&conn, &webhook_id, limit.unwrap_or(50))
        .map_err(|e| format!("Failed to get deliveries: {e}").into())
}
