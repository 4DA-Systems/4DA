use super::*;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    ensure_telemetry_table(&conn).unwrap();
    conn
}

#[test]
fn test_ensure_table_idempotent() {
    let conn = setup_test_db();
    // Calling twice should not error
    ensure_telemetry_table(&conn).unwrap();
}

#[test]
fn test_record_and_count_events() {
    let conn = setup_test_db();
    record_event(&conn, "app_launch", None, None, Some("s1")).unwrap();
    record_event(
        &conn,
        "view_open:results",
        Some("results"),
        None,
        Some("s1"),
    )
    .unwrap();
    record_event(
        &conn,
        "search_query",
        None,
        Some(r#"{"q":"rust"}"#),
        Some("s1"),
    )
    .unwrap();

    let report = get_usage_report(&conn, 1).unwrap();
    assert_eq!(report.total_events, 3);
    assert_eq!(report.sessions, 1);
    assert_eq!(report.search_count, 1);
}

#[test]
fn test_ghost_click_rate() {
    let conn = setup_test_db();
    for _ in 0..10 {
        record_event(&conn, "ghost_preview_shown", None, None, Some("s1")).unwrap();
    }
    for _ in 0..3 {
        record_event(&conn, "ghost_preview_clicked", None, None, Some("s1")).unwrap();
    }
    let report = get_usage_report(&conn, 1).unwrap();
    assert_eq!(report.ghost_preview_impressions, 10);
    assert_eq!(report.ghost_preview_clicks, 3);
    assert!((report.ghost_click_rate - 0.3).abs() < 0.001);
}

#[test]
fn test_empty_report() {
    let conn = setup_test_db();
    let report = get_usage_report(&conn, 7).unwrap();
    assert_eq!(report.total_events, 0);
    assert_eq!(report.sessions, 0);
    assert_eq!(report.ghost_click_rate, 0.0);
    assert_eq!(report.avg_session_views, 0.0);
    assert!(report.most_active_day.is_none());
}

// ========================================================================
// Error Telemetry Tests
// ========================================================================

#[test]
fn test_record_error_basic() {
    let conn = setup_test_db();
    record_error(
        &conn,
        "source_fetch",
        "Connection timeout",
        Some("hackernews"),
    )
    .unwrap();

    let errors = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].category, "source_fetch");
    assert_eq!(errors[0].message, "Connection timeout");
    assert_eq!(errors[0].context.as_deref(), Some("hackernews"));
    assert_eq!(errors[0].count, 1);
}

#[test]
fn test_record_error_upsert_increments_count() {
    let conn = setup_test_db();
    // Same category+message recorded three times
    record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();
    record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();
    record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();

    let errors = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(errors.len(), 1); // Single row via upsert
    assert_eq!(errors[0].count, 3); // Count incremented
}

#[test]
fn test_record_error_different_messages_separate_rows() {
    let conn = setup_test_db();
    record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
    record_error(&conn, "source_fetch", "DNS resolution failed", None).unwrap();
    record_error(&conn, "llm", "Connection timeout", None).unwrap();

    let errors = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(errors.len(), 3); // Three distinct category+message combos
}

#[test]
fn test_error_summary() {
    let conn = setup_test_db();
    record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
    record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
    record_error(&conn, "source_fetch", "DNS failure", None).unwrap();
    record_error(&conn, "llm", "API key invalid", None).unwrap();

    let summary = get_error_summary(&conn).unwrap();
    assert_eq!(summary.total_errors, 3); // 3 unique errors
    assert_eq!(summary.total_occurrences, 4); // 4 total occurrences
    assert_eq!(summary.by_category.len(), 2); // source_fetch and llm

    // source_fetch should be first (most occurrences)
    assert_eq!(summary.by_category[0].category, "source_fetch");
    assert_eq!(summary.by_category[0].unique_errors, 2);
    assert_eq!(summary.by_category[0].total_occurrences, 3);
}

#[test]
fn test_clear_old_errors() {
    let conn = setup_test_db();
    // Insert an error with old timestamp
    conn.execute(
        "INSERT INTO error_telemetry (category, message, count, first_seen, last_seen)
         VALUES ('old_cat', 'old error', 1, datetime('now', '-60 days'), datetime('now', '-60 days'))",
        [],
    )
    .unwrap();
    // Insert a fresh error
    record_error(&conn, "fresh_cat", "fresh error", None).unwrap();

    let deleted = clear_old_errors(&conn, 30).unwrap();
    assert_eq!(deleted, 1);

    let remaining = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].category, "fresh_cat");
}

#[test]
fn test_error_summary_empty() {
    let conn = setup_test_db();
    let summary = get_error_summary(&conn).unwrap();
    assert_eq!(summary.total_errors, 0);
    assert_eq!(summary.total_occurrences, 0);
    assert!(summary.by_category.is_empty());
    assert!(summary.top_errors.is_empty());
}

#[test]
fn test_error_message_truncation() {
    let conn = setup_test_db();
    let long_message = "x".repeat(2000);
    record_error(&conn, "test", &long_message, None).unwrap();

    let errors = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message.len(), 1000); // Truncated to 1000 chars
}

#[test]
fn test_error_telemetry_table_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_error_telemetry_table(&conn).unwrap();
    // Second call should not error
    ensure_error_telemetry_table(&conn).unwrap();
}

#[test]
fn test_recent_errors_respects_limit() {
    let conn = setup_test_db();
    for i in 0..20 {
        record_error(&conn, "test", &format!("error {}", i), None).unwrap();
    }
    let errors = get_recent_errors(&conn, 5).unwrap();
    assert_eq!(errors.len(), 5);
}

#[test]
fn test_upsert_updates_context() {
    let conn = setup_test_db();
    record_error(&conn, "source_fetch", "timeout", None).unwrap();
    record_error(&conn, "source_fetch", "timeout", Some("reddit")).unwrap();

    let errors = get_recent_errors(&conn, 10).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].count, 2);
    assert_eq!(errors[0].context.as_deref(), Some("reddit")); // Context updated
}
