// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use rusqlite::Connection;
use uuid::Uuid;

fn setup_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory DB");
    // Minimal slice of the team_crypto schema — matches migrations.rs.
    conn.execute_batch(
        "CREATE TABLE team_crypto (
                team_id TEXT PRIMARY KEY,
                our_public_key BLOB NOT NULL,
                our_private_key_enc BLOB NOT NULL,
                team_symmetric_key_enc BLOB
            );",
    )
    .expect("create schema");
    conn
}

fn seed_team(conn: &Connection, team_id: &str, priv_bytes: &[u8], sym_bytes: Option<&[u8]>) {
    conn.execute(
        "INSERT INTO team_crypto (team_id, our_public_key, our_private_key_enc, team_symmetric_key_enc)
             VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            team_id,
            vec![0u8; 32], // public key placeholder — unused in these tests
            priv_bytes.to_vec(),
            sym_bytes.map(|b| b.to_vec()),
        ],
    )
    .expect("seed team_crypto");
}

fn fresh_team_id(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

#[test]
fn persist_then_read_returns_same_private_key() {
    let conn = setup_conn();
    let team_id = fresh_team_id("tk-roundtrip-priv");
    forget_team_keys(&team_id);
    let key = [7u8; 32];
    let db_bytes = persist_team_private_key(&team_id, &key);
    // Seed the row with whatever `persist_*` returned for the DB column.
    seed_team(&conn, &team_id, &db_bytes, None);

    let got = read_team_private_key(&conn, &team_id).expect("read");
    assert_eq!(got, key);

    forget_team_keys(&team_id);
}

#[test]
fn persist_then_read_returns_same_symmetric_key() {
    let conn = setup_conn();
    let team_id = fresh_team_id("tk-roundtrip-sym");
    forget_team_keys(&team_id);
    let priv_key = [3u8; 32];
    let sym_key = [9u8; 32];
    let priv_db = persist_team_private_key(&team_id, &priv_key);
    let sym_db = persist_team_symmetric_key(&team_id, &sym_key);
    seed_team(&conn, &team_id, &priv_db, Some(&sym_db));

    let got = read_team_symmetric_key(&conn, &team_id).expect("read");
    assert_eq!(got, Some(sym_key));

    forget_team_keys(&team_id);
}

#[test]
fn read_falls_back_to_db_plaintext_when_keychain_absent() {
    let conn = setup_conn();
    let team_id = fresh_team_id("tk-fallback");
    forget_team_keys(&team_id);
    let priv_key = [0x42; 32];
    // Seed the DB directly with a plaintext private key — no keystore call.
    // This simulates a pre-migration row from a prior install.
    seed_team(&conn, &team_id, &priv_key, None);

    let got = read_team_private_key(&conn, &team_id).expect("read via DB fallback");
    assert_eq!(got, priv_key);

    forget_team_keys(&team_id);
}

#[test]
fn read_symmetric_returns_none_when_db_is_null() {
    let conn = setup_conn();
    let team_id = fresh_team_id("tk-sym-null");
    forget_team_keys(&team_id);
    // Pre-delivery member: private key seeded, sym key NULL.
    seed_team(&conn, &team_id, &[1u8; 32], None);

    let got = read_team_symmetric_key(&conn, &team_id).expect("read");
    assert_eq!(got, None, "pre-delivery state must surface as Ok(None)");

    forget_team_keys(&team_id);
}

#[test]
fn read_private_key_errors_when_both_sources_absent() {
    let conn = setup_conn();
    let team_id = fresh_team_id("tk-missing");
    forget_team_keys(&team_id);
    seed_team(&conn, &team_id, &[], None); // zero-length sentinel

    let result = read_team_private_key(&conn, &team_id);
    assert!(
        result.is_err(),
        "empty keychain + empty DB must fail loudly on private-key read"
    );

    forget_team_keys(&team_id);
}

#[test]
fn keystore_keys_are_unique_per_team() {
    // Ensures two teams on the same install don't collide in the keychain.
    let a = fresh_team_id("tk-uniq-a");
    let b = fresh_team_id("tk-uniq-b");
    assert_ne!(team_privkey_keystore_key(&a), team_privkey_keystore_key(&b));
    assert_ne!(team_symkey_keystore_key(&a), team_symkey_keystore_key(&b));
    // And the two axes within one team must also be disjoint.
    assert_ne!(team_privkey_keystore_key(&a), team_symkey_keystore_key(&a));
}
