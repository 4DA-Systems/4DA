// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Conditional VACUUM for `db::history` maintenance (split out for the file-size gate).

use rusqlite::Result as SqliteResult;
use tracing::info;

/// A VACUUM rewrites the WHOLE file under the connection lock — 30-53 s on the
/// founder's ~2 GB database — and returns only what the freelist holds. The
/// daily job used to run it unconditionally (twice on a day with a large
/// prune, plus a weekly one) against a freelist of ~1.8 MB (audit 2026-10-09).
/// It now runs only when there is something worth returning:
pub(crate) const VACUUM_MIN_RECLAIM_BYTES: i64 = 64 * 1024 * 1024;
/// …or when free pages are a large share of a small file.
pub(crate) const VACUUM_MIN_FREE_FRACTION: f64 = 0.10;

/// Whether a VACUUM is worth its full-file rewrite: more than
/// [`VACUUM_MIN_RECLAIM_BYTES`] on the freelist, or more than
/// [`VACUUM_MIN_FREE_FRACTION`] of all pages free. A large prune trips the
/// first bound by itself, so "VACUUM after a big delete" needs no row count.
pub(crate) fn vacuum_worthwhile(freelist_pages: i64, page_count: i64, page_size: i64) -> bool {
    if freelist_pages <= 0 {
        return false;
    }
    freelist_pages.saturating_mul(page_size) > VACUUM_MIN_RECLAIM_BYTES
        || (page_count > 0 && freelist_pages as f64 / page_count as f64 > VACUUM_MIN_FREE_FRACTION)
}

/// `Database::vacuum_if_reclaimable` on an already-locked connection.
pub(super) fn vacuum_conn_if_reclaimable(conn: &rusqlite::Connection) -> SqliteResult<bool> {
    let freelist: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let reclaimable_mb = freelist.saturating_mul(page_size) as f64 / (1024.0 * 1024.0);
    if !vacuum_worthwhile(freelist, pages, page_size) {
        info!(
            target: "4da::db",
            reclaimable_mb, freelist, pages, "VACUUM skipped — nothing worth reclaiming"
        );
        return Ok(false);
    }
    info!(target: "4da::db", reclaimable_mb, freelist, pages, "Running VACUUM");
    conn.execute_batch("VACUUM;")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{vacuum_worthwhile, VACUUM_MIN_RECLAIM_BYTES};
    use crate::test_utils::{insert_test_item, test_db};

    /// The bounds: an empty freelist never vacuums; > 64 MiB free always does;
    /// a small file vacuums once more than 10% of its pages are free.
    #[test]
    fn test_vacuum_worthwhile_bounds() {
        let page = 4096;
        // Founder DB 2026-10-09: ~2 GB, ~1.8 MB free -> skip.
        let pages_2gb = 2_000_000_000 / page;
        assert!(!vacuum_worthwhile(1_800_000 / page, pages_2gb, page));
        assert!(!vacuum_worthwhile(0, pages_2gb, page));
        assert!(!vacuum_worthwhile(0, 0, page));
        // Exactly 64 MiB is not "more than"; one page past it is.
        let at_bound = VACUUM_MIN_RECLAIM_BYTES / page;
        assert!(!vacuum_worthwhile(at_bound, pages_2gb, page));
        assert!(vacuum_worthwhile(at_bound + 1, pages_2gb, page));
        // Small file: 11% free vacuums, 9% does not.
        assert!(vacuum_worthwhile(11, 100, page));
        assert!(!vacuum_worthwhile(9, 100, page));
    }

    /// `run_maintenance` reports whether it actually compacted, and a large
    /// delete that leaves a big freelist does compact.
    #[test]
    fn test_run_maintenance_vacuums_only_when_reclaimable() {
        let db = test_db();
        insert_test_item(&db, "hackernews", "keep", "Keep", "content");
        let result = db.run_maintenance(30).unwrap();
        assert!(!result.vacuumed, "nothing freed -> no full-file rewrite");

        {
            let conn = db.conn.lock();
            conn.execute_batch(
                "CREATE TABLE ballast (b BLOB);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 400)
                 INSERT INTO ballast SELECT randomblob(8192) FROM n;
                 DROP TABLE ballast;",
            )
            .unwrap();
        }
        let result = db.run_maintenance(30).unwrap();
        assert!(
            result.vacuumed,
            "a freelist past 10% of the file is reclaimed"
        );
        let free: i64 = db
            .conn
            .lock()
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))
            .unwrap();
        assert_eq!(free, 0);
    }
}
