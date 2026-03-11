// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for the enterprise audit log module.

use crate::audit::*;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create an in-memory SQLite database with the audit_log table.
fn setup_test_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(
        "CREATE TABLE audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            team_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            actor_display_name TEXT NOT NULL,
            action TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT,
            details TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX idx_audit_team_time ON audit_log(team_id, created_at DESC);
        CREATE INDEX idx_audit_actor ON audit_log(actor_id, created_at DESC);
        CREATE INDEX idx_audit_action ON audit_log(action);",
    )
    .expect("create audit_log table");
    conn
}

/// Insert a test audit entry with a specific created_at timestamp.
fn insert_test_entry(
    conn: &rusqlite::Connection,
    team_id: &str,
    actor_id: &str,
    action: &str,
    resource_type: &str,
    created_at: &str,
) {
    let event_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO audit_log (event_id, team_id, actor_id, actor_display_name, action, resource_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![event_id, team_id, actor_id, "Test User", action, resource_type, created_at],
    )
    .expect("insert test entry");
}

// ============================================================================
// log_audit Tests
// ============================================================================

#[test]
fn test_log_audit_writes_entry() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-1",
        "actor-1",
        "Alice",
        "settings.update",
        "settings",
        Some("settings-main"),
        Some(&serde_json::json!({"field": "llm_provider", "old": "none", "new": "anthropic"})),
    );

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .expect("count query");
    assert_eq!(count, 1);

    // Verify fields
    let (action, resource_type, resource_id, details_str): (String, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT action, resource_type, resource_id, details FROM audit_log WHERE team_id = 'team-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("select query");

    assert_eq!(action, "settings.update");
    assert_eq!(resource_type, "settings");
    assert_eq!(resource_id.as_deref(), Some("settings-main"));
    assert!(details_str.is_some());

    let details: serde_json::Value =
        serde_json::from_str(&details_str.unwrap()).expect("parse details");
    assert_eq!(details["field"], "llm_provider");
}

#[test]
fn test_log_audit_without_optional_fields() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-2",
        "actor-2",
        "Bob",
        "analysis.run",
        "analysis",
        None,
        None,
    );

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .expect("count query");
    assert_eq!(count, 1);

    let (resource_id, details): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT resource_id, details FROM audit_log WHERE team_id = 'team-2'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("select query");

    assert!(resource_id.is_none());
    assert!(details.is_none());
}

#[test]
fn test_log_audit_failure_does_not_panic() {
    // Use a connection to a DB without the audit_log table — INSERT will fail.
    let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");

    // This should not panic — it should silently log a warning.
    log_audit(
        &conn,
        "team-bad",
        "actor-bad",
        "Bad Actor",
        "test.fail",
        "test",
        None,
        None,
    );

    // If we get here, the test passes — no panic occurred.
}

#[test]
fn test_log_audit_generates_unique_event_ids() {
    let conn = setup_test_db();

    for _ in 0..10 {
        log_audit(
            &conn,
            "team-uniq",
            "actor-1",
            "Alice",
            "item.view",
            "item",
            None,
            None,
        );
    }

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT event_id) FROM audit_log WHERE team_id = 'team-uniq'",
            [],
            |row| row.get(0),
        )
        .expect("count distinct");
    assert_eq!(count, 10);
}

// ============================================================================
// get_audit_entries Tests
// ============================================================================

#[test]
fn test_get_audit_entries_no_filters() {
    let conn = setup_test_db();

    // Insert 3 entries
    for i in 0..3 {
        log_audit(
            &conn,
            "team-q",
            &format!("actor-{i}"),
            &format!("User {i}"),
            "item.view",
            "item",
            None,
            None,
        );
    }

    let entries = get_audit_entries(&conn, "team-q", &AuditFilter::default()).expect("get entries");
    assert_eq!(entries.len(), 3);

    // All entries should belong to team-q
    for entry in &entries {
        assert_eq!(entry.team_id, "team-q");
    }
}

