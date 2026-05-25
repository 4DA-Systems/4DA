// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Webhook signing-secret storage (keychain with DB fallback).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use tracing::{info, warn};

use crate::settings::keystore;

/// Keychain key-name for a webhook's signing secret.
fn webhook_keystore_key(webhook_id: &str) -> String {
    format!("webhook_secret__{webhook_id}")
}

/// Persist a webhook signing secret. Tries the platform keychain first; if the
/// keychain accepts the write AND a read-back returns the same value, the DB
/// column is blanked (the schema requires NOT NULL, so empty-string is the
/// migrated sentinel). Otherwise the plaintext stays in the DB column so
/// dispatch can still sign -- same graceful-degradation posture as
/// `keystore::store_secret` for the other API keys.
///
/// The write-then-read-back check is load-bearing: some keyring backends
/// (observed on certain Windows Credential Manager configurations and on CI
/// hosts with incomplete DBus setup) return `Ok(())` from `set_password` but
/// then return `NoEntry` from the next `get_password`. Trusting the write
/// without verification would silently lose the secret.
pub(super) fn persist_webhook_secret(
    conn: &Connection,
    webhook_id: &str,
    secret: &str,
) -> Result<()> {
    let key = webhook_keystore_key(webhook_id);
    let written =
        keystore::store_secret(&key, secret).context("Write webhook secret to keychain")?;
    let round_trip_ok = if written {
        matches!(
            keystore::get_secret(&key).context("Verify keychain round-trip")?,
            Some(ref v) if v == secret
        )
    } else {
        false
    };
    let db_value = if round_trip_ok { "" } else { secret };
    conn.execute(
        "UPDATE webhooks SET secret = ?1 WHERE id = ?2",
        params![db_value, webhook_id],
    )
    .context("Update webhook DB secret column")?;
    if round_trip_ok {
        info!(target: "4da::webhooks", webhook_id = %webhook_id, "Webhook secret stored in keychain (verified)");
    } else if written {
        warn!(target: "4da::webhooks", webhook_id = %webhook_id, "Keychain accepted the write but read-back failed — keeping plaintext DB fallback");
    } else {
        warn!(target: "4da::webhooks", webhook_id = %webhook_id, "Keychain unavailable — webhook secret remains in plaintext DB fallback");
    }
    Ok(())
}

/// Read a webhook's signing secret. Checks the keychain first; falls back to
/// the DB column for rows that pre-date this migration or for hosts without a
/// keychain. If the DB column holds a plaintext secret (i.e. keychain was down
/// at registration), this call opportunistically migrates it into the keychain
/// and clears the DB column -- so each dispatch is a self-healing step for any
/// remaining plaintext rows.
///
/// Returns an error only if neither source has the secret, which means the
/// webhook cannot sign and dispatch should skip it.
pub(super) fn read_webhook_secret(conn: &Connection, webhook_id: &str) -> Result<String> {
    let key = webhook_keystore_key(webhook_id);
    if let Some(secret) = keystore::get_secret(&key).context("Read webhook secret from keychain")? {
        return Ok(secret);
    }
    let db_secret: String = conn
        .query_row(
            "SELECT secret FROM webhooks WHERE id = ?1",
            params![webhook_id],
            |row| row.get(0),
        )
        .context("Read webhook secret from DB")?;
    if db_secret.is_empty() {
        anyhow::bail!(
            "Webhook {} has no signing secret (keychain empty and DB column empty)",
            webhook_id
        );
    }
    // Lazy migration: try to move plaintext into keychain with the same
    // write-then-read-back verify as the explicit persist path. If the
    // round-trip fails we just keep returning from DB on future reads until
    // the keychain becomes available.
    if let Ok(true) = keystore::store_secret(&key, &db_secret) {
        let verified = matches!(
            keystore::get_secret(&key),
            Ok(Some(ref v)) if v == &db_secret
        );
        if verified {
            let _ = conn.execute(
                "UPDATE webhooks SET secret = '' WHERE id = ?1",
                params![webhook_id],
            );
            info!(target: "4da::webhooks", webhook_id = %webhook_id, "Lazy-migrated webhook secret to keychain (verified)");
        }
    }
    Ok(db_secret)
}

/// Delete a webhook's secret from the keychain. Never errors -- matches the
/// posture of `keystore::delete_secret` which also tolerates keychain
/// unavailability.
pub(super) fn forget_webhook_secret(webhook_id: &str) {
    let _ = keystore::delete_secret(&webhook_keystore_key(webhook_id));
}
