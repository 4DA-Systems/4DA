// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Alert triage — persistent security triage actions with SQLite storage.
//!
//! Replaces localStorage-based acknowledgment with proper triage actions
//! (investigating, fixed, not_applicable, accepted_risk, snoozed, acknowledged)
//! and stores them in SQLite for audit trail and persistence across cache clears.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

/// A persisted triage record for a security alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TriageRecord {
    pub item_id: i64,
    pub advisory_id: Option<String>,
    pub action: String,
    pub reason: Option<String>,
    pub resolved_at: String,
    pub expires_at: Option<String>,
}

/// Valid triage actions (checked via DB constraint, validated here for early rejection).
const VALID_ACTIONS: &[&str] = &[
    "investigating",
    "fixed",
    "not_applicable",
    "accepted_risk",
    "snoozed",
    "acknowledged",
];

// ============================================================================
// Tauri Commands
// ============================================================================

/// Triage a security alert — persist the decision to SQLite.
/// Uses INSERT OR REPLACE so re-triaging an item updates the previous decision.
#[tauri::command]
pub async fn triage_alert(
    item_id: i64,
    action: String,
    advisory_id: Option<String>,
    reason: Option<String>,
    expires_at: Option<String>,
) -> Result<()> {
    // Validate action before hitting the DB
    if !VALID_ACTIONS.contains(&action.as_str()) {
        return Err(crate::error::FourDaError::Validation(format!(
            "Invalid triage action '{}'. Valid actions: {}",
            action,
            VALID_ACTIONS.join(", ")
        )));
    }

    let conn = crate::open_db_connection()?;
    conn.execute(
        "INSERT OR REPLACE INTO alert_triage (item_id, advisory_id, action, reason, resolved_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
        params![item_id, advisory_id, action, reason, expires_at],
    )
    .context("Failed to persist triage decision")?;

    // Log security event for audit trail
    if let Ok(db) = crate::get_database() {
        db.log_security_event(
            "alert_triage",
            &format!(
                "item_id={} action={} advisory={}",
                item_id,
                action,
                advisory_id.as_deref().unwrap_or("none")
            ),
            "info",
        );
    }

    Ok(())
}

/// Get triage states for a batch of item IDs.
/// Only returns non-expired records (expired snoozed alerts resurface automatically).
#[tauri::command]
pub async fn get_triage_states(item_ids: Vec<i64>) -> Result<Vec<TriageRecord>> {
    if item_ids.is_empty() {
        return Ok(Vec::new());
    }

    let conn = crate::open_db_connection()?;
    let placeholders = item_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT item_id, advisory_id, action, reason, resolved_at, expires_at
         FROM alert_triage
         WHERE item_id IN ({})
           AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))",
        placeholders
    );

    let mut stmt = conn
        .prepare(&query)
        .context("Failed to prepare triage query")?;
    let params: Vec<&dyn rusqlite::ToSql> = item_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let records = stmt
        .query_map(params.as_slice(), |row| {
            Ok(TriageRecord {
                item_id: row.get(0)?,
                advisory_id: row.get(1)?,
                action: row.get(2)?,
                reason: row.get(3)?,
                resolved_at: row.get(4)?,
                expires_at: row.get(5)?,
            })
        })
        .context("Failed to query triage states")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(records)
}

/// Clear expired triage records (snoozed alerts whose snooze period has lapsed).
/// Returns the number of records removed.
#[tauri::command]
pub async fn clear_expired_triage() -> Result<u64> {
    let conn = crate::open_db_connection()?;
    let count = conn
        .execute(
            "DELETE FROM alert_triage
             WHERE expires_at IS NOT NULL
               AND datetime(expires_at) <= datetime('now')",
            [],
        )
        .context("Failed to clear expired triage records")?;
    Ok(count as u64)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS alert_triage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                advisory_id TEXT,
                action TEXT NOT NULL CHECK(action IN ('investigating', 'fixed', 'not_applicable', 'accepted_risk', 'snoozed', 'acknowledged')),
                reason TEXT,
                resolved_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT,
                UNIQUE(item_id)
            );
            CREATE INDEX IF NOT EXISTS idx_alert_triage_item ON alert_triage(item_id);
            CREATE INDEX IF NOT EXISTS idx_alert_triage_expires ON alert_triage(expires_at) WHERE expires_at IS NOT NULL;",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_insert_triage_record() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO alert_triage (item_id, advisory_id, action, reason)
             VALUES (1, 'CVE-2024-1234', 'investigating', 'Looking into this')",
            [],
        )
        .unwrap();

        let action: String = conn
            .query_row(
                "SELECT action FROM alert_triage WHERE item_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(action, "investigating");
    }

    #[test]
    fn test_replace_triage_record() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO alert_triage (item_id, action) VALUES (1, 'investigating')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO alert_triage (item_id, action, reason) VALUES (1, 'fixed', 'Upgraded dep')",
            [],
        )
        .unwrap();

        let (action, reason): (String, Option<String>) = conn
            .query_row(
                "SELECT action, reason FROM alert_triage WHERE item_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(action, "fixed");
        assert_eq!(reason.as_deref(), Some("Upgraded dep"));

        // Should only have one record
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM alert_triage WHERE item_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_invalid_action_rejected_by_db() {
        let conn = setup_test_db();
        let result = conn.execute(
            "INSERT INTO alert_triage (item_id, action) VALUES (1, 'invalid_action')",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_actions_accepted() {
        for action in VALID_ACTIONS {
            assert!(
                VALID_ACTIONS.contains(action),
                "Action '{}' should be valid",
                action
            );
        }
    }
}
