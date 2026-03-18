//! Standing Queries — persistent intelligence monitoring.
//!
//! Pro-gated system that lets users define persistent search queries
//! evaluated on each monitoring cycle. New matches surface as alerts.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

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
        .map(|w| w.to_string())
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
    crate::settings::require_pro_feature("standing_queries")?;

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
    crate::settings::require_pro_feature("standing_queries")?;

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
    crate::settings::require_pro_feature("standing_queries")?;

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
    crate::settings::require_pro_feature("standing_queries")?;

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
         LIMIT {limit}"
    );

    let mut stmt = conn.prepare(&sql).context("Query error")?;

    let rows = stmt
        .query_map([], |row| {
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

// ============================================================================
// Suggestion Generation
// ============================================================================

/// Generate standing query suggestions based on user engagement patterns.
///
/// Finds topics the user has positively engaged with (from topic_affinities)
/// and dependencies they actively use (from user_dependencies) that don't
/// already have standing queries. Returns up to 5 suggestions.
pub(crate) fn generate_standing_query_suggestions(
    conn: &rusqlite::Connection,
) -> Vec<StandingQuerySuggestion> {
    // 1. Collect existing standing query keywords to avoid duplicates
    let existing_keywords = collect_existing_query_keywords(conn);

    let mut suggestions: Vec<StandingQuerySuggestion> = Vec::new();

    // 2. Check topic_affinities for topics with 3+ positive interactions
    if let Ok(mut stmt) = conn.prepare(
        "SELECT topic, positive_signals, affinity_score
         FROM topic_affinities
         WHERE positive_signals >= 3 AND affinity_score > 0.0
         ORDER BY positive_signals DESC, affinity_score DESC
         LIMIT 10",
    ) {
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, f64>(2)?,
            ))
        });

        if let Ok(rows) = rows {
            for row in rows.flatten() {
                let (topic, positive_signals, _score) = row;
                let topic_lower = topic.to_lowercase();

                // Skip if an existing standing query already covers this topic
                if existing_keywords
                    .iter()
                    .any(|kw| kw.contains(&topic_lower) || topic_lower.contains(kw.as_str()))
                {
                    continue;
                }

                suggestions.push(StandingQuerySuggestion {
                    topic: topic.clone(),
                    reason: format!(
                        "You engaged with {} articles about this topic",
                        positive_signals
                    ),
                    engagement_count: positive_signals,
                    query_type: "topic".to_string(),
                });
            }
        }
    }

    // 3. Check user_dependencies for actively-used direct dependencies
    //    that could benefit from version/security monitoring
    if suggestions.len() < 5 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT package_name, ecosystem, COUNT(*) as project_count
             FROM user_dependencies
             WHERE is_direct = 1
             GROUP BY package_name, ecosystem
             HAVING project_count >= 1
             ORDER BY project_count DESC
             LIMIT 10",
        ) {
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            });

            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    if suggestions.len() >= 5 {
                        break;
                    }
                    let (package_name, ecosystem, project_count) = row;
                    let pkg_lower = package_name.to_lowercase();

                    // Skip if an existing standing query already covers this dependency
                    if existing_keywords
                        .iter()
                        .any(|kw| kw.contains(&pkg_lower) || pkg_lower.contains(kw.as_str()))
                    {
                        continue;
                    }

                    // Skip if we already suggested a topic that covers this
                    if suggestions.iter().any(|s| {
                        s.topic.to_lowercase().contains(&pkg_lower)
                            || pkg_lower.contains(&s.topic.to_lowercase())
                    }) {
                        continue;
                    }

                    let reason = if project_count > 1 {
                        format!("Used in {} projects ({})", project_count, ecosystem)
                    } else {
                        format!("Active dependency ({})", ecosystem)
                    };

                    suggestions.push(StandingQuerySuggestion {
                        topic: package_name,
                        reason,
                        engagement_count: project_count,
                        query_type: "dependency".to_string(),
                    });
                }
            }
        }
    }

    // 4. Limit to 5 suggestions
    suggestions.truncate(5);

    debug!(
        target: "4da::watches",
        count = suggestions.len(),
        "Generated standing query suggestions"
    );

    suggestions
}

