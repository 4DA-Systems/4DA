// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Temporal Event Store for 4DA Innovation Features
//!
//! Provides recording and querying of temporal events, project dependencies,
//! and cross-item relationships used by predictive context, semantic diff,
//! signal chains, knowledge decay, and attention tracking.

use crate::error::{Result, ResultExt};
use rusqlite::params;
use serde::{Deserialize, Serialize};

// ============================================================================
// Path Canonicalization
// ============================================================================

/// Normalize a project path for consistent DB storage on Windows.
/// Lowercases the drive letter and path segments so `Documents` and `documents`
/// resolve to the same UNIQUE key. Uses forward slashes for uniformity.
fn canonicalize_project_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.replace('\\', "/").to_string()
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEvent {
    pub id: i64,
    pub event_type: String,
    pub subject: String,
    pub data: serde_json::Value,
    pub source_item_id: Option<i64>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDependency {
    pub id: i64,
    pub project_path: String,
    pub manifest_type: String,
    pub package_name: String,
    pub version: Option<String>,
    pub is_dev: bool,
    /// Whether this is a direct dependency (listed in manifest) vs transitive
    /// (pulled in via lockfile). Direct deps default to true for backwards
    /// compatibility — existing rows without the column get is_direct=1.
    pub is_direct: bool,
    pub language: String,
    pub last_scanned: String,
}

// ============================================================================
// Project Dependencies
// ============================================================================

/// Upsert a project dependency.
///
/// `is_direct` indicates whether this dependency is declared directly in a
/// manifest file (`true`) or is a transitive dependency discovered from a
/// lockfile (`false`). On conflict the `is_direct` value is only *upgraded*
/// (transitive -> direct) but never downgraded, so a lockfile upsert cannot
/// demote a previously-seen direct dep.
///
/// `project_relevance` is a 0.0..1.0 score from ACE path/git analysis.
/// Example/demo/test directories get low scores (0.1x). The column defaults
/// to 1.0 in the schema, so existing rows and callers passing 1.0 are unaffected.
pub fn upsert_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    is_direct: bool,
    language: &str,
    project_relevance: f32,
) -> Result<()> {
    upsert_dependency_with_platform(
        conn,
        project_path,
        manifest_type,
        package_name,
        version,
        is_dev,
        is_direct,
        language,
        project_relevance,
        None,
        true,
    )
}

/// Like [`upsert_dependency`], but records platform relevance. `target_cfg` is the
/// gating spec (e.g. `cfg(windows)`) or `None` for unconditional deps;
/// `platform_active` is `false` when the dep is not built on the host. These feed
/// the relevance gate so platform-irrelevant advisories can be de-emphasised. The
/// columns default to (NULL, 1) so callers that don't care stay unaffected.
#[allow(clippy::too_many_arguments)]
pub fn upsert_dependency_with_platform(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    is_direct: bool,
    language: &str,
    project_relevance: f32,
    target_cfg: Option<&str>,
    platform_active: bool,
) -> Result<()> {
    let canonical_path = canonicalize_project_path(project_path);
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language, project_relevance, target_cfg, platform_active, last_scanned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET version = ?4, is_dev = ?5, is_direct = MAX(project_dependencies.is_direct, ?6), project_relevance = ?8, target_cfg = ?9, platform_active = ?10, last_scanned = datetime('now')",
        params![canonical_path, manifest_type, package_name, version, is_dev as i32, is_direct as i32, language, project_relevance, target_cfg, platform_active as i32],
    )
    .context("Failed to upsert dependency")?;
    Ok(())
}

