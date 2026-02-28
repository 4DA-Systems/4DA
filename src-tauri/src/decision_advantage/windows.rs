//! Decision window detection and lifecycle management.
//!
//! Scans recent source_items for time-bounded opportunities, cross-references
//! with project_dependencies and digested_intelligence, then creates and manages
//! decision window records in the database.

use rusqlite::{params, Connection};
use tracing::{info, warn};

use super::DecisionWindow;

const SECURITY_KEYWORDS: &[&str] = &[
    "cve",
    "vulnerability",
    "security advisory",
    "exploit",
    "security patch",
    "security update",
    "security fix",
];
const MIGRATION_KEYWORDS: &[&str] = &[
    "breaking change",
    "deprecated",
    "deprecation",
    "migration guide",
    "major version",
    "end of life",
    "eol",
    "upgrade guide",
];
const ADOPTION_KEYWORDS: &[&str] = &[
    "launched",
    "released",
    "alternative to",
    "better than",
    "introducing",
    "announcing",
    "new release",
    "now available",
];

// ============================================================================
// Public API
// ============================================================================

/// Detect new decision windows from recent source items.
/// Scans last 48h, cross-references with project_dependencies and
/// digested_intelligence. Deduplicates against existing open windows.
pub(crate) fn detect_decision_windows(conn: &Connection) -> Vec<DecisionWindow> {
    let mut windows = Vec::new();
    detect_security_windows(conn, &mut windows);
    detect_migration_windows(conn, &mut windows);
    detect_adoption_windows(conn, &mut windows);
    detect_knowledge_windows(conn, &mut windows);
    deduplicate_and_store(conn, &mut windows);
    if !windows.is_empty() {
        info!(target: "4da::decision_advantage", count = windows.len(), "Decision windows detected");
    }
    windows
}

/// Get all open decision windows, ordered by urgency descending.
pub(crate) fn get_open_windows(conn: &Connection) -> Vec<DecisionWindow> {
    let mut stmt = match conn.prepare(
        "SELECT id, window_type, title, description, urgency, relevance,
                dependency, status, opened_at, expires_at, lead_time_hours, streets_engine
         FROM decision_windows WHERE status = 'open'
         ORDER BY urgency DESC, opened_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::decision_advantage", error = %e, "Query open windows failed");
            return Vec::new();
        }
    };
    stmt.query_map([], row_to_window)
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Get the decision journal: acted and closed windows, most recent first (up to 50).
pub(crate) fn get_decision_journal(conn: &Connection) -> Vec<DecisionWindow> {
    let mut stmt = match conn.prepare(
        "SELECT id, window_type, title, description, urgency, relevance,
                dependency, status, opened_at, expires_at, lead_time_hours, streets_engine
         FROM decision_windows WHERE status IN ('acted', 'closed')
         ORDER BY COALESCE(acted_at, closed_at) DESC
         LIMIT 50",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::decision_advantage", error = %e, "Query decision journal failed");
            return Vec::new();
        }
    };
    stmt.query_map([], row_to_window)
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}
/// Transition a window to a new status (acted, expired, closed).
/// Calculates lead_time_hours as elapsed time since opened_at.
pub(crate) fn transition_window(
    conn: &Connection,
    id: i64,
    status: &str,
    outcome: Option<&str>,
) -> Result<(), String> {
    if !matches!(status, "acted" | "expired" | "closed") {
        return Err(format!("Invalid window status: {status}"));
    }
    let lead_time_hours = conn
        .query_row(
            "SELECT opened_at FROM decision_windows WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|opened| {
            chrono::NaiveDateTime::parse_from_str(&opened, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| (chrono::Utc::now().naive_utc() - dt).num_minutes() as f32 / 60.0)
        });
    let time_col = if status == "acted" {
        "acted_at"
    } else {
        "closed_at"
    };
    let sql = format!(
        "UPDATE decision_windows SET status = ?1, {time_col} = datetime('now'), \
         outcome = ?2, lead_time_hours = ?3 WHERE id = ?4"
    );
    let affected = conn
        .execute(
            &sql,
            params![status, outcome.unwrap_or(""), lead_time_hours, id],
        )
        .map_err(|e| format!("Failed to transition window {id}: {e}"))?;
    if affected == 0 {
        return Err(format!("Window {id} not found"));
    }
    info!(target: "4da::decision_advantage", id, status, lead_time_hours = ?lead_time_hours, "Window transitioned");
    Ok(())
}

