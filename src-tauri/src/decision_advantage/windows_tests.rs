use super::*;

const TEST_SCHEMA: &str = "
    CREATE TABLE source_items (id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT DEFAULT 'test', source_id TEXT DEFAULT '', url TEXT, title TEXT DEFAULT '', content TEXT DEFAULT '', content_hash TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now')), last_seen TEXT DEFAULT (datetime('now')));
    CREATE TABLE project_dependencies (id INTEGER PRIMARY KEY AUTOINCREMENT, project_path TEXT NOT NULL, manifest_type TEXT DEFAULT 'package.json', package_name TEXT NOT NULL, version TEXT, is_dev INTEGER DEFAULT 0, is_direct INTEGER DEFAULT 1, language TEXT DEFAULT 'javascript', last_scanned TEXT DEFAULT (datetime('now')), UNIQUE(project_path, package_name));
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

// -- truncate --

#[test]
fn truncate_shorter_than_max() {
    assert_eq!(truncate("hello", 10), "hello");
}

#[test]
fn truncate_exact_boundary() {
    assert_eq!(truncate("hello", 5), "hello");
}

#[test]
fn truncate_longer_adds_ellipsis() {
    assert_eq!(truncate("hello world", 5), "hello...");
}

#[test]
fn truncate_empty_string() {
    assert_eq!(truncate("", 10), "");
}

#[test]
fn truncate_multibyte_utf8() {
    // 4-byte emoji: should back up to char boundary
    let s = "hey \u{1F600} there"; // "hey 😀 there"
    let result = truncate(s, 5);
    assert!(result.ends_with("..."));
    assert!(result.len() <= 8); // 5 bytes + "..."
}

// -- find_matching_dep --

#[test]
fn find_dep_in_title() {
    let deps = vec!["lodash".to_string()];
    assert_eq!(
        find_matching_dep("lodash vulnerability found", "details here", &deps),
        Some("lodash".to_string())
    );
}

#[test]
fn find_dep_in_content() {
    let deps = vec!["express".to_string()];
    assert_eq!(
        find_matching_dep("Security alert", "express has a CVE", &deps),
        Some("express".to_string())
    );
}

#[test]
fn find_dep_no_match() {
    let deps = vec!["lodash".to_string()];
    assert_eq!(
        find_matching_dep("unrelated title", "unrelated content", &deps),
        None
    );
}

#[test]
fn find_dep_empty_deps() {
    assert_eq!(find_matching_dep("anything", "anything", &[]), None);
}

#[test]
fn find_dep_case_insensitive() {
    // title is lowered, so "lodash" should match "LODASH" in title
    let deps = vec!["lodash".to_string()];
    assert_eq!(
        find_matching_dep("LODASH update", "", &deps),
        Some("lodash".to_string())
    );
}

// -- streets_engine_for --

#[test]
fn streets_engine_all_known_types() {
    assert_eq!(
        streets_engine_for("security_patch"),
        Some("Automation".into())
    );
    assert_eq!(streets_engine_for("migration"), Some("Consulting".into()));
    assert_eq!(
        streets_engine_for("adoption"),
        Some("Digital Products".into())
    );
    assert_eq!(streets_engine_for("knowledge"), Some("Education".into()));
}

#[test]
fn streets_engine_unknown_returns_none() {
    assert_eq!(streets_engine_for("unknown"), None);
    assert_eq!(streets_engine_for(""), None);
}

// -- make_window --

#[test]
fn make_window_basic_fields() {
    let w = make_window(
        "adoption",
        Some("bun".into()),
        "Adoption: bun",
        0.5,
        0.7,
        None,
    );
    assert_eq!(w.window_type, "adoption");
    assert_eq!(w.title, "Adoption: bun");
    assert_eq!(w.dependency.as_deref(), Some("bun"));
    assert_eq!(w.status, "open");
    assert_eq!(w.streets_engine.as_deref(), Some("Digital Products"));
    assert!(w.expires_at.is_none());
}

#[test]
fn make_window_long_title_truncated_in_description() {
    let long_title = "A".repeat(300);
    let w = make_window("knowledge", None, &long_title, 0.5, 0.5, None);
    assert!(w.description.len() < 210); // 200 + "..."
    assert!(w.description.ends_with("..."));
}