#[test]
fn test_get_audit_entries_filter_by_action() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-f",
        "a1",
        "Alice",
        "settings.update",
        "settings",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-f",
        "a2",
        "Bob",
        "item.view",
        "item",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-f",
        "a1",
        "Alice",
        "settings.update",
        "settings",
        None,
        None,
    );

    let filters = AuditFilter {
        action: Some("settings.update".to_string()),
        ..AuditFilter::default()
    };

    let entries = get_audit_entries(&conn, "team-f", &filters).expect("get entries");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.action == "settings.update"));
}

#[test]
fn test_get_audit_entries_filter_by_actor() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-a",
        "alice",
        "Alice",
        "item.view",
        "item",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-a",
        "bob",
        "Bob",
        "item.view",
        "item",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-a",
        "alice",
        "Alice",
        "analysis.run",
        "analysis",
        None,
        None,
    );

    let filters = AuditFilter {
        actor_id: Some("alice".to_string()),
        ..AuditFilter::default()
    };

    let entries = get_audit_entries(&conn, "team-a", &filters).expect("get entries");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.actor_id == "alice"));
}

#[test]
fn test_get_audit_entries_filter_by_resource_type() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-r",
        "a1",
        "Alice",
        "settings.update",
        "settings",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-r",
        "a1",
        "Alice",
        "item.view",
        "item",
        None,
        None,
    );

    let filters = AuditFilter {
        resource_type: Some("item".to_string()),
        ..AuditFilter::default()
    };

    let entries = get_audit_entries(&conn, "team-r", &filters).expect("get entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].resource_type, "item");
}

#[test]
fn test_get_audit_entries_respects_limit_and_offset() {
    let conn = setup_test_db();

    for i in 0..10 {
        insert_test_entry(
            &conn,
            "team-p",
            "actor",
            "item.view",
            "item",
            &format!("2026-03-{:02}T12:00:00", i + 1),
        );
    }

    let filters = AuditFilter {
        limit: Some(3),
        offset: Some(0),
        ..AuditFilter::default()
    };

    let entries = get_audit_entries(&conn, "team-p", &filters).expect("get entries");
    assert_eq!(entries.len(), 3);

    // With offset
    let filters_offset = AuditFilter {
        limit: Some(3),
        offset: Some(3),
        ..AuditFilter::default()
    };
    let entries_offset = get_audit_entries(&conn, "team-p", &filters_offset).expect("get entries");
    assert_eq!(entries_offset.len(), 3);

    // Should not overlap
    let ids1: Vec<i64> = entries.iter().map(|e| e.id).collect();
    let ids2: Vec<i64> = entries_offset.iter().map(|e| e.id).collect();
    assert!(ids1.iter().all(|id| !ids2.contains(id)));
}

#[test]
fn test_get_audit_entries_wrong_team_returns_empty() {
    let conn = setup_test_db();

    log_audit(
        &conn,
        "team-x",
        "a1",
        "Alice",
        "item.view",
        "item",
        None,
        None,
    );

    let entries = get_audit_entries(&conn, "team-y", &AuditFilter::default()).expect("get entries");
    assert!(entries.is_empty());
}

// ============================================================================
// get_audit_summary Tests
// ============================================================================

#[test]
fn test_get_audit_summary_counts() {
    let conn = setup_test_db();

    // Insert entries with recent timestamps
    log_audit(
        &conn,
        "team-s",
        "alice",
        "Alice",
        "settings.update",
        "settings",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-s",
        "alice",
        "Alice",
        "settings.update",
        "settings",
        None,
        None,
    );
    log_audit(
        &conn,
        "team-s",
        "bob",
        "Bob",
        "item.view",
        "item",
        None,
        None,
    );

    let summary = get_audit_summary(&conn, "team-s", 30).expect("get summary");

    assert_eq!(summary.total_events, 3);
    assert!(!summary.events_by_action.is_empty());
    assert!(!summary.events_by_actor.is_empty());

    // settings.update should appear with count 2
    let settings_count = summary
        .events_by_action
        .iter()
        .find(|(a, _)| a == "settings.update")
        .map(|(_, c)| *c);
    assert_eq!(settings_count, Some(2));

    // item.view should appear with count 1
    let view_count = summary
        .events_by_action
        .iter()
        .find(|(a, _)| a == "item.view")
        .map(|(_, c)| *c);
    assert_eq!(view_count, Some(1));
}

