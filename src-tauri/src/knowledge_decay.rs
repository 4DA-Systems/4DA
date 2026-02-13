//! Knowledge Decay Alerting for 4DA
//!
//! Cross-references project dependencies with source items to detect
//! knowledge gaps - things you should know about but haven't engaged with.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub dependency: String,
    pub version: Option<String>,
    pub project_path: String,
    pub missed_items: Vec<MissedItem>,
    pub gap_severity: GapSeverity,
    pub days_since_last_engagement: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedItem {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl GapSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            GapSeverity::Critical => "critical",
            GapSeverity::High => "high",
            GapSeverity::Medium => "medium",
            GapSeverity::Low => "low",
        }
    }
}

// ============================================================================
// Implementation
// ============================================================================

/// Detect knowledge gaps across all tracked dependencies
pub fn detect_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>, String> {
    // Get all tracked dependencies
    let deps = crate::temporal::get_all_dependencies(conn)?;
    if deps.is_empty() {
        return Ok(vec![]);
    }

    let mut gaps = Vec::new();

    for dep in &deps {
        // Search source items for mentions of this dependency
        let missed = find_missed_items(conn, &dep.package_name)?;

        if missed.is_empty() {
            continue;
        }

        // Check if user has engaged with any items about this dep
        let days_since = days_since_last_engagement(conn, &dep.package_name)?;

        // Classify severity
        let severity = classify_severity(&missed, days_since);

        if severity == GapSeverity::Low && days_since < 14 {
            continue; // Skip low-severity recent items
        }

        gaps.push(KnowledgeGap {
            dependency: dep.package_name.clone(),
            version: dep.version.clone(),
            project_path: dep.project_path.clone(),
            missed_items: missed,
            gap_severity: severity,
            days_since_last_engagement: days_since,
        });
    }

    // Sort by severity (critical first)
    gaps.sort_by(|a, b| {
        severity_rank(&a.gap_severity)
            .cmp(&severity_rank(&b.gap_severity))
            .then(
                b.days_since_last_engagement
                    .cmp(&a.days_since_last_engagement),
            )
    });

    gaps.truncate(20);
    info!(target: "4da::knowledge_decay", gaps = gaps.len(), "Knowledge gap detection complete");
    Ok(gaps)
}

fn find_missed_items(
    conn: &rusqlite::Connection,
    package_name: &str,
) -> Result<Vec<MissedItem>, String> {
    // Search source items where title or content mentions the package
    // Use word-boundary-like matching (check for package name as substring)
    let pattern = format!("%{}%", package_name);

    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.url, si.source_type, si.created_at
             FROM source_items si
             LEFT JOIN feedback f ON f.source_item_id = si.id
             WHERE (si.title LIKE ?1 OR si.content LIKE ?1)
               AND si.created_at >= datetime('now', '-30 days')
               AND f.id IS NULL
             ORDER BY si.created_at DESC
             LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let items: Vec<MissedItem> = stmt
        .query_map(params![pattern], |row| {
            Ok(MissedItem {
                item_id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                source_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

fn days_since_last_engagement(
    conn: &rusqlite::Connection,
    package_name: &str,
) -> Result<u32, String> {
    let pattern = format!("%{}%", package_name);

    let result: Option<String> = conn
        .query_row(
            "SELECT MAX(f.created_at)
             FROM feedback f
             JOIN source_items si ON si.id = f.source_item_id
             WHERE si.title LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    match result {
        Some(date_str) => {
            if let Ok(date) = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S")
            {
                let now = chrono::Utc::now().naive_utc();
                let days = (now - date).num_days().max(0) as u32;
                Ok(days)
            } else {
                Ok(999) // Can't parse date, treat as very old
            }
        }
        None => Ok(999), // No engagement ever
    }
}

fn classify_severity(missed: &[MissedItem], days_since: u32) -> GapSeverity {
    // Check for security-related titles
    let has_security = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        title_lower.contains("cve")
            || title_lower.contains("vulnerability")
            || title_lower.contains("security")
            || title_lower.contains("exploit")
    });

    // Check for breaking changes
    let has_breaking = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        title_lower.contains("breaking")
            || title_lower.contains("deprecated")
            || title_lower.contains("eol")
            || title_lower.contains("end of life")
    });

    if has_security {
        GapSeverity::Critical
    } else if has_breaking {
        GapSeverity::High
    } else if days_since > 30 && missed.len() >= 3 {
        GapSeverity::High
    } else if days_since > 14 {
        GapSeverity::Medium
    } else {
        GapSeverity::Low
    }
}

fn severity_rank(severity: &GapSeverity) -> u8 {
    match severity {
        GapSeverity::Critical => 0,
        GapSeverity::High => 1,
        GapSeverity::Medium => 2,
        GapSeverity::Low => 3,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_knowledge_gaps() -> Result<Vec<KnowledgeGap>, String> {
    let conn = crate::open_db_connection()?;
    detect_knowledge_gaps(&conn)
}

#[tauri::command]
pub fn get_knowledge_gap_count() -> Result<usize, String> {
    let conn = crate::open_db_connection()?;
    let gaps = detect_knowledge_gaps(&conn)?;
    let critical_count = gaps
        .iter()
        .filter(|g| g.gap_severity == GapSeverity::Critical || g.gap_severity == GapSeverity::High)
        .count();
    Ok(critical_count)
}
