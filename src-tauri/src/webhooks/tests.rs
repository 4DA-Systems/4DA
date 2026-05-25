// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for webhook secret storage and lifecycle.

use rusqlite::{params, Connection};
use uuid::Uuid;

use super::delivery::check_circuit_breaker;
use super::ensure_webhook_tables;
use super::management::delete_webhook;
use super::secrets::{forget_webhook_secret, persist_webhook_secret, read_webhook_secret};
use super::sign_payload;

fn setup_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory DB");
    ensure_webhook_tables(&conn).expect("create schema");
    conn
}

fn insert_raw_webhook(conn: &Connection, id: &str, plaintext_secret: &str) {
    conn.execute(
        "INSERT INTO webhooks (id, team_id, name, url, events, secret, active, failure_count, created_at)
         VALUES (?1, 't', 'n', 'http://x', '[]', ?2, 1, 0, '2026-01-01T00:00:00Z')",
        params![id, plaintext_secret],
    ).expect("insert raw webhook");
}

fn db_secret_column(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT secret FROM webhooks WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    )
    .expect("read secret column")
}

// These tests assert the observable behavior (a webhook can always be
// signed after it's been registered) rather than whether a given host's
// keychain is available -- that's explicitly a graceful-degradation axis,
// not an invariant. On CI / sandboxed hosts the keychain may silently
// refuse writes even after `store_secret` returned Ok(true), and
// `get_secret` may return None immediately after a round-trip. The
// plaintext DB fallback must cover those cases.

#[test]
fn persist_then_read_returns_same_secret() {
    let conn = setup_conn();
    let id = format!("wh-roundtrip-{}", Uuid::new_v4());
    // Start the row empty so the only way to read back "new-secret" is
    // through the path that persist_webhook_secret took.
    insert_raw_webhook(&conn, &id, "");
    forget_webhook_secret(&id);

    persist_webhook_secret(&conn, &id, "new-secret").expect("persist");
    let got = read_webhook_secret(&conn, &id).expect("read");
    assert_eq!(got, "new-secret");

    forget_webhook_secret(&id);
}

#[test]
fn read_falls_back_to_db_plaintext_when_keychain_absent() {
    let conn = setup_conn();
    let id = format!("wh-fallback-{}", Uuid::new_v4());
    insert_raw_webhook(&conn, &id, "plaintext-signing-secret");
    forget_webhook_secret(&id);

    // The first read is the load-bearing assertion: a pre-migration row
    // with a plaintext DB secret must be readable even when the keychain
    // is empty for that key.
    let got = read_webhook_secret(&conn, &id).expect("read via DB fallback");
    assert_eq!(got, "plaintext-signing-secret");

    forget_webhook_secret(&id);
}

#[test]
fn read_errors_when_both_sources_are_empty() {
    let conn = setup_conn();
    let id = format!("wh-empty-{}", Uuid::new_v4());
    insert_raw_webhook(&conn, &id, "");
    forget_webhook_secret(&id);

    let result = read_webhook_secret(&conn, &id);
    assert!(
        result.is_err(),
        "a webhook with no secret in either source must fail-loud"
    );

    forget_webhook_secret(&id);
}

#[test]
fn delete_webhook_removes_db_row() {
    let conn = setup_conn();
    let id = format!("wh-del-{}", Uuid::new_v4());
    insert_raw_webhook(&conn, &id, "to-be-deleted");

    delete_webhook(&conn, &id).expect("delete");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM webhooks WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 0);
    // The keychain-scrub side effect is fire-and-forget; verifying it ran
    // would tie us to keychain availability (which this test suite
    // cannot assume). The production log line at forget_webhook_secret
    // covers the diagnostic.
}

#[test]
fn persist_is_idempotent() {
    let conn = setup_conn();
    let id = format!("wh-idem-{}", Uuid::new_v4());
    insert_raw_webhook(&conn, &id, "");
    forget_webhook_secret(&id);

    persist_webhook_secret(&conn, &id, "s1").expect("persist 1");
    persist_webhook_secret(&conn, &id, "s2").expect("persist 2");
    let got = read_webhook_secret(&conn, &id).expect("read");
    assert_eq!(got, "s2", "second persist must overwrite the first");

    forget_webhook_secret(&id);
}

#[test]
fn sign_payload_is_stable_across_reads() {
    // Full integration: register-like flow -> dispatch-like flow.
    let conn = setup_conn();
    let id = format!("wh-sign-{}", Uuid::new_v4());
    insert_raw_webhook(&conn, &id, "");
    forget_webhook_secret(&id);
    persist_webhook_secret(&conn, &id, "signing-key").expect("persist");

    let s1 = read_webhook_secret(&conn, &id).expect("first read");
    let s2 = read_webhook_secret(&conn, &id).expect("second read");
    let body = r#"{"event":"x"}"#;
    assert_eq!(sign_payload(&s1, body), sign_payload(&s2, body));

    // And unused column state must never break a subsequent read.
    let _ = db_secret_column(&conn, &id);
    let s3 = read_webhook_secret(&conn, &id).expect("third read");
    assert_eq!(sign_payload(&s3, body), sign_payload(&s1, body));

    forget_webhook_secret(&id);
}
