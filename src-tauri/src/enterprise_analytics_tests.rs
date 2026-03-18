// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for enterprise analytics — organization-level usage and intelligence metrics.
//!
//! Covers: `get_org_analytics`, `export_org_analytics_csv`, engagement scoring,
//! multi-team aggregation, CSV formatting, and edge cases (dormant teams, empty orgs).

use crate::enterprise_analytics::*;
use rusqlite::{params, Connection};

// ============================================================================
// Helpers
// ============================================================================

/// Create an in-memory SQLite database with the tables required by enterprise analytics.
fn setup_analytics_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE org_teams (
            org_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            PRIMARY KEY (org_id, team_id)
        );
        CREATE TABLE team_members_cache (
            team_id TEXT NOT NULL,
            client_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'member',
            last_seen TEXT,
            PRIMARY KEY (team_id, client_id)
        );
        CREATE TABLE team_sync_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id TEXT NOT NULL,
            client_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch('now'))
        );
        CREATE TABLE briefing_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT,
            generated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE org_admins (
            org_id TEXT NOT NULL,
            member_id TEXT NOT NULL
        );",
    )
    .unwrap();
    conn
}

/// Insert a sync queue entry with a given JSON operation (uses default `created_at = now`).
fn insert_sync_op(conn: &Connection, team_id: &str, client_id: &str, op_json: &str) {
    conn.execute(
        "INSERT INTO team_sync_queue (team_id, client_id, operation) VALUES (?1, ?2, ?3)",
        params![team_id, client_id, op_json],
    )
    .unwrap();
}

/// Register an org → team relationship.
fn add_org_team(conn: &Connection, org_id: &str, team_id: &str) {
    conn.execute(
        "INSERT INTO org_teams (org_id, team_id) VALUES (?1, ?2)",
        params![org_id, team_id],
    )
    .unwrap();
}

/// Add a member to the team members cache.
fn add_member(conn: &Connection, team_id: &str, client_id: &str, name: &str) {
    conn.execute(
        "INSERT INTO team_members_cache (team_id, client_id, display_name, role) \
         VALUES (?1, ?2, ?3, 'member')",
        params![team_id, client_id, name],
    )
    .unwrap();
}

// ============================================================================
// Test 1: Empty org returns zero metrics
// ============================================================================

#[test]
fn test_empty_org_returns_zero_metrics() {
    let conn = setup_analytics_db();

    let result = get_org_analytics(&conn, "org-empty", 30).unwrap();

    assert_eq!(result.active_seats, 0);
    assert_eq!(result.total_seats, 0);
    assert_eq!(result.signals_detected, 0);
    assert_eq!(result.signals_resolved, 0);
    assert_eq!(result.decisions_tracked, 0);
    assert_eq!(result.briefings_generated, 0);
    assert!(result.team_activity.is_empty());
    assert!(result.top_signal_categories.is_empty());
    assert_eq!(result.period, "Last 30 days");
}

// ============================================================================
// Test 2: Signal counting from ShareSignal ops
// ============================================================================

#[test]
fn test_signal_counting() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // Insert ShareSignal operations with different categories
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "security"}"#,
    );
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "security"}"#,
    );
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "frontend"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 3);

    // Check category breakdown
    let security_count = result
        .top_signal_categories
        .iter()
        .find(|(cat, _)| cat == "security")
        .map(|(_, c)| *c)
        .unwrap_or(0);
    assert_eq!(security_count, 2);

    let frontend_count = result
        .top_signal_categories
        .iter()
        .find(|(cat, _)| cat == "frontend")
        .map(|(_, c)| *c)
        .unwrap_or(0);
    assert_eq!(frontend_count, 1);
}

// ============================================================================
// Test 3: Decision counting from ProposeDecision ops
// ============================================================================

#[test]
fn test_decision_counting() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ProposeDecision"}"#);
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ProposeDecision"}"#);
    // Non-decision ops should not count as decisions
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.decisions_tracked, 2);
    // The ShareSignal should still count as a signal
    assert_eq!(result.signals_detected, 1);
}

// ============================================================================
// Test 4: Active seat calculation
// ============================================================================

#[test]
fn test_active_seats_calculation() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");

    // 3 members total
    add_member(&conn, "team-a", "user-1", "Alice");
    add_member(&conn, "team-a", "user-2", "Bob");
    add_member(&conn, "team-a", "user-3", "Carol");

    // Only 2 have recent activity
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );
    insert_sync_op(&conn, "team-a", "user-2", r#"{"type": "ProposeDecision"}"#);
    // user-3 has no activity

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.total_seats, 3);
    assert_eq!(result.active_seats, 2);
}

