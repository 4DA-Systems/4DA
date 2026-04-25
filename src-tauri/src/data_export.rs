// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Comprehensive data export for GDPR compliance and data portability.
//!
//! Provides commands to export all user data stored in 4DA's local database.
//! Available on ALL tiers (not feature-gated) — GDPR compliance is universal.
//!
//! **Privacy safeguards:** All exports strip API keys, tokens, passwords, and
//! secrets before writing. Exports are logged to the audit trail.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use tracing::{info, warn};
use ts_rs::TS;

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

/// Manifest describing a completed data export.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExportManifest {
    pub export_id: String,
    pub created_at: String,
    pub format: String,
    pub sections: Vec<String>,
    pub total_records: u32,
}

/// All known export sections.
const ALL_SECTIONS: &[&str] = &[
    "user_profile",
    "decisions",
    "signals",
    "sources",
    "briefings",
    "feedback",
    "learned_behavior",
];

// ============================================================================
// Schema
// ============================================================================

/// Ensure the data_exports tracking table exists.
fn ensure_tables(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS data_exports (
            id TEXT PRIMARY KEY,
            format TEXT NOT NULL,
            sections TEXT NOT NULL,
            total_records INTEGER DEFAULT 0,
            file_path TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

// ============================================================================
// Privacy: Sensitive Field Stripping
// ============================================================================

/// Fields that must NEVER appear in exports.
const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "apiKey",
    "api_keys",
    "token",
    "secret",
    "password",
    "private_key",
    "privateKey",
    "access_token",
    "accessToken",
    "refresh_token",
    "refreshToken",
    "secret_key",
    "secretKey",
    "x_api_key",
    "openai_key",
    "anthropic_key",
    "groq_key",
    "openrouter_key",
];

/// Recursively strip sensitive fields from a JSON value.
pub(crate) fn strip_sensitive_fields(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            let keys_to_remove: Vec<String> = map
                .keys()
                .filter(|k| {
                    let lower = k.to_lowercase();
                    SENSITIVE_KEYS.iter().any(|s| lower == s.to_lowercase())
                })
                .cloned()
                .collect();

            for key in keys_to_remove {
                map.insert(key, JsonValue::String("[REDACTED]".to_string()));
            }

            for (_k, v) in map.iter_mut() {
                strip_sensitive_fields(v);
            }
        }
        JsonValue::Array(arr) => {
            for item in arr.iter_mut() {
                strip_sensitive_fields(item);
            }
        }
        _ => {}
    }
}

// ============================================================================
// Export Helpers
// ============================================================================

/// Check if a table exists in the database.
#[inline]
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Allowed table names for export — prevents SQL identifier injection.
const ALLOWED_EXPORT_TABLES: &[&str] = &[
    "sovereign_profile",
    "detected_tech",
    "active_topics",
    "detected_projects",
    "selected_stacks",
    "developer_decisions",
    "validated_signals",
    "team_signals",
    "decision_windows",
    "sources",
    "source_preferences",
    "briefings",
    "feedback",
    "interactions",
    "accuracy_metrics",
    "topic_affinities",
    "anti_topics",
    "activity_patterns",
];