/// Remove direct dependencies for a (project, language) that were NOT present in
/// the latest manifest scan.
///
/// Called right after the freshly-parsed manifest deps are upserted, so any dep
/// dropped from the manifest — or now intentionally skipped (local `path`/`git`
/// crates, `file:`/`workspace:` npm specs) — stops lingering as a stale row.
/// Stale rows otherwise surface as bogus "unmonitored" blind spots (e.g. an
/// internal workspace crate the user removed weeks ago).
///
/// Scoped to `is_direct = 1`: direct rows are the only ones written from manifest
/// scans, so transitive rows from other code paths are never touched here.
/// No-op when `current_names` is empty, so a parse that yields nothing (parse
/// failure, or a genuinely dep-less manifest) cannot wipe a project's deps.
///
/// Returns the number of stale rows removed.
pub fn prune_removed_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
    language: &str,
    current_names: &[String],
) -> Result<usize> {
    if current_names.is_empty() {
        return Ok(0);
    }
    let canonical_path = canonicalize_project_path(project_path);
    let placeholders = vec!["?"; current_names.len()].join(", ");
    let sql = format!(
        "DELETE FROM project_dependencies
         WHERE project_path = ? AND language = ? AND is_direct = 1
           AND package_name NOT IN ({placeholders})"
    );
    // Bind params in positional order: project_path, language, then keep-list.
    let mut values: Vec<String> = Vec::with_capacity(current_names.len() + 2);
    values.push(canonical_path);
    values.push(language.to_string());
    values.extend(current_names.iter().cloned());
    let removed = conn
        .execute(&sql, rusqlite::params_from_iter(values.iter()))
        .context("Failed to prune removed dependencies")?;
    Ok(removed)
}

/// Get all dependencies for a project
pub fn get_project_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<Vec<ProjectDependency>> {
    let canonical = canonicalize_project_path(project_path);
    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
             FROM project_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )
        ?;

    let results: Vec<ProjectDependency> = stmt
        .query_map(params![canonical], map_project_dependency_row)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .collect();

    Ok(results)
}

/// Map a row from the project_dependencies table to a `ProjectDependency`.
/// Column order must be: id, project_path, manifest_type, package_name,
///                        version, is_dev, is_direct, language, last_scanned.
fn map_project_dependency_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDependency> {
    Ok(ProjectDependency {
        id: row.get(0)?,
        project_path: row.get(1)?,
        manifest_type: row.get(2)?,
        package_name: row.get(3)?,
        version: row.get(4)?,
        is_dev: row.get::<_, i32>(5)? != 0,
        is_direct: row.get::<_, i32>(6).unwrap_or(1) != 0,
        language: row.get(7)?,
        last_scanned: row.get(8)?,
    })
}

/// Get all tracked dependencies, scoped to projects with recent git activity.
/// Only includes deps from project trees that have commits in the last 60 days.
/// Falls back to all deps if no git signals exist (first run).
pub fn get_all_dependencies(conn: &rusqlite::Connection) -> Result<Vec<ProjectDependency>> {
    // Get active repo roots from git_signals (repos with recent commits)
    let active_roots: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT repo_path FROM git_signals
             WHERE commit_hash IS NOT NULL AND commit_hash != ''
             AND timestamp > datetime('now', '-60 days')",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows
                .filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Row processing failed in temporal: {e}");
                        None
                    }
                })
                .collect())
        })
        .unwrap_or_default();

    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
             FROM project_dependencies
             ORDER BY project_path, package_name",
        )
        ?;

    let all_deps: Vec<ProjectDependency> = stmt
        .query_map([], map_project_dependency_row)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .collect();

    // Filter to deps from active project trees only
    if active_roots.is_empty() {
        // No git signals yet — return all deps (first run fallback)
        return Ok(all_deps);
    }

    let filtered: Vec<ProjectDependency> = all_deps
        .into_iter()
        .filter(|dep| {
            // Normalize for case-insensitive comparison on Windows
            let dep_path = dep.project_path.to_lowercase();
            active_roots.iter().any(|root| {
                let root_lower = root.to_lowercase();
                dep_path.starts_with(&root_lower) || root_lower.starts_with(&dep_path)
            })
        })
        .collect();

    // If filtering eliminated everything, fall back to all deps
    if filtered.is_empty() {
        return Ok(conn
            .prepare(
                "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
                 FROM project_dependencies ORDER BY project_path, package_name",
            )
            ?
            .query_map([], map_project_dependency_row)
            ?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in temporal: {e}");
                    None
                }
            })
            .collect());
    }

    Ok(filtered)
}