// ============================================================================
// Test 5: Engagement score computation
// ============================================================================

#[test]
fn test_engagement_score_computation() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");

    // 2 members
    add_member(&conn, "team-a", "user-1", "Alice");
    add_member(&conn, "team-a", "user-2", "Bob");

    // Both active with 6 actions each → 12 total (above 10 cap)
    for _ in 0..6 {
        insert_sync_op(
            &conn,
            "team-a",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "test"}"#,
        );
    }
    for _ in 0..6 {
        insert_sync_op(&conn, "team-a", "user-2", r#"{"type": "ProposeDecision"}"#);
    }

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.team_activity.len(), 1);

    let team = &result.team_activity[0];
    // member_ratio = 2/2 = 1.0
    // action_density = 12/10 = 1.2 → capped to 1.0
    // engagement = 1.0 * 0.6 + 1.0 * 0.4 = 1.0
    assert!(
        (team.engagement_score - 1.0).abs() < 0.01,
        "Expected engagement ~1.0, got {}",
        team.engagement_score
    );
}

// ============================================================================
// Test 6: Team activity breakdown with multiple teams
// ============================================================================

#[test]
fn test_multi_team_activity() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_org_team(&conn, "org-1", "team-b");

    add_member(&conn, "team-a", "user-1", "Alice");
    add_member(&conn, "team-b", "user-2", "Bob");

    // Team A: 3 signals
    for _ in 0..3 {
        insert_sync_op(
            &conn,
            "team-a",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "backend"}"#,
        );
    }
    // Team B: 1 decision
    insert_sync_op(&conn, "team-b", "user-2", r#"{"type": "ProposeDecision"}"#);

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.team_activity.len(), 2);
    assert_eq!(result.signals_detected, 3);
    assert_eq!(result.decisions_tracked, 1);
    assert_eq!(result.total_seats, 2);
    assert_eq!(result.active_seats, 2);

    // Verify per-team breakdowns
    let team_a = result
        .team_activity
        .iter()
        .find(|t| t.team_id == "team-a")
        .unwrap();
    let team_b = result
        .team_activity
        .iter()
        .find(|t| t.team_id == "team-b")
        .unwrap();
    assert_eq!(team_a.signals_this_period, 3);
    assert_eq!(team_a.decisions_this_period, 0);
    assert_eq!(team_b.signals_this_period, 0);
    assert_eq!(team_b.decisions_this_period, 1);

    // Team A has more actions → higher engagement
    assert!(
        team_a.engagement_score > team_b.engagement_score,
        "team-a ({}) should have higher engagement than team-b ({})",
        team_a.engagement_score,
        team_b.engagement_score
    );
}

// ============================================================================
// Test 7: Dormant team (no activity)
// ============================================================================

#[test]
fn test_dormant_team() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-dormant");
    add_member(&conn, "team-dormant", "user-1", "Ghost");
    // No sync activity at all

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.total_seats, 1);
    assert_eq!(result.active_seats, 0);
    assert_eq!(result.team_activity.len(), 1);

    let team = &result.team_activity[0];
    assert_eq!(team.team_id, "team-dormant");
    assert_eq!(team.active_members, 0);
    assert_eq!(team.signals_this_period, 0);
    assert_eq!(team.decisions_this_period, 0);
    assert_eq!(team.engagement_score, 0.0);
}

// ============================================================================
// Test 8: Single-member team engagement
// ============================================================================

#[test]
fn test_single_member_team() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-solo");
    add_member(&conn, "team-solo", "solo-user", "Solo Dev");

    insert_sync_op(
        &conn,
        "team-solo",
        "solo-user",
        r#"{"type": "ShareSignal", "chain_name": "solo-chain"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.total_seats, 1);
    assert_eq!(result.active_seats, 1);

    let team = &result.team_activity[0];
    assert_eq!(team.active_members, 1);
    // member_ratio = 1/1 = 1.0
    // action_density = 1/10 = 0.1
    // engagement = 1.0 * 0.6 + 0.1 * 0.4 = 0.64
    assert!(
        (team.engagement_score - 0.64).abs() < 0.01,
        "Expected engagement ~0.64, got {}",
        team.engagement_score
    );
}

