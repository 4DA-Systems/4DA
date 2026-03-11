//! Tests for organization management and enterprise features.

use crate::organization::{self, OrgPolicies};
use std::collections::HashMap;

/// Helper: create an in-memory DB with all required tables for org tests.
fn setup_test_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");

    // Organization tables
    organization::create_tables(&conn).expect("create org tables");

    // Team tables (required for org-team queries)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS team_members_cache (
            team_id      TEXT NOT NULL,
            client_id    TEXT NOT NULL,
            display_name TEXT NOT NULL,
            role         TEXT NOT NULL DEFAULT 'member',
            public_key   BLOB,
            last_seen    TEXT,
            PRIMARY KEY (team_id, client_id)
        );

        CREATE TABLE IF NOT EXISTS team_sync_queue (
            entry_id    TEXT PRIMARY KEY,
            team_id     TEXT NOT NULL,
            client_id   TEXT NOT NULL,
            operation   TEXT NOT NULL,
            hlc_ts      INTEGER NOT NULL,
            encrypted   BLOB,
            relay_seq   INTEGER,
            created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
            acked_at    INTEGER
        );

        CREATE TABLE IF NOT EXISTS team_sync_log (
            relay_seq   INTEGER NOT NULL,
            team_id     TEXT NOT NULL,
            client_id   TEXT NOT NULL,
            encrypted   BLOB NOT NULL,
            received_at INTEGER NOT NULL DEFAULT (unixepoch()),
            applied     INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (relay_seq, team_id)
        );

        CREATE TABLE IF NOT EXISTS source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL,
            url TEXT,
            title TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            content_hash TEXT NOT NULL,
            embedding BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(source_type, source_id)
        );

        CREATE TABLE IF NOT EXISTS temporal_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data JSON NOT NULL,
            embedding BLOB,
            source_item_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );",
    )
    .expect("create supporting tables");

    conn
}

/// Populate team members cache for testing.
fn add_team_member(
    conn: &rusqlite::Connection,
    team_id: &str,
    client_id: &str,
    display_name: &str,
) {
    conn.execute(
        "INSERT OR REPLACE INTO team_members_cache (team_id, client_id, display_name, role, last_seen)
         VALUES (?1, ?2, ?3, 'member', datetime('now'))",
        rusqlite::params![team_id, client_id, display_name],
    )
    .expect("insert team member");
}

/// Insert a team sync queue entry for testing.
fn insert_sync_entry(
    conn: &rusqlite::Connection,
    team_id: &str,
    client_id: &str,
    operation_json: &str,
) {
    let entry_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO team_sync_queue (entry_id, team_id, client_id, operation, hlc_ts, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())",
        rusqlite::params![entry_id, team_id, client_id, operation_json, 1000i64],
    )
    .expect("insert sync entry");
}

// ============================================================================
// Organization CRUD Tests
// ============================================================================

#[test]
fn test_create_organization() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "Acme Corp", Some("hash123"), "admin-001")
        .expect("create org");

    assert_eq!(org.name, "Acme Corp");
    assert!(!org.id.is_empty());
    assert_eq!(org.team_count, 0);
    assert_eq!(org.total_seats, 0);

    // Verify it can be retrieved
    let fetched = organization::get_organization(&conn, &org.id).expect("get org");
    assert_eq!(fetched.name, "Acme Corp");
    assert_eq!(fetched.id, org.id);
}

#[test]
fn test_get_organization_not_found() {
    let conn = setup_test_db();

    let result = organization::get_organization(&conn, "nonexistent");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found"),
        "Expected 'not found' error, got: {err_msg}"
    );
}

// ============================================================================
// Team Management Tests
// ============================================================================

