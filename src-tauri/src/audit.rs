// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Enterprise Audit Log module for 4DA.
//!
//! Provides immutable, non-blocking audit logging for team and enterprise tiers.
//! The `log_audit` function is designed to NEVER fail the caller — all errors
//! are caught internally and logged via `tracing::warn`.
//!
//! ## Table schema (created via migration, documented here for reference):
//!
//! ```sql
//! CREATE TABLE audit_log (
//!     id INTEGER PRIMARY KEY AUTOINCREMENT,
//!     event_id TEXT NOT NULL UNIQUE,
//!     team_id TEXT NOT NULL,
//!     actor_id TEXT NOT NULL,
//!     actor_display_name TEXT NOT NULL,
//!     action TEXT NOT NULL,               -- 'resource.verb' format
//!     resource_type TEXT NOT NULL,
//!     resource_id TEXT,
//!     details TEXT,                        -- JSON
//!     created_at TEXT DEFAULT (datetime('now'))
//! );
//! CREATE INDEX idx_audit_team_time ON audit_log(team_id, created_at DESC);
//! CREATE INDEX idx_audit_actor ON audit_log(actor_id, created_at DESC);
//! CREATE INDEX idx_audit_action ON audit_log(action);
//! ```

use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AuditEntry {
    pub id: i64,
    pub event_id: String,
    pub team_id: String,
    pub actor_id: String,
    pub actor_display_name: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    #[ts(type = "any | null")]
    pub details: Option<serde_json::Value>,
    pub created_at: String,
}

/// Filter criteria for querying audit entries.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub actor_id: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Aggregated audit summary over a time window.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AuditSummary {
    pub total_events: i64,
    pub events_by_action: Vec<(String, i64)>,
    pub events_by_actor: Vec<(String, i64)>,
    pub events_by_day: Vec<(String, i64)>,
}

// ============================================================================
// Core Logging (non-blocking, never fails the caller)
// ============================================================================

/// Bundled audit logging parameters.
pub struct AuditLogParams<'a> {
    pub conn: &'a rusqlite::Connection,
    pub team_id: &'a str,
    pub actor_id: &'a str,
    pub actor_display_name: &'a str,
    pub action: &'a str,
    pub resource_type: &'a str,
    pub resource_id: Option<&'a str>,
    pub details: Option<&'a serde_json::Value>,
}

/// Write a single audit entry to the database.
///
/// **Contract:** This function NEVER returns an error and NEVER panics.
/// On any failure (DB error, serialization error, etc.) it logs a warning
/// via `tracing::warn` and returns `()`.
pub fn log_audit(params: &AuditLogParams<'_>) {
    let event_id = uuid::Uuid::new_v4().to_string();

    let details_json = match params.details {
        Some(v) => match serde_json::to_string(v) {
            Ok(s) => Some(s),
            Err(e) => {
                warn!(
                    target: "4da::audit",
                    error = %e,
                    action = params.action,
                    "Failed to serialize audit details, recording without details"
                );
                None
            }
        },
        None => None,
    };

    let result = params.conn.execute(
        "INSERT INTO audit_log (event_id, team_id, actor_id, actor_display_name, action, resource_type, resource_id, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            event_id,
            params.team_id,
            params.actor_id,
            params.actor_display_name,
            params.action,
            params.resource_type,
            params.resource_id,
            details_json,
        ],
    );

    if let Err(e) = result {
        warn!(
            target: "4da::audit",
            error = %e,
            action = params.action,
            team_id = params.team_id,
            "Failed to write audit log entry"
        );
    }
}

/// Convenience wrapper that reads actor identity from team relay settings.
///
/// Extracts `team_id`, `client_id`, and `display_name` from the configured
/// `TeamRelayConfig`. If team relay is not configured, silently returns
/// without logging (audit is a team/enterprise feature).
///
/// **Contract:** Same as `log_audit` — never fails, never panics.
pub fn log_team_audit(
    conn: &rusqlite::Connection,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    details: Option<&serde_json::Value>,
) {
    let settings = crate::state::get_settings_manager().lock();
    let relay_cfg = match settings.get().team_relay.as_ref() {
        Some(cfg) => cfg.clone(),
        None => return, // No team config — audit not applicable
    };
    drop(settings); // Release lock before DB write

    let team_id = match relay_cfg.team_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return, // No team_id — can't log
    };

    let actor_id = relay_cfg
        .client_id
        .as_deref()
        .unwrap_or("unknown")
        .to_string();

    let actor_display_name = relay_cfg
        .display_name
        .as_deref()
        .unwrap_or("Unknown User")
        .to_string();

    log_audit(&AuditLogParams {
        conn,
        team_id: &team_id,
        actor_id: &actor_id,
        actor_display_name: &actor_display_name,
        action,
        resource_type,
        resource_id,
        details,
    });
}