/// Safely query all rows from a table, returning them as a JSON array.
/// Returns an empty array if the table does not exist.
fn safe_query_all(
    conn: &rusqlite::Connection,
    table_name: &str,
    order_by: &str,
) -> Result<(JsonValue, u32)> {
    // SECURITY INVARIANT (2026-04-19, ref self-audit MED-2):
    // This function constructs SQL via format!() — which is acceptable ONLY
    // while BOTH `table_name` and `order_by` pass every check below. Every
    // caller of safe_query_all in this module passes a literal string for
    // both arguments; treat that as an invariant and do not relax.
    //
    // Invariants enforced:
    //   1. table_name ∈ ALLOWED_EXPORT_TABLES (static whitelist).
    //   2. order_by contains only [a-zA-Z0-9_], space, and comma.
    //   3. order_by tokens restricted to identifier + optional DESC/ASC;
    //      SQL keywords and tokens commonly used for injection are blocked
    //      by an explicit denylist on top of the character whitelist.
    //
    // Reviewer checklist: if you add a new caller, it MUST pass literal
    // strings. If a new caller needs user-supplied ordering, add an
    // enum+match mapping to a hardcoded ORDER BY clause — never thread
    // user input into this function.

    // Invariant 1: table whitelist.
    if !ALLOWED_EXPORT_TABLES.contains(&table_name) {
        return Err(crate::error::FourDaError::Validation(format!(
            "Table '{}' is not allowed for export",
            table_name
        )));
    }
    // Invariant 2: character-class whitelist for ORDER BY.
    if !order_by
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == ' ' || c == ',')
    {
        return Err(crate::error::FourDaError::Validation(
            "Invalid ORDER BY clause".to_string(),
        ));
    }
    // Invariant 3: defense-in-depth keyword denylist. Blocks e.g. UNION,
    // SELECT, INSERT, UPDATE, DELETE, DROP, EXEC, LIMIT, OFFSET, INTO, JOIN,
    // WHERE, HAVING, CASE, WHEN. These tokens have no legitimate reason
    // to appear in an ORDER BY clause for any of the allowed tables.
    const FORBIDDEN_ORDER_BY_TOKENS: &[&str] = &[
        "UNION", "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "EXEC", "EXECUTE", "LIMIT",
        "OFFSET", "INTO", "JOIN", "WHERE", "HAVING", "CASE", "WHEN", "ATTACH", "DETACH", "PRAGMA",
        "VACUUM", "CREATE", "ALTER", "TRUNCATE",
    ];
    let order_upper = order_by.to_uppercase();
    for tok in FORBIDDEN_ORDER_BY_TOKENS {
        // Word-boundary match: token surrounded by non-identifier chars or
        // at start/end of string. Prevents false positives on column names
        // like "updated_at" matching "UPDATE".
        let bytes = order_upper.as_bytes();
        let tok_bytes = tok.as_bytes();
        for i in 0..=bytes.len().saturating_sub(tok_bytes.len()) {
            if &bytes[i..i + tok_bytes.len()] == tok_bytes {
                let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
                let after_ok = i + tok_bytes.len() == bytes.len()
                    || !is_ident_byte(bytes[i + tok_bytes.len()]);
                if before_ok && after_ok {
                    return Err(crate::error::FourDaError::Validation(format!(
                        "ORDER BY contains forbidden token '{}'",
                        tok
                    )));
                }
            }
        }
    }

    if !table_exists(conn, table_name) {
        return Ok((JsonValue::Array(vec![]), 0));
    }

    let sql = format!("SELECT * FROM {table_name} ORDER BY {order_by} LIMIT 50000");
    let mut stmt = conn
        .prepare(&sql)
        .context("Failed to prepare export query")?;

    let column_names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    let rows: Vec<JsonValue> = stmt
        .query_map([], |row| {
            let mut obj = serde_json::Map::new();
            for (i, col) in column_names.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i)?;
                let json_val = match val {
                    rusqlite::types::Value::Null => JsonValue::Null,
                    rusqlite::types::Value::Integer(n) => JsonValue::Number(n.into()),
                    rusqlite::types::Value::Real(f) => {
                        serde_json::Number::from_f64(f).map_or(JsonValue::Null, JsonValue::Number)
                    }
                    rusqlite::types::Value::Text(s) => {
                        // Try parsing as JSON for nested structures
                        if (s.starts_with('{') || s.starts_with('['))
                            && serde_json::from_str::<JsonValue>(&s).is_ok()
                        {
                            serde_json::from_str(&s).unwrap_or(JsonValue::String(s))
                        } else {
                            JsonValue::String(s)
                        }
                    }
                    rusqlite::types::Value::Blob(b) => {
                        // Skip binary blobs (embeddings) — not useful in export
                        JsonValue::String(format!("[binary: {} bytes]", b.len()))
                    }
                };
                obj.insert(col.clone(), json_val);
            }
            Ok(JsonValue::Object(obj))
        })
        .context("Failed to query export data")?
        .filter_map(std::result::Result::ok)
        .collect();

    let count = rows.len() as u32;
    Ok((JsonValue::Array(rows), count))
}