/// Collect lowercased keywords from all active standing queries.
fn collect_existing_query_keywords(conn: &rusqlite::Connection) -> HashSet<String> {
    let mut keywords = HashSet::new();

    let mut stmt =
        match conn.prepare("SELECT query_text, keywords FROM standing_queries WHERE active = 1") {
            Ok(s) => s,
            Err(_) => return keywords,
        };

    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return keywords,
    };

    for row in rows.flatten() {
        let (query_text, keywords_json) = row;
        // Add the full query text (lowercased)
        keywords.insert(query_text.to_lowercase());
        // Add individual keywords from JSON array
        if let Ok(kws) = serde_json::from_str::<Vec<String>>(&keywords_json) {
            for kw in kws {
                keywords.insert(kw.to_lowercase());
            }
        }
    }

    keywords
}

/// Get standing query suggestions based on engagement patterns.
#[tauri::command]
pub async fn get_standing_query_suggestions() -> Result<Vec<StandingQuerySuggestion>> {
    crate::settings::require_pro_feature("standing_queries")?;

    let conn = crate::open_db_connection()?;
    ensure_table(&conn)?;

    Ok(generate_standing_query_suggestions(&conn))
}

// ============================================================================
// Evaluation (called by monitoring cycle)
// ============================================================================

/// Evaluate all active standing queries against new content.
///
/// For each query, finds source_items created since `last_run` (or the last
/// 24 hours if never run) that match the query's keywords. Updates counters
/// and returns alerts for queries with new matches.
pub fn evaluate_standing_queries(conn: &rusqlite::Connection) -> Vec<StandingQueryAlert> {
    // Ensure table exists (no-op if already created)
    if let Err(e) = ensure_table(conn) {
        warn!(target: "4da::watches", error = %e, "Cannot evaluate standing queries — table creation failed");
        return Vec::new();
    }

    // Load all active queries
    let mut stmt = match conn
        .prepare("SELECT id, query_text, keywords, last_run FROM standing_queries WHERE active = 1")
    {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::watches", error = %e, "Failed to load standing queries for evaluation");
            return Vec::new();
        }
    };

    let queries: Vec<(i64, String, String, Option<String>)> = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    }) {
        Ok(rows) => rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in standing_queries: {e}");
                    None
                }
            })
            .collect(),
        Err(e) => {
            warn!(target: "4da::watches", error = %e, "Failed to iterate standing queries");
            return Vec::new();
        }
    };

    if queries.is_empty() {
        return Vec::new();
    }

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut alerts = Vec::new();

    for (id, query_text, keywords_json, last_run) in &queries {
        let keywords: Vec<String> = serde_json::from_str(keywords_json).unwrap_or_default();
        if keywords.is_empty() {
            continue;
        }

        // Determine the time boundary: last_run or 24 hours ago
        let since = match last_run {
            Some(lr) => lr.clone(),
            None => {
                let yesterday = chrono::Utc::now() - chrono::Duration::hours(24);
                yesterday.format("%Y-%m-%d %H:%M:%S").to_string()
            }
        };

        // Build LIKE conditions
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
            "SELECT s.id, s.title FROM source_items s
             WHERE ({where_clause}) AND s.created_at >= '{since}'
             ORDER BY s.last_seen DESC
             LIMIT 100"
        );

        let new_count: i64;
        let example_title: Option<String>;

        match conn.prepare(&sql) {
            Ok(mut match_stmt) => {
                let match_rows: Vec<(i64, String)> = match match_stmt.query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                }) {
                    Ok(rows) => rows
                        .filter_map(|r| match r {
                            Ok(v) => Some(v),
                            Err(e) => {
                                tracing::warn!("Row processing failed in standing_queries: {e}");
                                None
                            }
                        })
                        .collect(),
                    Err(e) => {
                        warn!(target: "4da::watches", error = %e, id = id, "Match query failed");
                        continue;
                    }
                };

                new_count = match_rows.len() as i64;
                example_title = match_rows.first().map(|(_, t)| t.clone());
            }
            Err(e) => {
                warn!(target: "4da::watches", error = %e, id = id, "Failed to prepare match query");
                continue;
            }
        }

        // Update last_run, total_matches, new_matches
        if let Err(e) = conn.execute(
            "UPDATE standing_queries SET last_run = ?1, total_matches = total_matches + ?2, new_matches = ?2 WHERE id = ?3",
            rusqlite::params![now, new_count, id],
        ) {
            warn!(target: "4da::watches", error = %e, id = id, "Failed to update standing query counters");
        }

        if new_count > 0 {
            info!(target: "4da::watches", id = id, new_count = new_count, query = %query_text, "Standing query matched new content");
            alerts.push(StandingQueryAlert {
                query_id: *id,
                query_text: query_text.clone(),
                new_matches: new_count,
                example_title,
            });
        }
    }

    if !alerts.is_empty() {
        info!(target: "4da::watches", total_alerts = alerts.len(), "Standing query evaluation complete");
    }

    alerts
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

    fn setup_suggestion_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Create required tables
        ensure_table(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS topic_affinities (
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
            CREATE TABLE IF NOT EXISTS user_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                ecosystem TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name, ecosystem)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn suggestions_empty_when_no_data() {
        let conn = setup_suggestion_db();
        let suggestions = generate_standing_query_suggestions(&conn);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn suggestions_from_topic_affinities() {
        let conn = setup_suggestion_db();
        conn.execute(
            "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
             VALUES ('WebAssembly', 5, 0.8)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
             VALUES ('Rust', 7, 0.9)",
            [],
        )
        .unwrap();
        // Below threshold (only 2 positive signals)
        conn.execute(
            "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
             VALUES ('Go', 2, 0.3)",
            [],
        )
        .unwrap();

        let suggestions = generate_standing_query_suggestions(&conn);
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].topic, "Rust"); // Higher positive_signals
        assert_eq!(suggestions[0].query_type, "topic");
        assert_eq!(suggestions[1].topic, "WebAssembly");
    }

    #[test]
    fn suggestions_skip_existing_queries() {
        let conn = setup_suggestion_db();
        conn.execute(
            "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
             VALUES ('Rust', 5, 0.8)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
             VALUES ('TypeScript', 4, 0.7)",
            [],
        )
        .unwrap();

        // Create an existing standing query for "Rust"
        conn.execute(
            "INSERT INTO standing_queries (query_text, keywords, active)
             VALUES ('Rust updates', '[\"rust\", \"updates\"]', 1)",
            [],
        )
        .unwrap();

        let suggestions = generate_standing_query_suggestions(&conn);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].topic, "TypeScript");
    }

    #[test]
    fn suggestions_include_dependencies() {
        let conn = setup_suggestion_db();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct)
             VALUES ('/project1', 'tokio', 'cargo', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct)
             VALUES ('/project2', 'tokio', 'cargo', 1)",
            [],
        )
        .unwrap();

        let suggestions = generate_standing_query_suggestions(&conn);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].topic, "tokio");
        assert_eq!(suggestions[0].query_type, "dependency");
        assert!(suggestions[0].reason.contains("2 projects"));
    }

    #[test]
    fn suggestions_capped_at_five() {
        let conn = setup_suggestion_db();
        for i in 0..10 {
            conn.execute(
                &format!(
                    "INSERT INTO topic_affinities (topic, positive_signals, affinity_score)
                     VALUES ('Topic{}', {}, 0.8)",
                    i,
                    10 - i
                ),
                [],
            )
            .unwrap();
        }

        let suggestions = generate_standing_query_suggestions(&conn);
        assert!(
            suggestions.len() <= 5,
            "Should cap at 5, got {}",
            suggestions.len()
        );
    }
}
