// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Fire-and-forget webhook dispatch engine.

use anyhow::{Context, Result};
use tracing::{info, warn};

use super::commands::get_webhook_team_id;
use super::delivery::{
    check_circuit_breaker, mark_delivered, mark_failed, record_delivery, record_failure,
    record_success,
};
use super::ensure_webhook_tables;
use super::management::list_webhooks;
use super::secrets::read_webhook_secret;
use super::sign_payload;
use super::types::WebhookPayload;

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

/// Send HTTP POST to the webhook URL.
/// Returns `Ok(true)` on 2xx, `Ok(false)` on non-2xx, `Err` on network failure.
pub(super) async fn dispatch_delivery_http(
    url: &str,
    secret: &str,
    delivery_id: &str,
    payload: &str,
) -> Result<bool> {
    let signature = sign_payload(secret, payload);
    let client = crate::http_client::HTTP_CLIENT.clone();

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

/// Dispatch a webhook event to all matching, active webhooks for the current team.
///
/// This is **fire-and-forget** -- errors are logged but never propagated to the caller.
/// Call this from any module after a significant event occurs.
///
/// The function is synchronous: it reads webhooks from the database on the current
/// thread, then spawns a tokio task per matching webhook for the actual HTTP delivery.
/// Callers do not need to `.await` anything.
///
/// # Example
///
/// ```rust,no_run
/// crate::webhooks::dispatch_webhook_event("signal.detected", &serde_json::json!({
///     "signal_id": "abc",
///     "severity": "high",
///     "topic": "security"
/// }));
/// ```
pub(crate) fn dispatch_webhook_event(event_type: &str, data: &serde_json::Value) {
    // 1. Read team_id from settings -- return early if no team configured
    let team_id = match get_webhook_team_id() {
        Ok(id) => id,
        Err(_) => return, // No team configured -- nothing to dispatch
    };

    // 2. Open DB connection
    let conn = match crate::state::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::webhooks", "dispatch_webhook_event: DB connection failed: {e}");
            return;
        }
    };

    // Ensure tables exist (idempotent)
    if let Err(e) = ensure_webhook_tables(&conn) {
        warn!(target: "4da::webhooks", "dispatch_webhook_event: table init failed: {e}");
        return;
    }

    // 3. Query active webhooks for this team that match the event
    let webhooks = match list_webhooks(&conn, &team_id) {
        Ok(w) => w,
        Err(e) => {
            warn!(target: "4da::webhooks", "dispatch_webhook_event: list_webhooks failed: {e}");
            return;
        }
    };

    let matching: Vec<_> = webhooks
        .iter()
        .filter(|w| w.active && event_matches(&w.events, event_type))
        .collect();

    if matching.is_empty() {
        return;
    }

    // Build the event envelope
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let envelope = WebhookPayload {
        event: event_type.to_string(),
        timestamp,
        data: data.clone(),
    };
    let payload_json = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::webhooks", "dispatch_webhook_event: payload serialization failed: {e}");
            return;
        }
    };

    // 4. For each matching webhook: sign, record, and spawn async delivery
    for webhook in &matching {
        // Check circuit breaker
        match check_circuit_breaker(&conn, &webhook.id) {
            Ok(true) => {
                warn!(target: "4da::webhooks", webhook_id = %webhook.id, "dispatch: circuit breaker open — skipping");
                continue;
            }
            Err(e) => {
                warn!(target: "4da::webhooks", webhook_id = %webhook.id, "dispatch: circuit breaker check failed: {e}");
                continue;
            }
            Ok(false) => {} // Circuit closed, proceed
        }

        // Read the secret for signing (keychain first, DB fallback)
        let secret = match read_webhook_secret(&conn, &webhook.id) {
            Ok(s) => s,
            Err(e) => {
                warn!(target: "4da::webhooks", webhook_id = %webhook.id, "dispatch: failed to read secret: {e}");
                continue;
            }
        };

        let signature = sign_payload(&secret, &payload_json);

        // Create a pending delivery record
        let delivery_id = match record_delivery(
            &conn,
            &webhook.id,
            event_type,
            &payload_json,
            "pending",
            None,
        ) {
            Ok(id) => id,
            Err(e) => {
                warn!(target: "4da::webhooks", webhook_id = %webhook.id, "dispatch: failed to record delivery: {e}");
                continue;
            }
        };

        // Clone values for the spawned task
        let wh_id = webhook.id.clone();
        let wh_url = webhook.url.clone();
        let del_id = delivery_id.clone();
        let payload = payload_json.clone();
        let sig = signature.clone();
        let evt = event_type.to_string();

        // Spawn async delivery -- fire and forget
        tokio::spawn(async move {
            let result = deliver_webhook(&del_id, &wh_url, &payload, &sig, &evt).await;

            // Open a fresh connection in the async context for status updates
            let conn = match crate::state::open_db_connection() {
                Ok(c) => c,
                Err(e) => {
                    warn!(target: "4da::webhooks", delivery_id = %del_id, "async delivery: DB reconnect failed: {e}");
                    return;
                }
            };

            match result {
                Ok(status) if (200..300).contains(&status) => {
                    // Success: mark delivered, reset failure count
                    if let Err(e) = mark_delivered(&conn, &del_id) {
                        warn!(target: "4da::webhooks", delivery_id = %del_id, "failed to mark delivered: {e}");
                    }
                    if let Err(e) = record_success(&conn, &wh_id) {
                        warn!(target: "4da::webhooks", webhook_id = %wh_id, "failed to record success: {e}");
                    }
                    info!(target: "4da::webhooks", webhook_id = %wh_id, delivery_id = %del_id, status, "Webhook delivered");
                }
                Ok(status) => {
                    // Non-2xx response: mark failed, increment failure count
                    if let Err(e) = mark_failed(&conn, &del_id, 1, Some(status as i32)) {
                        warn!(target: "4da::webhooks", delivery_id = %del_id, "failed to mark failed: {e}");
                    }
                    if let Err(e) = record_failure(&conn, &wh_id, Some(status as i32)) {
                        warn!(target: "4da::webhooks", webhook_id = %wh_id, "failed to record failure: {e}");
                    }
                    warn!(target: "4da::webhooks", webhook_id = %wh_id, delivery_id = %del_id, status, "Webhook delivery got non-2xx");

                    // Check if we should trip the circuit breaker
                    if let Err(e) = check_circuit_breaker(&conn, &wh_id) {
                        warn!(target: "4da::webhooks", webhook_id = %wh_id, "circuit breaker check failed: {e}");
                    }
                }
                Err(e) => {
                    // Network/timeout error: mark failed, increment failure count
                    if let Err(e2) = mark_failed(&conn, &del_id, 1, None) {
                        warn!(target: "4da::webhooks", delivery_id = %del_id, "failed to mark failed: {e2}");
                    }
                    if let Err(e2) = record_failure(&conn, &wh_id, None) {
                        warn!(target: "4da::webhooks", webhook_id = %wh_id, "failed to record failure: {e2}");
                    }
                    warn!(target: "4da::webhooks", webhook_id = %wh_id, delivery_id = %del_id, error = %e, "Webhook delivery failed");

                    // Check if we should trip the circuit breaker
                    if let Err(e2) = check_circuit_breaker(&conn, &wh_id) {
                        warn!(target: "4da::webhooks", webhook_id = %wh_id, "circuit breaker check failed: {e2}");
                    }
                }
            }
        });
    }

    info!(
        target: "4da::webhooks",
        event_type,
        team_id = %team_id,
        count = matching.len(),
        "Webhook event dispatched"
    );
}

/// Deliver a webhook payload via HTTP POST.
///
/// Returns the HTTP status code on success, or an error on network/timeout failure.
/// Timeout is 10 seconds to prevent blocking the async pool for too long.
async fn deliver_webhook(
    delivery_id: &str,
    url: &str,
    payload: &str,
    signature: &str,
    event_type: &str,
) -> Result<u16> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Build HTTP client")?;

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-4DA-Signature-256", format!("sha256={signature}"))
        .header("X-4DA-Event", event_type)
        .header("X-4DA-Delivery", delivery_id)
        .header("User-Agent", "4DA-Webhook/1.0")
        .body(payload.to_string())
        .send()
        .await
        .context("Webhook HTTP POST failed")?;

    Ok(response.status().as_u16())
}
