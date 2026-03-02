use super::*;

// ---- Helper: minimal in-memory DB with source_items + temporal_events ----

fn setup_test_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "
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
            );
            ",
    )
    .expect("create tables");
    conn
}

/// Insert a source item with a specific created_at date for testing.
fn insert_item(
    conn: &rusqlite::Connection,
    title: &str,
    source_type: &str,
    content: &str,
    created_at: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, zeroblob(0), ?6)",
        rusqlite::params![
            source_type,
            format!("{}_{}", title, created_at),
            title,
            content,
            format!("hash_{}", title),
            created_at,
        ],
    )
    .expect("insert source item");
    conn.last_insert_rowid()
}

// ---- classify_chain_signal tests ----

#[test]
fn classify_security_keywords() {
    assert_eq!(
        classify_chain_signal("CVE-2026-1234 found in openssl"),
        "security_alert"
    );
    assert_eq!(
        classify_chain_signal("Critical vulnerability in log4j"),
        "security_alert"
    );
    assert_eq!(
        classify_chain_signal("New Security patch released"),
        "security_alert"
    );
    assert_eq!(
        classify_chain_signal("Remote exploit discovered"),
        "security_alert"
    );
}

#[test]
fn classify_breaking_change_keywords() {
    assert_eq!(
        classify_chain_signal("Breaking change in React 20"),
        "breaking_change"
    );
    assert_eq!(
        classify_chain_signal("API deprecated in v5"),
        "breaking_change"
    );
    assert_eq!(
        classify_chain_signal("Feature removed from Node 22"),
        "breaking_change"
    );
    assert_eq!(
        classify_chain_signal("Python 2.7 EOL reminder"),
        "breaking_change"
    );
}

#[test]
fn classify_tool_discovery_keywords() {
    assert_eq!(
        classify_chain_signal("Bun v2 released today"),
        "tool_discovery"
    );
    assert_eq!(
        classify_chain_signal("Major update to VS Code"),
        "tool_discovery"
    );
    assert_eq!(classify_chain_signal("Deno v3 is here"), "tool_discovery");
    assert_eq!(
        classify_chain_signal("New framework launch"),
        "tool_discovery"
    );
}

#[test]
fn classify_tech_trend_keywords() {
    assert_eq!(
        classify_chain_signal("WebAssembly adoption growing fast"),
        "tech_trend"
    );
    assert_eq!(classify_chain_signal("Rust trend in 2026"), "tech_trend");
    assert_eq!(
        classify_chain_signal("Popular new pattern emerges"),
        "tech_trend"
    );
}

#[test]
fn classify_defaults_to_learning() {
    assert_eq!(
        classify_chain_signal("How to write better tests"),
        "learning"
    );
    assert_eq!(
        classify_chain_signal("Understanding async/await"),
        "learning"
    );
    assert_eq!(classify_chain_signal(""), "learning");
}

// ---- priority_rank tests ----

#[test]
fn priority_rank_ordering() {
    assert_eq!(priority_rank("critical"), 0);
    assert_eq!(priority_rank("high"), 1);
    assert_eq!(priority_rank("medium"), 2);
    assert_eq!(priority_rank("low"), 3);
    // Unknown priorities fall to lowest
    assert_eq!(priority_rank("unknown"), 3);
    assert_eq!(priority_rank(""), 3);
}

// ---- Serde roundtrip tests ----

#[test]
fn chain_resolution_serde_roundtrip() {
    let variants = vec![
        (ChainResolution::Open, "\"open\""),
        (ChainResolution::Resolved, "\"resolved\""),
        (ChainResolution::Expired, "\"expired\""),
        (ChainResolution::Snoozed, "\"snoozed\""),
    ];
    for (variant, expected_json) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_json);
        let deserialized: ChainResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn signal_chain_serde_roundtrip() {
    let chain = SignalChain {
        id: "chain_rust_2026-02-25".to_string(),
        chain_name: "rust signal chain (3 events)".to_string(),
        links: vec![
            ChainLink {
                signal_type: "security_alert".to_string(),
                source_item_id: 1,
                title: "CVE in rustls".to_string(),
                timestamp: "2026-02-25T10:00:00Z".to_string(),
                description: "security_alert via hackernews".to_string(),
            },
            ChainLink {
                signal_type: "tool_discovery".to_string(),
                source_item_id: 2,
                title: "Rust 1.85 released".to_string(),
                timestamp: "2026-02-26T10:00:00Z".to_string(),
                description: "tool_discovery via reddit".to_string(),
            },
        ],
        overall_priority: "critical".to_string(),
        resolution: ChainResolution::Open,
        suggested_action: "Review security implications for rust in your projects".to_string(),
        created_at: "2026-02-28T00:00:00Z".to_string(),
        updated_at: "2026-02-28T00:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&chain).unwrap();
    let deserialized: SignalChain = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "chain_rust_2026-02-25");
    assert_eq!(deserialized.links.len(), 2);
    assert_eq!(deserialized.overall_priority, "critical");
    assert_eq!(deserialized.resolution, ChainResolution::Open);
    assert_eq!(deserialized.links[0].signal_type, "security_alert");
    assert_eq!(deserialized.links[1].source_item_id, 2);
}