// ============================================================================
// Test 9: CSV export format
// ============================================================================

#[test]
fn test_csv_export_format() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );

    let csv = export_org_analytics_csv(&conn, "org-1", 30).unwrap();

    // Header section
    assert!(csv.contains("# Organization Analytics Report"));
    assert!(csv.contains("Last 30 days"));

    // Summary metrics
    assert!(csv.contains("Metric,Value"));
    assert!(csv.contains("Active Seats,1"));
    assert!(csv.contains("Total Seats,1"));
    assert!(csv.contains("Signals Detected,1"));
    assert!(csv.contains("Signals Resolved,0"));
    assert!(csv.contains("Decisions Tracked,0"));

    // Signal categories
    assert!(csv.contains("Signal Category,Count"));
    assert!(csv.contains("test"));

    // Team activity
    assert!(csv.contains("Team ID,Active Members,Signals,Decisions,Engagement Score"));
    assert!(csv.contains("team-a"));
}

// ============================================================================
// Test 10: Period label formatting
// ============================================================================

#[test]
fn test_period_label_formatting() {
    let conn = setup_analytics_db();

    let result_1day = get_org_analytics(&conn, "org-x", 1).unwrap();
    assert_eq!(result_1day.period, "Last 24 hours");

    let result_7day = get_org_analytics(&conn, "org-x", 7).unwrap();
    assert_eq!(result_7day.period, "Last 7 days");

    let result_30day = get_org_analytics(&conn, "org-x", 30).unwrap();
    assert_eq!(result_30day.period, "Last 30 days");

    let result_90day = get_org_analytics(&conn, "org-x", 90).unwrap();
    assert_eq!(result_90day.period, "Last 90 days");
}

// ============================================================================
// Test 11: Resolved signals counting
// ============================================================================

#[test]
fn test_resolved_signals_counting() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // 2 signals shared, 1 resolved
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ResolveSignal"}"#);

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 2);
    assert_eq!(result.signals_resolved, 1);
}

// ============================================================================
// Test 12: Briefing count from cache
// ============================================================================

#[test]
fn test_briefing_count() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // Insert recent briefings (default generated_at = now)
    conn.execute(
        "INSERT INTO briefing_cache (content, generated_at) VALUES ('brief1', datetime('now'))",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO briefing_cache (content, generated_at) VALUES ('brief2', datetime('now'))",
        [],
    )
    .unwrap();

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.briefings_generated, 2);
}

// ============================================================================
// Test 13: CSV export for empty org
// ============================================================================

#[test]
fn test_csv_export_empty_org() {
    let conn = setup_analytics_db();

    let csv = export_org_analytics_csv(&conn, "org-empty", 7).unwrap();

    assert!(csv.contains("# Organization Analytics Report"));
    assert!(csv.contains("Last 7 days"));
    assert!(csv.contains("Active Seats,0"));
    assert!(csv.contains("Total Seats,0"));
    assert!(csv.contains("Signals Detected,0"));
    assert!(csv.contains("Signals Resolved,0"));
    assert!(csv.contains("Decisions Tracked,0"));
    assert!(csv.contains("Briefings Generated,0"));
}

// ============================================================================
// Test 14: Mixed operation types
// ============================================================================

#[test]
fn test_mixed_operation_types() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // Insert one of each operation type
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "security"}"#,
    );
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ResolveSignal"}"#);
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ProposeDecision"}"#);

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 1);
    assert_eq!(result.signals_resolved, 1);
    assert_eq!(result.decisions_tracked, 1);
    assert_eq!(result.active_seats, 1);
}

// ============================================================================
// Test 15: Cross-team member deduplication for total seats
// ============================================================================

#[test]
fn test_cross_team_seat_deduplication() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_org_team(&conn, "org-1", "team-b");

    // Same client_id in both teams — should count as 1 seat
    add_member(&conn, "team-a", "shared-user", "Alice");
    add_member(&conn, "team-b", "shared-user", "Alice");
    add_member(&conn, "team-b", "unique-user", "Bob");

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    // client_id "shared-user" appears in both teams but COUNT(DISTINCT client_id) = 2
    assert_eq!(result.total_seats, 2);
}

// ============================================================================
// Test 16: Signal category ordering (top categories first)
// ============================================================================

