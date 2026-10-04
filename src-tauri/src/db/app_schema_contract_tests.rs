// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The app-schema contract: the database schema a running 4DA writes, frozen in
//! `src-tauri/contract/app-schema.sql`.
//!
//! `@4da/mcp-server` (github.com/4DA-Systems/4da-mcp-server) opens this same
//! database file and queries about 30 of its tables. It lives in its own
//! repository, so a migration here cannot see its queries and its tests cannot
//! see a migration. This file is the hand-off: every schema change shows up as a
//! diff to `app-schema.sql`, and CI (`validate.yml`, job `mcp-contract`)
//! re-prepares every statement the MCP server issues against that file. A
//! migration that drops or renames something the server reads fails there,
//! before merge, instead of in a user's agent session.
//!
//! The schema is what a long-running install holds, not only what
//! `Database::new` migrates: the ACE tables, the context-engine tables and
//! `engine_runs` are created by their own modules at startup, and the MCP server
//! reads them too.
//!
//! Regenerate after a schema change:
//! `UPDATE_APP_SCHEMA_CONTRACT=1 cargo test --lib app_schema_contract`

use rusqlite::Connection;

use super::Database;

/// The schema a fresh install holds once every startup initialiser has run.
fn build_app_schema() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = Database::new(&dir.path().join("4da.db")).expect("Database::new migrates");
    crate::ace::db::migrate(&db.conn).expect("ACE schema");
    crate::context_engine::ContextEngine::new(db.conn.clone()).expect("context-engine schema");
    crate::engine_runs::ensure_table(&db.conn.lock()).expect("engine_runs schema");
    (dir, db)
}

/// A deterministic SQL rendering of the schema: tables, then indexes, triggers
/// and views, each sorted by name. Shadow tables (FTS5 / vec0 internals) are
/// left out because `CREATE VIRTUAL TABLE` recreates them.
fn render_schema(conn: &Connection) -> String {
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .expect("schema_version");
    let names = |sql: &str| -> Vec<String> {
        conn.prepare(sql)
            .and_then(|mut s| {
                s.query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("schema names")
    };
    // FTS5 reports its internals as shadow tables; sqlite-vec does not, so its
    // `<table>_chunks`, `_rowids`, `_info` and `_vector_chunksNN` are matched
    // by the vec0 table's name prefix instead.
    let shadow = names("SELECT name FROM pragma_table_list WHERE type = 'shadow'");
    let vec0_prefixes: Vec<String> = names(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%USING vec0%'",
    )
    .into_iter()
    .map(|n| format!("{n}_"))
    .collect();
    let is_internal = |name: &str| {
        shadow.iter().any(|s| s.as_str() == name)
            || vec0_prefixes.iter().any(|p| name.starts_with(p))
    };
    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_master
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
             ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1
                                WHEN 'trigger' THEN 2 ELSE 3 END, name",
        )
        .expect("sqlite_master");
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .expect("schema rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("schema rows");

    let mut out = format!(
        "-- 4DA app-schema contract. GENERATED, do not edit by hand.\n\
         -- Regenerate: UPDATE_APP_SCHEMA_CONTRACT=1 cargo test --lib app_schema_contract\n\
         -- Consumer: github.com/4DA-Systems/4da-mcp-server (pnpm run contract)\n\
         -- schema_version: {version}\n\n"
    );
    for (kind, name, sql) in rows {
        if kind == "table" && is_internal(&name) {
            continue;
        }
        // Normalise line endings so the file is byte-identical on every OS.
        let sql = sql.replace("\r\n", "\n");
        out.push_str(sql.trim_end());
        out.push_str(";\n\n");
    }
    out.push_str(&format!(
        "INSERT INTO schema_version (version) VALUES ({version});\n"
    ));
    out
}

/// The contract is the schema of the build users install, which uses the
/// default features (release.yml passes only `--target` to tauri-action).
/// Optional features may add tables (`experimental` creates the achievement
/// tables through a migration hook that is a stub otherwise). A superset can
/// never break a query the MCP server issues, and the default-feature CI leg
/// checks the file, so the other legs skip the comparison rather than each
/// needing its own copy.
#[cfg(not(any(
    feature = "experimental",
    feature = "team-sync",
    feature = "enterprise"
)))]
#[test]
fn app_schema_contract_is_current() {
    let (_dir, db) = build_app_schema();
    let rendered = render_schema(&db.conn.lock());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("contract")
        .join("app-schema.sql");

    if std::env::var_os("UPDATE_APP_SCHEMA_CONTRACT").is_some() {
        std::fs::create_dir_all(path.parent().expect("contract dir")).expect("mkdir contract");
        std::fs::write(&path, &rendered).expect("write app-schema.sql");
        return;
    }

    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == rendered,
        "src-tauri/contract/app-schema.sql is out of date with the migrations.\n\
         Regenerate it: UPDATE_APP_SCHEMA_CONTRACT=1 cargo test --lib app_schema_contract\n\
         and commit the diff. CI then checks the change against @4da/mcp-server's queries."
    );
}

#[test]
fn app_schema_contract_excludes_shadow_tables_and_reloads_cleanly() {
    // The consumer loads the file into a fresh SQLite. Shadow tables would make
    // `CREATE VIRTUAL TABLE ... USING fts5` fail with "table already exists".
    let (_dir, db) = build_app_schema();
    let rendered = render_schema(&db.conn.lock());
    crate::register_sqlite_vec_extension();
    let fresh = Connection::open_in_memory().expect("in-memory");
    fresh
        .execute_batch(&rendered)
        .expect("app-schema.sql must load into an empty database");
}
