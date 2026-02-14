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

/// Build the user's tech domain from declared + detected tech.
/// Only dependencies matching this domain produce knowledge gaps.
fn build_tech_domain(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut domain = std::collections::HashSet::new();

    // Declared tech from onboarding (tech_stack.technology)
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                domain.insert(tech.to_lowercase());
            }
        }
    }

    // Auto-detected tech (Language, Framework, Database, Library — not Platform)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name FROM detected_tech WHERE category IN ('Language', 'Framework', 'Database', 'Library') AND confidence >= 0.5",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                domain.insert(tech.to_lowercase());
            }
        }
    }

    // Declared interests (explicit_interests.topic)
    if let Ok(mut stmt) = conn.prepare("SELECT topic FROM explicit_interests") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for topic in rows.flatten() {
                domain.insert(topic.to_lowercase());
            }
        }
    }

    domain
}

/// Load the user's primary stack from onboarding for competing tech filtering
fn load_primary_stack(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut stack = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                stack.insert(tech.to_lowercase());
            }
        }
    }
    stack
}

/// Get project paths the user has actively committed to in the last 30 days
fn get_active_project_paths(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut paths = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT DISTINCT repo_path FROM git_signals WHERE timestamp > datetime('now', '-30 days')",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for path in rows.flatten() {
                paths.insert(path);
            }
        }
    }
    paths
}

/// Check if a dependency name is relevant to the user's tech domain.
/// A dep is relevant if its name appears in the domain set, or if it's a real
/// package name (>= 4 chars, not a common English word).
fn is_dep_in_domain(dep_name: &str, domain: &std::collections::HashSet<String>) -> bool {
    let lower = dep_name.to_lowercase();

    // Direct match against domain
    if domain.contains(&lower) {
        return true;
    }

    // Check if the dep name is a common non-tech word that produces false positives.
    // These are real English words that appear as package names but match irrelevant articles.
    const GENERIC_WORDS: &[&str] = &[
        "space",
        "time",
        "image",
        "color",
        "event",
        "signal",
        "query",
        "table",
        "value",
        "error",
        "block",
        "chain",
        "field",
        "point",
        "path",
        "link",
        "node",
        "tree",
        "hash",
        "lock",
        "pool",
        "pipe",
        "ring",
        "slot",
        "core",
        "base",
        "data",
        "text",
        "font",
        "icon",
        "form",
        "grid",
        "card",
        "chip",
        "port",
        "test",
        "mock",
        "seed",
        "rand",
        "once",
        "sync",
        "glob",
        "term",
        "proc",
        "nano",
        "meta",
        "auto",
        "crypto",
        "audio",
        "video",
        "media",
        "style",
        "theme",
        "toast",
        "modal",
        "badge",
        "alert",
        "popup",
        // Common non-tech words that become package names
        "apple",
        "fashion",
        "dining",
        "sport",
        "music",
        "photo",
        "movie",
        "cosmos",
        "stellar",
        "orbit",
        "rocket",
        "matrix",
        "nova",
        "pulse",
        "amber",
        "coral",
        "ivory",
        "slate",
        "storm",
        // Words that are real package names but match too many unrelated articles
        "open",
        "next",
        "express",
        "run",
        "serve",
        "mini",
        "fast",
        "safe",
        "pure",
        "lite",
        "tiny",
        "super",
        "make",
        "copy",
        "move",
        "drop",
        "match",
        "type",
        "kind",
        "view",
        "page",
        "route",
        "state",
        "store",
        "model",
        "group",
        "just",
        "level",
        "simple",
        "clean",
        "fresh",
        "smart",
        "sharp",
        "craft",
        "prime",
        "solid",
        // Cross-ecosystem ambiguous names (exist in Rust, JS, C++, Python etc.)
        "async",
        "bytes",
        "config",
        "derive",
        "either",
        "futures",
        "http",
        "lazy",
        "mutex",
        "num",
        "regex",
        "string",
        "uuid",
        "chrono",
        "toml",
        "yaml",
        "build",
        "bench",
        "macro",
        "buffer",
        "stream",
        "channel",
        "runtime",
        "executor",
        "scheduler",
        "parallel",
        "pin",
    ];

    if GENERIC_WORDS.contains(&lower.as_str()) {
        return false;
    }

    // If domain is empty (no onboarding done), allow all deps (backward compat)
    if domain.is_empty() {
        return true;
    }

    // For deps not in domain and not obviously generic: check if any domain tech
    // is a substring match (e.g., dep "rusqlite" matches domain "rust" or "sqlite")
    domain
        .iter()
        .any(|tech| lower.contains(tech.as_str()) || tech.contains(lower.as_str()))
}

