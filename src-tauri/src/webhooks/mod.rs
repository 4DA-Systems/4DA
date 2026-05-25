// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
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

mod commands;
mod delivery;
mod dispatch;
mod management;
mod secrets;
mod types;

#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rusqlite::Connection;
use sha2::Sha256;

// Re-export public API at the same paths the rest of the crate expects.
pub use types::{Webhook, WebhookDelivery};

pub use commands::{
    delete_webhook_cmd, get_webhook_deliveries_cmd, list_webhooks_cmd, register_webhook_cmd,
    test_webhook_cmd,
};

pub(crate) use delivery::{check_circuit_breaker, next_retry_at, reset_circuit_breaker};
pub(crate) use dispatch::dispatch_webhook_event;
pub(crate) use management::{
    delete_webhook, get_webhook_deliveries, list_webhooks, register_webhook,
};

/// Create the webhook tables if they don't exist.
pub(crate) fn ensure_webhook_tables(conn: &Connection) -> Result<()> {
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

/// Sign a payload using HMAC-SHA256 (RFC 2104).
///
/// Returns the hex-encoded MAC. Used to populate the `X-4DA-Signature-256`
/// header in format `sha256=<hex>`.
pub(crate) fn sign_payload(secret: &str, body: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    // HMAC-SHA256 accepts any key length per RFC 2104; this cannot fail in practice.
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        // Fallback: return empty signature rather than panicking
        return String::new();
    };
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
