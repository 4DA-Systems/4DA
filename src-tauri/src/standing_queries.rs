//! Standing Queries — persistent intelligence monitoring.
//!
//! Signal-gated system that lets users define persistent search queries
//! evaluated on each monitoring cycle. New matches surface as alerts.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

// Re-export from sibling modules so existing `standing_queries::X` paths still work
pub use crate::standing_queries_evaluation::evaluate_standing_queries;
pub(crate) use crate::standing_queries_suggestions::generate_standing_query_suggestions;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingQuerySuggestion {
    pub topic: String,
    pub reason: String,
    pub engagement_count: u32,
    pub query_type: String, // "topic" or "dependency"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingQuery {
    pub id: i64,
    pub query_text: String,
    pub keywords: Vec<String>,
    pub created_at: String,
    pub last_run: Option<String>,
    pub total_matches: i64,
    pub new_matches: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingQueryAlert {
    pub query_id: i64,
    pub query_text: String,
    pub new_matches: i64,
    pub example_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingQueryMatch {
    pub item_id: i64,
    pub title: String,
    pub source_type: String,
    pub url: Option<String>,
    pub discovered_at: Option<String>,
}

// ============================================================================
// Stop words (matches natural_language_search.rs)
// ============================================================================

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "that", "this", "was", "are", "be", "has", "have", "had", "do", "does",
    "did", "will", "would", "could", "should", "may", "might", "can", "shall", "not", "no", "so",
    "if", "then", "than", "when", "where", "what", "which", "who", "how", "all", "each", "every",
    "any", "some", "such", "only", "own", "same", "other", "into", "about", "up", "out", "just",
    "also", "very", "my", "me", "i", "we", "you", "your", "our", "they", "them", "their", "show",
    "find", "get", "give", "tell", "list", "display",
];

// ============================================================================
// Keyword extraction (same logic as NL search)
// ============================================================================

fn extract_keywords(query: &str) -> Vec<String> {
    let stop_set: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() > 2 && !stop_set.contains(w))
        .map(std::string::ToString::to_string)
        .collect()
}

// ============================================================================
// DB Migration
// ============================================================================

pub fn ensure_table(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS standing_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            keywords TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now')),
            last_run TEXT,
            total_matches INTEGER DEFAULT 0,
            new_matches INTEGER DEFAULT 0,
            active INTEGER DEFAULT 1
        );",
    )
    .context("Failed to create standing_queries table")?;

    debug!(target: "4da::watches", "standing_queries table ensured");
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Create a new standing query from natural language text.
/// Extracts keywords and stores them as a JSON array.
/// Max 10 active queries.
#[tauri::command]
pub async fn create_standing_query(query_text: String) -> Result<i64> {
    crate::settings::require_signal_feature("standing_queries")?;

    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query text cannot be empty".into());
    }

    let keywords = extract_keywords(&query_text);
    if keywords.is_empty() {
        return Err("No meaningful keywords found in query".into());
    }

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    // Check active query count
    let active_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM standing_queries WHERE active = 1",
            [],
            |row| row.get(0),
        )
        .context("Failed to count active queries")?;

    if active_count >= 10 {
        return Err(
            "Maximum of 10 active standing queries reached. Delete one to add another.".into(),
        );
    }

    let keywords_json = serde_json::to_string(&keywords).context("Failed to serialize keywords")?;

    conn.execute(
        "INSERT INTO standing_queries (query_text, keywords) VALUES (?1, ?2)",
        rusqlite::params![query_text, keywords_json],
    )
    .context("Failed to insert standing query")?;

    let id = conn.last_insert_rowid();
    info!(target: "4da::watches", id = id, query = %query_text, keywords = ?keywords, "Standing query created");

    Ok(id)
}

/// List all active standing queries, most recent first.
#[tauri::command]
pub async fn list_standing_queries() -> Result<Vec<StandingQuery>> {
    crate::settings::require_signal_feature("standing_queries")?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, query_text, keywords, created_at, last_run, total_matches, new_matches, active
             FROM standing_queries
             WHERE active = 1
             ORDER BY created_at DESC",
        )
        .context("Failed to prepare query")?;

    let rows = stmt
        .query_map([], |row| {
            let keywords_json: String = row.get(2)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                keywords_json,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, bool>(7)?,
            ))
        })
        .context("Failed to query standing queries")?;

    let mut queries = Vec::new();
    for row in rows {
        match row {
            Ok((
                id,
                query_text,
                keywords_json,
                created_at,
                last_run,
                total_matches,
                new_matches,
                active,
            )) => {
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();
                queries.push(StandingQuery {
                    id,
                    query_text,
                    keywords,
                    created_at,
                    last_run,
                    total_matches,
                    new_matches,
                    active,
                });
            }
            Err(e) => {
                warn!(target: "4da::watches", error = %e, "Failed to parse standing query row");
            }
        }
    }

    debug!(target: "4da::watches", count = queries.len(), "Listed standing queries");
    Ok(queries)
}