/// Normalize a title for deduplication: lowercase, strip punctuation, first 10 words
fn normalize_gap_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .take(10)
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Detect knowledge gaps across all tracked dependencies
pub fn detect_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>, String> {
    // Get all tracked dependencies
    let deps = crate::temporal::get_all_dependencies(conn)?;
    if deps.is_empty() {
        return Ok(vec![]);
    }

    // Build user's tech domain for filtering
    let domain = build_tech_domain(conn);

    // Load primary stack for competing tech filtering
    let primary_stack = load_primary_stack(conn);
    let anti_deps = crate::competing_tech::get_anti_dependencies(&primary_stack);

    // Get active project paths (committed to in last 30 days)
    let active_projects = get_active_project_paths(conn);

    // Deduplicate deps by package name (same dep across projects → one gap)
    let mut seen_deps: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for dep in &deps {
        seen_deps
            .entry(dep.package_name.clone())
            .or_default()
            .push(dep.project_path.clone());
    }

    info!(
        target: "4da::knowledge_decay",
        unique_deps = seen_deps.len(),
        total_deps = deps.len(),
        "Processing dependencies for knowledge gaps"
    );

    // Hard cap: only process first 50 unique deps to avoid scanning thousands.
    // The deps are already ordered by project_path so active projects come first.
    let mut processed_count: usize = 0;
    let mut gaps = Vec::new();

    for dep in &deps {
        // Skip if we already processed this dependency name
        let paths = match seen_deps.remove(&dep.package_name) {
            Some(p) => p,
            None => continue, // Already processed
        };

        processed_count += 1;
        if processed_count > 50 {
            break; // Hard cap: don't scan more than 50 unique deps
        }

        // Skip deps with very short names — too generic for LIKE matching
        if dep.package_name.len() < 5 {
            continue;
        }

        // Domain filter: only show gaps for deps relevant to user's tech stack
        if !is_dep_in_domain(&dep.package_name, &domain) {
            continue;
        }

        // Competing tech filter: skip deps that are competitors to user's chosen stack
        if anti_deps.contains(&dep.package_name.to_lowercase()) {
            continue;
        }

        // Active project scoping: skip deps from dormant projects
        if !active_projects.is_empty()
            && !active_projects
                .iter()
                .any(|ap| paths.iter().any(|dp| dp.contains(ap) || ap.contains(dp)))
        {
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

    // Cap at 10 gaps — quality over quantity
    gaps.truncate(10);
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

    // Deduplicate by normalized title (first 10 words, lowercased, stripped punctuation)
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    let items: Vec<MissedItem> = candidates
        .into_iter()
        .filter(|item| has_word_boundary_match(&item.title, &dep_lower))
        .filter(|item| {
            let normalized = normalize_gap_title(&item.title);
            seen_titles.insert(normalized) // true if this is NEW (not a duplicate)
        })
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_gap_title() {
        assert_eq!(
            normalize_gap_title("TypeScript 6.0 Beta: What's New!"),
            "typescript 60 beta whats new"
        );
        assert_eq!(
            normalize_gap_title("TypeScript 6.0 Beta — What's New?"),
            "typescript 60 beta whats new"
        );
    }

    #[test]
    fn test_normalize_deduplicates_similar_titles() {
        // These two titles differ only at word 11+, so first-10-words match
        let t1 =
            normalize_gap_title("TypeScript 6.0 Beta: What's New in the Big Release Update Today");
        let t2 = normalize_gap_title(
            "TypeScript 6.0 Beta: What's New in the Big Release Update Tomorrow",
        );
        assert_eq!(t1, t2);

        // Titles with different content should NOT match
        let t3 = normalize_gap_title("TypeScript 6.0 Beta: Performance Improvements");
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_generic_words_expanded() {
        let domain = std::collections::HashSet::new();
        // New additions should be filtered
        assert!(!is_dep_in_domain("open", &domain));
        assert!(!is_dep_in_domain("next", &domain));
        assert!(!is_dep_in_domain("express", &domain));
        assert!(!is_dep_in_domain("solid", &domain));
        assert!(!is_dep_in_domain("fresh", &domain));
        // Original generics still filtered
        assert!(!is_dep_in_domain("node", &domain));
        assert!(!is_dep_in_domain("space", &domain));
        // Cross-ecosystem ambiguous names should be filtered
        assert!(!is_dep_in_domain("futures", &domain));
        assert!(!is_dep_in_domain("async", &domain));
        assert!(!is_dep_in_domain("bytes", &domain));
        assert!(!is_dep_in_domain("config", &domain));
        assert!(!is_dep_in_domain("runtime", &domain));
    }

    #[test]
    fn test_domain_match_still_works() {
        let mut domain = std::collections::HashSet::new();
        domain.insert("tokio".to_string());
        domain.insert("serde".to_string());
        assert!(is_dep_in_domain("tokio", &domain));
        assert!(is_dep_in_domain("serde", &domain));
        // Substring match: rusqlite contains "sqlite" if sqlite is in domain
        domain.insert("sqlite".to_string());
        assert!(is_dep_in_domain("rusqlite", &domain));
    }

    #[test]
    fn test_word_boundary_match() {
        assert!(has_word_boundary_match("Next.js 15 Released", "next"));
        assert!(has_word_boundary_match("What's next for Rust", "next"));
        assert!(!has_word_boundary_match(
            "Unexpected behavior in Node",
            "next"
        ));
    }
}
