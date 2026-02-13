//! Reverse Relevance for 4DA
//!
//! Detects when YOUR projects/packages are being discussed in sources.
//! Scans incoming items for mentions of project names, package names,
//! GitHub repos, and custom identifiers.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseMention {
    pub source_item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub mentioned_project: String,
    pub mention_context: String,
    pub source_type: String,
    pub discovered_at: String,
}

// ============================================================================
// Implementation
// ============================================================================

/// Extract "my identifiers" from ACE - project names, package names, etc.
pub fn get_my_identifiers() -> Result<Vec<String>, String> {
    let mut identifiers = Vec::new();

    // Get project names from ACE detected tech
    if let Ok(ace) = crate::get_ace_engine() {
        if let Ok(tech) = ace.get_detected_tech() {
            for dt in tech {
                if dt.name.len() >= 3 {
                    identifiers.push(dt.name.to_lowercase());
                }
            }
        }
    }

    // Get package names from project_dependencies table
    let conn = crate::open_db_connection()?;
    let deps = crate::temporal::get_all_dependencies(&conn)?;

    // Collect unique project-level package names (not individual deps)
    // We want names like "4da", "my-app" not "serde", "react"
    let mut seen = std::collections::HashSet::new();
    for dep in &deps {
        // Extract project name from manifest path
        if let Some(project_name) = extract_project_name(&dep.project_path) {
            if project_name.len() >= 3 && seen.insert(project_name.clone()) {
                identifiers.push(project_name);
            }
        }
    }

    // Deduplicate and filter too-short/too-generic names
    identifiers.sort();
    identifiers.dedup();
    identifiers.retain(|id| id.len() >= 3 && !is_too_generic(id));

    Ok(identifiers)
}

fn extract_project_name(project_path: &str) -> Option<String> {
    // Get the last directory component as the project name
    let path = std::path::Path::new(project_path);
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
}

fn is_too_generic(name: &str) -> bool {
    matches!(
        name,
        "src" | "app" | "lib" | "test" | "tests" | "build" | "dist" | "node_modules" | "target"
    )
}

/// Scan source items for mentions of user's projects
pub fn scan_for_mentions(
    conn: &rusqlite::Connection,
    since_hours: u32,
) -> Result<Vec<ReverseMention>, String> {
    let identifiers = get_my_identifiers()?;
    if identifiers.is_empty() {
        return Ok(vec![]);
    }

    let since = format!("-{} hours", since_hours);
    let mut mentions = Vec::new();

    for identifier in &identifiers {
        let pattern = format!("%{}%", identifier);

        let mut stmt = conn
            .prepare(
                "SELECT id, title, url, source_type, created_at, content
                 FROM source_items
                 WHERE (title LIKE ?1 OR content LIKE ?1)
                   AND created_at >= datetime('now', ?2)
                 ORDER BY created_at DESC
                 LIMIT 20",
            )
            .map_err(|e| e.to_string())?;

        let items: Vec<ReverseMention> = stmt
            .query_map(params![pattern, since], |row| {
                let content: String = row.get(5)?;
                let title: String = row.get(1)?;
                // Extract context around the mention
                let context = extract_mention_context(&content, identifier)
                    .or_else(|| extract_mention_context(&title, identifier))
                    .unwrap_or_else(|| title.clone());

                Ok(ReverseMention {
                    source_item_id: row.get(0)?,
                    title,
                    url: row.get(2)?,
                    mentioned_project: identifier.clone(),
                    mention_context: context,
                    source_type: row.get(3)?,
                    discovered_at: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        // Verify word-boundary match (not just substring)
        for item in items {
            if has_word_boundary_match(&item.title, identifier)
                || has_word_boundary_match(&item.mention_context, identifier)
            {
                mentions.push(item);
            }
        }
    }

    // Deduplicate by source_item_id
    mentions.sort_by_key(|m| m.source_item_id);
    mentions.dedup_by_key(|m| m.source_item_id);

    // Store relationships
    for mention in &mentions {
        let metadata = serde_json::json!({
            "mentioned_project": mention.mentioned_project,
            "mention_context": mention.mention_context
        });
        let _ = crate::temporal::upsert_relationship(
            conn,
            mention.source_item_id,
            0, // self-reference for project mentions
            "mentions_project",
            1.0,
            Some(&metadata),
        );
    }

    info!(target: "4da::reverse", mentions = mentions.len(), "Reverse relevance scan complete");
    Ok(mentions)
}

fn extract_mention_context(text: &str, identifier: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let id_lower = identifier.to_lowercase();

    if let Some(pos) = lower.find(&id_lower) {
        let start = pos.saturating_sub(50);
        let end = (pos + id_lower.len() + 50).min(text.len());
        // Find safe UTF-8 boundaries
        let start = text[..start]
            .rfind(char::is_whitespace)
            .map(|p| p + 1)
            .unwrap_or(start);
        let end = text[end..]
            .find(char::is_whitespace)
            .map(|p| end + p)
            .unwrap_or(end);
        Some(format!("...{}...", &text[start..end]))
    } else {
        None
    }
}

fn has_word_boundary_match(text: &str, identifier: &str) -> bool {
    let lower = text.to_lowercase();
    let id_lower = identifier.to_lowercase();

    if let Some(pos) = lower.find(&id_lower) {
        let before_ok = pos == 0 || !lower.as_bytes()[pos - 1].is_ascii_alphanumeric();
        let after_pos = pos + id_lower.len();
        let after_ok =
            after_pos >= lower.len() || !lower.as_bytes()[after_pos].is_ascii_alphanumeric();
        before_ok && after_ok
    } else {
        false
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_reverse_mentions(since_hours: Option<u32>) -> Result<Vec<ReverseMention>, String> {
    let conn = crate::open_db_connection()?;
    scan_for_mentions(&conn, since_hours.unwrap_or(72))
}

#[tauri::command]
pub fn get_my_project_identifiers() -> Result<Vec<String>, String> {
    get_my_identifiers()
}