/// Soft-delete a standing query by setting active = 0.
#[tauri::command]
pub async fn delete_standing_query(id: i64) -> Result<()> {
    crate::settings::require_signal_feature("standing_queries")?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    let affected = conn
        .execute(
            "UPDATE standing_queries SET active = 0 WHERE id = ?1",
            rusqlite::params![id],
        )
        .context("Failed to delete standing query")?;

    if affected == 0 {
        return Err(format!("Standing query with id {id} not found").into());
    }

    info!(target: "4da::watches", id = id, "Standing query deleted (soft)");
    Ok(())
}

/// Get source_items matching a standing query's keywords.
#[tauri::command]
pub async fn get_standing_query_matches(
    id: i64,
    limit: Option<usize>,
) -> Result<Vec<StandingQueryMatch>> {
    crate::settings::require_signal_feature("standing_queries")?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    let limit = limit.unwrap_or(20);

    // Load the query's keywords
    let keywords_json: String = conn
        .query_row(
            "SELECT keywords FROM standing_queries WHERE id = ?1 AND active = 1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .with_context(|| format!("Standing query {id} not found"))?;

    let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    // Build LIKE conditions (same pattern as NL search)
    let conditions: Vec<String> = keywords
        .iter()
        .map(|k| {
            format!(
                "(LOWER(s.title) LIKE '%{kw}%' OR LOWER(s.content) LIKE '%{kw}%')",
                kw = k.replace('\'', "''")
            )
        })
        .collect();

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT s.id, s.title, s.source_type, s.url, s.created_at
         FROM source_items s
         WHERE {where_clause}
         ORDER BY s.last_seen DESC
         LIMIT ?1"
    );

    let mut stmt = conn.prepare(&sql).context("Query error")?;

    let rows = stmt
        .query_map(rusqlite::params![limit], |row| {
            Ok(StandingQueryMatch {
                item_id: row.get(0)?,
                title: row.get(1)?,
                source_type: row.get(2)?,
                url: row.get(3)?,
                discovered_at: row.get(4)?,
            })
        })
        .context("Query error")?;

    let mut matches = Vec::new();
    for row in rows {
        match row {
            Ok(m) => matches.push(m),
            Err(e) => {
                warn!(target: "4da::watches", error = %e, "Failed to parse match row");
            }
        }
    }

    debug!(target: "4da::watches", id = id, matches = matches.len(), "Got standing query matches");
    Ok(matches)
}

/// Get standing query suggestions based on engagement patterns.
#[tauri::command]
pub async fn get_standing_query_suggestions() -> Result<Vec<StandingQuerySuggestion>> {
    crate::settings::require_signal_feature("standing_queries")?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    Ok(generate_standing_query_suggestions(&conn))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_keywords_filters_stop_words() {
        let keywords = extract_keywords("show me the latest rust updates");
        assert!(
            !keywords.contains(&"show".to_string()),
            "Stop word 'show' should be filtered"
        );
        assert!(
            !keywords.contains(&"the".to_string()),
            "Stop word 'the' should be filtered"
        );
        assert!(
            keywords.contains(&"latest".to_string()),
            "'latest' should be kept"
        );
        assert!(
            keywords.contains(&"rust".to_string()),
            "'rust' should be kept"
        );
        assert!(
            keywords.contains(&"updates".to_string()),
            "'updates' should be kept"
        );
    }

    #[test]
    fn extract_keywords_lowercases_input() {
        let keywords = extract_keywords("Rust React TypeScript");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"react".to_string()));
        assert!(keywords.contains(&"typescript".to_string()));
    }

    #[test]
    fn extract_keywords_drops_short_words() {
        let keywords = extract_keywords("go is ok but rust is better");
        // "go", "is", "ok" are all <= 2 chars, should be filtered
        assert!(
            !keywords.contains(&"go".to_string()),
            "2-char words should be filtered out"
        );
        assert!(
            !keywords.contains(&"is".to_string()),
            "2-char stop words should be filtered out"
        );
        assert!(
            keywords.contains(&"rust".to_string()),
            "'rust' (4 chars) should be kept"
        );
        assert!(
            keywords.contains(&"better".to_string()),
            "'better' should be kept"
        );
    }

    #[test]
    fn extract_keywords_preserves_hyphens_and_underscores() {
        let keywords = extract_keywords("vue-router and nest_js");
        assert!(
            keywords.contains(&"vue-router".to_string()),
            "Hyphenated terms should be kept intact"
        );
        assert!(
            keywords.contains(&"nest_js".to_string()),
            "Underscored terms should be kept intact"
        );
    }

    #[test]
    fn extract_keywords_empty_input() {
        let keywords = extract_keywords("");
        assert!(keywords.is_empty(), "Empty input should return no keywords");
    }
}
