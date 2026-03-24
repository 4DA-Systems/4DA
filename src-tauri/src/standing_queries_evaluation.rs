//! Standing Query Evaluation — monitoring cycle integration.
//!
//! Evaluates all active standing queries against newly ingested content
//! and produces alerts for queries with new matches.

use tracing::{info, warn};

use crate::standing_queries::{ensure_table, StandingQueryAlert};

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
        let since = if let Some(lr) = last_run {
            lr.clone()
        } else {
            let yesterday = chrono::Utc::now() - chrono::Duration::hours(24);
            yesterday.format("%Y-%m-%d %H:%M:%S").to_string()
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
             WHERE ({where_clause}) AND s.created_at >= ?1
             ORDER BY s.last_seen DESC
             LIMIT 100"
        );

        let new_count: i64;
        let example_title: Option<String>;

        match conn.prepare(&sql) {
            Ok(mut match_stmt) => {
                let match_rows: Vec<(i64, String)> = match match_stmt
                    .query_map(rusqlite::params![&since], |row| {
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