// ============================================================================
// Temporal Events
// ============================================================================

/// Record a new temporal event
pub fn record_event(
    conn: &rusqlite::Connection,
    event_type: &str,
    subject: &str,
    data: &serde_json::Value,
    source_item_id: Option<i64>,
    expires_at: Option<&str>,
) -> Result<i64> {
    let data_str = serde_json::to_string(data)?;
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, subject, data_str, source_item_id, expires_at],
    )
    .context("Failed to record temporal event")?;
    Ok(conn.last_insert_rowid())
}

/// Query temporal events by type and optional time range
pub fn query_events(
    conn: &rusqlite::Connection,
    event_type: &str,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<TemporalEvent>> {
    let query = if since.is_some() {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1 AND created_at >= ?2
         ORDER BY created_at DESC LIMIT ?3"
    } else {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1
         ORDER BY created_at DESC LIMIT ?2"
    };

    let mut stmt = conn.prepare(query)?;

    let results: Vec<TemporalEvent> = if let Some(since_val) = since {
        stmt.query_map(params![event_type, since_val, limit as i64], map_event)
    } else {
        stmt.query_map(params![event_type, limit as i64], map_event)
    }?
    .filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in temporal: {e}");
            None
        }
    })
    .collect();

    Ok(results)
}

fn map_event(row: &rusqlite::Row) -> rusqlite::Result<TemporalEvent> {
    let data_str: String = row.get(3)?;
    let data = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
    Ok(TemporalEvent {
        id: row.get(0)?,
        event_type: row.get(1)?,
        subject: row.get(2)?,
        data,
        source_item_id: row.get(4)?,
        created_at: row.get(5)?,
        expires_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn record_and_query_event_roundtrip() {
        let conn = setup_test_db();
        let data = serde_json::json!({"key": "value", "count": 42});
        let id = record_event(&conn, "test_type", "test_subject", &data, Some(100), None).unwrap();
        assert!(id > 0);

        let events = query_events(&conn, "test_type", None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
        assert_eq!(events[0].event_type, "test_type");
        assert_eq!(events[0].subject, "test_subject");
        assert_eq!(events[0].source_item_id, Some(100));
        assert_eq!(events[0].data["key"], "value");
        assert_eq!(events[0].data["count"], 42);
    }

    #[test]
    fn query_events_respects_limit() {
        let conn = setup_test_db();
        let data = serde_json::json!({});
        for i in 0..5 {
            record_event(&conn, "bulk", &format!("subj_{}", i), &data, None, None).unwrap();
        }
        let events = query_events(&conn, "bulk", None, 3).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn query_events_filters_by_type() {
        let conn = setup_test_db();
        let data = serde_json::json!({});
        record_event(&conn, "type_a", "s1", &data, None, None).unwrap();
        record_event(&conn, "type_b", "s2", &data, None, None).unwrap();
        record_event(&conn, "type_a", "s3", &data, None, None).unwrap();

        let a_events = query_events(&conn, "type_a", None, 10).unwrap();
        assert_eq!(a_events.len(), 2);
        let b_events = query_events(&conn, "type_b", None, 10).unwrap();
        assert_eq!(b_events.len(), 1);
    }

    #[test]
    fn temporal_event_serde_roundtrip() {
        let event = TemporalEvent {
            id: 1,
            event_type: "version_release".to_string(),
            subject: "react".to_string(),
            data: serde_json::json!({"version": "19.0.0", "breaking": true}),
            source_item_id: Some(42),
            created_at: "2026-02-28T10:00:00".to_string(),
            expires_at: Some("2026-03-28T10:00:00".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TemporalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, "version_release");
        assert_eq!(deserialized.data["version"], "19.0.0");
        assert_eq!(deserialized.source_item_id, Some(42));
        assert!(deserialized.expires_at.is_some());
    }

    fn setup_deps_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL DEFAULT 'cargotoml',
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT NOT NULL DEFAULT 'rust',
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                project_relevance REAL DEFAULT 1.0,
                target_cfg TEXT,
                platform_active INTEGER DEFAULT 1,
                UNIQUE(project_path, package_name)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn prune_removes_dropped_direct_deps_only() {
        let conn = setup_deps_db();
        let proj = "D:/proj/app";
        // Two current direct deps + one stale (removed) direct dep.
        upsert_dependency(
            &conn,
            proj,
            "cargotoml",
            "serde",
            None,
            false,
            true,
            "rust",
            1.0,
        )
        .unwrap();
        upsert_dependency(
            &conn,
            proj,
            "cargotoml",
            "tokio",
            None,
            false,
            true,
            "rust",
            1.0,
        )
        .unwrap();
        upsert_dependency(
            &conn,
            proj,
            "cargotoml",
            "removed_crate",
            None,
            false,
            true,
            "rust",
            1.0,
        )
        .unwrap();
        // A transitive dep (is_direct = 0) must NOT be pruned.
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language, is_direct)
             VALUES (?1, 'cargotoml', 'transitive_dep', 'rust', 0)",
            params![canonicalize_project_path(proj)],
        )
        .unwrap();
        // A different-language direct dep must NOT be pruned by a rust scan.
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language, is_direct)
             VALUES (?1, 'packagejson', 'react', 'javascript', 1)",
            params![canonicalize_project_path(proj)],
        )
        .unwrap();

        let current = vec!["serde".to_string(), "tokio".to_string()];
        let removed = prune_removed_dependencies(&conn, proj, "rust", &current).unwrap();
        assert_eq!(removed, 1, "only the dropped direct rust dep is removed");

        let names: Vec<String> = conn
            .prepare("SELECT package_name FROM project_dependencies ORDER BY package_name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(names.contains(&"serde".to_string()));
        assert!(names.contains(&"tokio".to_string()));
        assert!(!names.contains(&"removed_crate".to_string()));
        assert!(
            names.contains(&"transitive_dep".to_string()),
            "transitive dep must survive"
        );
        assert!(
            names.contains(&"react".to_string()),
            "other-language dep must survive"
        );
    }

    #[test]
    fn prune_is_noop_on_empty_keep_list() {
        let conn = setup_deps_db();
        let proj = "D:/proj/app";
        upsert_dependency(
            &conn,
            proj,
            "cargotoml",
            "serde",
            None,
            false,
            true,
            "rust",
            1.0,
        )
        .unwrap();
        // An empty keep-list must NOT wipe deps (guards against a parse failure
        // deleting a whole project's dependency set).
        let removed = prune_removed_dependencies(&conn, proj, "rust", &[]).unwrap();
        assert_eq!(removed, 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM project_dependencies", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn record_event_with_null_source_item_id() {
        let conn = setup_test_db();
        let data = serde_json::json!({"note": "no source"});
        let id = record_event(&conn, "manual", "user_action", &data, None, None).unwrap();
        let events = query_events(&conn, "manual", None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
        assert_eq!(events[0].source_item_id, None);
    }

    #[test]
    fn test_canonicalize_windows_paths() {
        let result = canonicalize_project_path(r"D:\Users\Admin\Documents\my-project");
        if cfg!(windows) {
            assert_eq!(result, "d:/users/admin/documents/my-project");
        } else {
            assert_eq!(result, "D:/Users/Admin/Documents/my-project");
        }
    }

    #[test]
    fn test_canonicalize_merges_case_variants() {
        let a = canonicalize_project_path(r"C:\Users\Dev\Documents\kairos-mvp");
        let b = canonicalize_project_path(r"C:\Users\Dev\documents\kairos-mvp");
        if cfg!(windows) {
            assert_eq!(a, b, "Case variants should canonicalize to the same key");
        }
    }

    #[test]
    fn test_canonicalize_forward_slashes() {
        let result = canonicalize_project_path("C:/Users/Dev/project");
        if cfg!(windows) {
            assert_eq!(result, "c:/users/dev/project");
        } else {
            assert_eq!(result, "C:/Users/Dev/project");
        }
    }
}
