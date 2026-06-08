// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

const TEST_SCHEMA: &str = "
    CREATE TABLE source_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT DEFAULT '',
        content TEXT DEFAULT '',
        created_at TEXT DEFAULT (datetime('now')),
        source_type TEXT DEFAULT 'test',
        content_type TEXT,
        cve_ids TEXT,
        signal_type TEXT
    );
    CREATE TABLE decision_windows (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        window_type TEXT NOT NULL,
        title TEXT NOT NULL,
        description TEXT DEFAULT '',
        urgency REAL DEFAULT 0.5,
        relevance REAL DEFAULT 0.5,
        source_item_ids TEXT DEFAULT '[]',
        signal_chain_id INTEGER,
        dependency TEXT,
        status TEXT DEFAULT 'open',
        opened_at TEXT DEFAULT (datetime('now')),
        expires_at TEXT,
        acted_at TEXT,
        closed_at TEXT,
        outcome TEXT,
        lead_time_hours REAL,
        streets_engine TEXT
    );
    CREATE TABLE preemption_wins (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        alert_id TEXT,
        alert_title TEXT,
        alerted_at TEXT,
        incident_at TEXT,
        lead_time_hours REAL,
        affected_deps TEXT,
        user_acted INTEGER DEFAULT 0,
        verified INTEGER DEFAULT 0,
        created_at TEXT DEFAULT (datetime('now'))
    );
";

fn db() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(TEST_SCHEMA).unwrap();
    c
}

