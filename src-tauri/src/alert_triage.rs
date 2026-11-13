// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Alert triage — persistent security triage actions with SQLite storage.
//!
//! One store for every surface that dismisses an alert (audit 2026-11-11):
//! the critical banner (source items, `item:<id>`) and the Preemption list
//! (evidence items, `evidence:<id>`) both write here, keyed by `alert_key`.
//! Every decision expires so an alert that is still true comes back, and
//! `untriage_alert` undoes one. How long a decision holds depends on what it
//! says ([`default_days`]): a deliberate "not applicable" or "accept the
//! risk" holds [`DELIBERATE_DAYS`] (90) so it does not nag weekly;
//! investigating, snoozed, acknowledged and fixed hold [`DISMISSAL_DAYS`] (7).

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

/// How long a triage decision holds when the caller names no expiry.
pub(crate) const DISMISSAL_DAYS: i64 = 7;

/// How long a deliberate judgement holds: the user looked and decided the
/// alert does not apply, or accepted the risk (product decision 2026-11-13).
pub(crate) const DELIBERATE_DAYS: i64 = 90;

/// Default lifetime of a decision with no explicit expiry.
pub(crate) fn default_days(action: &str) -> i64 {
    match action {
        "not_applicable" | "accepted_risk" => DELIBERATE_DAYS,
        _ => DISMISSAL_DAYS,
    }
}

/// Prefix of the Preemption (evidence item) keys.
pub(crate) const EVIDENCE_KEY_PREFIX: &str = "evidence:";

/// The store key for an alert: the caller's `alert_key`, else `item:<id>`.
fn store_key(item_id: Option<i64>, alert_key: Option<String>) -> Result<String> {
    match (alert_key.filter(|k| !k.trim().is_empty()), item_id) {
        (Some(key), _) => Ok(key),
        (None, Some(id)) => Ok(format!("item:{id}")),
        (None, None) => Err(crate::error::FourDaError::Validation(
            "A triage needs an item id or an alert key".into(),
        )),
    }
}

/// Phase 131: rebuild `alert_triage` keyed by `alert_key` (item id
/// optional), carrying every row as `item:<id>`. A row with no expiry gets
/// one counted from when it was made, by [`default_days`]: 90 days for
/// not_applicable / accepted_risk, 7 for the rest. "Investigate" used to
/// triage with no expiry and hid the alert for good.
pub(crate) fn migrate_to_alert_keys(c: &rusqlite::Connection) -> rusqlite::Result<()> {
    c.execute_batch(&format!(
        "CREATE TABLE alert_triage_v131 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            alert_key TEXT NOT NULL UNIQUE,
            item_id INTEGER,
            advisory_id TEXT,
            action TEXT NOT NULL CHECK(action IN ('investigating', 'fixed', 'not_applicable', 'accepted_risk', 'snoozed', 'acknowledged')),
            reason TEXT,
            resolved_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );
        INSERT INTO alert_triage_v131 (id, alert_key, item_id, advisory_id, action, reason, resolved_at, expires_at)
            SELECT id, 'item:' || item_id, item_id, advisory_id, action, reason, resolved_at,
                   COALESCE(expires_at, datetime(resolved_at, '+' ||
                       CASE WHEN action IN ('not_applicable', 'accepted_risk')
                            THEN {DELIBERATE_DAYS} ELSE {DISMISSAL_DAYS} END || ' days'))
            FROM alert_triage;
        DROP TABLE alert_triage;
        ALTER TABLE alert_triage_v131 RENAME TO alert_triage;
        CREATE INDEX IF NOT EXISTS idx_alert_triage_item ON alert_triage(item_id);
        CREATE INDEX IF NOT EXISTS idx_alert_triage_expires ON alert_triage(expires_at) WHERE expires_at IS NOT NULL;"
    ))
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Triage an alert — persist the decision. Re-triaging the same alert
/// replaces the previous decision. Without `expires_at` it holds for
/// [`default_days`] for its action.
#[tauri::command]
pub async fn triage_alert(
    item_id: Option<i64>,
    alert_key: Option<String>,
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
    let key = store_key(item_id, alert_key)?;
    let conn = crate::open_db_connection()?;
    let decision = Decision {
        key: &key,
        item_id,
        action: &action,
        advisory_id: advisory_id.as_deref(),
        reason: reason.as_deref(),
        expires_at: expires_at.as_deref(),
    };
    record(&conn, &decision).context("Failed to persist triage decision")?;

    // Log security event for audit trail
    if let Ok(db) = crate::get_database() {
        db.log_security_event(
            "alert_triage",
            &format!(
                "alert={} action={} advisory={}",
                key,
                action,
                advisory_id.as_deref().unwrap_or("none")
            ),
            "info",
        );
    }

    Ok(())
}

/// One triage decision as stored.
struct Decision<'a> {
    key: &'a str,
    item_id: Option<i64>,
    action: &'a str,
    advisory_id: Option<&'a str>,
    reason: Option<&'a str>,
    expires_at: Option<&'a str>,
}

fn record(conn: &rusqlite::Connection, d: &Decision<'_>) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR REPLACE INTO alert_triage
             (alert_key, item_id, advisory_id, action, reason, resolved_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'),
                 COALESCE(?6, datetime('now', '+' || ?7 || ' days')))",
        params![
            d.key,
            d.item_id,
            d.advisory_id,
            d.action,
            d.reason,
            d.expires_at,
            default_days(d.action)
        ],
    )
}

