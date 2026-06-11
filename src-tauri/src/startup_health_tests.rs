// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for startup_health module.

use crate::startup_health::*;
use std::fs;

#[test]
fn test_run_startup_health_check_returns_vec() {
    // Should never panic, regardless of environment state.
    let issues = run_startup_health_check();
    // We can't assert exact count (depends on environment), but type is correct.
    assert!(issues.iter().all(|i| !i.component.is_empty()));
}

#[test]
fn test_check_database_missing() {
    let tmp = std::env::temp_dir().join("4da_health_test_db_missing");
    let _ = fs::create_dir_all(&tmp);
    let _ = fs::remove_file(tmp.join("4da.db"));

    let mut issues = Vec::new();
    check_database(&tmp, &mut issues);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].component, "database");
    assert_eq!(issues[0].severity, HealthSeverity::Warning);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_database_empty() {
    let tmp = std::env::temp_dir().join("4da_health_test_db_empty");
    let _ = fs::create_dir_all(&tmp);
    fs::write(tmp.join("4da.db"), b"").expect("write empty db");

    let mut issues = Vec::new();
    check_database(&tmp, &mut issues);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, HealthSeverity::Warning);
    assert!(issues[0].message.contains("empty"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_database_valid() {
    let tmp = std::env::temp_dir().join("4da_health_test_db_valid");
    let _ = fs::create_dir_all(&tmp);
    fs::write(tmp.join("4da.db"), b"SQLite format 3\0").expect("write fake db");

    let mut issues = Vec::new();
    check_database(&tmp, &mut issues);
    assert!(issues.is_empty(), "Valid DB should produce no issues");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_settings_missing() {
    let tmp = std::env::temp_dir().join("4da_health_test_settings_missing");
    let _ = fs::create_dir_all(&tmp);
    let _ = fs::remove_file(tmp.join("settings.json"));

    let mut issues = Vec::new();
    check_settings(&tmp, &mut issues);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, HealthSeverity::Warning);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_settings_invalid_json() {
    let tmp = std::env::temp_dir().join("4da_health_test_settings_bad");
    let _ = fs::create_dir_all(&tmp);
    fs::write(tmp.join("settings.json"), "{ not valid json !!!").expect("write bad json");

    let mut issues = Vec::new();
    check_settings(&tmp, &mut issues);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, HealthSeverity::Error);
    assert!(issues[0].message.contains("invalid JSON"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_settings_valid() {
    let tmp = std::env::temp_dir().join("4da_health_test_settings_ok");
    let _ = fs::create_dir_all(&tmp);
    fs::write(
        tmp.join("settings.json"),
        r#"{"llm": {"provider": "none"}}"#,
    )
    .expect("write valid json");

    let mut issues = Vec::new();
    check_settings(&tmp, &mut issues);
    assert!(issues.is_empty(), "Valid settings should produce no issues");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_embedding_no_settings() {
    let tmp = std::env::temp_dir().join("4da_health_test_embed_none");
    let _ = fs::create_dir_all(&tmp);
    let _ = fs::remove_file(tmp.join("settings.json"));

    let mut issues = Vec::new();
    check_embedding_provider(&tmp, &mut issues);
    assert!(
        issues.is_empty(),
        "No settings file should produce no embedding issues"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_embedding_provider_none() {
    let tmp = std::env::temp_dir().join("4da_health_test_embed_provider_none");
    let _ = fs::create_dir_all(&tmp);
    fs::write(
        tmp.join("settings.json"),
        r#"{"llm": {"provider": "none", "api_key": ""}}"#,
    )
    .expect("write settings");

    let mut issues = Vec::new();
    check_embedding_provider(&tmp, &mut issues);
    assert!(issues.is_empty(), "Provider 'none' should be fine");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_embedding_provider_missing_key() {
    let tmp = std::env::temp_dir().join("4da_health_test_embed_missing_key");
    let _ = fs::create_dir_all(&tmp);
    fs::write(
        tmp.join("settings.json"),
        r#"{"llm": {"provider": "anthropic", "api_key": ""}}"#,
    )
    .expect("write settings");

    let mut issues = Vec::new();
    // Use inner variant with check_keychain=false so the real platform keychain
    // (which may hold a live key on dev machines) doesn't mask the missing-key path.
    check_embedding_provider_inner(&tmp, &mut issues, false);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, HealthSeverity::Warning);
    assert!(issues[0].message.contains("API key is empty"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_embedding_provider_ollama() {
    let tmp = std::env::temp_dir().join("4da_health_test_embed_ollama");
    let _ = fs::create_dir_all(&tmp);
    fs::write(
        tmp.join("settings.json"),
        r#"{"llm": {"provider": "ollama", "api_key": ""}}"#,
    )
    .expect("write settings");

    let mut issues = Vec::new();
    check_embedding_provider(&tmp, &mut issues);
    assert!(issues.is_empty(), "Ollama should not require API key");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_embedding_provider_with_valid_key() {
    let tmp = std::env::temp_dir().join("4da_health_test_embed_valid_key");
    let _ = fs::create_dir_all(&tmp);
    fs::write(
        tmp.join("settings.json"),
        r#"{"llm": {"provider": "openai", "api_key": "sk-test123"}}"#,
    )
    .expect("write settings");

    let mut issues = Vec::new();
    check_embedding_provider(&tmp, &mut issues);
    assert!(issues.is_empty(), "Valid API key should produce no issues");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_check_disk_write_writable() {
    let tmp = std::env::temp_dir().join("4da_health_test_disk_ok");
    let _ = fs::create_dir_all(&tmp);

    let mut issues = Vec::new();
    check_disk_write(&tmp, &mut issues);
    assert!(issues.is_empty(), "Temp dir should be writable");

    // Verify probe file was cleaned up.
    assert!(!tmp.join(".4da_health_probe").exists());

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_health_severity_serialize() {
    let issue = HealthIssue {
        component: "test",
        severity: HealthSeverity::Error,
        message: "test message".to_string(),
    };
    let json = serde_json::to_string(&issue).expect("serialize");
    assert!(json.contains("\"error\""));
    assert!(json.contains("\"test\""));
}

#[test]
fn test_health_severity_warning_serialize() {
    let issue = HealthIssue {
        component: "disk",
        severity: HealthSeverity::Warning,
        message: "low space".to_string(),
    };
    let json = serde_json::to_string(&issue).expect("serialize");
    assert!(json.contains("\"warning\""));
    assert!(json.contains("\"disk\""));
}

// ============================================================================
// Zero-embedding coverage check
// ============================================================================

/// Minimal in-memory source_items table + N recent rows with the given
/// embedding blobs.
fn embedding_test_conn(blobs: &[Vec<u8>]) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            embedding BLOB,
            created_at TEXT DEFAULT (datetime('now'))
        );",
    )
    .expect("create table");
    for blob in blobs {
        conn.execute(
            "INSERT INTO source_items (embedding) VALUES (?1)",
            rusqlite::params![blob],
        )
        .expect("insert row");
    }
    conn
}

fn zero_blob() -> Vec<u8> {
    vec![0u8; 32]
}

fn real_blob() -> Vec<u8> {
    let mut b = vec![0u8; 32];
    b[0] = 0x3f; // non-zero byte -> not a zero vector
    b
}

#[test]
fn test_embedding_coverage_warns_on_majority_zero() {
    // 30 recent items, 24 zero-vector (80%) -> warning fires.
    let mut blobs = vec![zero_blob(); 24];
    blobs.extend(vec![real_blob(); 6]);
    let conn = embedding_test_conn(&blobs);

    let mut issues = Vec::new();
    check_embedding_coverage_with_conn(&conn, &mut issues);
    assert_eq!(issues.len(), 1, "majority-zero recent embeddings must warn");
    assert_eq!(issues[0].component, "embedding");
    assert_eq!(issues[0].severity, HealthSeverity::Warning);
    assert!(
        issues[0].message.contains("Semantic matching is degraded"),
        "copy must explain the degradation: {}",
        issues[0].message
    );
    assert!(
        issues[0].message.contains("80%"),
        "copy must include the measured share: {}",
        issues[0].message
    );
}

#[test]
fn test_embedding_coverage_silent_when_embeddings_healthy() {
    // 30 recent items, only 3 zero-vector (10%) -> no warning.
    let mut blobs = vec![real_blob(); 27];
    blobs.extend(vec![zero_blob(); 3]);
    let conn = embedding_test_conn(&blobs);

    let mut issues = Vec::new();
    check_embedding_coverage_with_conn(&conn, &mut issues);
    assert!(issues.is_empty(), "healthy embeddings must not warn");
}

#[test]
fn test_embedding_coverage_silent_below_min_sample() {
    // Only 5 recent items, all zero -> too small a sample, stay silent
    // (first-run / low-traffic installs must not see a false alarm).
    let conn = embedding_test_conn(&vec![zero_blob(); 5]);

    let mut issues = Vec::new();
    check_embedding_coverage_with_conn(&conn, &mut issues);
    assert!(issues.is_empty(), "tiny samples must not warn");
}

#[test]
fn test_embedding_coverage_ignores_old_items() {
    // 30 zero-vector items, but all older than 24h -> silent.
    let conn = embedding_test_conn(&[]);
    for _ in 0..30 {
        conn.execute(
            "INSERT INTO source_items (embedding, created_at)
             VALUES (?1, datetime('now', '-3 days'))",
            rusqlite::params![zero_blob()],
        )
        .expect("insert old row");
    }

    let mut issues = Vec::new();
    check_embedding_coverage_with_conn(&conn, &mut issues);
    assert!(issues.is_empty(), "stale zero embeddings must not warn");
}

#[test]
fn test_embedding_coverage_missing_table_is_silent() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    let mut issues = Vec::new();
    check_embedding_coverage_with_conn(&conn, &mut issues);
    assert!(issues.is_empty(), "missing table must be silent, not panic");
}