#[test]
fn test_get_audit_summary_empty_team() {
    let conn = setup_test_db();

    let summary = get_audit_summary(&conn, "team-empty", 30).expect("get summary");

    assert_eq!(summary.total_events, 0);
    assert!(summary.events_by_action.is_empty());
    assert!(summary.events_by_actor.is_empty());
    assert!(summary.events_by_day.is_empty());
}

// ============================================================================
// export_audit_csv Tests
// ============================================================================

#[test]
fn test_export_audit_csv_format() {
    let conn = setup_test_db();

    // Insert with specific timestamps
    insert_test_entry(
        &conn,
        "team-csv",
        "alice",
        "settings.update",
        "settings",
        "2026-03-01 10:00:00",
    );
    insert_test_entry(
        &conn,
        "team-csv",
        "bob",
        "item.view",
        "item",
        "2026-03-02 14:00:00",
    );

    let csv = export_audit_csv(&conn, "team-csv", "2026-03-01", "2026-03-03").expect("export csv");

    let lines: Vec<&str> = csv.lines().collect();

    // Header + 2 data rows
    assert_eq!(lines.len(), 3, "Expected header + 2 data rows, got: {csv}");

    // Check header
    assert_eq!(
        lines[0],
        "event_id,team_id,actor_id,actor_display_name,action,resource_type,resource_id,details,created_at"
    );

    // Check data rows contain expected values
    assert!(lines[1].contains("team-csv"));
    assert!(lines[1].contains("alice"));
    assert!(lines[1].contains("settings.update"));
    assert!(lines[2].contains("bob"));
    assert!(lines[2].contains("item.view"));
}

#[test]
fn test_export_audit_csv_empty_range() {
    let conn = setup_test_db();

    insert_test_entry(
        &conn,
        "team-csv2",
        "alice",
        "test.action",
        "test",
        "2026-03-01 10:00:00",
    );

    // Query a range that excludes the entry
    let csv = export_audit_csv(&conn, "team-csv2", "2026-04-01", "2026-04-30").expect("export csv");

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 1, "Expected only header for empty range");
}

#[test]
fn test_export_audit_csv_escapes_commas() {
    let conn = setup_test_db();

    // Insert entry with details containing commas
    let event_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO audit_log (event_id, team_id, actor_id, actor_display_name, action, resource_type, details, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            event_id,
            "team-esc",
            "actor",
            "Alice, Admin",
            "test.action",
            "test",
            r#"{"key": "value, with comma"}"#,
            "2026-03-01 10:00:00",
        ],
    )
    .expect("insert");

    let csv = export_audit_csv(&conn, "team-esc", "2026-03-01", "2026-03-02").expect("export csv");

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2);

    // The display name with comma should be quoted
    assert!(lines[1].contains("\"Alice, Admin\""));
}

// ============================================================================
// cleanup_expired_audit Tests
// ============================================================================

