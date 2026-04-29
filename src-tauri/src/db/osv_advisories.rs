// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV Advisory storage — CRUD operations for the local OSV mirror.

use rusqlite::{params, Result as SqliteResult};

use super::Database;
use crate::osv::types::{StoredAdvisory, SyncStatus};

// ============================================================================
// Database Operations
// ============================================================================

impl Database {
    /// Upsert an advisory into the local mirror.
    /// Key: (advisory_id, package_name, ecosystem).
    pub fn upsert_osv_advisory(
        &self,
        advisory_id: &str,
        summary: &str,
        details: Option<&str>,
        package_name: &str,
        ecosystem: &str,
        affected_ranges: Option<&str>,
        fixed_versions: Option<&str>,
        severity_type: Option<&str>,
        cvss_score: Option<f64>,
        source_url: Option<&str>,
        published_at: Option<&str>,
        modified_at: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO osv_advisories (advisory_id, summary, details, package_name, ecosystem,
                affected_ranges, fixed_versions, severity_type, cvss_score, source_url,
                published_at, modified_at, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
             ON CONFLICT(advisory_id, package_name, ecosystem) DO UPDATE SET
                summary = ?2,
                details = COALESCE(?3, osv_advisories.details),
                affected_ranges = COALESCE(?6, osv_advisories.affected_ranges),
                fixed_versions = COALESCE(?7, osv_advisories.fixed_versions),
                severity_type = COALESCE(?8, osv_advisories.severity_type),
                cvss_score = COALESCE(?9, osv_advisories.cvss_score),
                source_url = COALESCE(?10, osv_advisories.source_url),
                published_at = COALESCE(?11, osv_advisories.published_at),
                modified_at = COALESCE(?12, osv_advisories.modified_at),
                synced_at = datetime('now')",
            params![
                advisory_id,
                summary,
                details,
                package_name,
                ecosystem,
                affected_ranges,
                fixed_versions,
                severity_type,
                cvss_score,
                source_url,
                published_at,
                modified_at,
            ],
        )?;
        Ok(())
    }

    /// Get all advisories for a specific package.
    pub fn get_osv_advisories_for_package(
        &self,
        package_name: &str,
        ecosystem: &str,
    ) -> SqliteResult<Vec<StoredAdvisory>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, advisory_id, summary, details, package_name, ecosystem,
                    affected_ranges, fixed_versions, severity_type, cvss_score,
                    source_url, published_at, modified_at, synced_at
             FROM osv_advisories
             WHERE package_name = ?1 AND ecosystem = ?2
             ORDER BY cvss_score DESC NULLS LAST, published_at DESC",
        )?;

        let rows = stmt.query_map(params![package_name, ecosystem], map_advisory_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all stored advisories.
    pub fn get_all_osv_advisories(&self) -> SqliteResult<Vec<StoredAdvisory>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, advisory_id, summary, details, package_name, ecosystem,
                    affected_ranges, fixed_versions, severity_type, cvss_score,
                    source_url, published_at, modified_at, synced_at
             FROM osv_advisories
             ORDER BY cvss_score DESC NULLS LAST, published_at DESC",
        )?;

        let rows = stmt.query_map([], map_advisory_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get advisory count per ecosystem.
    pub fn get_osv_advisory_count_by_ecosystem(&self) -> SqliteResult<Vec<(String, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ecosystem, COUNT(*) FROM osv_advisories GROUP BY ecosystem ORDER BY ecosystem",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Update the sync status for an ecosystem.
    pub fn update_osv_sync_status(
        &self,
        ecosystem: &str,
        advisory_count: i64,
        error: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO osv_sync_status (ecosystem, last_synced_at, advisory_count, error)
             VALUES (?1, datetime('now'), ?2, ?3)
             ON CONFLICT(ecosystem) DO UPDATE SET
                last_synced_at = datetime('now'),
                advisory_count = ?2,
                error = ?3",
            params![ecosystem, advisory_count, error],
        )?;
        Ok(())
    }

    /// Get sync status for all ecosystems.
    pub fn get_osv_sync_statuses(&self) -> SqliteResult<Vec<SyncStatus>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ecosystem, last_synced_at, advisory_count, error
             FROM osv_sync_status
             ORDER BY ecosystem",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SyncStatus {
                ecosystem: row.get(0)?,
                last_synced_at: row.get(1)?,
                advisory_count: row.get(2)?,
                error: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete stale advisories not refreshed in the latest sync.
    /// Call after a full ecosystem sync to remove advisories that OSV
    /// no longer lists (i.e. withdrawn).
    pub fn delete_stale_osv_advisories(
        &self,
        ecosystem: &str,
        before_synced_at: &str,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let count = conn.execute(
            "DELETE FROM osv_advisories WHERE ecosystem = ?1 AND synced_at < ?2",
            params![ecosystem, before_synced_at],
        )?;
        Ok(count)
    }
}

