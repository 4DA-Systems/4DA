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

    // Deduplicate deps by package name (same dep across projects → one gap)
    let mut seen_deps: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for dep in &deps {
        seen_deps
            .entry(dep.package_name.clone())
            .or_default()
            .push(dep.project_path.clone());
    }

    let mut gaps = Vec::new();

    for dep in &deps {
        // Skip if we already processed this dependency name
        let paths = match seen_deps.remove(&dep.package_name) {
            Some(p) => p,
            None => continue, // Already processed
        };

        // Skip deps with very short names — too generic for LIKE matching
        if dep.package_name.len() < 4 {
            continue;
        }

        // Search source items for mentions of this dependency (title only)
        let missed = find_missed_items(conn, &dep.package_name)?;

        if missed.is_empty() {
            continue;
        }

        // Check if user has engaged with any items about this dep
        let days_since = days_since_last_engagement(conn, &dep.package_name)?;

        // Classify severity
        let severity = classify_severity(&missed, days_since, &dep.package_name);

        if severity == GapSeverity::Low && days_since < 14 {
            continue; // Skip low-severity recent items
        }

        // Merge project paths for display
        let project_display = if paths.len() == 1 {
            paths[0].clone()
        } else {
            format!("{} (+{} more)", paths[0], paths.len() - 1)
        };

        gaps.push(KnowledgeGap {
            dependency: dep.package_name.clone(),
            version: dep.version.clone(),
            project_path: project_display,
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
    // Title-only matching (content LIKE is too noisy for short dep names)
    let pattern = format!("%{}%", package_name);

    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.url, si.source_type, si.created_at
             FROM source_items si
             LEFT JOIN feedback f ON f.source_item_id = si.id
             WHERE si.title LIKE ?1
               AND si.created_at >= datetime('now', '-30 days')
               AND f.id IS NULL
             ORDER BY si.created_at DESC
             LIMIT 30",
        )
        .map_err(|e| e.to_string())?;

    let candidates: Vec<MissedItem> = stmt
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

    // Post-filter: verify word-boundary match in title to avoid false positives
    // e.g. "next" should match "Next.js" or "next release" but not "unexpected"
    let dep_lower = package_name.to_lowercase();
    let items: Vec<MissedItem> = candidates
        .into_iter()
        .filter(|item| has_word_boundary_match(&item.title, &dep_lower))
        .take(5)
        .collect();

    Ok(items)
}

/// Check if `text` contains `term` at a word boundary (not embedded in a larger word)
fn has_word_boundary_match(text: &str, term: &str) -> bool {
    let lower = text.to_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find(term) {
        let abs_pos = search_from + pos;
        let before_ok = abs_pos == 0 || !lower.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + term.len();
        let after_ok = after_pos >= lower.len()
            || !lower.as_bytes()[after_pos].is_ascii_alphanumeric()
            || lower[after_pos..].starts_with(".js")
            || lower[after_pos..].starts_with(".ts")
            || lower[after_pos..].starts_with(".rs");
        if before_ok && after_ok {
            return true;
        }
        search_from = abs_pos + 1;
    }
    false
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

fn classify_severity(missed: &[MissedItem], days_since: u32, dep_name: &str) -> GapSeverity {
    let dep_lower = dep_name.to_lowercase();

    // Check for security-related titles that specifically mention this dependency
    let has_security = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        let is_security = title_lower.contains("cve")
            || title_lower.contains("vulnerability")
            || title_lower.contains("security")
            || title_lower.contains("exploit");
        // Only count as security if the dep name is also prominent in the title
        is_security && title_lower.contains(&dep_lower)
    });

    // Check for breaking changes
    let has_breaking = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        (title_lower.contains("breaking")
            || title_lower.contains("deprecated")
            || title_lower.contains("eol")
            || title_lower.contains("end of life"))
            && title_lower.contains(&dep_lower)
    });

    if has_security {
        GapSeverity::Critical
    } else if has_breaking {
        GapSeverity::High
    } else if days_since > 30 && missed.len() >= 3 {
        GapSeverity::High
    } else if days_since > 14 || missed.len() >= 3 {
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
