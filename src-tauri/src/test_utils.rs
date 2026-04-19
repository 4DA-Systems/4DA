// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared test utilities for 4DA.
//!
//! Gated behind `#[cfg(test)]` — zero production cost.
//! Provides common helpers to avoid duplicating test infrastructure
//! across unit tests, integration tests, and benchmarks.

use std::path::Path;

use crate::db::Database;

/// In-memory SQLite with sqlite-vec loaded and all migrations applied.
pub fn test_db() -> Database {
    crate::register_sqlite_vec_extension();
    Database::new(Path::new(":memory:")).expect("in-memory DB")
}

/// Deterministic 384-dim normalized embedding from a seed string.
/// Identical seeds produce identical embeddings (good for test assertions).
/// Uses a simple hash-based approach — NOT cryptographically random,
/// but deterministic and reproducible across runs.
pub fn seed_embedding(seed: &str) -> Vec<f32> {
    let mut embedding = vec![0.0f32; 384];
    let bytes = seed.as_bytes();
    for (i, slot) in embedding.iter_mut().enumerate() {
        let b1 = bytes[i % bytes.len()] as f32;
        let b2 = bytes[(i + 7) % bytes.len()] as f32;
        *slot = ((b1 * 0.00391 + b2 * 0.00197 + (i as f32) * 0.001).sin()) * 0.5;
    }
    // Normalize to unit vector
    let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut embedding {
            *v /= norm;
        }
    }
    embedding
}

/// Insert a test source item with a seeded embedding, return its DB id.
pub fn insert_test_item(
    db: &Database,
    source_type: &str,
    source_id: &str,
    title: &str,
    content: &str,
) -> i64 {
    let emb = seed_embedding(&format!("{source_type}:{source_id}"));
    db.upsert_source_item(source_type, source_id, None, title, content, &emb)
        .expect("insert_test_item failed")
}

/// Insert a test source item with a URL and seeded embedding.
pub fn insert_test_item_with_url(
    db: &Database,
    source_type: &str,
    source_id: &str,
    url: &str,
    title: &str,
    content: &str,
) -> i64 {
    let emb = seed_embedding(&format!("{source_type}:{source_id}"));
    db.upsert_source_item(source_type, source_id, Some(url), title, content, &emb)
        .expect("insert_test_item_with_url failed")
}

/// Empty scoring context (no interests, no ACE, no stacks).
/// Only used in unit tests (not integration tests, since ScoringContext is pub(crate)).
#[cfg(test)]
pub(crate) fn empty_scoring_context() -> crate::scoring::ScoringContext {
    crate::scoring::ScoringContext::builder().build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_db_creates_valid_database() {
        let db = test_db();
        assert_eq!(db.context_count().unwrap(), 0);
        assert_eq!(db.total_item_count().unwrap(), 0);
    }

    #[test]
    fn test_seed_embedding_is_deterministic() {
        let emb1 = seed_embedding("hello");
        let emb2 = seed_embedding("hello");
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_seed_embedding_is_384_dim() {
        let emb = seed_embedding("test");
        assert_eq!(emb.len(), 384);
    }

    #[test]
    fn test_seed_embedding_is_normalized() {
        let emb = seed_embedding("test");
        let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Embedding should be unit-normalized, got norm={}",
            norm
        );
    }

    #[test]
    fn test_different_seeds_produce_different_embeddings() {
        let emb1 = seed_embedding("alpha");
        let emb2 = seed_embedding("beta");
        assert_ne!(emb1, emb2);
    }

    #[test]
    fn test_insert_test_item_roundtrip() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "hn_001", "Test Title", "Test content");
        assert!(id > 0);
        assert_eq!(db.total_item_count().unwrap(), 1);
    }

    #[test]
    fn test_insert_test_item_with_url_roundtrip() {
        let db = test_db();
        let id = insert_test_item_with_url(
            &db,
            "hackernews",
            "hn_002",
            "https://example.com",
            "URL Test",
            "Content",
        );
        assert!(id > 0);
    }

    #[test]
    fn test_empty_scoring_context_builds() {
        let ctx = empty_scoring_context();
        assert!(ctx.interests.is_empty());
        assert!(ctx.ace_ctx.detected_tech.is_empty());
    }
}