#[test]
fn test_add_and_remove_team() {
    let conn = setup_test_db();

    let org =
        organization::create_organization(&conn, "TeamOrg", None, "admin-002").expect("create org");

    // Add two teams
    organization::add_team_to_org(&conn, &org.id, "team-alpha").expect("add team alpha");
    organization::add_team_to_org(&conn, &org.id, "team-beta").expect("add team beta");

    // Verify teams
    let teams = organization::get_org_teams(&conn, &org.id).expect("get org teams");
    assert_eq!(teams.len(), 2);

    // Verify org team count
    let updated_org = organization::get_organization(&conn, &org.id).expect("get updated org");
    assert_eq!(updated_org.team_count, 2);

    // Remove one team
    organization::remove_team_from_org(&conn, &org.id, "team-alpha").expect("remove team alpha");
    let teams_after = organization::get_org_teams(&conn, &org.id).expect("get teams after remove");
    assert_eq!(teams_after.len(), 1);
    assert_eq!(teams_after[0].team_id, "team-beta");
}

#[test]
fn test_add_team_to_nonexistent_org() {
    let conn = setup_test_db();

    let result = organization::add_team_to_org(&conn, "no-such-org", "team-x");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_add_team_duplicate_is_noop() {
    let conn = setup_test_db();

    let org =
        organization::create_organization(&conn, "DupOrg", None, "admin-003").expect("create org");

    organization::add_team_to_org(&conn, &org.id, "team-dup").expect("first add");
    organization::add_team_to_org(&conn, &org.id, "team-dup")
        .expect("duplicate add should not error");

    let teams = organization::get_org_teams(&conn, &org.id).expect("get teams");
    assert_eq!(
        teams.len(),
        1,
        "Duplicate add should not create second entry"
    );
}

#[test]
fn test_remove_team_not_in_org_is_noop() {
    let conn = setup_test_db();

    let org =
        organization::create_organization(&conn, "NoopOrg", None, "admin-004").expect("create org");

    // Removing a team that was never added should succeed silently
    let result = organization::remove_team_from_org(&conn, &org.id, "ghost-team");
    assert!(result.is_ok());
}

#[test]
fn test_org_team_summary_with_members() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "MemberOrg", None, "admin-005")
        .expect("create org");
    organization::add_team_to_org(&conn, &org.id, "team-m1").expect("add team");

    // Add members to the team
    add_team_member(&conn, "team-m1", "user-1", "Alice");
    add_team_member(&conn, "team-m1", "user-2", "Bob");

    let teams = organization::get_org_teams(&conn, &org.id).expect("get teams");
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].member_count, 2);

    // Total seats should reflect members
    let org_info = organization::get_organization(&conn, &org.id).expect("get org");
    assert_eq!(org_info.total_seats, 2);
}

// ============================================================================
// Org Admin Tests
// ============================================================================

#[test]
fn test_org_admin_check() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "AdminOrg", None, "admin-006")
        .expect("create org");

    // The creator should be an admin (org_owner)
    assert!(
        organization::is_org_admin(&conn, &org.id, "admin-006").expect("check admin"),
        "Creator should be org admin"
    );

    // Random member should not be admin
    assert!(
        !organization::is_org_admin(&conn, &org.id, "random-user").expect("check non-admin"),
        "Random user should not be org admin"
    );
}

#[test]
fn test_org_admin_check_nonexistent_org() {
    let conn = setup_test_db();

    // Should return false, not error
    let is_admin = organization::is_org_admin(&conn, "no-org", "no-member")
        .expect("admin check on missing org");
    assert!(!is_admin);
}

// ============================================================================
// Retention Policy Tests
// ============================================================================

#[test]
fn test_retention_policy_set_and_get() {
    let conn = setup_test_db();

    // Set policies for a team
    organization::set_retention_policy(&conn, "team-r1", "source_items", 90)
        .expect("set source_items policy");
    organization::set_retention_policy(&conn, "team-r1", "team_sync_log", 30)
        .expect("set sync_log policy");

    let policies = organization::get_retention_policies(&conn, "team-r1").expect("get policies");
    assert_eq!(policies.len(), 2);

    // Verify values (sorted by resource_type)
    let source_policy = policies.iter().find(|p| p.resource_type == "source_items");
    assert!(source_policy.is_some());
    assert_eq!(source_policy.unwrap().retention_days, 90);

    let sync_policy = policies.iter().find(|p| p.resource_type == "team_sync_log");
    assert!(sync_policy.is_some());
    assert_eq!(sync_policy.unwrap().retention_days, 30);
}

