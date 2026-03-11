// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for the enterprise webhook system.

use super::*;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create an in-memory SQLite database with webhook tables.
fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    ensure_webhook_tables(&conn).expect("create webhook tables");
    conn
}

/// Register a test webhook with sensible defaults.
fn register_test_webhook(conn: &Connection, name: &str, events: &[&str]) -> Webhook {
    let events: Vec<String> = events.iter().map(|s| s.to_string()).collect();
    register_webhook(
        conn,
        "team-1",
        name,
        "https://example.com/hook",
        &events,
        Some("tester"),
    )
    .expect("register webhook")
}

// ============================================================================
// Registration + Listing
// ============================================================================

#[test]
fn register_and_list_webhooks() {
    let conn = setup_test_db();

    let wh1 = register_test_webhook(&conn, "CI Notifier", &["signal.created"]);
    let wh2 = register_test_webhook(&conn, "Slack Alerts", &["signal.*", "anomaly.*"]);

    assert!(!wh1.id.is_empty());
    assert_eq!(wh1.name, "CI Notifier");
    assert_eq!(wh1.team_id, "team-1");
    assert!(wh1.active);
    assert_eq!(wh1.failure_count, 0);
    assert_eq!(wh1.events, vec!["signal.created"]);

    let all = list_webhooks(&conn, "team-1").expect("list webhooks");
    assert_eq!(all.len(), 2);

    // Listing with a different team_id should return nothing
    let other = list_webhooks(&conn, "team-other").expect("list other team");
    assert!(other.is_empty());

    // Verify second webhook events
    let wh2_listed = all.iter().find(|w| w.id == wh2.id).expect("find wh2");
    assert_eq!(wh2_listed.events, vec!["signal.*", "anomaly.*"]);
}

// ============================================================================
// Deletion
// ============================================================================

#[test]
fn delete_webhook_removes_deliveries() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Deletable", &["*"]);

    // Create a delivery record manually
    create_delivery_record(&conn, &wh.id, "test.event", r#"{"test":true}"#)
        .expect("create delivery");

    let deliveries = get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries");
    assert_eq!(deliveries.len(), 1);

    // Delete the webhook — should cascade to deliveries
    delete_webhook(&conn, &wh.id).expect("delete webhook");

    let remaining = list_webhooks(&conn, "team-1").expect("list after delete");
    assert!(remaining.is_empty());

    // Verify deliveries are also gone
    let remaining_deliveries =
        get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries after delete");
    assert!(remaining_deliveries.is_empty());
}

#[test]
fn delete_nonexistent_webhook_fails() {
    let conn = setup_test_db();
    let result = delete_webhook(&conn, "does-not-exist");
    assert!(result.is_err());
}

// ============================================================================
// Active State
// ============================================================================

#[test]
fn set_webhook_active_toggles() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Toggleable", &["*"]);

    // Disable
    set_webhook_active(&conn, &wh.id, false).expect("disable");
    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    assert!(!webhooks[0].active);

    // Re-enable
    set_webhook_active(&conn, &wh.id, true).expect("enable");
    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    assert!(webhooks[0].active);
}

#[test]
fn set_active_nonexistent_webhook_fails() {
    let conn = setup_test_db();
    let result = set_webhook_active(&conn, "does-not-exist", true);
    assert!(result.is_err());
}

// ============================================================================
// Payload Signing
// ============================================================================

