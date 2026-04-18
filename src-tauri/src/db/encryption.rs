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
#[allow(dead_code)]
const DB_KEY_LENGTH: usize = 64; // 256-bit key as hex string

/// Get or generate the database encryption key from the OS keychain.
///
/// On first call: generates a random 256-bit key, stores in keychain, returns it.
/// On subsequent calls: retrieves the existing key from keychain.
/// On keychain failure: returns None (database runs unencrypted).
pub(crate) fn get_or_create_db_key() -> Option<String> {
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

/// Check if a database file is encrypted by attempting to read its header.
/// SQLCipher-encrypted databases have a non-standard header (not "SQLite format 3\0").
#[allow(dead_code)]
pub(crate) fn is_database_encrypted(db_path: &std::path::Path) -> bool {
    if !db_path.exists() {
        return false;
    }
    match std::fs::read(db_path) {
        Ok(data) if data.len() >= 16 => {
            // Standard SQLite header starts with "SQLite format 3\0"
            let header = &data[..16];
            header != b"SQLite format 3\0"
        }
        _ => false,
    }
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
    if getrandom::getrandom(&mut random_bytes).is_ok() {
        hasher.update(&random_bytes);
    }

    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_hex_key_length() {
        let key = generate_hex_key();
        assert_eq!(key.len(), DB_KEY_LENGTH);
    }

    #[test]
    fn test_generate_hex_key_uniqueness() {
        let key1 = generate_hex_key();
        let key2 = generate_hex_key();
        assert_ne!(key1, key2, "Two generated keys should be different");
    }

    #[test]
    fn test_is_database_encrypted_plaintext() {
        // Create a temp database (plaintext) with a fully unique name to avoid
        // conflicts when tests run in parallel within the same process.
        let unique_id = format!(
            "{}_{:?}_{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join("4da_test_enc");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(format!("test_plain_{unique_id}.db"));
        // Clean up from any prior run
        std::fs::remove_file(&path).ok();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE enc_test (id INTEGER);")
            .unwrap();
        drop(conn);

        assert!(!is_database_encrypted(&path));

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_apply_key_none_is_noop() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Should not error with None key
        assert!(apply_key_to_connection(&conn, None).is_ok());
    }
}
