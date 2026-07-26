// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared free functions for the DB layer — datetime parsing, content hashing,
//! and embedding blob conversion.
//!
//! Extracted from `db/mod.rs` unchanged (2026-07-26): the module root had
//! reached the 1000-line hard limit, and these are pure, stateless helpers used
//! across every DB submodule, so they are the cohesive piece to lift out.
//! Re-exported from `db` (`pub(crate) use helpers::*`), so every existing
//! `crate::db::parse_datetime` / `super::blob_to_embedding` path is unchanged.

use sha2::{Digest, Sha256};

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse datetime string to chrono DateTime
pub(crate) fn parse_datetime(s: String) -> chrono::DateTime<chrono::Utc> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| Utc.from_utc_datetime(&dt))
        .unwrap_or_else(|_| {
            tracing::warn!("Failed to parse datetime '{}', falling back to now", s);
            Utc::now()
        })
}

/// Parse an optional SQLite datetime column STRICTLY. Unlike
/// `parse_datetime`, a NULL or unparseable value yields `None` — never a
/// fabricated `now()`, which for `published_at` would fake freshness.
pub(crate) fn parse_datetime_opt(s: Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    s.and_then(|s| {
        NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
            .map(|dt| Utc.from_utc_datetime(&dt))
            .ok()
    })
}

/// Hash content for deduplication
pub(crate) fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash multiple content parts for deduplication without intermediate allocation.
pub(crate) fn hash_content_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub(crate) const EMBEDDING_DIM: usize = crate::EMBEDDING_DIMS;

/// Convert f32 embedding to blob for storage.
/// Validates dimension before conversion — rejects wrong-sized vectors.
pub(crate) fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    if !embedding.is_empty() && embedding.len() != EMBEDDING_DIM {
        tracing::error!(
            target: "4da::db",
            "Embedding dimension mismatch: expected {} but got {} — storing anyway",
            EMBEDDING_DIM, embedding.len()
        );
    }
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob back to f32 embedding.
/// Returns empty vec on invalid blobs instead of panicking.
pub(crate) fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    if blob.is_empty() {
        return Vec::new();
    }
    if blob.len() % 4 != 0 {
        tracing::warn!(
            target: "4da::db",
            "Invalid embedding blob size: {} bytes (not divisible by 4) — returning empty",
            blob.len()
        );
        return Vec::new();
    }
    blob.chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0u8; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("hello world");
        let hash2 = hash_content("hello world");
        let hash3 = hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_embedding_roundtrip() {
        let embedding = vec![0.1, 0.2, 0.3, -0.5, 1.0];
        let blob = embedding_to_blob(&embedding);
        let recovered = blob_to_embedding(&blob);

        assert_eq!(embedding.len(), recovered.len());
        for (a, b) in embedding.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }
}
