//! Database concurrency & integrity tests — Phase 3.5 hardening.
//!
//! Covers: concurrent readers, concurrent readers + writer, transaction
//! rollback, WAL mode verification, foreign key enforcement, and migration
//! idempotency.

#[cfg(test)]
mod tests {
    use crate::db::Database;
    use crate::test_utils::test_db;
    use rusqlite::{params, Connection};
    use std::sync::{Arc, Barrier};

    // ========================================================================
    // Helpers
    // ========================================================================

    /// RAII guard that removes a temp DB file (plus WAL/SHM sidecars) on drop.
    struct TempDbGuard(std::path::PathBuf);

    impl Drop for TempDbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
            let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
        }
    }

    /// Open a raw in-memory connection with sqlite-vec, basic pragmas, and a
    /// simple test table.  Suitable for low-level concurrency tests that don't
    /// need the full 4DA schema.
    fn raw_test_conn() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA busy_timeout = 5000;
            CREATE TABLE IF NOT EXISTS test_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                value TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        conn
    }

    /// Create a temp-file-backed database path with a unique name.
    /// Returns the path and an RAII guard that cleans up on drop.
    fn temp_db(label: &str) -> (std::path::PathBuf, TempDbGuard) {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "4da_concurrency_test_{}_{}.db",
            label,
            std::process::id()
        ));
        let guard = TempDbGuard(p.clone());
        (p, guard)
    }

    // ========================================================================
    // 1. Concurrent readers — no SQLITE_BUSY, no panics
    // ========================================================================

    #[test]
    fn test_concurrent_readers() {
        let (db_path, _guard) = temp_db("readers");

        crate::register_sqlite_vec_extension();

        // Seed data through a single connection.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA busy_timeout = 5000;
                CREATE TABLE IF NOT EXISTS test_items (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    value TEXT NOT NULL
                );
                ",
            )
            .unwrap();
            for i in 0..100 {
                conn.execute(
                    "INSERT INTO test_items (value) VALUES (?1)",
                    params![format!("item_{}", i)],
                )
                .unwrap();
            }
        }

        let path = Arc::new(db_path);
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = Vec::new();

        for t in 0..10 {
            let p = Arc::clone(&path);
            let b = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                crate::register_sqlite_vec_extension();
                let conn = Connection::open(p.as_ref()).unwrap();
                conn.execute_batch("PRAGMA busy_timeout = 5000;").unwrap();
                // Synchronise so all 10 threads read at roughly the same time.
                b.wait();

                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM test_items", [], |r| r.get(0))
                    .unwrap_or_else(|e| panic!("Reader thread {} failed: {}", t, e));
                assert_eq!(count, 100, "Reader {} saw {} rows, expected 100", t, count);

                // Read all rows to stress shared-cache / page cache.
                let mut stmt = conn.prepare("SELECT id, value FROM test_items").unwrap();
                let rows: Vec<(i64, String)> = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(rows.len(), 100);
            }));
        }

        for h in handles {
            h.join().expect("reader thread should not panic");
        }
    }

    // ========================================================================
    // 2. Concurrent readers + writer — consistent data, no partial rows
    // ========================================================================

    #[test]
    fn test_concurrent_readers_writer() {
        let (db_path, _guard) = temp_db("rw");

        crate::register_sqlite_vec_extension();

        // Create table.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA busy_timeout = 5000;
                CREATE TABLE IF NOT EXISTS kv (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL,
                    val TEXT NOT NULL
                );
                ",
            )
            .unwrap();
        }

        let path = Arc::new(db_path);
        let barrier = Arc::new(Barrier::new(11)); // 10 readers + 1 writer
        let mut handles = Vec::new();

        // Writer thread: inserts 200 rows inside transactions of 10 each.
        {
            let p = Arc::clone(&path);
            let b = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                crate::register_sqlite_vec_extension();
                let mut conn = Connection::open(p.as_ref()).unwrap();
                conn.execute_batch("PRAGMA busy_timeout = 5000;").unwrap();
                b.wait();

                for batch in 0..20 {
                    let tx = conn.transaction().unwrap();
                    for i in 0..10 {
                        let idx = batch * 10 + i;
                        tx.execute(
                            "INSERT INTO kv (key, val) VALUES (?1, ?2)",
                            params![format!("k_{}", idx), format!("v_{}", idx)],
                        )
                        .unwrap();
                    }
                    tx.commit().unwrap();
                }
            }));
        }

        // 10 reader threads: each performs repeated reads.
        for t in 0..10 {
            let p = Arc::clone(&path);
            let b = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                crate::register_sqlite_vec_extension();
                let conn = Connection::open(p.as_ref()).unwrap();
                conn.execute_batch("PRAGMA busy_timeout = 5000;").unwrap();
                b.wait();

                for _ in 0..50 {
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM kv", [], |r| r.get(0))
                        .unwrap_or_else(|e| panic!("Reader {} count failed: {}", t, e));

                    // Count must be a multiple of 10 (writer commits in batches
                    // of 10), proving we never see partial transactions.
                    assert!(
                        count % 10 == 0,
                        "Reader {} saw count={} which is not a multiple of 10 — partial transaction visible",
                        t,
                        count
                    );
                }
            }));
        }

        for h in handles {
            h.join().expect("rw thread should not panic");
        }
    }

    // ========================================================================
    // 3. Transaction rollback on error
    // ========================================================================

    #[test]
    fn test_transaction_rollback_on_error() {
        let mut conn = raw_test_conn();

        // Insert a row outside a transaction so we have a baseline.
        conn.execute("INSERT INTO test_items (value) VALUES ('baseline')", [])
            .unwrap();

        // Begin a transaction, insert more data, then trigger an error.
        {
            let tx = conn.transaction().unwrap();
            tx.execute("INSERT INTO test_items (value) VALUES ('inside_tx_1')", [])
                .unwrap();
            tx.execute("INSERT INTO test_items (value) VALUES ('inside_tx_2')", [])
                .unwrap();

            // Force a constraint violation by adding a UNIQUE constraint table
            // and inserting a duplicate.
            tx.execute_batch(
                "CREATE TABLE unique_test (id INTEGER PRIMARY KEY, name TEXT UNIQUE);",
            )
            .unwrap();
            tx.execute("INSERT INTO unique_test (name) VALUES ('dup')", [])
                .unwrap();

            let dup_result = tx.execute("INSERT INTO unique_test (name) VALUES ('dup')", []);
            assert!(
                dup_result.is_err(),
                "Duplicate insert should fail with UNIQUE constraint"
            );

            // Do NOT commit — explicitly roll back.
            tx.rollback().unwrap();
        }

        // The baseline row should still exist.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM test_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 1,
            "After rollback, only the baseline row should remain (got {})",
            count
        );

        // The unique_test table should NOT exist (it was created inside the
        // rolled-back transaction).
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='unique_test'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !table_exists,
            "unique_test table should not exist after rollback"
        );
    }

    // ========================================================================
    // 4. WAL mode enabled
    // ========================================================================

    #[test]
    fn test_wal_mode_enabled() {
        // Use the production Database::new which sets WAL mode in its pragmas.
        let db = test_db();
        let conn = db.conn.lock();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();

        // In-memory databases report "memory" for journal_mode, so also
        // accept that.  For file-backed DBs this would be "wal".
        assert!(
            journal_mode == "wal" || journal_mode == "memory",
            "Expected WAL or memory journal mode, got '{}'",
            journal_mode
        );

        // Also verify via a file-backed DB for certainty.
        let (db_path, _guard) = temp_db("wal_check");

        let file_db = Database::new(&db_path).unwrap();
        let file_conn = file_db.conn.lock();
        let file_journal: String = file_conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            file_journal, "wal",
            "File-backed DB should use WAL, got '{}'",
            file_journal
        );
    }

    // ========================================================================
    // 5. Foreign key enforcement
    // ========================================================================

    #[test]
    fn test_foreign_key_enforcement() {
        // Use test_db() which runs through Database::new and sets PRAGMA foreign_keys = ON.
        let db = test_db();
        let conn = db.conn.lock();

        // Verify the pragma is enabled.
        let fk_enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1, "PRAGMA foreign_keys should be ON (1)");

        // The `feedback` table has a FOREIGN KEY on source_item_id referencing
        // source_items(id).  Inserting a feedback row with a non-existent
        // source_item_id should fail.
        let result = conn.execute(
            "INSERT INTO feedback (source_item_id, relevant, created_at) VALUES (99999, 1, datetime('now'))",
            [],
        );
        assert!(
            result.is_err(),
            "Inserting feedback with non-existent source_item_id should fail with FK violation"
        );

        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("FOREIGN KEY") || err_msg.contains("foreign key"),
            "Error should mention FOREIGN KEY constraint, got: {}",
            err_msg
        );
    }

    // ========================================================================
    // 6. Migration / table creation idempotent
    // ========================================================================

    #[test]
    fn test_database_creation_idempotent() {
        // Database::new runs migrate() internally.  Calling it twice on the
        // same file should not error (CREATE TABLE IF NOT EXISTS pattern).
        let (db_path, _guard) = temp_db("idempotent");

        // First creation — runs full migration.
        let db1 = Database::new(&db_path).unwrap();

        // Insert some data so we can verify it survives the second migration.
        {
            let conn = db1.conn.lock();
            conn.execute(
                "INSERT INTO sources (source_type, name, enabled) VALUES ('test_src', 'Test', 1)",
                [],
            )
            .unwrap();
        }
        // Drop first handle to release the file lock.
        drop(db1);

        // Second creation on the exact same file — should succeed and preserve data.
        let db2 = Database::new(&db_path).unwrap();
        {
            let conn = db2.conn.lock();

            // Tables should still exist.
            let tables: Vec<String> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
                    )
                    .unwrap();
                stmt.query_map([], |r| r.get(0))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            };
            assert!(
                tables.contains(&"context_chunks".to_string()),
                "context_chunks table missing after second init"
            );
            assert!(
                tables.contains(&"source_items".to_string()),
                "source_items table missing after second init"
            );
            assert!(
                tables.contains(&"feedback".to_string()),
                "feedback table missing after second init"
            );
            assert!(
                tables.contains(&"sources".to_string()),
                "sources table missing after second init"
            );

            // Data inserted before second init should still be present.
            let src_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sources WHERE source_type = 'test_src'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                src_count, 1,
                "Data inserted before second Database::new should survive"
            );
        }
    }
}
