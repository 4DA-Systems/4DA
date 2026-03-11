//! Tests for team_intelligence — team analytics aggregation.

use crate::team_intelligence::*;
use crate::team_sync_types::*;

/// Helper: create an in-memory DB with all required team sync tables.
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

        CREATE TABLE team_sync_log (
            relay_seq   INTEGER NOT NULL,
            team_id     TEXT NOT NULL,
            client_id   TEXT NOT NULL,
            encrypted   BLOB NOT NULL,
            received_at INTEGER NOT NULL DEFAULT (unixepoch()),
            applied     INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (relay_seq, team_id)
        );

        CREATE TABLE team_sync_state (
            team_id         TEXT PRIMARY KEY,
            last_relay_seq  INTEGER NOT NULL DEFAULT 0,
            last_sync_at    INTEGER
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

/// Helper: insert a member into the cache.
fn insert_member(conn: &rusqlite::Connection, team_id: &str, client_id: &str, name: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO team_members_cache (team_id, client_id, display_name, role, last_seen)
         VALUES (?1, ?2, ?3, 'member', datetime('now'))",
        rusqlite::params![team_id, client_id, name],
    )
    .unwrap();
}

/// Helper: insert a ShareDnaSummary entry into team_sync_log as an applied entry.
fn insert_dna_summary(
    conn: &rusqlite::Connection,
    relay_seq: i64,
    team_id: &str,
    client_id: &str,
    primary_stack: Vec<&str>,
    interests: Vec<&str>,
    blind_spots: Vec<&str>,
) {
    let entry = TeamMetadataEntry {
        entry_id: format!("e-dna-{}", relay_seq),
        client_id: client_id.to_string(),
        hlc_timestamp: relay_seq as u64 * 1000,
        operation: TeamOp::ShareDnaSummary {
            primary_stack: primary_stack.iter().map(|s| s.to_string()).collect(),
            interests: interests.iter().map(|s| s.to_string()).collect(),
            blind_spots: blind_spots.iter().map(|s| s.to_string()).collect(),
            identity_summary: format!("Developer {}", client_id),
        },
    };
    let blob = serde_json::to_vec(&entry).unwrap();

    conn.execute(
        "INSERT INTO team_sync_log (relay_seq, team_id, client_id, encrypted, received_at, applied)
         VALUES (?1, ?2, ?3, ?4, ?1, 1)",
        rusqlite::params![relay_seq, team_id, client_id, blob],
    )
    .unwrap();
}

/// Helper: insert a ShareSignal entry into team_sync_log as applied.
fn insert_signal(
    conn: &rusqlite::Connection,
    relay_seq: i64,
    team_id: &str,
    client_id: &str,
    signal_id: &str,
    chain_name: &str,
    priority: &str,
    topics: Vec<&str>,
    received_at: i64,
) {
    let entry = TeamMetadataEntry {
        entry_id: format!("e-sig-{}", relay_seq),
        client_id: client_id.to_string(),
        hlc_timestamp: relay_seq as u64 * 1000,
        operation: TeamOp::ShareSignal {
            signal_id: signal_id.to_string(),
            chain_name: chain_name.to_string(),
            priority: priority.to_string(),
            tech_topics: topics.iter().map(|s| s.to_string()).collect(),
            suggested_action: format!("Review {}", chain_name),
        },
    };
    let blob = serde_json::to_vec(&entry).unwrap();

    conn.execute(
        "INSERT INTO team_sync_log (relay_seq, team_id, client_id, encrypted, received_at, applied)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        rusqlite::params![relay_seq, team_id, client_id, blob, received_at],
    )
    .unwrap();
}

/// Helper: insert a ResolveSignal entry.
fn insert_resolve_signal(
    conn: &rusqlite::Connection,
    relay_seq: i64,
    team_id: &str,
    client_id: &str,
    signal_id: &str,
) {
    let entry = TeamMetadataEntry {
        entry_id: format!("e-resolve-{}", relay_seq),
        client_id: client_id.to_string(),
        hlc_timestamp: relay_seq as u64 * 1000,
        operation: TeamOp::ResolveSignal {
            signal_id: signal_id.to_string(),
            resolution_notes: "Fixed".to_string(),
        },
    };
    let blob = serde_json::to_vec(&entry).unwrap();

    conn.execute(
        "INSERT INTO team_sync_log (relay_seq, team_id, client_id, encrypted, received_at, applied)
         VALUES (?1, ?2, ?3, ?4, ?1, 1)",
        rusqlite::params![relay_seq, team_id, client_id, blob],
    )
    .unwrap();
}

// ============================================================================
// Tests: Empty Team
// ============================================================================