#[test]
fn test_retention_policy_update_existing() {
    let conn = setup_test_db();

    organization::set_retention_policy(&conn, "team-r2", "source_items", 90).expect("set initial");
    organization::set_retention_policy(&conn, "team-r2", "source_items", 60)
        .expect("update policy");

    let policies = organization::get_retention_policies(&conn, "team-r2").expect("get policies");
    assert_eq!(policies.len(), 1, "Should have single policy after upsert");
    assert_eq!(
        policies[0].retention_days, 60,
        "Should reflect updated value"
    );
}

#[test]
fn test_retention_policy_empty_team() {
    let conn = setup_test_db();

    let policies =
        organization::get_retention_policies(&conn, "no-such-team").expect("get empty policies");
    assert!(policies.is_empty());
}

#[test]
fn test_enforce_retention_purges_source_items() {
    let conn = setup_test_db();

    // Set a 1-day retention for source_items
    organization::set_retention_policy(&conn, "team-purge", "source_items", 1).expect("set policy");

    // Insert an old source item (3 days ago)
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding, created_at)
         VALUES ('test', 'old-1', 'Old Item', 'content', 'hash1', zeroblob(0), datetime('now', '-3 days'))",
        [],
    )
    .expect("insert old item");

    // Insert a recent item
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding, created_at)
         VALUES ('test', 'new-1', 'New Item', 'content', 'hash2', zeroblob(0), datetime('now'))",
        [],
    )
    .expect("insert new item");

    // Enforce retention
    let purged = organization::enforce_retention(&conn, "team-purge").expect("enforce retention");
    assert!(purged >= 1, "Should purge at least the old item");

    // Verify new item survives
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
        .expect("count remaining");
    assert_eq!(
        remaining, 1,
        "New item should survive retention enforcement"
    );
}

#[test]
fn test_enforce_retention_unknown_resource_type_is_ignored() {
    let conn = setup_test_db();

    organization::set_retention_policy(&conn, "team-unknown", "weird_type", 7)
        .expect("set policy with unknown type");

    // Should not error, just skip the unknown type
    let purged =
        organization::enforce_retention(&conn, "team-unknown").expect("enforce with unknown type");
    assert_eq!(purged, 0);
}

// ============================================================================
// Org Policies Tests
// ============================================================================

#[test]
fn test_set_and_get_org_policies() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "PolicyOrg", None, "admin-007")
        .expect("create org");

    let mut retention_defaults = HashMap::new();
    retention_defaults.insert("source_items".to_string(), 90);
    retention_defaults.insert("team_sync_log".to_string(), 30);

    let policies = OrgPolicies {
        default_retention_days: retention_defaults,
        min_monitoring_interval: Some(300),
        require_decision_tracking: true,
    };

    organization::set_org_policies(&conn, &org.id, &policies).expect("set policies");

    let fetched = organization::get_org_policies(&conn, &org.id).expect("get policies");
    assert_eq!(
        fetched.default_retention_days.get("source_items"),
        Some(&90)
    );
    assert_eq!(fetched.min_monitoring_interval, Some(300));
    assert!(fetched.require_decision_tracking);
}

#[test]
fn test_get_org_policies_defaults_when_none_set() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "DefaultPolicyOrg", None, "admin-008")
        .expect("create org");

    let policies = organization::get_org_policies(&conn, &org.id).expect("get default policies");
    assert!(policies.default_retention_days.is_empty());
    assert_eq!(policies.min_monitoring_interval, None);
    assert!(!policies.require_decision_tracking);
}