/// Insert an open decision window. Returns its id.
fn insert_window(
    conn: &Connection,
    window_type: &str,
    dep: Option<&str>,
    opened_at: &str,
    source_item_ids: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO decision_windows (window_type, title, dependency, status, opened_at, source_item_ids) \
         VALUES (?1, ?2, ?3, 'open', ?4, ?5)",
        params![
            window_type,
            format!("{window_type} window"),
            dep,
            opened_at,
            source_item_ids
        ],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Insert a source_item. Returns its id.
fn insert_item(
    conn: &Connection,
    title: &str,
    content: &str,
    created_at: &str,
    content_type: Option<&str>,
    cve_ids: Option<&str>,
    signal_type: Option<&str>,
) -> i64 {
    conn.execute(
        "INSERT INTO source_items (title, content, created_at, content_type, cve_ids, signal_type) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![title, content, created_at, content_type, cve_ids, signal_type],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn window_status(conn: &Connection, id: i64) -> (String, Option<String>, Option<f32>) {
    conn.query_row(
        "SELECT status, outcome, lead_time_hours FROM decision_windows WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn win_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM preemption_wins", [], |r| r.get(0))
        .unwrap()
}

// 1. Grounded win.
#[test]
fn grounded_security_win() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_item(
        &conn,
        "Axios advisory: SSRF in axios",
        "axios is affected",
        "2026-06-01 15:00:00", // +5h
        Some("security_advisory"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 1);

    let (status, outcome, lead) = window_status(&conn, wid);
    assert_eq!(status, "closed");
    assert_eq!(outcome.as_deref(), Some("validated"));
    assert!(
        (lead.unwrap() - 5.0).abs() < 0.01,
        "lead ~= 5.0, got {lead:?}"
    );

    assert_eq!(win_count(&conn), 1);
    let (incident_at, verified, deps): (String, i64, String) = conn
        .query_row(
            "SELECT incident_at, verified, affected_deps FROM preemption_wins",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(incident_at, "2026-06-01 15:00:00");
    assert_eq!(verified, 1);
    assert_eq!(deps, "axios");
}

// 2. Ungrounded → no win (advisory is for react, window dep is axios).
#[test]
fn ungrounded_no_win() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_item(
        &conn,
        "React advisory: XSS in react",
        "react is affected",
        "2026-06-01 15:00:00",
        Some("security_advisory"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
    assert_eq!(win_count(&conn), 0);
}

// 3. Wrong event type → no win (release_notes for a security_patch window).
#[test]
fn wrong_event_type_no_win() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_item(
        &conn,
        "axios v2 release notes",
        "axios changelog",
        "2026-06-01 15:00:00",
        Some("release_notes"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
    assert_eq!(win_count(&conn), 0);
}

// 4. Incident-before-open guard → no win (no negative lead).
#[test]
fn incident_before_open_no_win() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    // created_at BEFORE opened_at — fails the `created_at > opened_at` SQL guard.
    insert_item(
        &conn,
        "axios advisory",
        "axios affected",
        "2026-06-01 09:00:00",
        Some("security_advisory"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
    assert_eq!(win_count(&conn), 0);
}

// 5. Earliest-event wins (two qualifying advisories: +3h and +8h → use +3h).
#[test]
fn earliest_event_used() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_item(
        &conn,
        "axios advisory (later)",
        "axios affected",
        "2026-06-01 18:00:00", // +8h
        Some("security_advisory"),
        None,
        None,
    );
    insert_item(
        &conn,
        "axios advisory (earlier)",
        "axios affected",
        "2026-06-01 13:00:00", // +3h
        None,
        Some("CVE-2026-0001"),
        None,
    );

    assert_eq!(validate_open_windows(&conn), 1);
    let (_, _, lead) = window_status(&conn, wid);
    assert!(
        (lead.unwrap() - 3.0).abs() < 0.01,
        "earliest lead ~= 3.0, got {lead:?}"
    );
}

// 6. Adoption deferred → no win even with a perfect match.
#[test]
fn adoption_deferred_no_win() {
    let conn = db();
    let wid = insert_window(&conn, "adoption", Some("bun"), "2026-06-01 10:00:00", "[]");
    insert_item(
        &conn,
        "bun advisory",
        "bun affected",
        "2026-06-01 15:00:00",
        Some("security_advisory"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
    assert_eq!(win_count(&conn), 0);
}

// 7. Dedup: closed window ignored; source_item_ids excluded as incident.
#[test]
fn closed_window_ignored() {
    let conn = db();
    // An already-closed window — must not be re-validated.
    conn.execute(
        "INSERT INTO decision_windows (window_type, title, dependency, status, opened_at) \
         VALUES ('security_patch', 'closed one', 'axios', 'closed', '2026-06-01 10:00:00')",
        [],
    )
    .unwrap();
    insert_item(
        &conn,
        "axios advisory",
        "axios affected",
        "2026-06-01 15:00:00",
        Some("security_advisory"),
        None,
        None,
    );

    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(win_count(&conn), 0);
}

#[test]
fn self_trigger_item_excluded() {
    let conn = db();
    // The item that opened the window (its id in source_item_ids) can't be the incident.
    let opener_id = insert_item(
        &conn,
        "axios advisory (opener)",
        "axios affected",
        "2026-06-01 15:00:00",
        Some("security_advisory"),
        None,
        None,
    );
    let wid = insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        &format!("[{opener_id}]"),
    );

    // Only the opener qualifies, and it's excluded → no win.
    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
    assert_eq!(win_count(&conn), 0);

    // A DIFFERENT later item DOES validate (proves exclusion is the only reason above).
    insert_item(
        &conn,
        "axios advisory (real incident)",
        "axios affected",
        "2026-06-01 16:00:00",
        Some("security_advisory"),
        None,
        None,
    );
    assert_eq!(validate_open_windows(&conn), 1);
    assert_eq!(win_count(&conn), 1);
}

// 8. Cold-start silent: no qualifying incidents → 0 wins, table stays empty.
#[test]
fn cold_start_silent() {
    let conn = db();
    insert_window(
        &conn,
        "security_patch",
        Some("axios"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_window(
        &conn,
        "migration",
        Some("react"),
        "2026-06-01 10:00:00",
        "[]",
    );
    // No source_items at all.
    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(win_count(&conn), 0);
}

// Extra: migration window validated by a breaking_change signal.
#[test]
fn migration_breaking_change_win() {
    let conn = db();
    let wid = insert_window(
        &conn,
        "migration",
        Some("react"),
        "2026-06-01 10:00:00",
        "[]",
    );
    insert_item(
        &conn,
        "React 20 ships breaking changes",
        "react migration required",
        "2026-06-01 16:00:00", // +6h
        None,
        None,
        Some("breaking_change"),
    );

    assert_eq!(validate_open_windows(&conn), 1);
    let (status, outcome, lead) = window_status(&conn, wid);
    assert_eq!(status, "closed");
    assert_eq!(outcome.as_deref(), Some("validated"));
    assert!((lead.unwrap() - 6.0).abs() < 0.01);
}

// Extra: window with no dependency is skipped (grounding required).
#[test]
fn no_dependency_skipped() {
    let conn = db();
    let wid = insert_window(&conn, "security_patch", None, "2026-06-01 10:00:00", "[]");
    insert_item(
        &conn,
        "Some advisory",
        "anything",
        "2026-06-01 15:00:00",
        Some("security_advisory"),
        None,
        None,
    );
    assert_eq!(validate_open_windows(&conn), 0);
    assert_eq!(window_status(&conn, wid).0, "open");
}
