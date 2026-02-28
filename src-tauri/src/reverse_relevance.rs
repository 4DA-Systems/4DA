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
        // Walk backward up to 50 bytes to find a safe char boundary
        let mut start = pos.saturating_sub(50);
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        // Walk forward up to 50 bytes past the match end
        let mut end = (pos + id_lower.len() + 50).min(text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        // Snap to word boundaries if possible
        if let Some(p) = text[..start].rfind(char::is_whitespace) {
            start = p + 1;
            // Ensure we're still on a char boundary after +1
            while start < text.len() && !text.is_char_boundary(start) {
                start += 1;
            }
        }
        if let Some(p) = text[end..].find(char::is_whitespace) {
            end += p;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_mention_serde_roundtrip() {
        let mention = ReverseMention {
            source_item_id: 42,
            title: "4DA mentioned on HN".to_string(),
            url: Some("https://news.ycombinator.com/item?id=123".to_string()),
            mentioned_project: "4da".to_string(),
            mention_context: "...discussing 4DA as a privacy-first...".to_string(),
            source_type: "hackernews".to_string(),
            discovered_at: "2026-03-01T10:00:00".to_string(),
        };
        let json = serde_json::to_string(&mention).unwrap();
        let deserialized: ReverseMention = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_item_id, 42);
        assert_eq!(deserialized.mentioned_project, "4da");
        assert!(deserialized.url.is_some());
    }

    #[test]
    fn reverse_mention_with_none_url() {
        let mention = ReverseMention {
            source_item_id: 1,
            title: "Test".to_string(),
            url: None,
            mentioned_project: "test-proj".to_string(),
            mention_context: "context".to_string(),
            source_type: "reddit".to_string(),
            discovered_at: "2026-03-01".to_string(),
        };
        let json = serde_json::to_string(&mention).unwrap();
        assert!(json.contains("null"));
        let deserialized: ReverseMention = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.url, None);
    }

    #[test]
    fn extract_project_name_from_unix_path() {
        assert_eq!(
            extract_project_name("/home/user/projects/my-app"),
            Some("my-app".to_string())
        );
    }

    #[test]
    fn extract_project_name_from_windows_path() {
        assert_eq!(
            extract_project_name("D:\\Projects\\4DA"),
            Some("4da".to_string())
        );
    }

    #[test]
    fn extract_project_name_root_path() {
        // Root paths might not have a file_name
        let result = extract_project_name("/");
        // Path::file_name for "/" returns None
        assert!(result.is_none());
    }

    #[test]
    fn is_too_generic_rejects_common_dirs() {
        assert!(is_too_generic("src"));
        assert!(is_too_generic("app"));
        assert!(is_too_generic("lib"));
        assert!(is_too_generic("test"));
        assert!(is_too_generic("tests"));
        assert!(is_too_generic("build"));
        assert!(is_too_generic("dist"));
        assert!(is_too_generic("node_modules"));
        assert!(is_too_generic("target"));
    }

    #[test]
    fn is_too_generic_accepts_real_project_names() {
        assert!(!is_too_generic("4da"));
        assert!(!is_too_generic("my-app"));
        assert!(!is_too_generic("tauri"));
        assert!(!is_too_generic("react"));
    }

    #[test]
    fn has_word_boundary_match_exact_match() {
        assert!(has_word_boundary_match("Using 4da for development", "4da"));
    }

    #[test]
    fn has_word_boundary_match_at_start() {
        assert!(has_word_boundary_match("4da is great", "4da"));
    }

    #[test]
    fn has_word_boundary_match_at_end() {
        assert!(has_word_boundary_match("I love 4da", "4da"));
    }

    #[test]
    fn has_word_boundary_match_rejects_substring() {
        // "react" inside "reactivity" should NOT match
        assert!(!has_word_boundary_match(
            "reactivity is the future",
            "react"
        ));
    }

    #[test]
    fn has_word_boundary_match_case_insensitive() {
        assert!(has_word_boundary_match("Using TAURI for desktop", "tauri"));
        assert!(has_word_boundary_match("using tauri for desktop", "TAURI"));
    }

    #[test]
    fn has_word_boundary_match_no_match() {
        assert!(!has_word_boundary_match("nothing relevant here", "4da"));
    }

    #[test]
    fn extract_mention_context_returns_surrounding_text() {
        let text = "Some discussion about how 4da handles privacy and local processing effectively for developers.";
        let ctx = extract_mention_context(text, "4da");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert!(ctx.contains("4da"));
        assert!(ctx.starts_with("..."));
        assert!(ctx.ends_with("..."));
    }

    #[test]
    fn extract_mention_context_not_found() {
        let text = "Nothing about that project here";
        let ctx = extract_mention_context(text, "4da");
        assert!(ctx.is_none());
    }

    #[test]
    fn extract_mention_context_at_start_of_text() {
        let text = "4da is a privacy-first developer tool with great features";
        let ctx = extract_mention_context(text, "4da");
        assert!(ctx.is_some());
        assert!(ctx.unwrap().contains("4da"));
    }
}