// ============================================================================
// Section Exporters
// ============================================================================

/// Export user profile: sovereign profile facts, tech stack, interests, exclusions.
fn export_user_profile(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    let mut data = serde_json::Map::new();
    let mut total = 0u32;

    // Sovereign profile
    if table_exists(conn, "sovereign_profile") {
        let (rows, count) = safe_query_all(conn, "sovereign_profile", "category, key")?;
        data.insert("sovereign_profile".to_string(), rows);
        total += count;
    }

    // User context (tech_stack, interests, exclusions from settings)
    // These are stored in settings.json, not DB — export what we can from DB
    if table_exists(conn, "detected_tech") {
        let (rows, count) = safe_query_all(conn, "detected_tech", "confidence DESC")?;
        data.insert("detected_tech".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "active_topics") {
        let (rows, count) = safe_query_all(conn, "active_topics", "weight DESC")?;
        data.insert("active_topics".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "detected_projects") {
        let (rows, count) = safe_query_all(conn, "detected_projects", "last_activity DESC")?;
        data.insert("detected_projects".to_string(), rows);
        total += count;
    }

    // Selected stacks
    if table_exists(conn, "selected_stacks") {
        let (rows, count) = safe_query_all(conn, "selected_stacks", "created_at DESC")?;
        data.insert("selected_stacks".to_string(), rows);
        total += count;
    }

    Ok((JsonValue::Object(data), total))
}

/// Export decision journal.
fn export_decisions(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    safe_query_all(conn, "developer_decisions", "created_at DESC")
}

/// Export signal chains (computed live from source items).
fn export_signals(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    let mut data = serde_json::Map::new();
    let mut total = 0u32;

    // Validated signals
    if table_exists(conn, "validated_signals") {
        let (rows, count) = safe_query_all(conn, "validated_signals", "timestamp DESC")?;
        data.insert("validated_signals".to_string(), rows);
        total += count;
    }

    // Team signals (if team feature active)
    if table_exists(conn, "team_signals") {
        let (rows, count) = safe_query_all(conn, "team_signals", "first_detected DESC")?;
        data.insert("team_signals".to_string(), rows);
        total += count;
    }

    // Decision windows
    if table_exists(conn, "decision_windows") {
        let (rows, count) = safe_query_all(conn, "decision_windows", "opened_at DESC")?;
        data.insert("decision_windows".to_string(), rows);
        total += count;
    }

    Ok((JsonValue::Object(data), total))
}

/// Export source configuration — with all secrets stripped.
fn export_sources(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    let mut data = serde_json::Map::new();
    let mut total = 0u32;

    if table_exists(conn, "sources") {
        let (mut rows, count) = safe_query_all(conn, "sources", "source_type")?;
        strip_sensitive_fields(&mut rows);
        data.insert("sources".to_string(), rows);
        total += count;
    }

    // Source preferences (learned behavior, no secrets)
    if table_exists(conn, "source_preferences") {
        let (rows, count) = safe_query_all(conn, "source_preferences", "score DESC")?;
        data.insert("source_preferences".to_string(), rows);
        total += count;
    }

    Ok((JsonValue::Object(data), total))
}

/// Export briefing history.
fn export_briefings(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    safe_query_all(conn, "briefings", "created_at DESC")
}

/// Export user feedback / engagement data.
fn export_feedback(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    let mut data = serde_json::Map::new();
    let mut total = 0u32;

    if table_exists(conn, "feedback") {
        let (rows, count) = safe_query_all(conn, "feedback", "created_at DESC")?;
        data.insert("feedback".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "interactions") {
        let (rows, count) = safe_query_all(conn, "interactions", "timestamp DESC")?;
        data.insert("interactions".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "accuracy_metrics") {
        let (rows, count) = safe_query_all(conn, "accuracy_metrics", "metric_date DESC")?;
        data.insert("accuracy_metrics".to_string(), rows);
        total += count;
    }

    Ok((JsonValue::Object(data), total))
}

/// Export learned behavior: topic affinities and anti-topics.
fn export_learned_behavior(conn: &rusqlite::Connection) -> Result<(JsonValue, u32)> {
    let mut data = serde_json::Map::new();
    let mut total = 0u32;

    if table_exists(conn, "topic_affinities") {
        let (rows, count) = safe_query_all(conn, "topic_affinities", "affinity_score DESC")?;
        data.insert("topic_affinities".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "anti_topics") {
        let (rows, count) = safe_query_all(conn, "anti_topics", "confidence DESC")?;
        data.insert("anti_topics".to_string(), rows);
        total += count;
    }

    if table_exists(conn, "activity_patterns") {
        let (rows, count) = safe_query_all(conn, "activity_patterns", "pattern_type")?;
        data.insert("activity_patterns".to_string(), rows);
        total += count;
    }

    Ok((JsonValue::Object(data), total))
}

/// Route a section name to its exporter function.
fn export_section_data(conn: &rusqlite::Connection, section: &str) -> Result<(JsonValue, u32)> {
    match section {
        "user_profile" => export_user_profile(conn),
        "decisions" => export_decisions(conn),
        "signals" => export_signals(conn),
        "sources" => export_sources(conn),
        "briefings" => export_briefings(conn),
        "feedback" => export_feedback(conn),
        "learned_behavior" => export_learned_behavior(conn),
        _ => Err(format!("Unknown export section: {section}").into()),
    }
}

// ============================================================================
// Export Directory
// ============================================================================

/// Get the exports directory path, creating it if necessary.
fn get_exports_dir() -> Result<PathBuf> {
    let dir = crate::runtime_paths::RuntimePaths::get().exports_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Cannot create exports directory: {}", dir.display()))?;
    Ok(dir)
}

// ============================================================================
// CSV Conversion
// ============================================================================

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

/// Convert a JSON array of objects to CSV string.
fn json_array_to_csv(value: &JsonValue) -> String {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return String::new(),
    };

    if arr.is_empty() {
        return String::new();
    }

    // Collect all unique keys from all objects for the header
    let mut headers: Vec<String> = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    let mut csv = headers
        .iter()
        .map(|h| csv_escape(h))
        .collect::<Vec<_>>()
        .join(",");
    csv.push('\n');

    for item in arr {
        if let Some(obj) = item.as_object() {
            let row: Vec<String> = headers
                .iter()
                .map(|h| {
                    let val = obj.get(h).unwrap_or(&JsonValue::Null);
                    match val {
                        JsonValue::String(s) => csv_escape(s),
                        JsonValue::Null => String::new(),
                        other => csv_escape(&other.to_string()),
                    }
                })
                .collect();
            csv.push_str(&row.join(","));
            csv.push('\n');
        }
    }

    csv
}

/// Convert a nested export (Object with section keys containing arrays) to CSV.
fn nested_to_csv(value: &JsonValue) -> String {
    match value {
        JsonValue::Array(_) => json_array_to_csv(value),
        JsonValue::Object(map) => {
            let mut csv = String::new();
            for (section_name, section_data) in map {
                if let Some(arr) = section_data.as_array() {
                    if !arr.is_empty() {
                        csv.push_str(&format!("# Section: {section_name}\n"));
                        csv.push_str(&json_array_to_csv(section_data));
                        csv.push('\n');
                    }
                }
            }
            csv
        }
        _ => String::new(),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Export ALL user data sections to a file. Returns the export manifest.
///
/// The `format` parameter accepts "json" or "csv".
/// Exported files are written to `data/exports/`.
/// API keys, tokens, and secrets are NEVER included.
#[tauri::command]
pub async fn export_all_data(format: String) -> Result<ExportManifest> {
    let format = format.to_lowercase();
    if format != "json" && format != "csv" {
        return Err("Invalid format: must be 'json' or 'csv'".into());
    }

    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let export_id = uuid::Uuid::new_v4().to_string();
    let mut all_data = serde_json::Map::new();
    let mut total_records = 0u32;
    let sections: Vec<String> = ALL_SECTIONS
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    for section in ALL_SECTIONS {
        match export_section_data(&conn, section) {
            Ok((mut data, count)) => {
                strip_sensitive_fields(&mut data);
                all_data.insert(section.to_string(), data);
                total_records += count;
            }
            Err(e) => {
                warn!(target: "4da::data_export",
                    section = section,
                    error = %e,
                    "Failed to export section, skipping");
                all_data.insert(
                    section.to_string(),
                    serde_json::json!({"error": format!("Export failed: {e}")}),
                );
            }
        }
    }

    // Build the export document
    let export_doc = serde_json::json!({
        "export_id": export_id,
        "app": "4DA",
        "version": env!("CARGO_PKG_VERSION"),
        "format": format,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "sections": sections,
        "total_records": total_records,
        "data": all_data,
    });

    // Write to file
    let exports_dir = get_exports_dir()?;
    let extension = if format == "csv" { "csv" } else { "json" };
    let file_name = format!("export-{}.{}", &export_id[..8], extension);
    let file_path = exports_dir.join(&file_name);

    let file_content = if format == "csv" {
        // For CSV, convert each section
        // Start with UTF-8 BOM for Excel compatibility (prevents garbled CJK text)
        let mut csv = String::from("\u{FEFF}# 4DA Data Export\n");
        csv.push_str(&format!("# Export ID: {export_id}\n"));
        csv.push_str(&format!("# Created: {}\n", chrono::Utc::now().to_rfc3339()));
        csv.push_str(&format!("# Total Records: {total_records}\n\n"));
        for (section_name, section_data) in &all_data {
            csv.push_str(&format!("## {section_name}\n"));
            csv.push_str(&nested_to_csv(section_data));
            csv.push('\n');
        }
        csv
    } else {
        serde_json::to_string_pretty(&export_doc).context("Failed to serialize export data")?
    };

    std::fs::write(&file_path, &file_content)
        .with_context(|| format!("Failed to write export file: {}", file_path.display()))?;

    let created_at = chrono::Utc::now().to_rfc3339();

    // Record in tracking table
    let sections_json = serde_json::to_string(&sections).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO data_exports (id, format, sections, total_records, file_path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            export_id,
            format,
            sections_json,
            total_records,
            file_path.to_string_lossy().to_string(),
            created_at,
        ],
    )?;

    // Audit trail (non-blocking)
    crate::audit::log_team_audit(
        &conn,
        "data_export.create",
        "data_export",
        Some(&export_id),
        Some(&serde_json::json!({
            "format": format,
            "sections": sections,
            "total_records": total_records,
        })),
    );

    info!(target: "4da::data_export",
        export_id = %export_id,
        format = %format,
        total_records = total_records,
        file = %file_path.display(),
        "Data export completed");

    Ok(ExportManifest {
        export_id,
        created_at,
        format,
        sections,
        total_records,
    })
}

/// Export a single data section. Returns the data as a string (JSON or CSV).
#[tauri::command]
pub async fn export_section(section: String, format: String) -> Result<String> {
    let format = format.to_lowercase();
    if format != "json" && format != "csv" {
        return Err("Invalid format: must be 'json' or 'csv'".into());
    }

    if !ALL_SECTIONS.contains(&section.as_str()) {
        return Err(format!(
            "Unknown section '{}'. Valid sections: {}",
            section,
            ALL_SECTIONS.join(", ")
        )
        .into());
    }

    let conn = crate::state::open_db_connection()?;
    let (mut data, _count) = export_section_data(&conn, &section)?;
    strip_sensitive_fields(&mut data);

    let output = if format == "csv" {
        nested_to_csv(&data)
    } else {
        serde_json::to_string_pretty(&data).context("Failed to serialize section data")?
    };

    info!(target: "4da::data_export",
        section = %section,
        format = %format,
        "Section export completed");

    Ok(output)
}

/// List all previous exports from the data_exports tracking table.
#[tauri::command]
pub async fn list_exports() -> Result<Vec<ExportManifest>> {
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT id, format, sections, total_records, created_at
         FROM data_exports
         ORDER BY created_at DESC
         LIMIT 100",
    )?;

    let manifests = stmt
        .query_map([], |row| {
            let sections_str: String = row.get(2)?;
            let sections: Vec<String> = serde_json::from_str(&sections_str).unwrap_or_default();

            Ok(ExportManifest {
                export_id: row.get(0)?,
                format: row.get(1)?,
                sections,
                total_records: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .filter_map(|r| match r {
            Ok(m) => Some(m),
            Err(e) => {
                warn!(target: "4da::data_export", error = %e, "Failed to read export manifest row");
                None
            }
        })
        .collect();

    Ok(manifests)
}

/// Delete an export file and its tracking record.
#[tauri::command]
pub async fn delete_export(export_id: String) -> Result<()> {
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    // Get file path before deleting the record
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM data_exports WHERE id = ?1",
            params![export_id],
            |row| row.get(0),
        )
        .ok();

    // Delete the file if it exists
    if let Some(ref path) = file_path {
        let path = PathBuf::from(path);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to delete export file: {}", path.display()))?;
        }
    }

    // Delete the tracking record
    let deleted = conn.execute("DELETE FROM data_exports WHERE id = ?1", params![export_id])?;

    if deleted == 0 {
        return Err(format!("Export not found: {export_id}").into());
    }

    // Audit trail
    crate::audit::log_team_audit(
        &conn,
        "data_export.delete",
        "data_export",
        Some(&export_id),
        None,
    );

    info!(target: "4da::data_export",
        export_id = %export_id,
        "Export deleted");

    Ok(())
}

/// Complete data wipe — deletes ALL user data for GDPR compliance.
/// This is irreversible. Clears:
/// - All database tables (content, events, decisions, feedback, history, logs)
/// - All telemetry
/// - Cached calibrations
/// - Export files
/// Does NOT delete: settings.json, keychain entries (user manages those separately)
#[tauri::command]
pub async fn factory_reset() -> Result<()> {
    warn!(target: "4da::privacy", "Factory reset initiated — wiping all user data");

    let conn = crate::state::open_db_connection()?;

    // Get list of all user tables (exclude sqlite internals)
    let tables: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .context("Failed to list tables")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .context("Failed to query table names")?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    // Delete all rows from every table
    for table in &tables {
        // Skip virtual tables (sqlite-vec) — they need special handling
        if table.ends_with("_vec") || table.starts_with("vss_") {
            continue;
        }
        if let Err(e) = conn.execute(&format!("DELETE FROM \"{}\"", table), []) {
            warn!(target: "4da::privacy", table = %table, error = %e, "Failed to clear table during factory reset");
        }
    }

    // Clear calibration cache files, export files, and logs
    let paths = crate::runtime_paths::RuntimePaths::get();
    let data_dir = &paths.data_dir;

    for subdir in &["calibrations", "exports", "logs"] {
        let dir = data_dir.join(subdir);
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    // Also clear the cache directory
    if paths.cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&paths.cache_dir);
        let _ = std::fs::create_dir_all(&paths.cache_dir);
    }

    warn!(target: "4da::privacy", tables_cleared = tables.len(), "Factory reset complete — all user data wiped");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();

        // Create minimal tables for testing
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sovereign_profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                raw_output TEXT,
                source_command TEXT,
                source_lesson TEXT,
                confidence REAL DEFAULT 1.0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(category, key)
            );
            CREATE TABLE IF NOT EXISTS developer_decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                decision TEXT NOT NULL,
                rationale TEXT,
                alternatives_rejected TEXT DEFAULT '[]',
                context_tags TEXT DEFAULT '[]',
                confidence REAL NOT NULL DEFAULT 0.8,
                status TEXT NOT NULL DEFAULT 'active',
                superseded_by INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS topic_affinities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                embedding BLOB,
                positive_signals INTEGER DEFAULT 0,
                negative_signals INTEGER DEFAULT 0,
                total_exposures INTEGER DEFAULT 0,
                affinity_score REAL DEFAULT 0.0,
                confidence REAL DEFAULT 0.0,
                last_interaction TEXT DEFAULT (datetime('now')),
                decay_applied INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS anti_topics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                rejection_count INTEGER DEFAULT 0,
                confidence REAL DEFAULT 0.0,
                auto_detected INTEGER DEFAULT 1,
                user_confirmed INTEGER DEFAULT 0,
                first_rejection TEXT DEFAULT (datetime('now')),
                last_rejection TEXT DEFAULT (datetime('now')),
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config TEXT,
                last_fetch TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS briefings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                model TEXT,
                item_count INTEGER NOT NULL DEFAULT 0,
                tokens_used INTEGER,
                latency_ms INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_ensure_tables_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();
        ensure_tables(&conn).unwrap(); // Second call should not error
    }

    #[test]
    fn test_table_exists() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(!table_exists(&conn, "nonexistent_table"));
        conn.execute_batch("CREATE TABLE test_table (id INTEGER PRIMARY KEY)")
            .unwrap();
        assert!(table_exists(&conn, "test_table"));
    }

    #[test]
    fn test_strip_sensitive_fields() {
        let mut value = serde_json::json!({
            "name": "Test Source",
            "api_key": "sk-secret-123",
            "token": "tok-abc",
            "config": {
                "url": "https://example.com",
                "secret": "hidden",
                "password": "letmein"
            },
            "items": [
                {"id": 1, "accessToken": "tok-xyz"}
            ]
        });

        strip_sensitive_fields(&mut value);

        assert_eq!(value["name"], "Test Source");
        assert_eq!(value["api_key"], "[REDACTED]");
        assert_eq!(value["token"], "[REDACTED]");
        assert_eq!(value["config"]["url"], "https://example.com");
        assert_eq!(value["config"]["secret"], "[REDACTED]");
        assert_eq!(value["config"]["password"], "[REDACTED]");
        assert_eq!(value["items"][0]["id"], 1);
        assert_eq!(value["items"][0]["accessToken"], "[REDACTED]");
    }

    #[test]
    fn test_export_user_profile_empty_db() {
        let conn = setup_db();
        let (data, count) = export_user_profile(&conn).unwrap();
        assert!(data.is_object());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_export_user_profile_with_data() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO sovereign_profile (category, key, value) VALUES (?1, ?2, ?3)",
            params!["identity", "role", "backend engineer"],
        )
        .unwrap();

        let (data, count) = export_user_profile(&conn).unwrap();
        assert_eq!(count, 1);
        let profile = data["sovereign_profile"].as_array().unwrap();
        assert_eq!(profile.len(), 1);
    }

    #[test]
    fn test_export_decisions() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision) VALUES (?1, ?2, ?3)",
            params!["adoption", "Rust", "Adopt for backend"],
        )
        .unwrap();

        let (data, count) = export_decisions(&conn).unwrap();
        assert_eq!(count, 1);
        let arr = data.as_array().unwrap();
        assert_eq!(arr[0]["subject"], "Rust");
    }

    #[test]
    fn test_export_sources_strips_secrets() {
        let conn = setup_db();
        let config = serde_json::json!({
            "api_key": "secret-key-123",
            "url": "https://api.example.com"
        });
        conn.execute(
            "INSERT INTO sources (source_type, name, config) VALUES (?1, ?2, ?3)",
            params!["test_source", "Test", config.to_string()],
        )
        .unwrap();

        let (data, count) = export_sources(&conn).unwrap();
        assert_eq!(count, 1);
        // The source config JSON is stored as a string in the config column.
        // Our safe_query_all parses JSON strings, then strip_sensitive_fields redacts.
        // But export_sources calls strip_sensitive_fields on the whole result.
        let sources = &data["sources"];
        let source_arr = sources.as_array().unwrap();
        let config_val = &source_arr[0]["config"];
        if let Some(config_obj) = config_val.as_object() {
            assert_eq!(config_obj.get("api_key").unwrap(), "[REDACTED]");
            assert_eq!(config_obj.get("url").unwrap(), "https://api.example.com");
        }
    }

    #[test]
    fn test_export_feedback() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
            params![42, 1],
        )
        .unwrap();

        let (data, count) = export_feedback(&conn).unwrap();
        assert_eq!(count, 1);
        let feedback = data["feedback"].as_array().unwrap();
        assert_eq!(feedback[0]["source_item_id"], 42);
    }

    #[test]
    fn test_export_learned_behavior() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO topic_affinities (topic, affinity_score, confidence) VALUES (?1, ?2, ?3)",
            params!["rust", 0.9, 0.85],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO anti_topics (topic, rejection_count, confidence) VALUES (?1, ?2, ?3)",
            params!["crypto", 5, 0.7],
        )
        .unwrap();

        let (data, count) = export_learned_behavior(&conn).unwrap();
        assert_eq!(count, 2);
        let affinities = data["topic_affinities"].as_array().unwrap();
        assert_eq!(affinities[0]["topic"], "rust");
        let anti = data["anti_topics"].as_array().unwrap();
        assert_eq!(anti[0]["topic"], "crypto");
    }

    #[test]
    fn test_export_missing_table_graceful() {
        // Test against a bare database with no tables at all
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let (data, count) = export_decisions(&conn).unwrap();
        assert_eq!(count, 0);
        assert!(data.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_export_section_data_unknown() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = export_section_data(&conn, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_json_array_to_csv() {
        let data = serde_json::json!([
            {"name": "Alice", "score": 95},
            {"name": "Bob", "score": 87}
        ]);
        let csv = json_array_to_csv(&data);
        assert!(csv.contains("name,score"));
        assert!(csv.contains("Alice,95"));
        assert!(csv.contains("Bob,87"));
    }

    #[test]
    fn test_json_array_to_csv_empty() {
        let data = serde_json::json!([]);
        let csv = json_array_to_csv(&data);
        assert!(csv.is_empty());
    }

    #[test]
    fn test_csv_escape_special_chars() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_export_manifest_tracking() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();

        let id = "test-export-001";
        let sections = serde_json::to_string(&vec!["decisions", "feedback"]).unwrap();
        conn.execute(
            "INSERT INTO data_exports (id, format, sections, total_records, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, "json", sections, 42, "/tmp/test.json"],
        )
        .unwrap();

        let stored: (String, String, i64) = conn
            .query_row(
                "SELECT format, sections, total_records FROM data_exports WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(stored.0, "json");
        assert_eq!(stored.2, 42);
        let stored_sections: Vec<String> = serde_json::from_str(&stored.1).unwrap();
        assert_eq!(stored_sections, vec!["decisions", "feedback"]);
    }

    #[test]
    fn test_delete_tracking_record() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();

        conn.execute(
            "INSERT INTO data_exports (id, format, sections, total_records) VALUES (?1, ?2, ?3, ?4)",
            params!["del-001", "json", "[]", 0],
        )
        .unwrap();

        let deleted = conn
            .execute("DELETE FROM data_exports WHERE id = ?1", params!["del-001"])
            .unwrap();
        assert_eq!(deleted, 1);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM data_exports WHERE id = ?1",
                params!["del-001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
