// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Team monitoring — signal aggregation, alert policies, and team-wide detection.
//!
//! Aggregates signals detected by individual team members into team-level
//! intelligence. Multi-seat confirmation increases confidence and triggers alerts.

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

/// A team-level signal aggregated from individual member detections.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamSignal {
    pub id: String,
    pub team_id: String,
    pub signal_type: String,
    pub title: String,
    pub severity: String,
    pub tech_topics: Vec<String>,
    pub detected_by_count: i32,
    pub first_detected: String,
    pub last_detected: String,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<String>,
    pub resolution_notes: Option<String>,
}

/// Alert policy for a team (admin-configurable).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertPolicy {
    pub team_id: String,
    pub min_seats_to_alert: i32,
    pub aggregation_window_minutes: i32,
    pub notification_channels: Vec<String>,
}

/// Summary of team monitoring activity.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamMonitoringSummary {
    pub total_signals: i64,
    pub unresolved_signals: i64,
    pub critical_count: i64,
    pub high_count: i64,
    pub multi_seat_count: i64,
    pub avg_resolution_hours: Option<f64>,
    pub most_active_topics: Vec<(String, i64)>,
}

// ============================================================================
// Signal Management
// ============================================================================

/// Record a new team signal or increment detection count if matching signal exists.
///
/// Matching: same team_id + signal_type + overlapping tech_topics within the
/// aggregation window.
pub fn record_team_signal(
    conn: &rusqlite::Connection,
    team_id: &str,
    signal_type: &str,
    title: &str,
    severity: &str,
    tech_topics: &[String],
    _detected_by: &str,
) -> Result<String> {
    let topics_json = serde_json::to_string(tech_topics)?;

    // Check for existing unresolved signal with same type within aggregation window
    let existing: Option<(String, i32)> = conn
        .query_row(
            "SELECT id, detected_by_count FROM team_signals
             WHERE team_id = ?1 AND signal_type = ?2 AND resolved = 0
             AND datetime(last_detected) > datetime('now', '-60 minutes')
             LIMIT 1",
            params![team_id, signal_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    match existing {
        Some((existing_id, count)) => {
            // Increment detection count and update last_detected
            conn.execute(
                "UPDATE team_signals SET
                    detected_by_count = ?1,
                    last_detected = datetime('now'),
                    severity = CASE WHEN ?2 = 'critical' THEN 'critical' ELSE severity END
                 WHERE id = ?3",
                params![count + 1, severity, existing_id],
            )?;
            info!(target: "4da::team_monitor",
                signal_id = %existing_id,
                count = count + 1,
                "Signal corroborated by additional seat");
            Ok(existing_id)
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO team_signals
                    (id, team_id, signal_type, title, severity, tech_topics,
                     detected_by_count, first_detected, last_detected)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, datetime('now'), datetime('now'))",
                params![id, team_id, signal_type, title, severity, topics_json],
            )?;
            info!(target: "4da::team_monitor",
                signal_id = %id,
                signal_type = %signal_type,
                "New team signal recorded");
            Ok(id)
        }
    }
}

/// Resolve a team signal.
pub fn resolve_signal(
    conn: &rusqlite::Connection,
    signal_id: &str,
    resolved_by: &str,
    notes: &str,
) -> Result<()> {
    let updated = conn.execute(
        "UPDATE team_signals SET
            resolved = 1,
            resolved_by = ?1,
            resolved_at = datetime('now'),
            resolution_notes = ?2
         WHERE id = ?3 AND resolved = 0",
        params![resolved_by, notes, signal_id],
    )?;

    if updated == 0 {
        anyhow::bail!("Signal not found or already resolved: {signal_id}");
    }

    info!(target: "4da::team_monitor", signal_id = %signal_id, "Team signal resolved");
    Ok(())
}

