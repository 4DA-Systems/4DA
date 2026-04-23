// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL DEFAULT '',
            url TEXT,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            content_hash TEXT NOT NULL DEFAULT '',
            content_type TEXT DEFAULT NULL,
            embedding BLOB,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen TEXT NOT NULL DEFAULT (datetime('now')),
            summary TEXT,
            embedding_status TEXT DEFAULT 'pending',
            embed_text TEXT
        );
        CREATE TABLE feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER NOT NULL,
            relevant INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (source_item_id) REFERENCES source_items(id)
        );
        CREATE TABLE digested_intelligence (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            digest_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            sample_size INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT,
            superseded_by INTEGER
        );",
    )
    .expect("create tables");
    conn
}

/// Helper: insert a source item and return its id.
fn insert_item(
    conn: &Connection,
    source_type: &str,
    title: &str,
    content_type: Option<&str>,
) -> i64 {
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content_type)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            source_type,
            format!("id_{}", title.len()),
            title,
            content_type
        ],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Helper: insert feedback for an item.
fn insert_feedback(conn: &Connection, item_id: i64, relevant: bool) {
    conn.execute(
        "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
        params![item_id, relevant as i64],
    )
    .unwrap();
}

#[test]
fn test_detect_archetypes_empty() {
    let conn = setup_test_db();
    let archetypes = detect_archetypes(&conn, 90);
    assert!(archetypes.is_empty());
}

#[test]
fn test_extract_keywords() {
    let keywords = extract_keywords("How to deploy Kubernetes clusters with Terraform");
    assert!(keywords.contains(&"deploy".to_string()));
    assert!(keywords.contains(&"kubernetes".to_string()));
    assert!(keywords.contains(&"terraform".to_string()));
    assert!(!keywords.contains(&"how".to_string()));
    assert!(!keywords.contains(&"with".to_string()));
}

#[test]
fn test_extract_keywords_max_five() {
    let keywords =
        extract_keywords("alpha bravo charlie delta echo foxtrot golf hotel india juliet");
    assert!(keywords.len() <= 5);
}

#[test]
fn test_detect_archetypes_high_dismissal() {
    let conn = setup_test_db();

    // 10 items about "kubernetes" from "hackernews" — dismiss 9 (90%)
    for i in 0..10 {
        let id = insert_item(
            &conn,
            "hackernews",
            &format!("Kubernetes security vulnerability report {i}"),
            Some("security_advisory"),
        );
        insert_feedback(&conn, id, i == 0);
    }

    let archetypes = detect_archetypes(&conn, 90);
    let k8s = archetypes.iter().find(|a| a.topic == "kubernetes");
    assert!(k8s.is_some(), "Expected archetype for 'kubernetes'");
    let k8s = k8s.unwrap();
    assert_eq!(k8s.source_type, "hackernews");
    assert_eq!(k8s.content_type, "security_advisory");
    assert!((k8s.dismissal_rate - 0.9).abs() < 0.01);
    assert_eq!(k8s.sample_size, 10);
    assert!(k8s.suggested_penalty > 0.0);
    assert!(k8s.suggested_penalty <= MAX_PENALTY);
}

#[test]
fn test_detect_archetypes_below_threshold() {
    let conn = setup_test_db();

    // 10 items about "golang" — dismiss only 5 (50%, below 70% threshold)
    for i in 0..10 {
        let id = insert_item(
            &conn,
            "reddit",
            &format!("Golang performance optimization tips {i}"),
            Some("discussion"),
        );
        insert_feedback(&conn, id, i < 5);
    }

    let archetypes = detect_archetypes(&conn, 90);
    let golang = archetypes.iter().find(|a| a.topic == "golang");
    assert!(
        golang.is_none(),
        "50% dismissal should not trigger archetype"
    );
}