#[test]
fn test_signal_categories_sorted_by_count_descending() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // 5 security, 3 frontend, 1 backend
    for _ in 0..5 {
        insert_sync_op(
            &conn,
            "team-a",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "security"}"#,
        );
    }
    for _ in 0..3 {
        insert_sync_op(
            &conn,
            "team-a",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "frontend"}"#,
        );
    }
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "backend"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 9);
    assert_eq!(result.top_signal_categories.len(), 3);

    // First category should be security (highest count)
    assert_eq!(result.top_signal_categories[0].0, "security");
    assert_eq!(result.top_signal_categories[0].1, 5);
    assert_eq!(result.top_signal_categories[1].0, "frontend");
    assert_eq!(result.top_signal_categories[1].1, 3);
    assert_eq!(result.top_signal_categories[2].0, "backend");
    assert_eq!(result.top_signal_categories[2].1, 1);
}

// ============================================================================
// Test 17: Teams sorted by engagement score descending
// ============================================================================

#[test]
fn test_team_activity_sorted_by_engagement() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-active");
    add_org_team(&conn, "org-1", "team-moderate");
    add_org_team(&conn, "org-1", "team-dormant");

    add_member(&conn, "team-active", "user-1", "Alice");
    add_member(&conn, "team-moderate", "user-2", "Bob");
    add_member(&conn, "team-dormant", "user-3", "Carol");

    // team-active: 10+ actions → high engagement
    for _ in 0..12 {
        insert_sync_op(
            &conn,
            "team-active",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "test"}"#,
        );
    }
    // team-moderate: 2 actions → moderate engagement
    insert_sync_op(
        &conn,
        "team-moderate",
        "user-2",
        r#"{"type": "ShareSignal", "chain_name": "test"}"#,
    );
    insert_sync_op(
        &conn,
        "team-moderate",
        "user-2",
        r#"{"type": "ProposeDecision"}"#,
    );
    // team-dormant: no actions

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.team_activity.len(), 3);

    // Verify descending engagement order
    assert_eq!(result.team_activity[0].team_id, "team-active");
    assert_eq!(result.team_activity[1].team_id, "team-moderate");
    assert_eq!(result.team_activity[2].team_id, "team-dormant");

    assert!(result.team_activity[0].engagement_score > result.team_activity[1].engagement_score);
    assert!(result.team_activity[1].engagement_score > result.team_activity[2].engagement_score);
    assert_eq!(result.team_activity[2].engagement_score, 0.0);
}

// ============================================================================
// Test 18: Unknown operation types are ignored gracefully
// ============================================================================

#[test]
fn test_unknown_operation_types_ignored() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // Insert unknown operation types
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "SyncSettings"}"#);
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "UpdateProfile"}"#);
    // One valid signal
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "valid"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 1);
    assert_eq!(result.signals_resolved, 0);
    assert_eq!(result.decisions_tracked, 0);
    // Active seats should still count user-1 (they have sync activity)
    assert_eq!(result.active_seats, 1);
}

// ============================================================================
// Test 19: Malformed JSON in operations is silently skipped
// ============================================================================

#[test]
fn test_malformed_json_operations_skipped() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // Malformed JSON
    insert_sync_op(&conn, "team-a", "user-1", "not-json-at-all");
    insert_sync_op(&conn, "team-a", "user-1", "{broken: json}");
    // Valid operation
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "valid"}"#,
    );

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    // Only the valid ShareSignal should be counted
    assert_eq!(result.signals_detected, 1);
    // But all 3 entries contribute to active seats (any sync activity counts)
    assert_eq!(result.active_seats, 1);
}

// ============================================================================
// Test 20: Multi-user same team engagement math
// ============================================================================