#[test]
fn test_empty_team_returns_empty_profile() {
    let conn = setup_test_db();
    let team_id = "team-empty";

    let profile = get_team_profile(&conn, team_id).unwrap();

    assert_eq!(profile.team_id, team_id);
    assert_eq!(profile.member_count, 0);
    assert!(profile.collective_stack.is_empty());
    assert!(profile.blind_spots.is_empty());
    assert!(profile.overlap_zones.is_empty());
    assert!(profile.unique_strengths.is_empty());
    assert!((profile.stack_coverage - 1.0).abs() < f32::EPSILON); // No adjacencies = 100%
    assert!(!profile.generated_at.is_empty());
}

#[test]
fn test_empty_team_signals_returns_empty() {
    let conn = setup_test_db();
    let signals = get_team_signal_summary(&conn, "team-empty").unwrap();
    assert!(signals.is_empty());
}

#[test]
fn test_empty_team_blind_spots_returns_empty() {
    let conn = setup_test_db();
    let spots = get_team_blind_spots(&conn, "team-empty").unwrap();
    assert!(spots.is_empty());
}

#[test]
fn test_empty_team_bus_factor_returns_empty() {
    let conn = setup_test_db();
    let strengths = get_bus_factor_report(&conn, "team-empty").unwrap();
    assert!(strengths.is_empty());
}

// ============================================================================
// Tests: Collective Stack
// ============================================================================

#[test]
fn test_collective_stack_aggregates_members() {
    let conn = setup_test_db();
    let team_id = "team-stack";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust", "python"],
        vec!["wasm"],
        vec![],
    );
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "bob",
        vec!["rust", "go"],
        vec!["grpc"],
        vec![],
    );

    let profile = get_team_profile(&conn, team_id).unwrap();

    // Rust should appear with both members
    let rust_entry = profile
        .collective_stack
        .iter()
        .find(|e| e.tech == "rust")
        .expect("rust should be in collective stack");
    assert_eq!(rust_entry.members.len(), 2);
    assert!((rust_entry.team_confidence - 1.0).abs() < f32::EPSILON);

    // Python should appear with only Alice
    let python_entry = profile
        .collective_stack
        .iter()
        .find(|e| e.tech == "python")
        .expect("python should be in collective stack");
    assert_eq!(python_entry.members.len(), 1);
    assert!((python_entry.team_confidence - 0.5).abs() < f32::EPSILON);

    // Go should appear with only Bob
    let go_entry = profile
        .collective_stack
        .iter()
        .find(|e| e.tech == "go")
        .expect("go should be in collective stack");
    assert_eq!(go_entry.members.len(), 1);
}

#[test]
fn test_latest_dna_per_member_wins() {
    let conn = setup_test_db();
    let team_id = "team-latest";

    insert_member(&conn, team_id, "alice", "Alice");

    // Older DNA: rust + python
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust", "python"],
        vec![],
        vec![],
    );
    // Newer DNA: rust + go (should replace the older one)
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "alice",
        vec!["rust", "go"],
        vec![],
        vec![],
    );

    let profile = get_team_profile(&conn, team_id).unwrap();

    // Should use the latest DNA (relay_seq=2): rust + go
    let techs: Vec<&str> = profile
        .collective_stack
        .iter()
        .map(|e| e.tech.as_str())
        .collect();
    assert!(techs.contains(&"rust"), "rust should be present");
    assert!(techs.contains(&"go"), "go should be present");
    assert!(
        !techs.contains(&"python"),
        "python should NOT be present (superseded)"
    );
}

// ============================================================================
// Tests: Blind Spots
// ============================================================================

#[test]
fn test_blind_spots_finds_uncovered_adjacent_topics() {
    let conn = setup_test_db();
    let team_id = "team-blind";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    // Alice: rust stack — adjacent includes cargo, wasm, tokio, serde, async, unsafe, ffi
    // Bob: rust stack — same adjacencies
    // Neither covers "tokio" or "wasm" in their interests
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust"],
        vec!["cargo"],
        vec![],
    );
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "bob",
        vec!["rust"],
        vec!["serde"],
        vec![],
    );

    let spots = get_team_blind_spots(&conn, team_id).unwrap();

    // Collect all blind spot topics
    let spot_topics: Vec<&str> = spots.iter().map(|s| s.topic.as_str()).collect();

    // "wasm" is adjacent to rust but neither member covers it
    assert!(
        spot_topics.contains(&"wasm"),
        "wasm should be a blind spot: {:?}",
        spot_topics
    );

    // "tokio" is adjacent to rust but neither covers it
    assert!(
        spot_topics.contains(&"tokio"),
        "tokio should be a blind spot: {:?}",
        spot_topics
    );

    // "cargo" and "serde" are covered by interests, should NOT be blind spots
    assert!(
        !spot_topics.contains(&"cargo"),
        "cargo is covered by Alice's interests"
    );
    assert!(
        !spot_topics.contains(&"serde"),
        "serde is covered by Bob's interests"
    );

    // Verify blind spots reference the correct parent tech
    for spot in &spots {
        assert!(
            spot.related_to.contains(&"rust".to_string()),
            "blind spot {} should relate to rust",
            spot.topic
        );
    }
}

