// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Database encryption key management.
//!
//! Generates and stores the database encryption key in the OS keychain.
//! The key never touches disk — it lives only in the keychain and process memory.
//!
//! When SQLCipher is enabled (bundled-sqlcipher feature), the key is applied
//! via `PRAGMA key` immediately after opening each connection.

use tracing::{info, warn};

const DB_KEY_NAME: &str = "4da_db_encryption_key";

#[cfg(test)]
thread_local! {
    /// How many times this thread reached the keychain for the database key.
    static KEYCHAIN_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Get or generate the database encryption key from the OS keychain.
///
/// On first call: generates a random 256-bit key, stores in keychain, returns it.
/// On subsequent calls: retrieves the existing key from keychain.
/// On keychain failure: returns None (database runs unencrypted).
pub(crate) fn get_or_create_db_key() -> Option<String> {
    #[cfg(test)]
    KEYCHAIN_LOOKUPS.with(|c| c.set(c.get() + 1));
    // Try to retrieve existing key
    match crate::settings::keystore::get_secret(DB_KEY_NAME) {
        Ok(Some(key)) if !key.is_empty() => {
            info!(target: "4da::db::encryption", "Database encryption key retrieved from keychain");
            return Some(key);
        }
        Ok(_) => {
            // Key doesn't exist yet — generate one
        }
        Err(e) => {
            warn!(target: "4da::db::encryption", error = %e, "Keychain unavailable — database will run unencrypted");
            return None;
        }
    }

    // Generate new key. store_secret now distinguishes keychain-persisted
    // (true) from plaintext-fallback (false) — for DB encryption we ONLY
    // want to return Some(key) when the key is durably stored, because
    // otherwise the next boot won't find it and the DB becomes unreadable.
    let key = generate_hex_key();
    match crate::settings::keystore::store_secret(DB_KEY_NAME, &key) {
        Ok(true) => {
            info!(target: "4da::db::encryption", "Generated and stored new database encryption key in platform keychain");
            Some(key)
        }
        Ok(false) => {
            warn!(target: "4da::db::encryption", "Keychain unavailable; database encryption will be skipped to keep the database openable on next boot");
            None
        }
        Err(e) => {
            warn!(target: "4da::db::encryption", error = %e, "Failed to store encryption key — database will run unencrypted");
            None
        }
    }
}

/// The database key for `conn`, fetched only where it can actually be applied.
///
/// The keychain is consulted only when the linked SQLite is SQLCipher. This
/// build links plain SQLite (rusqlite `bundled`, see Cargo.toml), where
/// `PRAGMA key` is a no-op, so the lookup bought nothing and cost a keychain
/// round-trip on every open. On macOS it was worse than useless: a keychain
/// item created by one binary blocks any OTHER binary's read on an access
/// prompt, which hung every pipeline_integration test on the macOS runner
/// (PR #941). Existing keychain items are left untouched; when SQLCipher is
/// enabled the original get-or-create behaviour applies unchanged.
pub(crate) fn db_key_for(conn: &rusqlite::Connection) -> Option<String> {
    key_if_cipher(sqlcipher_linked(conn), get_or_create_db_key)
}

/// Run `fetch` only when encryption is available. Split out so the gate can
/// be tested with an injected key source.
fn key_if_cipher(cipher: bool, fetch: impl FnOnce() -> Option<String>) -> Option<String> {
    if cipher {
        fetch()
    } else {
        None
    }
}

/// Is the linked SQLite SQLCipher? `PRAGMA cipher_version` returns a row only
/// there; plain SQLite ignores the unknown pragma and returns nothing.
pub(crate) fn sqlcipher_linked(conn: &rusqlite::Connection) -> bool {
    conn.query_row("PRAGMA cipher_version", [], |r| r.get::<_, String>(0))
        .is_ok()
}

/// Apply the encryption key to an open SQLite connection.
///
/// This must be called IMMEDIATELY after opening the connection,
/// before any other PRAGMA or query.
///
/// No-op if key is None (unencrypted mode).
pub(crate) fn apply_key_to_connection(
    conn: &rusqlite::Connection,
    key: Option<&str>,
) -> rusqlite::Result<()> {
    if let Some(key) = key {
        // PRAGMA key must be the very first statement after opening
        conn.pragma_update(None, "key", key)?;
        info!(target: "4da::db::encryption", "Database encryption key applied");
    }
    Ok(())
}

/// Generate a random 256-bit key as a hex string.
fn generate_hex_key() -> String {
    use sha2::{Digest, Sha256};
    // Use multiple entropy sources for key generation
    let mut hasher = Sha256::new();

    // System time (nanosecond precision)
    hasher.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );

    // Process ID
    hasher.update(std::process::id().to_le_bytes());

    // Thread ID hash
    hasher.update(format!("{:?}", std::thread::current().id()).as_bytes());

    // Random bytes from the OS (primary entropy source)
    let mut random_bytes = [0u8; 32];
    if getrandom::fill(&mut random_bytes).is_ok() {
        hasher.update(&random_bytes);
    }

    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 256-bit key rendered as hex is 64 characters. SQLCipher's `PRAGMA key`
    /// expects exactly this width, so the length is a real invariant, not a
    /// restatement of the implementation.
    const EXPECTED_HEX_KEY_LEN: usize = 64;

    #[test]
    fn test_generate_hex_key_length() {
        let key = generate_hex_key();
        assert_eq!(key.len(), EXPECTED_HEX_KEY_LEN);
    }

    #[test]
    fn test_generate_hex_key_uniqueness() {
        let key1 = generate_hex_key();
        let key2 = generate_hex_key();
        assert_ne!(key1, key2, "Two generated keys should be different");
    }

    #[test]
    fn test_apply_key_none_is_noop() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Should not error with None key
        assert!(apply_key_to_connection(&conn, None).is_ok());
    }

    /// Opening a database on a build without SQLCipher must never touch the
    /// keychain. The key could not be applied anyway (`PRAGMA key` is a no-op
    /// on plain SQLite), and on macOS a keychain item created by another binary
    /// blocks the read on an access prompt: that hung all 13
    /// pipeline_integration tests on the macOS runner (PR #941).
    #[test]
    fn opening_a_database_without_sqlcipher_never_reaches_the_keychain() {
        let before = KEYCHAIN_LOOKUPS.with(std::cell::Cell::get);
        let _db = crate::db::Database::new(std::path::Path::new(":memory:")).expect("open");
        let dir = tempfile::tempdir().expect("tempdir");
        let _file_db = crate::db::Database::new(&dir.path().join("4da.db")).expect("open file db");
        assert_eq!(KEYCHAIN_LOOKUPS.with(std::cell::Cell::get), before);
    }

    #[test]
    fn plain_bundled_sqlite_is_not_sqlcipher() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(!sqlcipher_linked(&conn));
        assert_eq!(db_key_for(&conn), None);
    }

    #[test]
    fn the_key_source_runs_only_when_sqlcipher_is_linked() {
        let mut calls = 0;
        assert_eq!(
            key_if_cipher(false, || {
                calls += 1;
                Some("k".into())
            }),
            None
        );
        assert_eq!(calls, 0);
        assert_eq!(
            key_if_cipher(true, || {
                calls += 1;
                Some("k".into())
            }),
            Some("k".to_string())
        );
        assert_eq!(calls, 1);
    }
}
