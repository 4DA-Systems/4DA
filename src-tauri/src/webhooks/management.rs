// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Webhook CRUD operations (register, list, delete, query deliveries).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use tracing::info;
use uuid::Uuid;

use super::secrets::{forget_webhook_secret, persist_webhook_secret};
use super::types::{Webhook, WebhookDelivery};

/// Register a new webhook for a team.
pub(crate) fn register_webhook(
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

    // Insert with the plaintext secret first so the NOT NULL constraint is
    // satisfied, then `persist_webhook_secret` moves it to the keychain and
    // blanks the DB column on success. If the keychain is down, the DB keeps
    // the plaintext and dispatch still works -- identical posture to the
    // other API keys (see settings::keystore).
    conn.execute(
        "INSERT INTO webhooks (id, team_id, name, url, events, secret, active, failure_count, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, ?8)",
        params![id, team_id, name, url, events_json, secret, now, created_by],
    ).context("Insert webhook")?;
    persist_webhook_secret(conn, &id, &secret)?;

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
pub(crate) fn list_webhooks(conn: &Connection, team_id: &str) -> Result<Vec<Webhook>> {
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
pub(crate) fn delete_webhook(conn: &Connection, webhook_id: &str) -> Result<()> {
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
    // Scrub the keychain entry too. Tolerant of keychain unavailability.
    forget_webhook_secret(webhook_id);
    info!(target: "4da::webhooks", webhook_id = %webhook_id, "Webhook deleted");
    Ok(())
}

/// Get recent deliveries for a webhook.
pub(crate) fn get_webhook_deliveries(
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