#[test]
fn test_blind_spots_severity_scales_with_member_count() {
    let conn = setup_test_db();
    let team_id = "team-severity";

    // 3 members all have "docker" — adjacent "kubernetes" should be high severity
    insert_member(&conn, team_id, "a", "A");
    insert_member(&conn, team_id, "b", "B");
    insert_member(&conn, team_id, "c", "C");

    insert_dna_summary(&conn, 1, team_id, "a", vec!["docker"], vec![], vec![]);
    insert_dna_summary(&conn, 2, team_id, "b", vec!["docker"], vec![], vec![]);
    insert_dna_summary(&conn, 3, team_id, "c", vec!["docker"], vec![], vec![]);

    let spots = get_team_blind_spots(&conn, team_id).unwrap();

    let k8s = spots.iter().find(|s| s.topic == "kubernetes");
    assert!(
        k8s.is_some(),
        "kubernetes should be a blind spot for docker team"
    );
    assert_eq!(k8s.unwrap().severity, "high", "3+ members → high severity");
}

// ============================================================================
// Tests: Bus Factor
// ============================================================================

#[test]
fn test_bus_factor_flags_single_expert_tech() {
    let conn = setup_test_db();
    let team_id = "team-bus";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");
    insert_member(&conn, team_id, "charlie", "Charlie");

    // Alice: rust + kubernetes (only she knows kubernetes)
    // Bob: rust + python
    // Charlie: rust + python
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust", "kubernetes"],
        vec![],
        vec![],
    );
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "bob",
        vec!["rust", "python"],
        vec![],
        vec![],
    );
    insert_dna_summary(
        &conn,
        3,
        team_id,
        "charlie",
        vec!["rust", "python"],
        vec![],
        vec![],
    );

    let report = get_bus_factor_report(&conn, team_id).unwrap();

    // kubernetes is only known by Alice → bus factor risk
    let k8s = report.iter().find(|s| s.tech == "kubernetes");
    assert!(
        k8s.is_some(),
        "kubernetes should be flagged as single-expert"
    );
    assert_eq!(k8s.unwrap().sole_expert, "Alice");
    assert_eq!(k8s.unwrap().risk_level, "high");

    // rust is known by all 3 → NOT a bus factor risk
    assert!(
        report.iter().find(|s| s.tech == "rust").is_none(),
        "rust is known by 3 members, not a bus factor"
    );

    // python is known by 2 → NOT a bus factor risk
    assert!(
        report.iter().find(|s| s.tech == "python").is_none(),
        "python is known by 2 members, not a bus factor"
    );
}

#[test]
fn test_bus_factor_empty_when_all_shared() {
    let conn = setup_test_db();
    let team_id = "team-shared";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust", "python"],
        vec![],
        vec![],
    );
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "bob",
        vec!["rust", "python"],
        vec![],
        vec![],
    );

    let report = get_bus_factor_report(&conn, team_id).unwrap();
    assert!(
        report.is_empty(),
        "No bus factor risks when all tech is shared"
    );
}

// ============================================================================
// Tests: Overlap Zones
// ============================================================================

#[test]
fn test_overlap_zones_requires_three_plus_members() {
    let conn = setup_test_db();
    let team_id = "team-overlap";

    insert_member(&conn, team_id, "a", "Alice");
    insert_member(&conn, team_id, "b", "Bob");
    insert_member(&conn, team_id, "c", "Charlie");
    insert_member(&conn, team_id, "d", "Diana");

    // All 4 have rust, 3 have python, only 2 have go
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "a",
        vec!["rust", "python", "go"],
        vec![],
        vec![],
    );
    insert_dna_summary(
        &conn,
        2,
        team_id,
        "b",
        vec!["rust", "python", "go"],
        vec![],
        vec![],
    );
    insert_dna_summary(
        &conn,
        3,
        team_id,
        "c",
        vec!["rust", "python"],
        vec![],
        vec![],
    );
    insert_dna_summary(&conn, 4, team_id, "d", vec!["rust"], vec![], vec![]);

    let profile = get_team_profile(&conn, team_id).unwrap();
    let overlap_topics: Vec<&str> = profile
        .overlap_zones
        .iter()
        .map(|z| z.topic.as_str())
        .collect();

    assert!(
        overlap_topics.contains(&"rust"),
        "rust overlaps with 4 members"
    );
    assert!(
        overlap_topics.contains(&"python"),
        "python overlaps with 3 members"
    );
    assert!(
        !overlap_topics.contains(&"go"),
        "go only has 2 members, below threshold"
    );

    // Verify rust has the highest count
    let rust_zone = profile
        .overlap_zones
        .iter()
        .find(|z| z.topic == "rust")
        .unwrap();
    assert_eq!(rust_zone.member_count, 4);
}

