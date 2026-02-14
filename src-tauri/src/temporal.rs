//! Temporal Event Store for 4DA Innovation Features
//!
//! Provides recording and querying of temporal events, project dependencies,
//! and cross-item relationships used by predictive context, semantic diff,
//! signal chains, knowledge decay, and attention tracking.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

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
    pub language: String,
    pub last_scanned: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRelationship {
    pub id: i64,
    pub source_item_id: i64,
    pub related_item_id: i64,
    pub relationship_type: String,
    pub strength: f64,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
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
) -> Result<i64, String> {
    let data_str = serde_json::to_string(data).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, subject, data_str, source_item_id, expires_at],
    )
    .map_err(|e| format!("Failed to record temporal event: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Query temporal events by type and optional time range
pub fn query_events(
    conn: &rusqlite::Connection,
    event_type: &str,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<TemporalEvent>, String> {
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

    let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;

    let results: Vec<TemporalEvent> = if let Some(since_val) = since {
        stmt.query_map(params![event_type, since_val, limit as i64], map_event)
    } else {
        stmt.query_map(params![event_type, limit as i64], map_event)
    }
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(results)
}

/// Query temporal events by subject
pub fn query_events_by_subject(
    conn: &rusqlite::Connection,
    subject: &str,
    limit: usize,
) -> Result<Vec<TemporalEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
             FROM temporal_events
             WHERE subject = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<TemporalEvent> = stmt
        .query_map(params![subject, limit as i64], map_event)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Clean up expired temporal events
pub fn cleanup_expired(conn: &rusqlite::Connection) -> Result<usize, String> {
    let deleted = conn
        .execute(
            "DELETE FROM temporal_events WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
            [],
        )
        .map_err(|e| e.to_string())?;
    if deleted > 0 {
        debug!(target: "4da::temporal", deleted, "Cleaned up expired temporal events");
    }
    Ok(deleted)
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

// ============================================================================
// Project Dependencies
// ============================================================================

/// Upsert a project dependency
pub fn upsert_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    language: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language, last_scanned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET version = ?4, is_dev = ?5, last_scanned = datetime('now')",
        params![project_path, manifest_type, package_name, version, is_dev as i32, language],
    )
    .map_err(|e| format!("Failed to upsert dependency: {}", e))?;
    Ok(())
}

/// Get all dependencies for a project
pub fn get_project_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<Vec<ProjectDependency>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
             FROM project_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<ProjectDependency> = stmt
        .query_map(params![project_path], |row| {
            Ok(ProjectDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                manifest_type: row.get(2)?,
                package_name: row.get(3)?,
                version: row.get(4)?,
                is_dev: row.get::<_, i32>(5)? != 0,
                language: row.get(6)?,
                last_scanned: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Get all tracked dependencies, scoped to projects with recent git activity.
/// Only includes deps from project trees that have commits in the last 60 days.
/// Falls back to all deps if no git signals exist (first run).
pub fn get_all_dependencies(conn: &rusqlite::Connection) -> Result<Vec<ProjectDependency>, String> {
    // Get active repo roots from git_signals (repos with recent commits)
    let active_roots: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT repo_path FROM git_signals
             WHERE commit_hash IS NOT NULL AND commit_hash != ''
             AND timestamp > datetime('now', '-60 days')",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
             FROM project_dependencies
             ORDER BY project_path, package_name",
        )
        .map_err(|e| e.to_string())?;

    let all_deps: Vec<ProjectDependency> = stmt
        .query_map([], |row| {
            Ok(ProjectDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                manifest_type: row.get(2)?,
                package_name: row.get(3)?,
                version: row.get(4)?,
                is_dev: row.get::<_, i32>(5)? != 0,
                language: row.get(6)?,
                last_scanned: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
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
                "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
                 FROM project_dependencies ORDER BY project_path, package_name",
            )
            .map_err(|e| e.to_string())?
            .query_map([], |row| {
                Ok(ProjectDependency {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    manifest_type: row.get(2)?,
                    package_name: row.get(3)?,
                    version: row.get(4)?,
                    is_dev: row.get::<_, i32>(5)? != 0,
                    language: row.get(6)?,
                    last_scanned: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect());
    }

    Ok(filtered)
}

/// Search for a specific package across all projects
pub fn find_dependency(
    conn: &rusqlite::Connection,
    package_name: &str,
) -> Result<Vec<ProjectDependency>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
             FROM project_dependencies
             WHERE package_name = ?1",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<ProjectDependency> = stmt
        .query_map(params![package_name], |row| {
            Ok(ProjectDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                manifest_type: row.get(2)?,
                package_name: row.get(3)?,
                version: row.get(4)?,
                is_dev: row.get::<_, i32>(5)? != 0,
                language: row.get(6)?,
                last_scanned: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

// ============================================================================
// Item Relationships
// ============================================================================

/// Create or update an item relationship
pub fn upsert_relationship(
    conn: &rusqlite::Connection,
    source_item_id: i64,
    related_item_id: i64,
    relationship_type: &str,
    strength: f64,
    metadata: Option<&serde_json::Value>,
) -> Result<(), String> {
    let metadata_str = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
    conn.execute(
        "INSERT INTO item_relationships (source_item_id, related_item_id, relationship_type, strength, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_item_id, related_item_id, relationship_type)
         DO UPDATE SET strength = ?4, metadata = ?5",
        params![source_item_id, related_item_id, relationship_type, strength, metadata_str],
    )
    .map_err(|e| format!("Failed to upsert relationship: {}", e))?;
    Ok(())
}

/// Get relationships for a source item
pub fn get_relationships(
    conn: &rusqlite::Connection,
    source_item_id: i64,
    relationship_type: Option<&str>,
) -> Result<Vec<ItemRelationship>, String> {
    let (query, use_type) = if relationship_type.is_some() {
        (
            "SELECT id, source_item_id, related_item_id, relationship_type, strength, metadata, created_at
             FROM item_relationships
             WHERE source_item_id = ?1 AND relationship_type = ?2
             ORDER BY strength DESC",
            true,
        )
    } else {
        (
            "SELECT id, source_item_id, related_item_id, relationship_type, strength, metadata, created_at
             FROM item_relationships
             WHERE source_item_id = ?1
             ORDER BY strength DESC",
            false,
        )
    };

    let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;

    let results: Vec<ItemRelationship> = if use_type {
        stmt.query_map(
            params![source_item_id, relationship_type.unwrap()],
            map_relationship,
        )
    } else {
        stmt.query_map(params![source_item_id], map_relationship)
    }
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(results)
}

fn map_relationship(row: &rusqlite::Row) -> rusqlite::Result<ItemRelationship> {
    let metadata_str: Option<String> = row.get(5)?;
    let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
    Ok(ItemRelationship {
        id: row.get(0)?,
        source_item_id: row.get(1)?,
        related_item_id: row.get(2)?,
        relationship_type: row.get(3)?,
        strength: row.get(4)?,
        metadata,
        created_at: row.get(6)?,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_temporal_events(
    event_type: String,
    since: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TemporalEvent>, String> {
    let conn = crate::open_db_connection()?;
    query_events(&conn, &event_type, since.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub fn get_temporal_event_count(event_type: String) -> Result<usize, String> {
    let conn = crate::open_db_connection()?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM temporal_events WHERE event_type = ?1",
            params![event_type],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count as usize)
}

#[tauri::command]
pub fn get_dependencies(project_path: Option<String>) -> Result<Vec<ProjectDependency>, String> {
    let conn = crate::open_db_connection()?;
    if let Some(path) = project_path {
        get_project_dependencies(&conn, &path)
    } else {
        get_all_dependencies(&conn)
    }
}

#[tauri::command]
pub fn cleanup_temporal_events() -> Result<usize, String> {
    let conn = crate::open_db_connection()?;
    cleanup_expired(&conn)
}