#[test]
fn test_set_org_policies_nonexistent_org() {
    let conn = setup_test_db();

    let policies = OrgPolicies::default();
    let result = organization::set_org_policies(&conn, "no-such-org", &policies);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ============================================================================
// Cross-Team Correlation Tests
// ============================================================================

#[test]
fn test_cross_team_signal_detection_no_teams() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "EmptyOrg", None, "admin-009")
        .expect("create org");

    let correlations =
        organization::detect_cross_team_signals(&conn, &org.id).expect("detect signals");
    assert!(
        correlations.is_empty(),
        "No correlations expected for org with no teams"
    );
}

#[test]
fn test_cross_team_signal_detection_single_team() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "SingleTeamOrg", None, "admin-010")
        .expect("create org");
    organization::add_team_to_org(&conn, &org.id, "solo-team").expect("add team");

    let correlations =
        organization::detect_cross_team_signals(&conn, &org.id).expect("detect signals");
    assert!(
        correlations.is_empty(),
        "Cross-team correlation requires at least 2 teams"
    );
}

#[test]
fn test_cross_team_signal_detection_with_shared_topics() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "MultiTeamOrg", None, "admin-011")
        .expect("create org");
    organization::add_team_to_org(&conn, &org.id, "team-x").expect("add team-x");
    organization::add_team_to_org(&conn, &org.id, "team-y").expect("add team-y");

    // Insert ShareSignal entries with overlapping topics
    let signal_op_1 = serde_json::json!({
        "type": "ShareSignal",
        "signal_id": "sig-1",
        "chain_name": "CVE chain",
        "priority": "high",
        "tech_topics": ["openssl", "rust"],
        "suggested_action": "Update deps"
    })
    .to_string();

    let signal_op_2 = serde_json::json!({
        "type": "ShareSignal",
        "signal_id": "sig-2",
        "chain_name": "CVE chain",
        "priority": "medium",
        "tech_topics": ["openssl", "python"],
        "suggested_action": "Review deps"
    })
    .to_string();

    insert_sync_entry(&conn, "team-x", "client-x1", &signal_op_1);
    insert_sync_entry(&conn, "team-y", "client-y1", &signal_op_2);

    let correlations =
        organization::detect_cross_team_signals(&conn, &org.id).expect("detect signals");

    // Should detect "openssl" as a cross-team correlation
    let openssl_corr = correlations.iter().find(|c| c.teams_affected.len() >= 2);

    assert!(
        openssl_corr.is_some(),
        "Should detect cross-team correlation for shared topic 'openssl'. Got {} correlations.",
        correlations.len()
    );

    let corr = openssl_corr.unwrap();
    assert_eq!(corr.teams_affected.len(), 2);
    assert!(!corr.correlation_id.is_empty());
    assert!(!corr.recommendation.is_empty());
}

#[test]
fn test_cross_team_no_overlapping_topics() {
    let conn = setup_test_db();

    let org = organization::create_organization(&conn, "DisjointOrg", None, "admin-012")
        .expect("create org");
    organization::add_team_to_org(&conn, &org.id, "team-a").expect("add team-a");
    organization::add_team_to_org(&conn, &org.id, "team-b").expect("add team-b");

    let signal_a = serde_json::json!({
        "type": "ShareSignal",
        "signal_id": "sig-a",
        "chain_name": "Frontend",
        "priority": "low",
        "tech_topics": ["react", "typescript"],
        "suggested_action": "Review"
    })
    .to_string();

    let signal_b = serde_json::json!({
        "type": "ShareSignal",
        "signal_id": "sig-b",
        "chain_name": "Backend",
        "priority": "low",
        "tech_topics": ["python", "django"],
        "suggested_action": "Review"
    })
    .to_string();

    insert_sync_entry(&conn, "team-a", "client-a1", &signal_a);
    insert_sync_entry(&conn, "team-b", "client-b1", &signal_b);

    let correlations =
        organization::detect_cross_team_signals(&conn, &org.id).expect("detect signals");
    assert!(
        correlations.is_empty(),
        "Should not detect correlations when topics are disjoint"
    );
}