// ---- detect_chains tests ----

#[test]
fn detect_chains_empty_db_returns_empty() {
    let conn = setup_test_db();
    let chains = detect_chains(&conn).unwrap();
    assert!(chains.is_empty());
}

#[test]
fn detect_chains_single_day_no_chain() {
    let conn = setup_test_db();
    // Two items about Rust but on the same day — should NOT form a chain
    let today = chrono::Utc::now().format("%Y-%m-%d 12:00:00").to_string();
    insert_item(
        &conn,
        "Rust CVE-2026-001",
        "hackernews",
        "rust security issue",
        &today,
    );
    insert_item(
        &conn,
        "Rust update released",
        "reddit",
        "rust new version",
        &today,
    );

    let chains = detect_chains(&conn).unwrap();
    // Items on the same day won't form chains (need 2+ different days)
    assert!(
        chains.is_empty(),
        "Items on the same day should not form a chain"
    );
}

#[test]
fn detect_chains_multi_day_forms_chain() {
    let conn = setup_test_db();
    // Two items about "rust" spanning two different days within the last 7 days
    let day1 = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(2))
        .unwrap()
        .format("%Y-%m-%d 10:00:00")
        .to_string();
    let day2 = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(1))
        .unwrap()
        .format("%Y-%m-%d 10:00:00")
        .to_string();

    insert_item(
        &conn,
        "Rust CVE-2026-001",
        "hackernews",
        "rust security",
        &day1,
    );
    insert_item(
        &conn,
        "Rust update released",
        "reddit",
        "rust new version",
        &day2,
    );

    let chains = detect_chains(&conn).unwrap();
    assert!(
        !chains.is_empty(),
        "Items spanning 2+ days about the same topic should form a chain"
    );
    // The chain should be about "rust"
    let rust_chain = chains.iter().find(|c| c.chain_name.contains("rust"));
    assert!(
        rust_chain.is_some(),
        "Should have a chain related to 'rust'"
    );
}

#[test]
fn detect_chains_priority_sorting() {
    let conn = setup_test_db();
    let day1 = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(3))
        .unwrap()
        .format("%Y-%m-%d 10:00:00")
        .to_string();
    let day2 = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(1))
        .unwrap()
        .format("%Y-%m-%d 10:00:00")
        .to_string();

    // Security chain (critical) about "python"
    insert_item(
        &conn,
        "Python CVE-2026-999",
        "hackernews",
        "python vulnerability",
        &day1,
    );
    insert_item(
        &conn,
        "Python security patch",
        "reddit",
        "python fix",
        &day2,
    );

    // Learning chain (low) about "golang"
    insert_item(
        &conn,
        "Learning golang basics",
        "hackernews",
        "golang tutorial",
        &day1,
    );
    insert_item(
        &conn,
        "Golang for beginners",
        "reddit",
        "golang intro guide",
        &day2,
    );

    let chains = detect_chains(&conn).unwrap();
    if chains.len() >= 2 {
        // Critical should sort before low
        let first_rank = priority_rank(&chains[0].overall_priority);
        let last_rank = priority_rank(&chains[chains.len() - 1].overall_priority);
        assert!(
            first_rank <= last_rank,
            "Chains should be sorted by priority (critical first)"
        );
    }
}

// ---- resolve_chain test ----

#[test]
fn resolve_chain_records_temporal_event() {
    let conn = setup_test_db();
    resolve_chain(&conn, "chain_test_123", ChainResolution::Resolved).unwrap();

    // Verify a temporal event was recorded
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'chain_resolved' AND subject = 'chain_test_123'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "resolve_chain should record exactly one temporal event"
    );

    // Verify the resolution data is stored in the event
    let data: String = conn
        .query_row(
            "SELECT data FROM temporal_events WHERE subject = 'chain_test_123'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(parsed["chain_id"], "chain_test_123");
    assert_eq!(parsed["resolution"], "resolved");
}
