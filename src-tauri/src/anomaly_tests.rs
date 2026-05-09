// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // Create minimal schema needed for tests
    conn.execute_batch(
        r#"
            CREATE TABLE file_signals (
                id INTEGER PRIMARY KEY,
                path TEXT,
                change_type TEXT,
                timestamp TEXT,
                extracted_topics TEXT,
                content_hash TEXT,
                processed INTEGER DEFAULT 0
            );
            CREATE TABLE topic_affinities (
                id INTEGER PRIMARY KEY,
                topic TEXT UNIQUE,
                affinity_score REAL,
                confidence REAL,
                positive_signals INTEGER DEFAULT 0,
                negative_signals INTEGER DEFAULT 0,
                total_exposures INTEGER DEFAULT 0,
                last_interaction TEXT,
                last_decay_at TEXT
            );
            CREATE TABLE anti_topics (
                id INTEGER PRIMARY KEY,
                topic TEXT UNIQUE,
                confidence REAL,
                rejection_count INTEGER DEFAULT 0,
                auto_detected INTEGER DEFAULT 1,
                user_confirmed INTEGER DEFAULT 0,
                first_rejection TEXT,
                last_rejection TEXT
            );
            CREATE TABLE interactions (
                id INTEGER PRIMARY KEY,
                item_id INTEGER,
                action_type TEXT,
                action_data TEXT,
                item_topics TEXT,
                item_source TEXT,
                signal_strength REAL,
                timestamp TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE anomalies (
                id INTEGER PRIMARY KEY,
                anomaly_type TEXT NOT NULL,
                topic TEXT,
                description TEXT NOT NULL,
                confidence REAL DEFAULT 0.5,
                severity TEXT DEFAULT 'medium',
                evidence TEXT DEFAULT '[]',
                detected_at TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_anomalies_resolved ON anomalies(resolved);
            CREATE INDEX IF NOT EXISTS idx_anomalies_type ON anomalies(anomaly_type);
            "#,
    )
    .unwrap();
    conn
}

#[test]
fn test_detect_stale_data_no_signals() {
    let conn = setup_test_db();
    // No file signals at all = stale
    let anomalies = detect_stale_data(&conn).unwrap();
    assert!(
        !anomalies.is_empty(),
        "Should detect stale data when no signals exist"
    );
    assert_eq!(anomalies[0].anomaly_type, AnomalyType::StaleData);
}

#[test]
fn test_detect_stale_data_recent_signals() {
    let conn = setup_test_db();
    // Insert a recent file signal
    conn.execute(
        "INSERT INTO file_signals (path, change_type, timestamp) VALUES ('test.rs', 'modified', datetime('now'))",
        [],
    )
    .unwrap();
    let anomalies = detect_stale_data(&conn).unwrap();
    assert!(
        anomalies.is_empty(),
        "Should not detect stale data when recent signals exist"
    );
}

#[test]
fn test_detect_contradiction() {
    let conn = setup_test_db();
    // Insert a topic that's both an affinity AND an anti-topic
    conn.execute(
        "INSERT INTO topic_affinities (topic, affinity_score, confidence, last_interaction, positive_signals) VALUES ('rust', 0.8, 0.9, datetime('now'), 5)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO anti_topics (topic, confidence, rejection_count) VALUES ('rust', 0.7, 3)",
        [],
    )
    .unwrap();
    let anomalies = detect_contradictions(&conn).unwrap();
    assert!(!anomalies.is_empty(), "Should detect contradiction");
    assert_eq!(anomalies[0].anomaly_type, AnomalyType::Contradiction);
    assert_eq!(anomalies[0].topic, Some("rust".to_string()));
}

#[test]
fn test_store_and_retrieve_anomaly() {
    let conn = setup_test_db();
    let anomaly = Anomaly {
        id: None,
        anomaly_type: AnomalyType::StaleData,
        topic: Some("test".to_string()),
        description: "Test anomaly".to_string(),
        confidence: 0.8,
        severity: AnomalySeverity::Medium,
        evidence: vec!["evidence1".to_string()],
        detected_at: chrono::Utc::now().to_rfc3339(),
        resolved: false,
    };
    let id = store_anomaly(&conn, &anomaly).unwrap();
    assert!(id > 0);

    let unresolved = get_unresolved(&conn).unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].description, "Test anomaly");
}

#[test]
fn test_resolve_anomaly() {
    let conn = setup_test_db();
    let anomaly = Anomaly {
        id: None,
        anomaly_type: AnomalyType::Contradiction,
        topic: Some("python".to_string()),
        description: "Contradiction found".to_string(),
        confidence: 0.7,
        severity: AnomalySeverity::High,
        evidence: vec![],
        detected_at: chrono::Utc::now().to_rfc3339(),
        resolved: false,
    };
    let id = store_anomaly(&conn, &anomaly).unwrap();
    resolve_anomaly(&conn, id).unwrap();
    let unresolved = get_unresolved(&conn).unwrap();
    assert_eq!(unresolved.len(), 0);
}

#[test]
fn test_detect_confidence_mismatch() {
    let conn = setup_test_db();
    // High confidence but low interaction count
    conn.execute(
        "INSERT INTO topic_affinities (topic, affinity_score, confidence, last_interaction, positive_signals, negative_signals) VALUES ('obscure-topic', 0.7, 0.95, datetime('now'), 1, 0)",
        [],
    )
    .unwrap();
    let anomalies = detect_confidence_mismatch(&conn).unwrap();
    assert!(
        !anomalies.is_empty(),
        "Should detect confidence mismatch with <3 interactions"
    );
    assert_eq!(anomalies[0].anomaly_type, AnomalyType::ConfidenceMismatch);
}

#[test]
fn test_detect_all_runs_without_error() {
    let conn = setup_test_db();
    let anomalies = detect_all(&conn).unwrap();
    // Should at least detect stale data (no signals in test DB)
    assert!(
        !anomalies.is_empty(),
        "detect_all should find at least stale data anomaly"
    );
}

#[test]
fn test_anomaly_type_roundtrip() {
    let types = vec![
        AnomalyType::StaleData,
        AnomalyType::ContextDrift,
        AnomalyType::Contradiction,
        AnomalyType::AbnormalVolume,
        AnomalyType::ConfidenceMismatch,
    ];
    for t in types {
        let s = t.as_str();
        let recovered = AnomalyType::from_str(s);
        assert_eq!(t, recovered, "Roundtrip failed for {:?}", t);
    }
}

#[test]
fn test_anomaly_severity_ordering() {
    assert!(AnomalySeverity::Low < AnomalySeverity::Medium);
    assert!(AnomalySeverity::Medium < AnomalySeverity::High);
    assert!(AnomalySeverity::High < AnomalySeverity::Critical);
}

#[test]
fn test_resolve_nonexistent_anomaly() {
    let conn = setup_test_db();
    let result = resolve_anomaly(&conn, 99999);
    assert!(
        result.is_err(),
        "Should error when resolving nonexistent anomaly"
    );
}