// ============================================================================
// Query Functions
// ============================================================================

/// Retrieve audit entries for a team, filtered by optional criteria.
///
/// Results are ordered by `created_at DESC` (most recent first).
/// Default limit is 100, maximum is 1000.
pub fn get_audit_entries(
    conn: &rusqlite::Connection,
    team_id: &str,
    filters: &AuditFilter,
) -> anyhow::Result<Vec<AuditEntry>> {
    let mut sql = String::from(
        "SELECT id, event_id, team_id, actor_id, actor_display_name, action, \
         resource_type, resource_id, details, created_at \
         FROM audit_log WHERE team_id = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(team_id.to_string())];
    let mut param_idx = 2u32;

    if let Some(ref actor_id) = filters.actor_id {
        sql.push_str(&format!(" AND actor_id = ?{param_idx}"));
        params.push(Box::new(actor_id.clone()));
        param_idx += 1;
    }

    if let Some(ref action) = filters.action {
        sql.push_str(&format!(" AND action = ?{param_idx}"));
        params.push(Box::new(action.clone()));
        param_idx += 1;
    }

    if let Some(ref resource_type) = filters.resource_type {
        sql.push_str(&format!(" AND resource_type = ?{param_idx}"));
        params.push(Box::new(resource_type.clone()));
        param_idx += 1;
    }

    if let Some(ref from) = filters.from {
        sql.push_str(&format!(" AND created_at >= ?{param_idx}"));
        params.push(Box::new(from.clone()));
        param_idx += 1;
    }

    if let Some(ref to) = filters.to {
        sql.push_str(&format!(" AND created_at <= ?{param_idx}"));
        params.push(Box::new(to.clone()));
        param_idx += 1;
    }

    sql.push_str(" ORDER BY created_at DESC");

    let limit = filters.limit.unwrap_or(100).min(1000);
    sql.push_str(&format!(" LIMIT ?{param_idx}"));
    params.push(Box::new(limit));
    param_idx += 1;

    if let Some(offset) = filters.offset {
        sql.push_str(&format!(" OFFSET ?{param_idx}"));
        params.push(Box::new(offset));
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let details_str: Option<String> = row.get(8)?;
        let details = details_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(AuditEntry {
            id: row.get(0)?,
            event_id: row.get(1)?,
            team_id: row.get(2)?,
            actor_id: row.get(3)?,
            actor_display_name: row.get(4)?,
            action: row.get(5)?,
            resource_type: row.get(6)?,
            resource_id: row.get(7)?,
            details,
            created_at: row.get(9)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// Generate an aggregated summary of audit activity for a team over N days.
pub fn get_audit_summary(
    conn: &rusqlite::Connection,
    team_id: &str,
    days: i32,
) -> anyhow::Result<AuditSummary> {
    let cutoff = format!("-{days} days");

    // Total events
    let total_events: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_log WHERE team_id = ?1 AND created_at >= datetime('now', ?2)",
        rusqlite::params![team_id, cutoff],
        |row| row.get(0),
    )?;

    // Events by action
    let mut stmt = conn.prepare(
        "SELECT action, COUNT(*) as cnt FROM audit_log \
         WHERE team_id = ?1 AND created_at >= datetime('now', ?2) \
         GROUP BY action ORDER BY cnt DESC LIMIT 20",
    )?;
    let events_by_action: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params![team_id, cutoff], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Events by actor
    let mut stmt = conn.prepare(
        "SELECT actor_display_name, COUNT(*) as cnt FROM audit_log \
         WHERE team_id = ?1 AND created_at >= datetime('now', ?2) \
         GROUP BY actor_id ORDER BY cnt DESC LIMIT 20",
    )?;
    let events_by_actor: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params![team_id, cutoff], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Events by day
    let mut stmt = conn.prepare(
        "SELECT date(created_at) as day, COUNT(*) as cnt FROM audit_log \
         WHERE team_id = ?1 AND created_at >= datetime('now', ?2) \
         GROUP BY day ORDER BY day DESC",
    )?;
    let events_by_day: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params![team_id, cutoff], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(AuditSummary {
        total_events,
        events_by_action,
        events_by_actor,
        events_by_day,
    })
}

/// Export audit entries as a CSV string for a given time range.
///
/// Columns: `event_id,team_id,actor_id,actor_display_name,action,resource_type,resource_id,details,created_at`
pub fn export_audit_csv(
    conn: &rusqlite::Connection,
    team_id: &str,
    from: &str,
    to: &str,
) -> anyhow::Result<String> {
    let mut stmt = conn.prepare(
        "SELECT event_id, team_id, actor_id, actor_display_name, action, \
         resource_type, resource_id, details, created_at \
         FROM audit_log \
         WHERE team_id = ?1 AND created_at >= ?2 AND created_at <= ?3 \
         ORDER BY created_at ASC",
    )?;

    let mut csv = String::from(
        "event_id,team_id,actor_id,actor_display_name,action,resource_type,resource_id,details,created_at\n",
    );

    let rows = stmt.query_map(rusqlite::params![team_id, from, to], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    for row in rows {
        let (event_id, tid, actor_id, actor_name, action, rtype, rid, details, created_at) = row?;
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&event_id),
            csv_escape(&tid),
            csv_escape(&actor_id),
            csv_escape(&actor_name),
            csv_escape(&action),
            csv_escape(&rtype),
            csv_escape(&rid.unwrap_or_default()),
            csv_escape(&details.unwrap_or_default()),
            csv_escape(&created_at),
        ));
    }

    Ok(csv)
}

/// Escape a value for CSV output (RFC 4180 compliant + injection protection).
/// Prefixes formula-trigger characters with a single quote to prevent
/// Excel/Sheets from interpreting cell values as formulas.
fn csv_escape(value: &str) -> String {
    let sanitized = if value.starts_with('=')
        || value.starts_with('+')
        || value.starts_with('-')
        || value.starts_with('@')
        || value.starts_with('\t')
        || value.starts_with('\r')
    {
        format!("'{value}")
    } else {
        value.to_string()
    };

    if sanitized.contains(',') || sanitized.contains('"') || sanitized.contains('\n') {
        format!("\"{}\"", sanitized.replace('"', "\"\""))
    } else {
        sanitized
    }
}

// ============================================================================
// Retention / Cleanup
// ============================================================================

/// Delete audit entries older than `retention_days` for a given team.
///
/// Returns the number of rows deleted.
pub fn cleanup_expired_audit(
    conn: &rusqlite::Connection,
    team_id: &str,
    retention_days: i32,
) -> anyhow::Result<usize> {
    let cutoff = format!("-{retention_days} days");
    let deleted = conn.execute(
        "DELETE FROM audit_log WHERE team_id = ?1 AND created_at < datetime('now', ?2)",
        rusqlite::params![team_id, cutoff],
    )?;
    Ok(deleted)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Retrieve filtered audit log entries for the current team.
#[tauri::command]
pub async fn get_audit_log(
    action_filter: Option<String>,
    resource_type_filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> crate::error::Result<Vec<AuditEntry>> {
    let conn = crate::state::open_db_connection()?;

    let team_id = read_team_id()?;

    let filters = AuditFilter {
        actor_id: None,
        action: action_filter,
        resource_type: resource_type_filter,
        from: None,
        to: None,
        limit,
        offset,
    };

    let entries = get_audit_entries(&conn, &team_id, &filters)
        .map_err(|e| format!("Failed to query audit log: {e}"))?;

    Ok(entries)
}

/// Retrieve an aggregated audit summary for the current team.
#[tauri::command]
pub async fn get_audit_summary_cmd(days: Option<i32>) -> crate::error::Result<AuditSummary> {
    let conn = crate::state::open_db_connection()?;

    let team_id = read_team_id()?;
    let days = days.unwrap_or(30).max(1).min(365);

    let summary = get_audit_summary(&conn, &team_id, days)
        .map_err(|e| format!("Failed to generate audit summary: {e}"))?;

    Ok(summary)
}

/// Export audit log entries as CSV for a given date range.
#[tauri::command]
pub async fn export_audit_csv_cmd(from: String, to: String) -> crate::error::Result<String> {
    let conn = crate::state::open_db_connection()?;

    let team_id = read_team_id()?;

    let csv = export_audit_csv(&conn, &team_id, &from, &to)
        .map_err(|e| format!("Failed to export audit CSV: {e}"))?;

    Ok(csv)
}

/// Read the team_id from settings. Returns an error if team relay is not configured.
fn read_team_id() -> crate::error::Result<String> {
    let settings = crate::state::get_settings_manager().lock();
    let team_id = settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
        .unwrap_or_default();
    drop(settings);

    if team_id.is_empty() {
        return Err("Team not configured — audit log requires team relay setup".into());
    }
    Ok(team_id)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
