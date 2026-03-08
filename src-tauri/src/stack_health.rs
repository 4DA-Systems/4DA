//! Stack Health Engine — ambient intelligence about the user's detected technology stack.
//!
//! Computes per-technology health signals by cross-referencing ACE-detected tech
//! with source_items and knowledge_decay gaps. Free-tier accessible (the hook).

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackHealth {
    pub technologies: Vec<TechHealthEntry>,
    pub stack_score: u32,
    pub signals_this_week: u32,
    pub suggested_queries: Vec<String>,
    pub missed_signals: MissedIntelligence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechHealthEntry {
    pub name: String,
    pub category: String,
    pub status: String,
    pub signal_count_7d: u32,
    pub days_since_engagement: u32,
    pub has_knowledge_gap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedIntelligence {
    pub total_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
    pub example_titles: Vec<String>,
}

// ============================================================================
// Core Logic
// ============================================================================

/// Compute health status for every detected technology in the user's stack.
pub fn compute_stack_health(conn: &rusqlite::Connection) -> StackHealth {
    // 1. Get detected technologies (confidence >= 0.5)
    let techs = load_detected_techs(conn);
    if techs.is_empty() {
        debug!(target: "4da::stack_health", "No detected technologies found");
        return StackHealth {
            technologies: vec![],
            stack_score: 100,
            signals_this_week: 0,
            suggested_queries: vec![],
            missed_signals: MissedIntelligence {
                total_count: 0,
                critical_count: 0,
                high_count: 0,
                example_titles: vec![],
            },
        };
    }

    // 2. Load knowledge gaps for cross-referencing
    let gaps = match crate::knowledge_decay::detect_knowledge_gaps(conn) {
        Ok(g) => g,
        Err(e) => {
            warn!(target: "4da::stack_health", error = %e, "Failed to load knowledge gaps, continuing without");
            vec![]
        }
    };

    // 3. Build per-tech health entries
    let mut entries: Vec<TechHealthEntry> = Vec::with_capacity(techs.len());
    let mut total_signals_7d: u32 = 0;
    let mut suggested_queries: Vec<String> = Vec::new();

    for (name, category) in &techs {
        let signal_count_7d = count_signals_for_tech(conn, name, 7);
        let signal_count_14d = count_signals_for_tech(conn, name, 14);
        let days_since = days_since_engagement_for_tech(conn, name);

        // Check if any knowledge gap references this tech
        let gap_for_tech = gaps
            .iter()
            .find(|g| g.dependency.to_lowercase() == name.to_lowercase());
        let has_knowledge_gap = gap_for_tech.is_some();

        // Determine status
        let status = if let Some(gap) = gap_for_tech {
            match gap.gap_severity {
                crate::knowledge_decay::GapSeverity::Critical
                | crate::knowledge_decay::GapSeverity::High => "critical",
                crate::knowledge_decay::GapSeverity::Medium => "attention",
                crate::knowledge_decay::GapSeverity::Low => {
                    if signal_count_7d == 0 {
                        "stale"
                    } else {
                        "healthy"
                    }
                }
            }
        } else if signal_count_14d == 0 {
            "attention"
        } else if signal_count_7d == 0 {
            "stale"
        } else {
            "healthy"
        };

        total_signals_7d += signal_count_7d;

        // Generate suggested queries for non-healthy tech
        if status != "healthy" {
            let query = match status {
                "critical" => format!("security updates for {}", name),
                "attention" => format!("latest {} news", name),
                _ => format!("{} updates this week", name),
            };
            suggested_queries.push(query);
        }

        entries.push(TechHealthEntry {
            name: name.clone(),
            category: category.clone(),
            status: status.to_string(),
            signal_count_7d,
            days_since_engagement: days_since,
            has_knowledge_gap,
        });
    }

    // 4. Stack score = percentage of healthy techs
    let healthy_count = entries.iter().filter(|e| e.status == "healthy").count();
    let stack_score = if entries.is_empty() {
        100
    } else {
        ((healthy_count as f64 / entries.len() as f64) * 100.0).round() as u32
    };

    // 5. Missed signals
    let missed_signals = compute_missed_intelligence(conn, &techs, 30);

    // Sort entries: critical first, then attention, stale, healthy
    entries.sort_by(|a, b| {
        status_rank(&a.status)
            .cmp(&status_rank(&b.status))
            .then(b.signal_count_7d.cmp(&a.signal_count_7d))
    });

    // Cap suggested queries
    suggested_queries.truncate(5);

    debug!(
        target: "4da::stack_health",
        tech_count = entries.len(),
        stack_score,
        signals_7d = total_signals_7d,
        "Stack health computed"
    );

    StackHealth {
        technologies: entries,
        stack_score,
        signals_this_week: total_signals_7d,
        suggested_queries,
        missed_signals,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Load detected technologies with confidence >= 0.5 from the ACE detected_tech table.
fn load_detected_techs(conn: &rusqlite::Connection) -> Vec<(String, String)> {
    let result = conn.prepare(
        "SELECT name, category FROM detected_tech WHERE confidence >= 0.5 ORDER BY confidence DESC",
    );

    let mut stmt = match result {
        Ok(s) => s,
        Err(e) => {
            // Table may not exist yet (ACE not initialized)
            debug!(target: "4da::stack_health", error = %e, "detected_tech table not available");
            return vec![];
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    });

    match rows {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            warn!(target: "4da::stack_health", error = %e, "Failed to query detected_tech");
            vec![]
        }
    }
}

/// Count source_items from the last N days whose title or content mentions the tech.
fn count_signals_for_tech(conn: &rusqlite::Connection, tech_name: &str, days: u32) -> u32 {
    let pattern = format!("%{}%", tech_name.to_lowercase());
    let interval = format!("-{} days", days);

    let result = conn.query_row(
        "SELECT COUNT(*) FROM source_items
         WHERE created_at >= datetime('now', ?1)
           AND (LOWER(title) LIKE ?2 OR LOWER(content) LIKE ?2)",
        params![interval, pattern],
        |row| row.get::<_, i64>(0),
    );

    match result {
        Ok(count) => count.max(0) as u32,
        Err(e) => {
            debug!(target: "4da::stack_health", tech = tech_name, error = %e, "Signal count query failed");
            0
        }
    }
}

/// Days since the user last engaged (via feedback) with content mentioning this tech.
fn days_since_engagement_for_tech(conn: &rusqlite::Connection, tech_name: &str) -> u32 {
    let pattern = format!("%{}%", tech_name.to_lowercase());

    let result: Result<Option<String>, _> = conn.query_row(
        "SELECT MAX(f.created_at)
         FROM feedback f
         JOIN source_items si ON si.id = f.source_item_id
         WHERE LOWER(si.title) LIKE ?1",
        params![pattern],
        |row| row.get(0),
    );

    match result {
        Ok(Some(date_str)) => {
            if let Ok(date) = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S")
            {
                let now = chrono::Utc::now().naive_utc();
                (now - date).num_days().max(0) as u32
            } else {
                999
            }
        }
        _ => 999, // No engagement ever
    }
}

/// Compute missed intelligence: source_items matching any detected tech from last N days,
/// categorized by challenge keywords.
fn compute_missed_intelligence(
    conn: &rusqlite::Connection,
    techs: &[(String, String)],
    days: u32,
) -> MissedIntelligence {
    if techs.is_empty() {
        return MissedIntelligence {
            total_count: 0,
            critical_count: 0,
            high_count: 0,
            example_titles: vec![],
        };
    }

    // Build OR conditions for all tech names
    let conditions: Vec<String> = techs
        .iter()
        .map(|(name, _)| {
            let escaped = name.replace('\'', "''");
            format!(
                "(LOWER(title) LIKE '%{}%' OR LOWER(content) LIKE '%{}%')",
                escaped.to_lowercase(),
                escaped.to_lowercase()
            )
        })
        .collect();

    let where_clause = conditions.join(" OR ");
    let interval = format!("-{} days", days);

    let query = format!(
        "SELECT id, title FROM source_items
         WHERE created_at >= datetime('now', ?1)
           AND ({})
         ORDER BY created_at DESC
         LIMIT 200",
        where_clause
    );

    let result = conn.prepare(&query);
    let mut stmt = match result {
        Ok(s) => s,
        Err(e) => {
            debug!(target: "4da::stack_health", error = %e, "Missed intelligence query failed");
            return MissedIntelligence {
                total_count: 0,
                critical_count: 0,
                high_count: 0,
                example_titles: vec![],
            };
        }
    };

    let rows: Vec<(i64, String)> = match stmt.query_map(params![interval], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            debug!(target: "4da::stack_health", error = %e, "Missed intelligence query_map failed");
            return MissedIntelligence {
                total_count: 0,
                critical_count: 0,
                high_count: 0,
                example_titles: vec![],
            };
        }
    };

    let total_count = rows.len() as u32;

    // Critical keywords: CVE, deprecated, vulnerability, breaking, security
    let critical_keywords = ["cve", "deprecated", "vulnerability", "breaking", "security"];
    // High keywords: update, release, migration
    let high_keywords = ["update", "release", "migration"];

    let mut critical_count: u32 = 0;
    let mut high_count: u32 = 0;

    for (_id, title) in &rows {
        let lower = title.to_lowercase();
        if critical_keywords.iter().any(|kw| lower.contains(kw)) {
            critical_count += 1;
        } else if high_keywords.iter().any(|kw| lower.contains(kw)) {
            high_count += 1;
        }
    }

    // Top 3 titles (always shown, the hook)
    let example_titles: Vec<String> = rows.iter().take(3).map(|(_, t)| t.clone()).collect();

    MissedIntelligence {
        total_count,
        critical_count,
        high_count,
        example_titles,
    }
}

/// Rank status for sorting (lower = more urgent).
fn status_rank(status: &str) -> u8 {
    match status {
        "critical" => 0,
        "attention" => 1,
        "stale" => 2,
        "healthy" => 3,
        _ => 4,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get full stack health report.
/// NO Pro gate — this is the hook that makes free users want Pro.
#[tauri::command]
pub async fn get_stack_health() -> Result<StackHealth, String> {
    let conn = crate::open_db_connection()?;
    Ok(compute_stack_health(&conn))
}

/// Get missed intelligence signals for detected stack.
/// NO Pro gate on counts. The example_titles field shows top 3 always (the teaser).
#[tauri::command]
pub async fn get_missed_intelligence(days: Option<u32>) -> Result<MissedIntelligence, String> {
    let conn = crate::open_db_connection()?;
    let techs = load_detected_techs(&conn);
    let effective_days = days.unwrap_or(30);
    Ok(compute_missed_intelligence(&conn, &techs, effective_days))
}