#[test]
fn test_detect_archetypes_insufficient_samples() {
    let conn = setup_test_db();

    // Only 4 items (below MIN_SAMPLE_SIZE of 8) — all dismissed
    for i in 0..4 {
        let id = insert_item(
            &conn,
            "hackernews",
            &format!("Terraform cloud migration story {i}"),
            Some("discussion"),
        );
        insert_feedback(&conn, id, false);
    }

    let archetypes = detect_archetypes(&conn, 90);
    let terraform = archetypes.iter().find(|a| a.topic == "terraform");
    assert!(
        terraform.is_none(),
        "4 samples should not trigger archetype"
    );
}

#[test]
fn test_detect_archetypes_no_content_type_column() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL DEFAULT '',
            url TEXT, title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            content_hash TEXT NOT NULL DEFAULT '',
            embedding BLOB,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen TEXT NOT NULL DEFAULT (datetime('now')),
            summary TEXT, embedding_status TEXT DEFAULT 'pending', embed_text TEXT
        );
        CREATE TABLE feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER NOT NULL,
            relevant INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (source_item_id) REFERENCES source_items(id)
        );
        CREATE TABLE digested_intelligence (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            digest_type TEXT NOT NULL, subject TEXT NOT NULL,
            data TEXT NOT NULL, confidence REAL NOT NULL DEFAULT 0.5,
            sample_size INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT, superseded_by INTEGER
        );",
    )
    .expect("create tables without content_type");

    for i in 0..10 {
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, title)
             VALUES ('hackernews', ?1, ?2)",
            params![
                format!("hn_{i}"),
                format!("Docker container orchestration tips {i}")
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 0)",
            params![i + 1],
        )
        .unwrap();
    }

    let archetypes = detect_archetypes(&conn, 90);
    let docker = archetypes.iter().find(|a| a.topic == "docker");
    assert!(
        docker.is_some(),
        "Should detect archetype even without content_type column"
    );
    assert_eq!(docker.unwrap().content_type, "unknown");
}

#[test]
fn test_store_archetypes() {
    let conn = setup_test_db();
    let archetypes = vec![DismissalArchetype {
        archetype_id: "kubernetes:hackernews:security_advisory".to_string(),
        description: "test".to_string(),
        topic: "kubernetes".to_string(),
        source_type: "hackernews".to_string(),
        content_type: "security_advisory".to_string(),
        dismissal_rate: 0.9,
        sample_size: 10,
        suggested_penalty: 0.2,
    }];
    store_archetypes(&conn, &archetypes).expect("store should succeed");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence WHERE digest_type = 'dismissal_archetype'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_store_archetypes_supersedes_previous() {
    let conn = setup_test_db();
    let archetype = DismissalArchetype {
        archetype_id: "kubernetes:hackernews:security_advisory".to_string(),
        description: "test".to_string(),
        topic: "kubernetes".to_string(),
        source_type: "hackernews".to_string(),
        content_type: "security_advisory".to_string(),
        dismissal_rate: 0.8,
        sample_size: 10,
        suggested_penalty: 0.15,
    };
    store_archetypes(&conn, &[archetype.clone()]).expect("store v1");

    let updated = DismissalArchetype {
        dismissal_rate: 0.95,
        suggested_penalty: 0.22,
        ..archetype
    };
    store_archetypes(&conn, &[updated]).expect("store v2");

    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence WHERE digest_type = 'dismissal_archetype'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(total, 2);

    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence
             WHERE digest_type = 'dismissal_archetype' AND superseded_by IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active, 1);
}

#[test]
fn test_load_archetype_penalties_empty() {
    let conn = setup_test_db();
    let loaded = load_archetype_penalties(&conn);
    assert!(loaded.is_empty());
}

#[test]
fn test_load_archetype_penalties_roundtrip() {
    let conn = setup_test_db();
    let archetypes = vec![
        DismissalArchetype {
            archetype_id: "kubernetes:hackernews:security_advisory".to_string(),
            description: "test".to_string(),
            topic: "kubernetes".to_string(),
            source_type: "hackernews".to_string(),
            content_type: "security_advisory".to_string(),
            dismissal_rate: 0.9,
            sample_size: 20,
            suggested_penalty: 0.2,
        },
        DismissalArchetype {
            archetype_id: "docker:reddit:discussion".to_string(),
            description: "test".to_string(),
            topic: "docker".to_string(),
            source_type: "reddit".to_string(),
            content_type: "discussion".to_string(),
            dismissal_rate: 0.75,
            sample_size: 12,
            suggested_penalty: 0.125,
        },
    ];
    store_archetypes(&conn, &archetypes).expect("store");
    let loaded = load_archetype_penalties(&conn);

    assert_eq!(loaded.len(), 2);
    assert!((loaded["kubernetes:hackernews:security_advisory"] - 0.2).abs() < 0.01);
    assert!((loaded["docker:reddit:discussion"] - 0.125).abs() < 0.01);
}