/// Expire windows past their expires_at. Returns count of expired.
pub(crate) fn expire_stale_windows(conn: &Connection) -> i64 {
    match conn.execute(
        "UPDATE decision_windows SET status = 'expired', closed_at = datetime('now')
         WHERE status = 'open' AND expires_at IS NOT NULL AND expires_at < datetime('now')",
        [],
    ) {
        Ok(c) => {
            if c > 0 {
                info!(target: "4da::decision_advantage", count = c, "Stale windows expired");
            }
            c as i64
        }
        Err(e) => {
            warn!(target: "4da::decision_advantage", error = %e, "Expire stale windows failed");
            0
        }
    }
}

// ============================================================================
// Detection helpers
// ============================================================================

fn get_user_dependencies(conn: &Connection) -> Vec<String> {
    let mut stmt =
        match conn.prepare("SELECT DISTINCT LOWER(package_name) FROM project_dependencies") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

fn query_items_with_keywords(
    conn: &Connection,
    keywords: &[&str],
) -> Vec<(i64, String, String, String)> {
    if keywords.is_empty() {
        return Vec::new();
    }
    let where_clause = keywords
        .iter()
        .map(|kw| format!("LOWER(si.title) LIKE '%{kw}%'"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let sql = format!(
        "SELECT si.id, si.title, COALESCE(si.content, ''), si.source_type
         FROM source_items si
         WHERE si.created_at > datetime('now', '-2 days') AND ({where_clause})
         ORDER BY si.created_at DESC LIMIT 100"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

fn find_matching_dep(title: &str, content: &str, deps: &[String]) -> Option<String> {
    let (t, c) = (title.to_lowercase(), content.to_lowercase());
    deps.iter()
        .find(|d| t.contains(d.as_str()) || c.contains(d.as_str()))
        .cloned()
}

fn streets_engine_for(wtype: &str) -> Option<String> {
    match wtype {
        "security_patch" => Some("Automation".into()),
        "migration" => Some("Consulting".into()),
        "adoption" => Some("Digital Products".into()),
        "knowledge" => Some("Education".into()),
        _ => None,
    }
}

fn make_window(
    wtype: &str,
    dep: Option<String>,
    title: &str,
    urgency: f32,
    relevance: f32,
    expires: Option<&str>,
) -> DecisionWindow {
    DecisionWindow {
        id: 0,
        window_type: wtype.into(),
        title: title.into(),
        description: truncate(title, 200),
        urgency,
        relevance,
        dependency: dep,
        status: "open".into(),
        opened_at: String::new(),
        expires_at: expires.map(|s| s.into()),
        lead_time_hours: None,
        streets_engine: streets_engine_for(wtype),
    }
}

fn detect_security_windows(conn: &Connection, windows: &mut Vec<DecisionWindow>) {
    let deps = get_user_dependencies(conn);
    for (_id, title, content, _) in query_items_with_keywords(conn, SECURITY_KEYWORDS) {
        if let Some(dep) = find_matching_dep(&title, &content, &deps) {
            windows.push(make_window(
                "security_patch",
                Some(dep.clone()),
                &format!("Security: {dep} \u{2014} {}", truncate(&title, 80)),
                0.90,
                0.85,
                Some("+7 days"),
            ));
        }
    }
}

fn detect_migration_windows(conn: &Connection, windows: &mut Vec<DecisionWindow>) {
    let deps = get_user_dependencies(conn);
    for (_id, title, content, _) in query_items_with_keywords(conn, MIGRATION_KEYWORDS) {
        if let Some(dep) = find_matching_dep(&title, &content, &deps) {
            windows.push(make_window(
                "migration",
                Some(dep.clone()),
                &format!("Migration: {dep} \u{2014} {}", truncate(&title, 80)),
                0.70,
                0.75,
                Some("+30 days"),
            ));
        }
    }
}

fn detect_adoption_windows(conn: &Connection, windows: &mut Vec<DecisionWindow>) {
    let deps = get_user_dependencies(conn);
    for (_id, title, content, _) in query_items_with_keywords(conn, ADOPTION_KEYWORDS) {
        let dep = find_matching_dep(&title, &content, &deps);
        let relevance = if dep.is_some() { 0.70 } else { 0.50 };
        windows.push(make_window(
            "adoption",
            dep,
            &format!("Adoption: {}", truncate(&title, 100)),
            0.50,
            relevance,
            Some("+14 days"),
        ));
    }
}

fn detect_knowledge_windows(conn: &Connection, windows: &mut Vec<DecisionWindow>) {
    let mut stmt = match conn.prepare(
        "SELECT id, subject, data, confidence FROM digested_intelligence
         WHERE digest_type = 'knowledge_gap' AND superseded_by IS NULL
         ORDER BY confidence DESC LIMIT 10",
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let gaps: Vec<(i64, String, String, f32)> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, String>(2).unwrap_or_default(),
                r.get::<_, f32>(3).unwrap_or(0.5),
            ))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    for (_, subject, _data, confidence) in &gaps {
        let urgency = (*confidence * 0.8).clamp(0.3, 0.80);
        windows.push(make_window(
            "knowledge",
            Some(subject.clone()),
            &format!("Knowledge gap: {}", truncate(subject, 100)),
            urgency,
            *confidence,
            None,
        ));
    }
}

fn deduplicate_and_store(conn: &Connection, windows: &mut Vec<DecisionWindow>) {
    let existing: Vec<(String, Option<String>)> = {
        let mut stmt = match conn
            .prepare("SELECT window_type, dependency FROM decision_windows WHERE status = 'open'")
        {
            Ok(s) => s,
            Err(_) => return,
        };
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };
    windows.retain(|w| {
        !existing
            .iter()
            .any(|(et, ed)| et == &w.window_type && ed.as_deref() == w.dependency.as_deref())
    });

    let sql = "INSERT INTO decision_windows (window_type, title, description, urgency, relevance, dependency, status, expires_at, streets_engine) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', CASE WHEN ?7 IS NOT NULL THEN datetime('now', ?7) ELSE NULL END, ?8)";
    for w in windows.iter_mut() {
        match conn.execute(
            sql,
            params![
                w.window_type,
                w.title,
                w.description,
                w.urgency,
                w.relevance,
                w.dependency,
                w.expires_at,
                w.streets_engine
            ],
        ) {
            Ok(_) => {
                w.id = conn.last_insert_rowid();
                w.opened_at = conn
                    .query_row(
                        "SELECT opened_at FROM decision_windows WHERE id = ?1",
                        params![w.id],
                        |r| r.get(0),
                    )
                    .unwrap_or_default();
            }
            Err(e) => {
                warn!(target: "4da::decision_advantage", error = %e, wtype = %w.window_type, "Insert window failed")
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

fn row_to_window(row: &rusqlite::Row<'_>) -> rusqlite::Result<DecisionWindow> {
    Ok(DecisionWindow {
        id: row.get(0)?,
        window_type: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        urgency: row.get(4)?,
        relevance: row.get(5)?,
        dependency: row.get(6)?,
        status: row.get(7)?,
        opened_at: row.get(8)?,
        expires_at: row.get(9)?,
        lead_time_hours: row.get(10)?,
        streets_engine: row.get(11)?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCHEMA: &str = "
        CREATE TABLE source_items (id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT DEFAULT 'test', source_id TEXT DEFAULT '', url TEXT, title TEXT DEFAULT '', content TEXT DEFAULT '', content_hash TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now')), last_seen TEXT DEFAULT (datetime('now')));
        CREATE TABLE project_dependencies (id INTEGER PRIMARY KEY AUTOINCREMENT, project_path TEXT NOT NULL, manifest_type TEXT DEFAULT 'package.json', package_name TEXT NOT NULL, version TEXT, is_dev INTEGER DEFAULT 0, language TEXT DEFAULT 'javascript', last_scanned TEXT DEFAULT (datetime('now')), UNIQUE(project_path, package_name));
        CREATE TABLE decision_windows (id INTEGER PRIMARY KEY AUTOINCREMENT, window_type TEXT NOT NULL, title TEXT NOT NULL, description TEXT DEFAULT '', urgency REAL DEFAULT 0.5, relevance REAL DEFAULT 0.5, source_item_ids TEXT DEFAULT '[]', signal_chain_id INTEGER, dependency TEXT, status TEXT DEFAULT 'open', opened_at TEXT DEFAULT (datetime('now')), expires_at TEXT, acted_at TEXT, closed_at TEXT, outcome TEXT, lead_time_hours REAL, streets_engine TEXT);
        CREATE TABLE digested_intelligence (id INTEGER PRIMARY KEY AUTOINCREMENT, digest_type TEXT NOT NULL, subject TEXT NOT NULL, data TEXT NOT NULL, confidence REAL DEFAULT 0.5, sample_size INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')), expires_at TEXT, superseded_by INTEGER);
    ";

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(TEST_SCHEMA).unwrap();
        c
    }

    #[test]
    fn test_detect_security_window_with_dep_match() {
        let conn = db();
        conn.execute("INSERT INTO project_dependencies (project_path, package_name, language) VALUES ('/app', 'lodash', 'js')", []).unwrap();
        conn.execute("INSERT INTO source_items (source_type, title, content) VALUES ('hn', 'CVE-2025-1234: vulnerability in lodash', 'lodash prototype pollution')", []).unwrap();
        let wins = detect_decision_windows(&conn);
        let sec = wins
            .iter()
            .find(|w| w.window_type == "security_patch")
            .expect("security window");
        assert_eq!(sec.dependency.as_deref(), Some("lodash"));
        assert!(sec.urgency >= 0.85);
        assert_eq!(sec.streets_engine.as_deref(), Some("Automation"));
        assert!(sec.id > 0);
    }

    #[test]
    fn test_deduplication_prevents_duplicates() {
        let conn = db();
        conn.execute("INSERT INTO project_dependencies (project_path, package_name, language) VALUES ('/app', 'react', 'js')", []).unwrap();
        conn.execute("INSERT INTO source_items (source_type, title, content) VALUES ('hn', 'Breaking change in React 20', 'react breaking changes')", []).unwrap();
        let first = detect_decision_windows(&conn);
        assert!(
            first.iter().any(|w| w.window_type == "migration"),
            "first round creates migration"
        );
        let second = detect_decision_windows(&conn);
        assert!(
            !second.iter().any(|w| w.window_type == "migration"),
            "second round deduplicates"
        );
    }

    #[test]
    fn test_transition_and_expire() {
        let conn = db();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, opened_at, expires_at) VALUES ('security_patch', 'Stale', 'open', datetime('now', '-1 day'), datetime('now', '-1 hour'))", []).unwrap();
        assert_eq!(expire_stale_windows(&conn), 1);
        conn.execute("INSERT INTO decision_windows (window_type, title, status, opened_at) VALUES ('migration', 'Active', 'open', datetime('now', '-2 hours'))", []).unwrap();
        let id = conn.last_insert_rowid();
        transition_window(&conn, id, "acted", Some("Done")).unwrap();
        let s: String = conn
            .query_row(
                "SELECT status FROM decision_windows WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(s, "acted");
        let lt: Option<f32> = conn
            .query_row(
                "SELECT lead_time_hours FROM decision_windows WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(lt.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn test_open_windows_sorted_by_urgency() {
        let conn = db();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, urgency) VALUES ('adoption', 'Low', 'open', 0.3)", []).unwrap();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, urgency) VALUES ('security_patch', 'High', 'open', 0.95)", []).unwrap();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, urgency) VALUES ('migration', 'Closed', 'closed', 0.99)", []).unwrap();
        let open = get_open_windows(&conn);
        assert_eq!(open.len(), 2);
        assert!(open[0].urgency >= open[1].urgency);
    }
}
