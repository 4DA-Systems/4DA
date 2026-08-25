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

    /// Store a dependency alert, collapsing repeats onto a single row.
    ///
    /// Returns the row ID if inserted, or 0 when an existing row already
    /// carries this advisory (whether active or reopened).
    ///
    /// CHURN NOTE (2026-08-25): the existence check used to be scoped to
    /// `resolved_at IS NULL`, so a resolved row was invisible to it and the
    /// next scan inserted a *fresh duplicate*. Paired with an auto-resolver
    /// that ran on a different schedule, this oscillated forever — 129 rows
    /// accumulated for 13 distinct advisories, four more every six hours, and
    /// the alert blinked in and out of the UI on that cycle. The identity of an
    /// advisory does not change when someone dismisses it, so the lookup is
    /// unscoped and a returning finding REOPENS its original row.
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
        // Look up the advisory's row REGARDLESS of resolution state. Scoping
        // this to unresolved rows is what produced the duplicate churn.
        let existing: Option<(i64, Option<String>)> = conn
            .query_row(
                "SELECT id, resolved_at FROM dependency_alerts
                 WHERE package_name = ?1 AND ecosystem = ?2 AND title = ?3
                 ORDER BY detected_at ASC LIMIT 1",
                params![alert.package_name, ecosystem, alert.title],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((id, resolved_at)) = existing {
            if resolved_at.is_some() {
                // The advisory is being reported again by its source after
                // having been resolved — reopen the original row rather than
                // growing a parallel one. `detected_at` keeps its first-seen
                // value; only the resolution is cleared.
                conn.execute(
                    "UPDATE dependency_alerts SET resolved_at = NULL WHERE id = ?1",
                    params![id],
                )?;
            }
            return Ok(0); // Already tracked — no new row
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
    ///
    /// GROUNDING NOTE (read-path guard deliberately omitted, 2026-07-02): every
    /// write path into `dependency_alerts` is already gated to packages the user
    /// actually has installed, so alerts need no read-side grounding filter:
    /// - CVE scan (`monitoring_jobs::run_cve_scan`) cross-references advisories
    ///   against `get_relevant_user_dependencies()` (direct, non-dev, real deps)
    ///   with semver range matching before storing.
    /// - Local audit (`local_audit::run_local_audits`) stores findings reported
    ///   by `npm audit` / `cargo audit` against the user's actual lockfiles.
    /// Do NOT add an `is_ambiguous_package_name` filter here: a real dependency
    /// legitimately named like a common word (e.g. the `log` crate) has a REAL
    /// alert that must surface. Do NOT add a JOIN against the current dependency
    /// tables either: `resolve_patched_dependency_alerts` deliberately keeps
    /// alerts whose package is absent from the current auditable set ("a scan
    /// gap must never silently clear a real advisory") — a read-side existence
    /// JOIN would silently hide exactly those alerts.
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

    /// Retire audit-sourced alerts that their producing tool no longer reports.
    ///
    /// `npm audit` / `cargo audit` read the real lockfile and are the authority
    /// on what is vulnerable there. Creation was already authority-driven;
    /// resolution was not — it went through a semver re-derivation that
    /// answered a *different* question from a stored range. When those two
    /// disagreed the alert oscillated, and when the range was wrong the
    /// resolver closed live advisories. Alerts opened by an authority are now
    /// closed by the same authority: present in the fresh scan means keep,
    /// absent means retire.
    ///
    /// Safety: only ecosystems in `audited_ecosystems` — those where a tool
    /// actually ran to completion this cycle — are eligible. An ecosystem whose
    /// audit was skipped (tool absent, timed out, unparseable output, or a
    /// lockfile format the tool does not read) keeps every alert untouched, so
    /// a broken toolchain can never be mistaken for a clean bill of health.
    ///
    /// `current` holds `(package_name, normalized_ecosystem, title)` for every
    /// finding in this cycle. Returns the number of alerts retired.
    pub fn reconcile_audit_alerts(
        &self,
        audited_ecosystems: &std::collections::HashSet<String>,
        current: &std::collections::HashSet<(String, String, String)>,
    ) -> SqliteResult<usize> {
        if audited_ecosystems.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, package_name, ecosystem, title FROM dependency_alerts
             WHERE alert_type = 'audit' AND resolved_at IS NULL",
        )?;
        let rows: Vec<(i64, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in audit reconcile: {e}");
                    None
                }
            })
            .collect();
        drop(stmt);

        let mut retired = 0usize;
        for (id, package, ecosystem, title) in rows {
            // Ecosystem had no completed audit this cycle — say nothing.
            if !audited_ecosystems.contains(&ecosystem) {
                continue;
            }
            if current.contains(&(package.clone(), ecosystem.clone(), title.clone())) {
                continue; // still reported — leave open
            }
            conn.execute(
                "UPDATE dependency_alerts SET resolved_at = datetime('now') WHERE id = ?1",
                params![id],
            )?;
            retired += 1;
            tracing::info!(
                target: "4da::health",
                package = package.as_str(),
                ecosystem = ecosystem.as_str(),
                alert_id = id,
                "Retired audit alert — the audit tool no longer reports it"
            );
        }

        Ok(retired)
    }
}
