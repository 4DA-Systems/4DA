// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `osv_sync_status`: what each ecosystem's last advisory sync pass verified,
//! and when. Split out of `osv_advisories.rs` (file-size gate).
//!
//! `last_synced_at` is the time an ecosystem's advisory data was last
//! VERIFIED against the remote: its archive's ETag was confirmed, or a
//! download completed. A pass that could not check (offline, a proxy, the
//! archive host blocked) records its `error` and leaves that time alone. It
//! used to stamp "now" with no error after re-reading the archive on disk, so
//! a mirror that could not be refreshed read as freshly synced for as long as
//! the network stayed away (reproduced 2026-11-11), and every reader of the
//! time (readiness, the due check, the plan snapshot) believed it.

use rusqlite::{params, Result as SqliteResult};

use crate::db::Database;
use crate::osv::types::SyncStatus;

/// `kv_store` key prefix, one key per ecosystem: the archive last read into
/// the mirror (see [`Database::osv_archive_ingested`]).
const INGESTED_KV_PREFIX: &str = "osv.archive_ingested.v1:";

impl Database {
    /// Record how an ecosystem's sync pass ended. Without an `error` the
    /// data was verified now. With one, nothing was verified:
    /// [`Self::record_osv_sync_unverified`] keeps the last verified time.
    pub fn update_osv_sync_status(
        &self,
        ecosystem: &str,
        advisory_count: i64,
        error: Option<&str>,
    ) -> SqliteResult<()> {
        if let Some(error) = error {
            return self.record_osv_sync_unverified(ecosystem, Some(advisory_count), error, None);
        }
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO osv_sync_status (ecosystem, last_synced_at, advisory_count, error)
             VALUES (?1, datetime('now'), ?2, NULL)
             ON CONFLICT(ecosystem) DO UPDATE SET
                last_synced_at = datetime('now'),
                advisory_count = ?2,
                error = NULL",
            params![ecosystem, advisory_count],
        )?;
        Ok(())
    }

    /// Record a pass that could NOT confirm the ecosystem's advisory data is
    /// current. `error` says why, and keeps the ecosystem due for a retry.
    ///
    /// The verified time is never moved to "now". It becomes the later of
    /// what the row holds and `data_as_of`: the download time of the archive
    /// the pass served from (UTC `YYYY-MM-DD HH:MM:SS`), which is when that
    /// data was last known current. With neither, the time stays empty: the
    /// column is NOT NULL, the row exists only to carry the error, and
    /// [`Self::get_osv_sync_statuses`] reads an empty time as none.
    /// An `advisory_count` of `None` keeps the stored count.
    pub fn record_osv_sync_unverified(
        &self,
        ecosystem: &str,
        advisory_count: Option<i64>,
        error: &str,
        data_as_of: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO osv_sync_status (ecosystem, last_synced_at, advisory_count, error)
             VALUES (?1, COALESCE(?4, ''), COALESCE(?2, 0), ?3)
             ON CONFLICT(ecosystem) DO UPDATE SET
                last_synced_at = MAX(last_synced_at, COALESCE(?4, '')),
                advisory_count = COALESCE(?2, advisory_count),
                error = ?3",
            params![ecosystem, advisory_count, error, data_as_of],
        )?;
        Ok(())
    }

    /// Sync status for all ecosystems. `last_synced_at` is `None` for an
    /// ecosystem whose data was never verified.
    pub fn get_osv_sync_statuses(&self) -> SqliteResult<Vec<SyncStatus>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT ecosystem, last_synced_at, advisory_count, error
             FROM osv_sync_status
             ORDER BY ecosystem",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SyncStatus {
                ecosystem: row.get(0)?,
                last_synced_at: row
                    .get::<_, Option<String>>(1)?
                    .filter(|at| !at.trim().is_empty()),
                advisory_count: row.get(2)?,
                error: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// The fingerprint of the archive last read into the mirror for
    /// `ecosystem` (`osv::sync` computes it), or `None`. It lives beside the
    /// rows it describes, so a reset or restored database forgets it with
    /// them. Code that deletes advisory rows must call
    /// [`Self::clear_osv_archives_ingested`].
    pub(crate) fn osv_archive_ingested(&self, ecosystem: &str) -> Option<String> {
        self.get_kv(&format!("{INGESTED_KV_PREFIX}{ecosystem}"))
            .ok()
            .flatten()
            .filter(|mark| !mark.is_empty())
    }

    /// Remember the archive just read into the mirror for `ecosystem`.
    pub(crate) fn set_osv_archive_ingested(
        &self,
        ecosystem: &str,
        fingerprint: &str,
    ) -> SqliteResult<()> {
        self.set_kv(&format!("{INGESTED_KV_PREFIX}{ecosystem}"), fingerprint)
    }

    /// Forget every ecosystem's ingested archive: advisory rows were deleted,
    /// so the next pass must read its archive again.
    pub(crate) fn clear_osv_archives_ingested(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM kv_store WHERE key LIKE ?1",
            params![format!("{INGESTED_KV_PREFIX}%")],
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    fn set_time(db: &crate::db::Database, ecosystem: &str, at: &str) {
        db.conn
            .lock()
            .execute(
                "UPDATE osv_sync_status SET last_synced_at = ?2 WHERE ecosystem = ?1",
                rusqlite::params![ecosystem, at],
            )
            .unwrap();
    }

    fn row(db: &crate::db::Database, ecosystem: &str) -> crate::osv::types::SyncStatus {
        db.get_osv_sync_statuses()
            .unwrap()
            .into_iter()
            .find(|s| s.ecosystem == ecosystem)
            .expect("status row")
    }

    #[test]
    fn a_verified_pass_stamps_now_and_clears_the_error() {
        let db = test_db();
        db.record_osv_sync_unverified("npm", None, "offline", None)
            .unwrap();
        db.update_osv_sync_status("npm", 7, None).unwrap();
        let npm = row(&db, "npm");
        assert_eq!(npm.error, None);
        assert_eq!(npm.advisory_count, 7);
        let at = chrono::NaiveDateTime::parse_from_str(
            npm.last_synced_at.as_deref().expect("stamped"),
            "%Y-%m-%d %H:%M:%S",
        )
        .unwrap();
        let age = chrono::Utc::now().naive_utc().signed_duration_since(at);
        assert!(age.num_minutes() < 2, "stamped now, got {at}");
    }

    /// An unverified pass over an existing row: the error and the count are
    /// recorded, the verified time is not moved.
    #[test]
    fn an_unverified_pass_keeps_the_verified_time() {
        let db = test_db();
        db.update_osv_sync_status("npm", 600, None).unwrap();
        set_time(&db, "npm", "2026-11-08 02:00:00");

        db.record_osv_sync_unverified(
            "npm",
            Some(618),
            "could not check",
            Some("2026-11-07 23:16:58"),
        )
        .unwrap();
        let npm = row(&db, "npm");
        assert_eq!(npm.last_synced_at.as_deref(), Some("2026-11-08 02:00:00"));
        assert_eq!(npm.error.as_deref(), Some("could not check"));
        assert_eq!(npm.advisory_count, 618);

        // Nothing read this pass: the count stays.
        db.record_osv_sync_unverified("npm", None, "still offline", None)
            .unwrap();
        let npm = row(&db, "npm");
        assert_eq!(npm.advisory_count, 618);
        assert_eq!(npm.error.as_deref(), Some("still offline"));
        assert_eq!(npm.last_synced_at.as_deref(), Some("2026-11-08 02:00:00"));
    }

    /// No row yet (the first pass ever was offline, with an archive on disk):
    /// the time is the archive's download time, never "now".
    #[test]
    fn a_first_unverified_row_takes_the_archives_download_time() {
        let db = test_db();
        db.record_osv_sync_unverified("npm", Some(3), "offline", Some("2026-11-09 03:00:00"))
            .unwrap();
        assert_eq!(
            row(&db, "npm").last_synced_at.as_deref(),
            Some("2026-11-09 03:00:00")
        );
    }

    /// An archive downloaded after the row's time (the launch cache refresh
    /// downloads without touching the status) dates the data it serves.
    #[test]
    fn a_newer_archive_moves_the_time_to_its_download_never_past_it() {
        let db = test_db();
        db.update_osv_sync_status("npm", 1, None).unwrap();
        set_time(&db, "npm", "2026-11-01 00:00:00");
        db.record_osv_sync_unverified("npm", Some(2), "offline", Some("2026-11-05 12:00:00"))
            .unwrap();
        assert_eq!(
            row(&db, "npm").last_synced_at.as_deref(),
            Some("2026-11-05 12:00:00")
        );
    }

    /// No row and no archive: the row carries the error and no time at all.
    #[test]
    fn a_failure_with_no_data_has_an_error_and_no_time() {
        let db = test_db();
        db.record_osv_sync_unverified("npm", None, "offline on first run", None)
            .unwrap();
        let npm = row(&db, "npm");
        assert_eq!(npm.last_synced_at, None);
        assert_eq!(npm.error.as_deref(), Some("offline on first run"));
        // A later pass that has an archive dates it.
        db.record_osv_sync_unverified("npm", Some(4), "offline", Some("2026-11-09 03:00:00"))
            .unwrap();
        assert_eq!(
            row(&db, "npm").last_synced_at.as_deref(),
            Some("2026-11-09 03:00:00")
        );
    }

    #[test]
    fn the_ingested_archive_mark_is_per_ecosystem_and_clearable() {
        let db = test_db();
        assert_eq!(db.osv_archive_ingested("npm"), None);
        db.set_osv_archive_ingested("npm", "abc").unwrap();
        db.set_osv_archive_ingested("crates.io", "def").unwrap();
        assert_eq!(db.osv_archive_ingested("npm").as_deref(), Some("abc"));
        assert_eq!(db.osv_archive_ingested("crates.io").as_deref(), Some("def"));
        assert_eq!(db.clear_osv_archives_ingested().unwrap(), 2);
        assert_eq!(db.osv_archive_ingested("npm"), None);
        assert_eq!(db.osv_archive_ingested("crates.io"), None);
    }
}