#[test]
fn test_archetype_penalty_for_item_match() {
    let mut penalties = HashMap::new();
    penalties.insert(
        "kubernetes:hackernews:security_advisory".to_string(),
        0.2_f32,
    );
    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes cluster vulnerability found",
        Some("security_advisory"),
    );
    assert!((penalty - 0.2).abs() < 0.01);
}

#[test]
fn test_archetype_penalty_for_item_no_match() {
    let mut penalties = HashMap::new();
    penalties.insert(
        "kubernetes:hackernews:security_advisory".to_string(),
        0.2_f32,
    );
    let penalty = archetype_penalty_for_item(
        &penalties,
        "reddit",
        "Kubernetes cluster deployment guide",
        Some("discussion"),
    );
    assert!((penalty - 0.0).abs() < 0.01);
}

#[test]
fn test_archetype_penalty_for_item_unknown_fallback() {
    let mut penalties = HashMap::new();
    penalties.insert("kubernetes:hackernews:unknown".to_string(), 0.18_f32);
    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes networking deep dive",
        Some("discussion"),
    );
    assert!((penalty - 0.18).abs() < 0.01);
}

#[test]
fn test_archetype_penalty_for_item_empty_penalties() {
    let penalties = HashMap::new();
    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes deployment strategies",
        Some("discussion"),
    );
    assert!((penalty - 0.0).abs() < f32::EPSILON);
}

#[test]
fn test_archetype_penalty_for_item_max_not_sum() {
    let mut penalties = HashMap::new();
    penalties.insert("kubernetes:hackernews:discussion".to_string(), 0.1_f32);
    penalties.insert("cluster:hackernews:discussion".to_string(), 0.2_f32);
    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes cluster management best practices",
        Some("discussion"),
    );
    assert!((penalty - 0.2).abs() < 0.01);
}

#[test]
fn test_archetype_penalty_for_item_none_content_type() {
    let mut penalties = HashMap::new();
    penalties.insert("kubernetes:hackernews:unknown".to_string(), 0.15_f32);
    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes scaling strategies",
        None,
    );
    assert!((penalty - 0.15).abs() < 0.01);
}

#[test]
fn test_suggested_penalty_capped() {
    let penalty = (1.0_f32 - 0.5) * 0.5;
    assert!((penalty - MAX_PENALTY).abs() < f32::EPSILON);
    let capped = ((1.0_f32 - 0.5) * 0.5).min(MAX_PENALTY);
    assert!(capped <= MAX_PENALTY);
}

#[test]
fn test_detect_and_load_integration() {
    let conn = setup_test_db();

    for i in 0..12 {
        let id = insert_item(
            &conn,
            "hackernews",
            &format!("Kubernetes deployment troubleshooting guide {i}"),
            Some("discussion"),
        );
        insert_feedback(&conn, id, i == 0);
    }

    let archetypes = detect_archetypes(&conn, 90);
    assert!(
        !archetypes.is_empty(),
        "Should detect at least one archetype"
    );

    store_archetypes(&conn, &archetypes).expect("store");

    let penalties = load_archetype_penalties(&conn);
    assert!(!penalties.is_empty(), "Should load stored penalties");

    let penalty = archetype_penalty_for_item(
        &penalties,
        "hackernews",
        "Kubernetes production readiness checklist",
        Some("discussion"),
    );
    assert!(penalty > 0.0, "Should apply penalty to matching item");
    assert!(penalty <= MAX_PENALTY, "Penalty should not exceed cap");
}