#[test]
fn sign_payload_produces_consistent_hex() {
    let sig1 = sign_payload("my-secret", r#"{"event":"test"}"#);
    let sig2 = sign_payload("my-secret", r#"{"event":"test"}"#);

    // Same input -> same output
    assert_eq!(sig1, sig2);
    // SHA256 hex is 64 chars
    assert_eq!(sig1.len(), 64);
    // All hex characters
    assert!(sig1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn sign_payload_differs_with_different_secret() {
    let sig1 = sign_payload("secret-a", "payload");
    let sig2 = sign_payload("secret-b", "payload");
    assert_ne!(sig1, sig2);
}

#[test]
fn sign_payload_differs_with_different_body() {
    let sig1 = sign_payload("same-secret", "body-a");
    let sig2 = sign_payload("same-secret", "body-b");
    assert_ne!(sig1, sig2);
}

// ============================================================================
// Circuit Breaker
// ============================================================================

#[test]
fn circuit_breaker_stays_closed_below_threshold() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Resilient", &["*"]);

    // Simulate 9 failures (just below threshold of 10)
    conn.execute(
        "UPDATE webhooks SET failure_count = 9 WHERE id = ?1",
        params![wh.id],
    )
    .expect("set failure count");

    let tripped = check_circuit_breaker(&conn, &wh.id).expect("check breaker");
    assert!(!tripped, "Should NOT trip at 9 failures");

    // Verify still active
    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    assert!(webhooks[0].active, "Should remain active");
}

#[test]
fn circuit_breaker_trips_at_threshold() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Fragile", &["*"]);

    // Simulate exactly CIRCUIT_BREAKER_THRESHOLD failures
    conn.execute(
        "UPDATE webhooks SET failure_count = ?1 WHERE id = ?2",
        params![CIRCUIT_BREAKER_THRESHOLD, wh.id],
    )
    .expect("set failure count");

    let tripped = check_circuit_breaker(&conn, &wh.id).expect("check breaker");
    assert!(
        tripped,
        "Should trip at {} failures",
        CIRCUIT_BREAKER_THRESHOLD
    );

    // Verify webhook is now disabled
    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    let found = webhooks
        .iter()
        .find(|w| w.id == wh.id)
        .expect("find webhook");
    assert!(!found.active, "Should be auto-disabled by circuit breaker");
}

#[test]
fn circuit_breaker_trips_above_threshold() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Very Fragile", &["*"]);

    conn.execute(
        "UPDATE webhooks SET failure_count = 15 WHERE id = ?1",
        params![wh.id],
    )
    .expect("set failure count");

    let tripped = check_circuit_breaker(&conn, &wh.id).expect("check breaker");
    assert!(tripped, "Should trip above threshold");
}

#[test]
fn circuit_breaker_reset_clears_failures_and_re_enables() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Recoverable", &["*"]);

    // Trip the breaker
    conn.execute(
        "UPDATE webhooks SET failure_count = 10, active = 0 WHERE id = ?1",
        params![wh.id],
    )
    .expect("trip breaker");

    // Reset it
    reset_circuit_breaker(&conn, &wh.id).expect("reset breaker");

    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    let found = webhooks
        .iter()
        .find(|w| w.id == wh.id)
        .expect("find webhook");
    assert!(found.active, "Should be re-enabled after reset");
    assert_eq!(found.failure_count, 0, "Failure count should be zero");
}

#[test]
fn circuit_breaker_reset_nonexistent_fails() {
    let conn = setup_test_db();
    let result = reset_circuit_breaker(&conn, "does-not-exist");
    assert!(result.is_err());
}

// ============================================================================
// Delivery Records
// ============================================================================

#[test]
fn delivery_record_creation() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Delivery Test", &["test.*"]);

    let payload = r#"{"event":"test.created","data":{}}"#;
    let delivery_id =
        create_delivery_record(&conn, &wh.id, "test.created", payload).expect("create delivery");

    assert!(!delivery_id.is_empty());

    let deliveries = get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries");
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].id, delivery_id);
    assert_eq!(deliveries[0].webhook_id, wh.id);
    assert_eq!(deliveries[0].event_type, "test.created");
    assert_eq!(deliveries[0].status, "pending");
    assert_eq!(deliveries[0].attempt_count, 0);
    assert!(deliveries[0].delivered_at.is_none());
}

#[test]
fn delivery_status_transitions() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Status Test", &["*"]);

    let delivery_id =
        create_delivery_record(&conn, &wh.id, "test.event", "{}").expect("create delivery");

    // Mark failed
    mark_failed(&conn, &delivery_id, 1, Some(500)).expect("mark failed");
    let deliveries = get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries");
    assert_eq!(deliveries[0].status, "failed");
    assert_eq!(deliveries[0].attempt_count, 1);
    assert_eq!(deliveries[0].http_status, Some(500));

    // Mark delivered
    mark_delivered(&conn, &delivery_id).expect("mark delivered");
    let deliveries = get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries");
    assert_eq!(deliveries[0].status, "delivered");
    assert!(deliveries[0].delivered_at.is_some());
}

#[test]
fn delivery_exhaustion() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Exhaustion Test", &["*"]);

    let delivery_id =
        create_delivery_record(&conn, &wh.id, "test.event", "{}").expect("create delivery");

    mark_exhausted(&conn, &delivery_id, MAX_RETRY_ATTEMPTS).expect("mark exhausted");
    let deliveries = get_webhook_deliveries(&conn, &wh.id, 10).expect("get deliveries");
    assert_eq!(deliveries[0].status, "exhausted");
    assert_eq!(deliveries[0].attempt_count, MAX_RETRY_ATTEMPTS);
}

// ============================================================================
// Retry Schedule
// ============================================================================