#[test]
fn test_cleanup_expired_audit_removes_old_entries() {
    let conn = setup_test_db();

    // Insert old entries (> 90 days ago)
    insert_test_entry(
        &conn,
        "team-clean",
        "a1",
        "old.action",
        "test",
        "2025-01-01 10:00:00",
    );
    insert_test_entry(
        &conn,
        "team-clean",
        "a1",
        "old.action",
        "test",
        "2025-01-15 10:00:00",
    );

    // Insert recent entries
    insert_test_entry(
        &conn,
        "team-clean",
        "a1",
        "new.action",
        "test",
        "2026-03-10 10:00:00",
    );

    let before_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE team_id = 'team-clean'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(before_count, 3);

    let deleted = cleanup_expired_audit(&conn, "team-clean", 90).expect("cleanup");
    assert_eq!(deleted, 2, "Should have deleted the 2 old entries");

    let after_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE team_id = 'team-clean'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(after_count, 1);

    // Verify the remaining entry is the recent one
    let remaining_action: String = conn
        .query_row(
            "SELECT action FROM audit_log WHERE team_id = 'team-clean'",
            [],
            |row| row.get(0),
        )
        .expect("select");
    assert_eq!(remaining_action, "new.action");
}

#[test]
fn test_cleanup_expired_audit_different_team_unaffected() {
    let conn = setup_test_db();

    // Insert old entry for team-a
    insert_test_entry(
        &conn,
        "team-a",
        "a1",
        "old.action",
        "test",
        "2025-01-01 10:00:00",
    );
    // Insert old entry for team-b
    insert_test_entry(
        &conn,
        "team-b",
        "a1",
        "old.action",
        "test",
        "2025-01-01 10:00:00",
    );

    let deleted = cleanup_expired_audit(&conn, "team-a", 90).expect("cleanup");
    assert_eq!(deleted, 1);

    // team-b entry should still exist
    let team_b_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE team_id = 'team-b'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(team_b_count, 1);
}

#[test]
fn test_cleanup_expired_audit_zero_retention() {
    let conn = setup_test_db();

    // Even a very recent entry should be deleted with 0 retention days
    log_audit(
        &conn,
        "team-zero",
        "a1",
        "Alice",
        "test.action",
        "test",
        None,
        None,
    );

    let deleted = cleanup_expired_audit(&conn, "team-zero", 0).expect("cleanup");
    // With 0 days retention, datetime('now', '-0 days') = now, so nothing created
    // before now should match entries just inserted at datetime('now')
    // The entry has created_at = datetime('now') which is NOT < datetime('now', '-0 days')
    // so it should NOT be deleted.
    assert_eq!(deleted, 0);
}

// ============================================================================
// AuditFilter defaults
// ============================================================================

#[test]
fn test_audit_filter_default() {
    let filter = AuditFilter::default();
    assert!(filter.actor_id.is_none());
    assert!(filter.action.is_none());
    assert!(filter.resource_type.is_none());
    assert!(filter.from.is_none());
    assert!(filter.to.is_none());
    assert!(filter.limit.is_none());
    assert!(filter.offset.is_none());
}

// ============================================================================
// AuditEntry serialization
// ============================================================================

#[test]
fn test_audit_entry_serialization() {
    let entry = AuditEntry {
        id: 1,
        event_id: "evt-123".to_string(),
        team_id: "team-1".to_string(),
        actor_id: "actor-1".to_string(),
        actor_display_name: "Alice".to_string(),
        action: "settings.update".to_string(),
        resource_type: "settings".to_string(),
        resource_id: Some("main".to_string()),
        details: Some(serde_json::json!({"key": "value"})),
        created_at: "2026-03-11T12:00:00".to_string(),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let deserialized: AuditEntry = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.event_id, "evt-123");
    assert_eq!(deserialized.action, "settings.update");
    assert_eq!(deserialized.details.unwrap()["key"], "value");
}

#[test]
fn test_audit_summary_serialization() {
    let summary = AuditSummary {
        total_events: 42,
        events_by_action: vec![
            ("settings.update".to_string(), 20),
            ("item.view".to_string(), 22),
        ],
        events_by_actor: vec![("Alice".to_string(), 30), ("Bob".to_string(), 12)],
        events_by_day: vec![
            ("2026-03-10".to_string(), 25),
            ("2026-03-11".to_string(), 17),
        ],
    };

    let json = serde_json::to_string(&summary).expect("serialize");
    let deserialized: AuditSummary = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.total_events, 42);
    assert_eq!(deserialized.events_by_action.len(), 2);
}