/// Undo a triage decision: the alert shows again at once.
#[tauri::command]
pub async fn untriage_alert(item_id: Option<i64>, alert_key: Option<String>) -> Result<()> {
    let key = store_key(item_id, alert_key)?;
    let conn = crate::open_db_connection()?;
    conn.execute("DELETE FROM alert_triage WHERE alert_key = ?1", [&key])
        .context("Failed to undo triage decision")?;
    Ok(())
}

/// Get triage states for a batch of item IDs.
/// Only returns non-expired records (expired decisions resurface automatically).
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

/// The evidence ids dismissed from Preemption whose dismissal has not
/// expired — the visibility filter's input (AD-035).
pub(crate) fn live_evidence_dismissals(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT substr(alert_key, length(?1) + 1) FROM alert_triage
         WHERE substr(alert_key, 1, length(?1)) = ?1
           AND action <> 'investigating'
           AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))",
    )?;
    let rows = stmt.query_map([EVIDENCE_KEY_PREFIX], |r| r.get(0))?;
    rows.collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// The Phase 59 table, as every database before Phase 131 holds it.
    fn setup_v130_db() -> Connection {
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

    fn setup_test_db() -> Connection {
        let conn = setup_v130_db();
        migrate_to_alert_keys(&conn).unwrap();
        conn
    }

    fn decide<'a>(key: &'a str, item_id: Option<i64>, action: &'a str) -> Decision<'a> {
        Decision {
            key,
            item_id,
            action,
            advisory_id: None,
            reason: None,
            expires_at: None,
        }
    }

    fn live_count(conn: &Connection, key: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM alert_triage WHERE alert_key = ?1
               AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))",
            [key],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Audit 2026-11-11: "Investigate" triaged with no expiry, so opening a
    /// link hid the alert forever. Old rows get an expiry a week after they
    /// were made; nothing is lost.
    #[test]
    fn migration_keys_old_rows_and_expires_the_ones_that_never_would() {
        let conn = setup_v130_db();
        conn.execute_batch(
            "INSERT INTO alert_triage (item_id, advisory_id, action, resolved_at)
                 VALUES (1, 'GHSA-1', 'investigating', datetime('now', '-30 days'));
             INSERT INTO alert_triage (item_id, action, resolved_at)
                 VALUES (2, 'fixed', datetime('now', '-1 day'));
             INSERT INTO alert_triage (item_id, action, expires_at)
                 VALUES (3, 'snoozed', datetime('now', '+2 days'));
             INSERT INTO alert_triage (item_id, action, resolved_at)
                 VALUES (4, 'not_applicable', datetime('now', '-30 days'));",
        )
        .unwrap();
        migrate_to_alert_keys(&conn).unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM alert_triage", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(
            live_count(&conn, "item:1"),
            0,
            "a month-old Investigate no longer hides"
        );
        assert_eq!(
            live_count(&conn, "item:2"),
            1,
            "yesterday's Fixed still holds"
        );
        assert_eq!(live_count(&conn, "item:3"), 1, "an explicit expiry is kept");
        assert_eq!(
            live_count(&conn, "item:4"),
            1,
            "a month-old Not applicable holds for 90 days"
        );
    }

    fn days_held(conn: &Connection, key: &str) -> f64 {
        conn.query_row(
            "SELECT julianday(expires_at) - julianday('now') FROM alert_triage WHERE alert_key = ?1",
            [key],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Product decision 2026-11-13: a deliberate "not applicable" or an
    /// accepted risk holds 90 days so it does not nag weekly; everything
    /// else holds 7.
    #[test]
    fn every_decision_expires_by_default_and_undo_removes_it() {
        let conn = setup_test_db();
        for (key, action, days) in [
            ("item:9", "not_applicable", DELIBERATE_DAYS),
            ("item:10", "accepted_risk", DELIBERATE_DAYS),
            ("item:11", "acknowledged", DISMISSAL_DAYS),
            ("item:12", "snoozed", DISMISSAL_DAYS),
            ("item:13", "investigating", DISMISSAL_DAYS),
        ] {
            record(&conn, &decide(key, None, action)).unwrap();
            let held = days_held(&conn, key);
            assert!((held - days as f64).abs() < 0.01, "{action}: {held}");
        }
        assert_eq!(DELIBERATE_DAYS, 90);
        conn.execute("DELETE FROM alert_triage WHERE alert_key = ?1", ["item:9"])
            .unwrap();
        assert_eq!(live_count(&conn, "item:9"), 0);
    }

    /// One store: Preemption dismissals (no source item) sit beside banner
    /// rows and are read back as evidence ids.
    #[test]
    fn preemption_dismissals_share_the_store() {
        let conn = setup_test_db();
        record(&conn, &decide("item:4", Some(4), "acknowledged")).unwrap();
        record(
            &conn,
            &decide("evidence:osv-pkg-lodash-npm", None, "acknowledged"),
        )
        .unwrap();
        let mut expired = decide("evidence:osv-pkg-old-npm", None, "acknowledged");
        let past = "2020-01-01 00:00:00";
        expired.expires_at = Some(past);
        record(&conn, &expired).unwrap();
        assert_eq!(
            live_evidence_dismissals(&conn).unwrap(),
            vec!["osv-pkg-lodash-npm".to_string()]
        );
    }

    #[test]
    fn a_key_is_the_alert_key_or_the_item() {
        assert_eq!(store_key(Some(5), None).unwrap(), "item:5");
        assert_eq!(
            store_key(None, Some("evidence:x".into())).unwrap(),
            "evidence:x"
        );
        assert!(store_key(None, None).is_err());
    }

    #[test]
    fn test_invalid_action_rejected_by_db() {
        let conn = setup_test_db();
        let result = record(&conn, &decide("item:1", Some(1), "invalid_action"));
        assert!(result.is_err());
    }
}