#[test]
fn retry_schedule_produces_future_timestamps() {
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        let ts = next_retry_at(attempt);
        // Should parse as a valid timestamp
        assert!(ts.contains('T'), "Timestamp should contain 'T': {}", ts);
        assert!(ts.ends_with('Z'), "Timestamp should end with 'Z': {}", ts);
    }
}

#[test]
fn retry_schedule_increases_monotonically() {
    // The backoff delays should be strictly increasing
    for i in 1..RETRY_BACKOFF_SECS.len() {
        assert!(
            RETRY_BACKOFF_SECS[i] > RETRY_BACKOFF_SECS[i - 1],
            "Backoff should increase: {} > {}",
            RETRY_BACKOFF_SECS[i],
            RETRY_BACKOFF_SECS[i - 1]
        );
    }
}

#[test]
fn retry_schedule_clamps_beyond_max() {
    // Attempt beyond array bounds should use the last entry
    let ts_at_max = next_retry_at(MAX_RETRY_ATTEMPTS);
    let ts_beyond = next_retry_at(MAX_RETRY_ATTEMPTS + 5);
    // Both should produce valid timestamps (clamped to last backoff entry)
    assert!(ts_at_max.contains('T'));
    assert!(ts_beyond.contains('T'));
}

// ============================================================================
// Event Matching
// ============================================================================

#[test]
fn event_matching_exact() {
    let patterns = vec!["signal.created".to_string()];
    assert!(event_matches(&patterns, "signal.created"));
    assert!(!event_matches(&patterns, "signal.resolved"));
    assert!(!event_matches(&patterns, "anomaly.detected"));
}

#[test]
fn event_matching_wildcard_suffix() {
    let patterns = vec!["signal.*".to_string()];
    assert!(event_matches(&patterns, "signal.created"));
    assert!(event_matches(&patterns, "signal.resolved"));
    assert!(!event_matches(&patterns, "anomaly.detected"));
    // Should not match "signalx.created" (requires dot after prefix)
    assert!(!event_matches(&patterns, "signalx.created"));
}

#[test]
fn event_matching_global_wildcard() {
    let patterns = vec!["*".to_string()];
    assert!(event_matches(&patterns, "signal.created"));
    assert!(event_matches(&patterns, "anomaly.detected"));
    assert!(event_matches(&patterns, "anything.at.all"));
}

#[test]
fn event_matching_multiple_patterns() {
    let patterns = vec!["signal.created".to_string(), "anomaly.*".to_string()];
    assert!(event_matches(&patterns, "signal.created"));
    assert!(!event_matches(&patterns, "signal.resolved"));
    assert!(event_matches(&patterns, "anomaly.detected"));
    assert!(event_matches(&patterns, "anomaly.resolved"));
}

#[test]
fn event_matching_empty_patterns() {
    let patterns: Vec<String> = vec![];
    assert!(!event_matches(&patterns, "signal.created"));
}

// ============================================================================
// Success / Failure Recording
// ============================================================================

#[test]
fn record_success_resets_failure_count() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Success Test", &["*"]);

    // Simulate some failures
    conn.execute(
        "UPDATE webhooks SET failure_count = 5 WHERE id = ?1",
        params![wh.id],
    )
    .expect("set failures");

    record_success(&conn, &wh.id).expect("record success");

    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    let found = webhooks.iter().find(|w| w.id == wh.id).expect("find");
    assert_eq!(found.failure_count, 0, "Success should reset failure count");
    assert_eq!(found.last_status_code, Some(200));
    assert!(found.last_fired_at.is_some());
}

#[test]
fn record_failure_increments_failure_count() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Failure Test", &["*"]);

    record_failure(&conn, &wh.id, Some(502)).expect("record failure");
    record_failure(&conn, &wh.id, Some(503)).expect("record failure 2");

    let webhooks = list_webhooks(&conn, "team-1").expect("list");
    let found = webhooks.iter().find(|w| w.id == wh.id).expect("find");
    assert_eq!(found.failure_count, 2);
    assert_eq!(found.last_status_code, Some(503));
}

// ============================================================================
// Deliveries Limit
// ============================================================================

#[test]
fn get_deliveries_respects_limit() {
    let conn = setup_test_db();
    let wh = register_test_webhook(&conn, "Limit Test", &["*"]);

    for i in 0..5 {
        create_delivery_record(&conn, &wh.id, &format!("test.event.{}", i), "{}")
            .expect("create delivery");
    }

    let all = get_webhook_deliveries(&conn, &wh.id, 100).expect("get all");
    assert_eq!(all.len(), 5);

    let limited = get_webhook_deliveries(&conn, &wh.id, 3).expect("get limited");
    assert_eq!(limited.len(), 3);
}