#[test]
fn test_partial_team_engagement() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");

    // 4 members, only 2 active
    add_member(&conn, "team-a", "user-1", "Alice");
    add_member(&conn, "team-a", "user-2", "Bob");
    add_member(&conn, "team-a", "user-3", "Carol");
    add_member(&conn, "team-a", "user-4", "Dave");

    // 2 active members, 5 total actions (signals + decisions)
    for _ in 0..3 {
        insert_sync_op(
            &conn,
            "team-a",
            "user-1",
            r#"{"type": "ShareSignal", "chain_name": "test"}"#,
        );
    }
    insert_sync_op(&conn, "team-a", "user-2", r#"{"type": "ProposeDecision"}"#);
    insert_sync_op(&conn, "team-a", "user-2", r#"{"type": "ProposeDecision"}"#);

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    let team = &result.team_activity[0];

    assert_eq!(team.active_members, 2);
    // member_ratio = 2/4 = 0.5
    // action_density = 5/10 = 0.5
    // engagement = 0.5 * 0.6 + 0.5 * 0.4 = 0.30 + 0.20 = 0.50
    assert!(
        (team.engagement_score - 0.50).abs() < 0.01,
        "Expected engagement ~0.50, got {}",
        team.engagement_score
    );
}

// ============================================================================
// Test 21: CSV export with multiple categories and teams
// ============================================================================

#[test]
fn test_csv_export_rich_data() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_org_team(&conn, "org-1", "team-b");

    add_member(&conn, "team-a", "user-1", "Alice");
    add_member(&conn, "team-b", "user-2", "Bob");

    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "security"}"#,
    );
    insert_sync_op(
        &conn,
        "team-a",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "frontend"}"#,
    );
    insert_sync_op(&conn, "team-b", "user-2", r#"{"type": "ProposeDecision"}"#);
    insert_sync_op(&conn, "team-b", "user-2", r#"{"type": "ResolveSignal"}"#);

    let csv = export_org_analytics_csv(&conn, "org-1", 30).unwrap();

    // Verify all sections present
    assert!(csv.contains("Active Seats,2"));
    assert!(csv.contains("Total Seats,2"));
    assert!(csv.contains("Signals Detected,2"));
    assert!(csv.contains("Signals Resolved,1"));
    assert!(csv.contains("Decisions Tracked,1"));

    // Both teams appear in team activity section
    assert!(csv.contains("team-a"));
    assert!(csv.contains("team-b"));

    // Both categories appear
    assert!(csv.contains("security"));
    assert!(csv.contains("frontend"));
}

// ============================================================================
// Test 22: Org isolation — other orgs data is excluded
// ============================================================================

#[test]
fn test_org_isolation() {
    let conn = setup_analytics_db();

    // Two separate orgs with their own teams
    add_org_team(&conn, "org-A", "team-alpha");
    add_org_team(&conn, "org-B", "team-beta");

    add_member(&conn, "team-alpha", "user-1", "Alice");
    add_member(&conn, "team-beta", "user-2", "Bob");

    insert_sync_op(
        &conn,
        "team-alpha",
        "user-1",
        r#"{"type": "ShareSignal", "chain_name": "alpha-chain"}"#,
    );
    insert_sync_op(
        &conn,
        "team-beta",
        "user-2",
        r#"{"type": "ShareSignal", "chain_name": "beta-chain"}"#,
    );
    insert_sync_op(
        &conn,
        "team-beta",
        "user-2",
        r#"{"type": "ProposeDecision"}"#,
    );

    // Query org-A — should only see team-alpha's data
    let result_a = get_org_analytics(&conn, "org-A", 30).unwrap();
    assert_eq!(result_a.total_seats, 1);
    assert_eq!(result_a.signals_detected, 1);
    assert_eq!(result_a.decisions_tracked, 0);
    assert_eq!(result_a.team_activity.len(), 1);
    assert_eq!(result_a.team_activity[0].team_id, "team-alpha");

    // Query org-B — should only see team-beta's data
    let result_b = get_org_analytics(&conn, "org-B", 30).unwrap();
    assert_eq!(result_b.total_seats, 1);
    assert_eq!(result_b.signals_detected, 1);
    assert_eq!(result_b.decisions_tracked, 1);
    assert_eq!(result_b.team_activity.len(), 1);
    assert_eq!(result_b.team_activity[0].team_id, "team-beta");
}

// ============================================================================
// Test 23: ShareSignal without chain_name → "uncategorized"
// ============================================================================

#[test]
fn test_signal_without_chain_name_is_uncategorized() {
    let conn = setup_analytics_db();
    add_org_team(&conn, "org-1", "team-a");
    add_member(&conn, "team-a", "user-1", "Alice");

    // ShareSignal with no chain_name field
    insert_sync_op(&conn, "team-a", "user-1", r#"{"type": "ShareSignal"}"#);

    let result = get_org_analytics(&conn, "org-1", 30).unwrap();
    assert_eq!(result.signals_detected, 1);

    let uncat = result
        .top_signal_categories
        .iter()
        .find(|(cat, _)| cat == "uncategorized")
        .map(|(_, c)| *c)
        .unwrap_or(0);
    assert_eq!(uncat, 1);
}