// ============================================================================
// Tests: Signal Merging
// ============================================================================

#[test]
fn test_signal_merging_increases_confidence() {
    let conn = setup_test_db();
    let team_id = "team-signals";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    let base_time: i64 = 1700000000; // Fixed base timestamp

    // Alice detects a signal
    insert_signal(
        &conn,
        1,
        team_id,
        "alice",
        "sig-1",
        "CVE-2025-001",
        "high",
        vec!["rust", "openssl"],
        base_time,
    );

    // Bob detects the same chain 12 hours later (within 48h window)
    insert_signal(
        &conn,
        2,
        team_id,
        "bob",
        "sig-2",
        "CVE-2025-001",
        "high",
        vec!["rust", "tls"],
        base_time + 43200,
    );

    let signals = get_team_signal_summary(&conn, team_id).unwrap();

    // Should merge into one signal summary
    let cve_signal = signals.iter().find(|s| s.chain_name == "CVE-2025-001");
    assert!(cve_signal.is_some(), "CVE signal should exist");

    let cve = cve_signal.unwrap();
    assert_eq!(cve.detected_by.len(), 2, "Two members detected the signal");

    // Single detection = 0.5, two detections = 0.5 + 0.15 = 0.65
    assert!(
        (cve.team_confidence - 0.65).abs() < f32::EPSILON,
        "team_confidence should be 0.65 for 2 detectors, got {}",
        cve.team_confidence
    );

    // Tech topics should be merged
    assert!(cve.tech_topics.contains(&"rust".to_string()));
    assert!(cve.tech_topics.contains(&"openssl".to_string()));
    assert!(cve.tech_topics.contains(&"tls".to_string()));
}

#[test]
fn test_single_signal_has_base_confidence() {
    let conn = setup_test_db();
    let team_id = "team-single-sig";

    insert_member(&conn, team_id, "alice", "Alice");

    insert_signal(
        &conn,
        1,
        team_id,
        "alice",
        "sig-1",
        "Dep Update",
        "medium",
        vec!["npm"],
        1700000000,
    );

    let signals = get_team_signal_summary(&conn, team_id).unwrap();
    assert_eq!(signals.len(), 1);
    assert!(
        (signals[0].team_confidence - 0.5).abs() < f32::EPSILON,
        "Single detection should have base confidence 0.5, got {}",
        signals[0].team_confidence
    );
}

#[test]
fn test_signal_outside_merge_window_not_merged() {
    let conn = setup_test_db();
    let team_id = "team-no-merge";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    let base_time: i64 = 1700000000;

    // Alice detects a signal
    insert_signal(
        &conn,
        1,
        team_id,
        "alice",
        "sig-1",
        "Tech Trend",
        "low",
        vec!["wasm"],
        base_time,
    );

    // Bob detects same chain but 3 days later (outside 48h window)
    insert_signal(
        &conn,
        2,
        team_id,
        "bob",
        "sig-2",
        "Tech Trend",
        "low",
        vec!["wasm"],
        base_time + 259200,
    );

    let signals = get_team_signal_summary(&conn, team_id).unwrap();

    // Should have two separate entries since they're outside the merge window
    assert_eq!(
        signals.len(),
        2,
        "Signals outside 48h window should not merge"
    );
}

#[test]
fn test_resolved_signal_marked() {
    let conn = setup_test_db();
    let team_id = "team-resolve";

    insert_member(&conn, team_id, "alice", "Alice");

    insert_signal(
        &conn,
        1,
        team_id,
        "alice",
        "sig-resolved",
        "Old Chain",
        "high",
        vec!["rust"],
        1700000000,
    );
    insert_resolve_signal(&conn, 2, team_id, "alice", "sig-resolved");

    let signals = get_team_signal_summary(&conn, team_id).unwrap();
    assert_eq!(signals.len(), 1);
    assert!(signals[0].resolved, "Signal should be marked as resolved");
}

