//! Tests for team_sync — Phase 1 local sync primitives.

use crate::team_sync;
use crate::team_sync_types::*;

/// Helper: create an in-memory DB with team sync tables.
fn setup_test_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE team_sync_queue (
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
        CREATE INDEX idx_tsq_pending ON team_sync_queue(acked_at) WHERE acked_at IS NULL;

        CREATE TABLE team_sync_log (
            relay_seq   INTEGER NOT NULL,
            team_id     TEXT NOT NULL,
            client_id   TEXT NOT NULL,
            encrypted   BLOB NOT NULL,
            received_at INTEGER NOT NULL DEFAULT (unixepoch()),
            applied     INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (relay_seq, team_id)
        );
        CREATE INDEX idx_tsl_unapplied ON team_sync_log(applied) WHERE applied = 0;

        CREATE TABLE team_sync_state (
            team_id         TEXT PRIMARY KEY,
            last_relay_seq  INTEGER NOT NULL DEFAULT 0,
            last_sync_at    INTEGER
        );

        CREATE TABLE team_crypto (
            team_id             TEXT PRIMARY KEY,
            our_public_key      BLOB NOT NULL,
            our_private_key_enc BLOB NOT NULL,
            team_symmetric_key_enc BLOB,
            created_at          INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE TABLE team_members_cache (
            team_id      TEXT NOT NULL,
            client_id    TEXT NOT NULL,
            display_name TEXT NOT NULL,
            role         TEXT NOT NULL DEFAULT 'member',
            public_key   BLOB,
            last_seen    TEXT,
            PRIMARY KEY (team_id, client_id)
        );",
    )
    .unwrap();
    conn
}

#[test]
fn test_queue_entry_and_get_pending() {
    let conn = setup_test_db();
    let team_id = "team-001";
    let client_id = "client-aaa";

    let op = TeamOp::ShareSignal {
        signal_id: "sig-1".into(),
        chain_name: "CVE chain".into(),
        priority: "high".into(),
        tech_topics: vec!["rust".into(), "openssl".into()],
        suggested_action: "Review deps".into(),
    };

    let entry_id = team_sync::queue_entry(&conn, team_id, client_id, 1000, &op).unwrap();
    assert!(!entry_id.is_empty(), "entry_id should be a UUID");

    let pending = team_sync::get_pending_entries(&conn, team_id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, entry_id);

    // Payload should be valid JSON of the operation
    let payload_str = String::from_utf8(pending[0].1.clone()).unwrap();
    assert!(payload_str.contains("ShareSignal"));
}

#[test]
fn test_mark_entries_acked() {
    let conn = setup_test_db();
    let team_id = "team-002";
    let client_id = "client-bbb";

    let op = TeamOp::MemberJoined {
        display_name: "Alice".into(),
        role: "admin".into(),
    };

    let id1 = team_sync::queue_entry(&conn, team_id, client_id, 100, &op).unwrap();
    let id2 = team_sync::queue_entry(&conn, team_id, client_id, 200, &op).unwrap();

    // Both should be pending
    assert_eq!(
        team_sync::pending_outbound_count(&conn, team_id).unwrap(),
        2
    );

    // Ack one
    let acked = team_sync::mark_entries_acked(&conn, &[id1.clone()]).unwrap();
    assert_eq!(acked, 1);
    assert_eq!(
        team_sync::pending_outbound_count(&conn, team_id).unwrap(),
        1
    );

    // Ack the other
    let acked = team_sync::mark_entries_acked(&conn, &[id2]).unwrap();
    assert_eq!(acked, 1);
    assert_eq!(
        team_sync::pending_outbound_count(&conn, team_id).unwrap(),
        0
    );

    // Ack empty list should return 0
    assert_eq!(team_sync::mark_entries_acked(&conn, &[]).unwrap(), 0);
}

#[test]
fn test_store_and_apply_inbound_member_joined() {
    let conn = setup_test_db();
    let team_id = "team-003";
    let client_id = "client-ccc";

    let entry = TeamMetadataEntry {
        entry_id: "e-001".into(),
        client_id: client_id.into(),
        hlc_timestamp: 500,
        operation: TeamOp::MemberJoined {
            display_name: "Bob".into(),
            role: "member".into(),
        },
    };
    let payload = serde_json::to_vec(&entry).unwrap();

    // Store as inbound
    let inserted = team_sync::store_inbound_entry(&conn, 1, team_id, client_id, &payload).unwrap();
    assert!(inserted, "Should insert new inbound entry");

    // Duplicate should be ignored
    let dup = team_sync::store_inbound_entry(&conn, 1, team_id, client_id, &payload).unwrap();
    assert!(!dup, "Duplicate relay_seq should be ignored");

    // Apply pending inbound using a JSON deserializer as the "decrypt" function
    let applied =
        team_sync::apply_pending_inbound(&conn, team_id, &|blob| Ok(serde_json::from_slice(blob)?))
            .unwrap();
    assert_eq!(applied, 1);

    // Verify member was cached
    let members = team_sync::get_team_members(&conn, team_id).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].display_name, "Bob");
    assert_eq!(members[0].role, "member");

    // Verify sync state updated
    let seq = team_sync::get_last_relay_seq(&conn, team_id).unwrap();
    assert_eq!(seq, 1);
}

#[test]
fn test_member_left_removes_from_cache() {
    let conn = setup_test_db();
    let team_id = "team-004";
    let client_id = "client-ddd";

    // First, add a member
    let join_entry = TeamMetadataEntry {
        entry_id: "e-join".into(),
        client_id: client_id.into(),
        hlc_timestamp: 100,
        operation: TeamOp::MemberJoined {
            display_name: "Charlie".into(),
            role: "member".into(),
        },
    };
    team_sync::apply_entry(&conn, team_id, &join_entry).unwrap();
    assert_eq!(
        team_sync::get_team_members(&conn, team_id).unwrap().len(),
        1
    );

    // Then remove them
    let leave_entry = TeamMetadataEntry {
        entry_id: "e-leave".into(),
        client_id: client_id.into(),
        hlc_timestamp: 200,
        operation: TeamOp::MemberLeft {
            reason: "Resigned".into(),
        },
    };
    team_sync::apply_entry(&conn, team_id, &leave_entry).unwrap();
    assert_eq!(
        team_sync::get_team_members(&conn, team_id).unwrap().len(),
        0
    );
}

#[test]
fn test_get_sync_status() {
    let conn = setup_test_db();
    let team_id = "team-005";
    let client_id = "client-eee";

    // Queue two entries
    let op = TeamOp::ShareContextSummary {
        active_topics: vec!["rust".into()],
        tech_scores: vec![("rust".into(), 0.95)],
    };
    team_sync::queue_entry(&conn, team_id, client_id, 1, &op).unwrap();
    team_sync::queue_entry(&conn, team_id, client_id, 2, &op).unwrap();

    let status = team_sync::get_sync_status(&conn, team_id, client_id).unwrap();
    assert!(status.enabled);
    assert!(!status.connected);
    assert_eq!(status.pending_outbound, 2);
    assert_eq!(status.last_relay_seq, 0);
    assert_eq!(status.member_count, 0);
    assert_eq!(status.team_id.as_deref(), Some(team_id));
    assert_eq!(status.client_id.as_deref(), Some(client_id));
}
