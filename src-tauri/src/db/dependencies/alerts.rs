// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Alert operations on `impl Database`: existence check, store, retrieve, resolve.

use rusqlite::{params, Result as SqliteResult};

use crate::db::Database;

use super::mappers::map_alert_row;
use super::types::DependencyAlert;

impl Database {
    /// Check if an alert already exists for this package/ecosystem/title combination.
    pub fn alert_exists(
        &self,
        package_name: &str,
        ecosystem: &str,
        title: &str,
    ) -> SqliteResult<bool> {
        // Match against the canonical ecosystem so pre-checks align with stored rows.
        let ecosystem = crate::sources::cve_matching::normalize_ecosystem(ecosystem);
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM dependency_alerts WHERE package_name = ?1 AND ecosystem = ?2 AND title = ?3 AND resolved_at IS NULL",
            params![package_name, ecosystem, title],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Store a new dependency alert, skipping duplicates.
    /// Returns the row ID if inserted, or 0 if the alert already exists.
    pub fn store_dependency_alert(&self, alert: &DependencyAlert) -> SqliteResult<i64> {
        // Normalize on the write path so dependency_alerts keeps a single
        // canonical form: severity uppercase (CRITICAL/HIGH/MEDIUM/LOW) and
        // ecosystem canonicalized (e.g. "rust" -> "crates.io"). Without this,
        // CVE rows (uppercase, "rust") and local-audit rows (lowercase,
        // "crates.io") fragment grouping, dedup, and the severity sort.
        let ecosystem =
            crate::sources::cve_matching::normalize_ecosystem(&alert.ecosystem).to_string();
        let severity = alert.severity.trim().to_uppercase();
        let conn = self.conn.lock();
        // Check for existing unresolved alert with same package/ecosystem/title
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM dependency_alerts WHERE package_name = ?1 AND ecosystem = ?2 AND title = ?3 AND resolved_at IS NULL",
                params![alert.package_name, ecosystem, alert.title],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if exists {
            return Ok(0); // Duplicate — skip insertion
        }

        conn.execute(
            "INSERT INTO dependency_alerts (package_name, ecosystem, alert_type, severity, title, description, affected_versions, source_url, source_item_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                alert.package_name,
                ecosystem,
                alert.alert_type,
                severity,
                alert.title,
                alert.description,
                alert.affected_versions,
                alert.source_url,
                alert.source_item_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all active (unresolved) alerts.
    pub fn get_active_alerts(&self) -> SqliteResult<Vec<DependencyAlert>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, package_name, ecosystem, alert_type, severity, title, description,
                    affected_versions, source_url, source_item_id, detected_at, resolved_at
             FROM dependency_alerts
             WHERE resolved_at IS NULL
             ORDER BY
                CASE UPPER(severity)
                    WHEN 'CRITICAL' THEN 0
                    WHEN 'HIGH' THEN 1
                    WHEN 'MEDIUM' THEN 2
                    WHEN 'LOW' THEN 3
                    ELSE 4
                END,
                detected_at DESC",
        )?;

        let rows = stmt.query_map([], map_alert_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependency_alerts: {e}");
                    None
                }
            })
            .collect())
    }

    /// Resolve (dismiss) an alert by ID.
    pub fn resolve_alert(&self, alert_id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE dependency_alerts SET resolved_at = datetime('now') WHERE id = ?1",
            params![alert_id],
        )?;
        Ok(())
    }
}