#[test]
fn test_signal_priority_upgrade_on_merge() {
    let conn = setup_test_db();
    let team_id = "team-priority";

    insert_member(&conn, team_id, "alice", "Alice");
    insert_member(&conn, team_id, "bob", "Bob");

    let base_time: i64 = 1700000000;

    // Alice flags it as medium
    insert_signal(
        &conn,
        1,
        team_id,
        "alice",
        "sig-1",
        "Security Issue",
        "medium",
        vec!["openssl"],
        base_time,
    );

    // Bob flags same chain as critical (higher priority)
    insert_signal(
        &conn,
        2,
        team_id,
        "bob",
        "sig-2",
        "Security Issue",
        "critical",
        vec!["openssl"],
        base_time + 3600,
    );

    let signals = get_team_signal_summary(&conn, team_id).unwrap();
    let sec = signals
        .iter()
        .find(|s| s.chain_name == "Security Issue")
        .unwrap();
    assert_eq!(
        sec.priority, "critical",
        "Priority should be upgraded to the highest"
    );
}

// ============================================================================
// Tests: Stack Coverage
// ============================================================================

#[test]
fn test_stack_coverage_partial() {
    let conn = setup_test_db();
    let team_id = "team-coverage";

    insert_member(&conn, team_id, "alice", "Alice");

    // Alice has rust stack. Adjacent: cargo, wasm, tokio, serde, async, unsafe, ffi (7 topics)
    // She covers cargo and serde in interests → 2/7 covered
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["rust"],
        vec!["cargo", "serde"],
        vec![],
    );

    let profile = get_team_profile(&conn, team_id).unwrap();
    // The primary stack "rust" itself doesn't count as adjacent (it's already covered implicitly).
    // From the adjacency map: cargo, wasm, tokio, serde, async, unsafe, ffi
    // Alice's interests cover: cargo, serde → 2 out of 7
    let expected_coverage = 2.0 / 7.0;
    assert!(
        (profile.stack_coverage - expected_coverage).abs() < 0.01,
        "Stack coverage should be ~{:.2}, got {:.2}",
        expected_coverage,
        profile.stack_coverage
    );
}

#[test]
fn test_stack_coverage_full_when_no_adjacencies() {
    let conn = setup_test_db();
    let team_id = "team-no-adj";

    insert_member(&conn, team_id, "alice", "Alice");

    // "obscuretech" has no entry in the adjacency map → coverage = 1.0
    insert_dna_summary(
        &conn,
        1,
        team_id,
        "alice",
        vec!["obscuretech"],
        vec![],
        vec![],
    );

    let profile = get_team_profile(&conn, team_id).unwrap();
    assert!(
        (profile.stack_coverage - 1.0).abs() < f32::EPSILON,
        "No adjacencies → 100% coverage"
    );
}

// ============================================================================
// Tests: Three-detector confidence
// ============================================================================

#[test]
fn test_three_detector_confidence() {
    let conn = setup_test_db();
    let team_id = "team-3detect";

    insert_member(&conn, team_id, "a", "Alice");
    insert_member(&conn, team_id, "b", "Bob");
    insert_member(&conn, team_id, "c", "Charlie");

    let base_time: i64 = 1700000000;

    insert_signal(
        &conn,
        1,
        team_id,
        "a",
        "s1",
        "Trend X",
        "medium",
        vec!["x"],
        base_time,
    );
    insert_signal(
        &conn,
        2,
        team_id,
        "b",
        "s2",
        "Trend X",
        "medium",
        vec!["x"],
        base_time + 3600,
    );
    insert_signal(
        &conn,
        3,
        team_id,
        "c",
        "s3",
        "Trend X",
        "medium",
        vec!["x"],
        base_time + 7200,
    );

    let signals = get_team_signal_summary(&conn, team_id).unwrap();
    let trend = signals.iter().find(|s| s.chain_name == "Trend X").unwrap();

    assert_eq!(trend.detected_by.len(), 3);
    // 0.5 + (2 * 0.15) = 0.80
    assert!(
        (trend.team_confidence - 0.80).abs() < f32::EPSILON,
        "3 detectors → 0.80 confidence, got {}",
        trend.team_confidence
    );
}

// ============================================================================
// Tests: Member count
// ============================================================================

#[test]
fn test_member_count_from_cache() {
    let conn = setup_test_db();
    let team_id = "team-count";

    insert_member(&conn, team_id, "a", "Alice");
    insert_member(&conn, team_id, "b", "Bob");
    insert_member(&conn, team_id, "c", "Charlie");

    let profile = get_team_profile(&conn, team_id).unwrap();
    assert_eq!(profile.member_count, 3);
}