// ============================================================================
// Row Mapper
// ============================================================================

fn map_advisory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredAdvisory> {
    Ok(StoredAdvisory {
        id: row.get(0)?,
        advisory_id: row.get(1)?,
        summary: row.get(2)?,
        details: row.get(3)?,
        package_name: row.get(4)?,
        ecosystem: row.get(5)?,
        affected_ranges: row.get(6)?,
        fixed_versions: row.get(7)?,
        severity_type: row.get(8)?,
        cvss_score: row.get(9)?,
        source_url: row.get(10)?,
        published_at: row.get(11)?,
        modified_at: row.get(12)?,
        synced_at: row.get(13)?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    #[test]
    fn test_upsert_and_retrieve_advisory() {
        let db = test_db();

        db.upsert_osv_advisory(
            "GHSA-1234-5678-abcd",
            "XSS in react-router",
            Some("A cross-site scripting vulnerability..."),
            "react-router",
            "npm",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"6.4.5"}]}]"#),
            Some(r#"["6.4.5"]"#),
            Some("CVSS_V3"),
            Some(7.5),
            Some("https://github.com/advisories/GHSA-1234-5678-abcd"),
            Some("2026-03-15T10:00:00Z"),
            Some("2026-03-20T12:00:00Z"),
        )
        .unwrap();

        let advisories = db
            .get_osv_advisories_for_package("react-router", "npm")
            .unwrap();
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].advisory_id, "GHSA-1234-5678-abcd");
        assert_eq!(advisories[0].summary, "XSS in react-router");
        assert_eq!(advisories[0].cvss_score, Some(7.5));
        assert!(advisories[0].fixed_versions.is_some());
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = test_db();

        db.upsert_osv_advisory(
            "GHSA-0001",
            "Old summary",
            None,
            "serde",
            "crates.io",
            None,
            None,
            None,
            Some(5.0),
            None,
            None,
            None,
        )
        .unwrap();

        db.upsert_osv_advisory(
            "GHSA-0001",
            "Updated summary",
            Some("New details"),
            "serde",
            "crates.io",
            None,
            None,
            None,
            Some(7.0),
            None,
            None,
            None,
        )
        .unwrap();

        let advisories = db
            .get_osv_advisories_for_package("serde", "crates.io")
            .unwrap();
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].summary, "Updated summary");
        assert_eq!(advisories[0].cvss_score, Some(7.0));
    }

    #[test]
    fn test_sync_status_tracking() {
        let db = test_db();

        db.update_osv_sync_status("npm", 42, None).unwrap();
        db.update_osv_sync_status("crates.io", 15, None).unwrap();

        let statuses = db.get_osv_sync_statuses().unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].ecosystem, "crates.io");
        assert_eq!(statuses[0].advisory_count, 15);
        assert_eq!(statuses[1].ecosystem, "npm");
        assert_eq!(statuses[1].advisory_count, 42);
    }

    #[test]
    fn test_sync_status_update() {
        let db = test_db();

        db.update_osv_sync_status("npm", 10, None).unwrap();
        db.update_osv_sync_status("npm", 25, None).unwrap();

        let statuses = db.get_osv_sync_statuses().unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].advisory_count, 25);
    }

    #[test]
    fn test_advisory_count_by_ecosystem() {
        let db = test_db();

        db.upsert_osv_advisory(
            "A-001", "vuln 1", None, "react", "npm", None, None, None, None, None, None, None,
        )
        .unwrap();
        db.upsert_osv_advisory(
            "A-002", "vuln 2", None, "express", "npm", None, None, None, None, None, None, None,
        )
        .unwrap();
        db.upsert_osv_advisory(
            "A-003",
            "vuln 3",
            None,
            "serde",
            "crates.io",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let counts = db.get_osv_advisory_count_by_ecosystem().unwrap();
        assert_eq!(counts.len(), 2);

        let npm = counts.iter().find(|(e, _)| e == "npm").unwrap();
        assert_eq!(npm.1, 2);

        let crates = counts.iter().find(|(e, _)| e == "crates.io").unwrap();
        assert_eq!(crates.1, 1);
    }

    #[test]
    fn test_get_all_advisories() {
        let db = test_db();

        db.upsert_osv_advisory(
            "A-001",
            "vuln 1",
            None,
            "react",
            "npm",
            None,
            None,
            None,
            Some(9.0),
            None,
            None,
            None,
        )
        .unwrap();
        db.upsert_osv_advisory(
            "A-002",
            "vuln 2",
            None,
            "serde",
            "crates.io",
            None,
            None,
            None,
            Some(5.0),
            None,
            None,
            None,
        )
        .unwrap();

        let all = db.get_all_osv_advisories().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].cvss_score, Some(9.0));
    }
}