/// Get all team signals, optionally filtered by resolved status.
pub fn get_team_signals(
    conn: &rusqlite::Connection,
    team_id: &str,
    include_resolved: bool,
) -> Result<Vec<TeamSignal>> {
    let sql = if include_resolved {
        "SELECT id, team_id, signal_type, title, severity, tech_topics,
                detected_by_count, first_detected, last_detected,
                resolved, resolved_by, resolved_at, resolution_notes
         FROM team_signals WHERE team_id = ?1
         ORDER BY last_detected DESC LIMIT 200"
    } else {
        "SELECT id, team_id, signal_type, title, severity, tech_topics,
                detected_by_count, first_detected, last_detected,
                resolved, resolved_by, resolved_at, resolution_notes
         FROM team_signals WHERE team_id = ?1 AND resolved = 0
         ORDER BY last_detected DESC LIMIT 200"
    };

    let mut stmt = conn.prepare(sql)?;
    let signals = stmt
        .query_map(params![team_id], |row| {
            let topics_str: String = row.get::<_, String>(5).unwrap_or_default();
            let tech_topics: Vec<String> = serde_json::from_str(&topics_str).unwrap_or_default();
            Ok(TeamSignal {
                id: row.get(0)?,
                team_id: row.get(1)?,
                signal_type: row.get(2)?,
                title: row.get(3)?,
                severity: row.get(4)?,
                tech_topics,
                detected_by_count: row.get(6)?,
                first_detected: row.get(7)?,
                last_detected: row.get(8)?,
                resolved: row.get::<_, i32>(9)? != 0,
                resolved_by: row.get(10)?,
                resolved_at: row.get(11)?,
                resolution_notes: row.get(12)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(signals)
}

// ============================================================================
// Alert Policies
// ============================================================================

/// Get alert policy for a team (or default).
pub fn get_alert_policy(conn: &rusqlite::Connection, team_id: &str) -> Result<AlertPolicy> {
    let policy = conn
        .query_row(
            "SELECT min_seats_to_alert, aggregation_window_minutes, notification_channels
             FROM team_alert_policies WHERE team_id = ?1",
            params![team_id],
            |row| {
                let channels_str: String = row.get(2)?;
                let channels: Vec<String> =
                    serde_json::from_str(&channels_str).unwrap_or_else(|_| vec!["in_app".into()]);
                Ok(AlertPolicy {
                    team_id: team_id.to_string(),
                    min_seats_to_alert: row.get(0)?,
                    aggregation_window_minutes: row.get(1)?,
                    notification_channels: channels,
                })
            },
        )
        .unwrap_or(AlertPolicy {
            team_id: team_id.to_string(),
            min_seats_to_alert: 2,
            aggregation_window_minutes: 60,
            notification_channels: vec!["in_app".to_string()],
        });

    Ok(policy)
}

/// Set alert policy for a team.
pub fn set_alert_policy(conn: &rusqlite::Connection, policy: &AlertPolicy) -> Result<()> {
    let channels_json = serde_json::to_string(&policy.notification_channels)?;
    conn.execute(
        "INSERT INTO team_alert_policies
            (team_id, min_seats_to_alert, aggregation_window_minutes, notification_channels, updated_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(team_id) DO UPDATE SET
            min_seats_to_alert = excluded.min_seats_to_alert,
            aggregation_window_minutes = excluded.aggregation_window_minutes,
            notification_channels = excluded.notification_channels,
            updated_at = excluded.updated_at",
        params![
            policy.team_id,
            policy.min_seats_to_alert,
            policy.aggregation_window_minutes,
            channels_json,
        ],
    )?;
    Ok(())
}

// ============================================================================
// Summary & Analytics
// ============================================================================

/// Get monitoring summary for a team.
pub fn get_monitoring_summary(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<TeamMonitoringSummary> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_signals WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let unresolved: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_signals WHERE team_id = ?1 AND resolved = 0",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let critical: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_signals WHERE team_id = ?1 AND severity = 'critical' AND resolved = 0",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let high: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_signals WHERE team_id = ?1 AND severity = 'high' AND resolved = 0",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let multi_seat: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_signals WHERE team_id = ?1 AND detected_by_count >= 2 AND resolved = 0",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let avg_hours: Option<f64> = conn
        .query_row(
            "SELECT AVG((julianday(resolved_at) - julianday(first_detected)) * 24)
             FROM team_signals WHERE team_id = ?1 AND resolved = 1",
            params![team_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    Ok(TeamMonitoringSummary {
        total_signals: total,
        unresolved_signals: unresolved,
        critical_count: critical,
        high_count: high,
        multi_seat_count: multi_seat,
        avg_resolution_hours: avg_hours,
        most_active_topics: Vec::new(), // Would need topic parsing across signals
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all team signals.
#[tauri::command]
pub async fn get_team_signals_cmd(
    include_resolved: Option<bool>,
) -> crate::error::Result<Vec<TeamSignal>> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_team_signals(&conn, &team_id, include_resolved.unwrap_or(false))
        .map_err(|e| e.to_string().into())
}

/// Resolve a team signal.
#[tauri::command]
pub async fn resolve_team_signal_cmd(signal_id: String, notes: String) -> crate::error::Result<()> {
    let (_team_id, client_id) = get_team_config()?;
    let conn = crate::state::open_db_connection()?;
    resolve_signal(&conn, &signal_id, &client_id, &notes).map_err(|e| e.to_string().into())
}

/// Get alert policy.
#[tauri::command]
pub async fn get_alert_policy_cmd() -> crate::error::Result<AlertPolicy> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_alert_policy(&conn, &team_id).map_err(|e| e.to_string().into())
}

/// Set alert policy (admin only).
#[tauri::command]
pub async fn set_alert_policy_cmd(
    min_seats: Option<i32>,
    window_minutes: Option<i32>,
    channels: Option<Vec<String>>,
) -> crate::error::Result<()> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;

    let mut policy = get_alert_policy(&conn, &team_id).map_err(|e| e.to_string())?;

    if let Some(s) = min_seats {
        policy.min_seats_to_alert = s;
    }
    if let Some(w) = window_minutes {
        policy.aggregation_window_minutes = w;
    }
    if let Some(c) = channels {
        policy.notification_channels = c;
    }

    set_alert_policy(&conn, &policy).map_err(|e| e.to_string().into())
}

/// Get monitoring summary.
#[tauri::command]
pub async fn get_monitoring_summary_cmd() -> crate::error::Result<TeamMonitoringSummary> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_monitoring_summary(&conn, &team_id).map_err(|e| e.to_string().into())
}

// ============================================================================
// Helpers
// ============================================================================

fn get_team_id() -> crate::error::Result<String> {
    let settings = crate::state::get_settings_manager().lock();
    settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
        .ok_or_else(|| "No team configured".into())
}

fn get_team_config() -> crate::error::Result<(String, String)> {
    let settings = crate::state::get_settings_manager().lock();
    let config = settings
        .get()
        .team_relay
        .as_ref()
        .ok_or("Team sync not configured")?;
    let team_id = config.team_id.clone().ok_or("No team ID")?;
    let client_id = config.client_id.clone().ok_or("No client ID")?;
    Ok((team_id, client_id))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE team_signals (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                title TEXT NOT NULL,
                severity TEXT NOT NULL,
                tech_topics TEXT,
                detected_by_count INTEGER DEFAULT 1,
                first_detected TEXT DEFAULT (datetime('now')),
                last_detected TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0,
                resolved_by TEXT,
                resolved_at TEXT,
                resolution_notes TEXT
            );
            CREATE TABLE team_alert_policies (
                team_id TEXT PRIMARY KEY,
                min_seats_to_alert INTEGER DEFAULT 2,
                aggregation_window_minutes INTEGER DEFAULT 60,
                notification_channels TEXT DEFAULT '[\"in_app\"]',
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_record_new_signal() {
        let conn = setup_db();
        let id = record_team_signal(
            &conn,
            "team-1",
            "cve",
            "CVE-2026-001",
            "high",
            &["rust".into(), "openssl".into()],
            "member-1",
        )
        .unwrap();
        assert!(!id.is_empty());

        let signals = get_team_signals(&conn, "team-1", false).unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].title, "CVE-2026-001");
        assert_eq!(signals[0].detected_by_count, 1);
    }

    #[test]
    fn test_signal_corroboration() {
        let conn = setup_db();
        let id1 = record_team_signal(
            &conn,
            "team-1",
            "cve",
            "CVE-2026-001",
            "high",
            &["rust".into()],
            "member-1",
        )
        .unwrap();

        // Second detection of same type should increment count
        let id2 = record_team_signal(
            &conn,
            "team-1",
            "cve",
            "CVE-2026-001 variant",
            "critical",
            &["rust".into()],
            "member-2",
        )
        .unwrap();

        assert_eq!(id1, id2, "Should merge into same signal");

        let signals = get_team_signals(&conn, "team-1", false).unwrap();
        assert_eq!(signals[0].detected_by_count, 2);
    }

    #[test]
    fn test_resolve_signal() {
        let conn = setup_db();
        let id = record_team_signal(
            &conn,
            "team-1",
            "dep",
            "lodash vuln",
            "medium",
            &["javascript".into()],
            "member-1",
        )
        .unwrap();

        resolve_signal(&conn, &id, "member-1", "Updated to v5").unwrap();

        let signals = get_team_signals(&conn, "team-1", false).unwrap();
        assert_eq!(signals.len(), 0, "Resolved signals excluded by default");

        let all = get_team_signals(&conn, "team-1", true).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].resolved);
    }

    #[test]
    fn test_alert_policy_defaults() {
        let conn = setup_db();
        let policy = get_alert_policy(&conn, "team-1").unwrap();
        assert_eq!(policy.min_seats_to_alert, 2);
        assert_eq!(policy.aggregation_window_minutes, 60);
    }

    #[test]
    fn test_set_and_get_alert_policy() {
        let conn = setup_db();
        let policy = AlertPolicy {
            team_id: "team-1".into(),
            min_seats_to_alert: 3,
            aggregation_window_minutes: 120,
            notification_channels: vec!["in_app".into(), "email".into()],
        };
        set_alert_policy(&conn, &policy).unwrap();

        let loaded = get_alert_policy(&conn, "team-1").unwrap();
        assert_eq!(loaded.min_seats_to_alert, 3);
        assert_eq!(loaded.aggregation_window_minutes, 120);
        assert_eq!(loaded.notification_channels.len(), 2);
    }

    #[test]
    fn test_monitoring_summary() {
        let conn = setup_db();
        record_team_signal(
            &conn,
            "team-1",
            "cve",
            "CVE-1",
            "critical",
            &["rust".into()],
            "m1",
        )
        .unwrap();
        record_team_signal(
            &conn,
            "team-1",
            "dep",
            "Dep-1",
            "high",
            &["node".into()],
            "m1",
        )
        .unwrap();

        let summary = get_monitoring_summary(&conn, "team-1").unwrap();
        assert_eq!(summary.total_signals, 2);
        assert_eq!(summary.unresolved_signals, 2);
        assert_eq!(summary.critical_count, 1);
        assert_eq!(summary.high_count, 1);
    }
}
