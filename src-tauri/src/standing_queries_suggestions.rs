// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Standing Query Suggestions — auto-generated query recommendations.
//!
//! Analyzes user engagement patterns (topic affinities, dependencies)
//! to suggest standing queries the user might want to create.

use std::collections::HashSet;
use tracing::debug;

use crate::standing_queries::StandingQuerySuggestion;

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
                        "You engaged with {positive_signals} articles about this topic"
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
                        format!("Used in {project_count} projects ({ecosystem})")
                    } else {
                        format!("Active dependency ({ecosystem})")
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standing_queries::ensure_table;

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
