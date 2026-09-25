// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! One embedding space for every stored vector (INV-022).
//!
//! Three defects this module closes (found 2026-09-25):
//!
//! 1. A model change re-embedded only the `source_vec` search index. Scoring
//!    reads `source_items.embedding` and `context_chunks.embedding`, and
//!    neither was touched, so after a switch the pipeline kept comparing
//!    old-model items against old-model context. Search, meanwhile, ran on the
//!    new model. [`reembed_derived_stores`] re-embeds every store scoring reads,
//!    and clears the caches built from them.
//! 2. The "done" marker was written BEFORE the re-embed ran, so an interrupted
//!    re-embed left a mixed corpus forever. The marker is now written only after
//!    a complete pass ([`mark_space_verified`]).
//! 3. The identity was the SETTINGS string, not the model that produced the
//!    vectors. A user with a cloud LLM key and no Ollama had a corpus of Arctic
//!    vectors (the fastembed fallback) labelled `nomic-embed-text`. So nothing
//!    ever noticed when the fallback model changed. [`ensure_embedding_space`]
//!    measures instead of trusting the label: it re-embeds a sample with the
//!    current route and compares it with what is stored.
//!
//! Measured on the live corpus: same-space vectors agree at median cosine 1.000
//! (nomic via Ollama vs its fp16 ONNX build); even a lossy int8 build of the
//! same model stays ~0.90.
//! Different models share no space: Arctic vs nomic on the same text has median
//! 0.007 (max 0.10). So [`SAME_SPACE_MIN_COSINE`] = 0.6 is a wide, safe margin.

use std::sync::atomic::{AtomicU64, Ordering};

/// Bump to force every install to re-verify its corpus once.
const SPACE_VERSION: &str = "v2";

/// Median cosine (stored vs freshly embedded, same text) at or above which the
/// stored vectors are in the current model's space.
const SAME_SPACE_MIN_COSINE: f32 = 0.6;

/// How many recent items the startup probe re-embeds.
const PROBE_SAMPLE: i64 = 32;

const MARKER_KEY: &str = "embedding_space";

/// Advanced whenever stored vectors move to a new space, so in-memory caches of
/// vectors (the semantic topic cache) know to drop and reload.
static EMBEDDING_EPOCH: AtomicU64 = AtomicU64::new(0);

pub(crate) fn embedding_epoch() -> u64 {
    EMBEDDING_EPOCH.load(Ordering::Acquire)
}

fn bump_epoch() {
    EMBEDDING_EPOCH.fetch_add(1, Ordering::AcqRel);
}

/// The marker value a verified corpus carries: the embedding identity plus the
/// space version. A model change changes it, so it re-arms the probe.
pub(crate) fn expected_space_marker() -> String {
    format!(
        "{}|{SPACE_VERSION}",
        crate::reembed::effective_embedding_identity()
    )
}

fn space_marker(conn: &rusqlite::Connection) -> String {
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        rusqlite::params![MARKER_KEY],
        |row| row.get(0),
    )
    .unwrap_or_default()
}

/// Record that every stored vector is in the current space.
pub(crate) fn mark_space_verified(conn: &rusqlite::Connection) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO app_meta (key, value) VALUES (?1, ?2)",
        rusqlite::params![MARKER_KEY, expected_space_marker()],
    );
}

/// The text an item was embedded from at ingest: the same builder and the
/// same per-source compression (processor.rs / fetcher.rs).
pub(crate) fn item_embedding_text(source_type: &str, title: &str, content: &str) -> String {
    crate::build_embedding_text(
        title,
        &crate::compression_rules::compress(source_type, content),
    )
}

fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

pub(crate) fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn cosine(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let (mut dot, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    // A zero vector is a provider-outage placeholder, not a direction.
    if na < 1e-12 || nb < 1e-12 {
        return None;
    }
    Some(dot / (na.sqrt() * nb.sqrt()))
}

fn median(mut v: Vec<f32>) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

/// The probe's decision from the stored-vs-fresh cosines of a sample.
#[derive(Debug, PartialEq)]
enum SpaceVerdict {
    /// Stored vectors are in the current space.
    Same,
    /// Stored vectors are in another space: the corpus must be re-embedded.
    Different,
    /// Too few usable comparisons (e.g. the provider returned zero vectors).
    Unknown,
}

fn judge_space(cosines: &[Option<f32>]) -> SpaceVerdict {
    let usable: Vec<f32> = cosines.iter().flatten().copied().collect();
    if usable.is_empty() || usable.len() * 2 < cosines.len() {
        return SpaceVerdict::Unknown;
    }
    match median(usable) {
        Some(m) if m >= SAME_SPACE_MIN_COSINE => SpaceVerdict::Same,
        Some(_) => SpaceVerdict::Different,
        None => SpaceVerdict::Unknown,
    }
}

/// Startup check: are the stored vectors in the space the current route
/// produces? Cheap when already verified (one `app_meta` read). Otherwise it
/// re-embeds a sample of recent items and compares; a different space triggers
/// a full re-embed, which marks the corpus verified when it completes.
pub(crate) async fn ensure_embedding_space() {
    let Ok(db) = crate::state::get_database() else {
        return;
    };
    let sample: Vec<(String, String, String, Vec<u8>)> = {
        let conn = db.conn.lock();
        if space_marker(&conn) == expected_space_marker() {
            return;
        }
        conn.prepare(
            "SELECT source_type, title, COALESCE(content, ''), embedding FROM source_items
             WHERE embedding IS NOT NULL AND LENGTH(embedding) = ?1
             ORDER BY id DESC LIMIT ?2",
        )
        .and_then(|mut stmt| {
            stmt.query_map(
                rusqlite::params![(crate::EMBEDDING_DIMS * 4) as i64, PROBE_SAMPLE],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .and_then(|rows| rows.collect::<std::result::Result<Vec<_>, _>>())
        })
        .unwrap_or_default()
    };

    if sample.is_empty() {
        // Nothing embedded yet: whatever gets stored from now on is current.
        mark_space_verified(&db.conn.lock());
        return;
    }

    let texts: Vec<String> = sample
        .iter()
        .map(|(st, title, content, _)| item_embedding_text(st, title, content))
        .collect();
    let fresh = match crate::embed_texts(&texts).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "4da::embeddings", error = %e, "Embedding-space probe could not embed — will re-check next start");
            return;
        }
    };
    let cosines: Vec<Option<f32>> = sample
        .iter()
        .zip(&fresh)
        .map(|((_, _, _, blob), f)| cosine(&blob_to_vec(blob), f))
        .collect();
    let verdict = judge_space(&cosines);
    let med = median(cosines.iter().flatten().copied().collect());
    match verdict {
        SpaceVerdict::Same => {
            tracing::info!(target: "4da::embeddings", median_cosine = med, sample = sample.len(), "Stored vectors are in the current embedding space");
            mark_space_verified(&db.conn.lock());
        }
        SpaceVerdict::Unknown => {
            tracing::warn!(target: "4da::embeddings", "Embedding-space probe inconclusive (provider returned zero vectors) — will re-check next start");
        }
        SpaceVerdict::Different => {
            tracing::warn!(target: "4da::embeddings", median_cosine = med, sample = sample.len(), "Stored vectors are in a DIFFERENT embedding space — re-embedding the corpus");
            crate::reembed::reembed_all_items().await;
        }
    }
}

/// After the items are re-embedded, bring every other store that scoring reads
/// into the same space, and drop every cache built from the old vectors.
/// Returns `true` when every step succeeded.
pub(crate) async fn reembed_derived_stores(db: &crate::db::Database) -> bool {
    let mut ok = reembed_context_chunks(db).await;
    ok &= reembed_explicit_interests(db).await;
    {
        let conn = db.conn.lock();
        // Topic vectors refill lazily (semantic cache + monitoring job); a NULL
        // is a miss, never a stale hit. The item->context match cache is a pure
        // function of both vector sets, so all of it is stale now.
        for sql in [
            "UPDATE active_topics SET embedding = NULL WHERE embedding IS NOT NULL",
            "DELETE FROM topic_vec",
            "DELETE FROM embedding_cache",
            "DELETE FROM item_context_match",
            "DELETE FROM item_context_cache",
        ] {
            if let Err(e) = conn.execute(sql, []) {
                // A table an older schema lacks is not a failure of this pass.
                if !e.to_string().contains("no such table") {
                    tracing::warn!(target: "4da::embeddings", error = %e, sql, "Cache invalidation step failed");
                    ok = false;
                }
            }
        }
    }
    bump_epoch();
    crate::invalidate_context_engine();
    ok
}

async fn reembed_context_chunks(db: &crate::db::Database) -> bool {
    let chunks: Vec<(i64, String)> = {
        let conn = db.conn.lock();
        conn.prepare("SELECT id, text FROM context_chunks ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .and_then(|rows| rows.collect::<std::result::Result<Vec<_>, _>>())
            })
            .unwrap_or_default()
    };
    let mut ok = true;
    for batch in chunks.chunks(32) {
        let texts: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
        match crate::embed_texts(&texts).await {
            Ok(vectors) => {
                let conn = db.conn.lock();
                for ((id, _), v) in batch.iter().zip(&vectors) {
                    if let Err(e) = conn.execute(
                        "UPDATE context_chunks SET embedding = ?1, updated_at = datetime('now') WHERE id = ?2",
                        rusqlite::params![vec_to_blob(v), id],
                    ) {
                        tracing::warn!(target: "4da::embeddings", chunk = id, error = %e, "Context chunk re-embed write failed");
                        ok = false;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: "4da::embeddings", error = %e, "Context chunk re-embed batch failed");
                ok = false;
            }
        }
    }
    // context_vec is 100% derivable from context_chunks (migration 113 rebuilt
    // it the same way). One transaction, so readers never see it half-built.
    let conn = db.conn.lock();
    let rebuilt = conn.execute_batch(
        "BEGIN;
         DELETE FROM context_vec;
         INSERT INTO context_vec (id, grounds, embedding)
           SELECT id,
                  CASE WHEN source_type IN ('code','config') THEN 1 ELSE 0 END,
                  embedding
           FROM context_chunks
           WHERE embedding IS NOT NULL AND LENGTH(embedding) = 3072;
         COMMIT;",
    );
    if let Err(e) = rebuilt {
        let _ = conn.execute_batch("ROLLBACK;");
        tracing::warn!(target: "4da::embeddings", error = %e, "context_vec rebuild failed");
        ok = false;
    }
    tracing::info!(target: "4da::embeddings", chunks = chunks.len(), ok, "Re-embedded context chunks");
    ok
}

async fn reembed_explicit_interests(db: &crate::db::Database) -> bool {
    let interests: Vec<(i64, String)> = {
        let conn = db.conn.lock();
        conn.prepare("SELECT id, topic FROM explicit_interests ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .and_then(|rows| rows.collect::<std::result::Result<Vec<_>, _>>())
            })
            .unwrap_or_default()
    };
    if interests.is_empty() {
        return true;
    }
    // Explicit interests embed the bare topic (settings_commands_context.rs).
    let texts: Vec<String> = interests.iter().map(|(_, t)| t.clone()).collect();
    match crate::embed_texts(&texts).await {
        Ok(vectors) => {
            let conn = db.conn.lock();
            let mut ok = true;
            for ((id, _), v) in interests.iter().zip(&vectors) {
                if conn
                    .execute(
                        "UPDATE explicit_interests SET embedding = ?1 WHERE id = ?2",
                        rusqlite::params![vec_to_blob(v), id],
                    )
                    .is_err()
                {
                    ok = false;
                }
            }
            ok
        }
        Err(e) => {
            tracing::warn!(target: "4da::embeddings", error = %e, "Interest re-embed failed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_space_is_recognised_even_through_a_quantized_build() {
        // Even an int8 build of the same model measured ~0.90 against Ollama's.
        let c: Vec<Option<f32>> = vec![Some(0.90), Some(0.88), Some(0.93), Some(0.86)];
        assert_eq!(judge_space(&c), SpaceVerdict::Same);
    }

    #[test]
    fn a_different_model_is_recognised() {
        // Arctic vs nomic on the same text measured median 0.007, max 0.10.
        let c: Vec<Option<f32>> = vec![Some(0.01), Some(0.10), Some(-0.02), Some(0.05)];
        assert_eq!(judge_space(&c), SpaceVerdict::Different);
    }

    #[test]
    fn zero_vectors_make_the_probe_inconclusive_not_different() {
        // A provider outage must never trigger a corpus-wide re-embed.
        let c: Vec<Option<f32>> = vec![None, None, None, Some(0.02)];
        assert_eq!(judge_space(&c), SpaceVerdict::Unknown);
        assert_eq!(judge_space(&[]), SpaceVerdict::Unknown);
    }

    #[test]
    fn cosine_rejects_zero_and_mismatched_vectors() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), None);
        assert_eq!(cosine(&[1.0, 0.0], &[1.0]), None);
        let c = cosine(&[1.0, 0.0], &[1.0, 0.0]).expect("defined");
        assert!((c - 1.0).abs() < 1e-6);
    }

    #[test]
    fn blob_round_trip_preserves_the_vector() {
        let v = vec![0.5f32, -1.25, 3.0];
        assert_eq!(blob_to_vec(&vec_to_blob(&v)), v);
    }

    /// The claim the fallback switch rests on, checked through the SHIPPED code
    /// path: the in-process model writes the space Ollama's nomic-embed-text
    /// writes. Run: `cargo test --lib -- --ignored fastembed_and_ollama_share`.
    #[cfg(feature = "fastembed-local")]
    #[test]
    #[ignore = "needs the bundled ONNX model and a local Ollama serving nomic-embed-text"]
    fn fastembed_and_ollama_share_one_space() {
        let texts: Vec<String> = [
            "Tokio 1.48 released with a faster scheduler",
            "CVE-2026-1234: prototype pollution in lodash",
            "Tauri 2.3 adds a new updater plugin",
            "How we cut our Postgres bill in half",
            "Axum 0.9: extractor API changes",
            "React 19.3 release notes",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let local = crate::embeddings::fastembed_sync(&texts).expect("fastembed");
        let body = serde_json::json!({"model": "nomic-embed-text", "input": texts});
        let resp: serde_json::Value = reqwest::blocking::Client::new()
            .post("http://127.0.0.1:11434/api/embed")
            .json(&body)
            .send()
            .expect("ollama reachable")
            .json()
            .expect("json");
        let remote: Vec<Vec<f32>> =
            serde_json::from_value(resp["embeddings"].clone()).expect("vectors");
        let cosines: Vec<Option<f32>> = local
            .iter()
            .zip(&remote)
            .map(|(a, b)| cosine(a, b))
            .collect();
        eprintln!("fastembed vs Ollama nomic cosines: {cosines:?}");
        assert_eq!(judge_space(&cosines), SpaceVerdict::Same);
    }

    #[test]
    fn item_text_matches_the_ingest_builder() {
        // Same builder the ingest path uses: title twice, then the content.
        let t = item_embedding_text("devto", "Tokio 1.48", "Released today.");
        assert_eq!(t, "Tokio 1.48\n\nTokio 1.48\n\nReleased today.");
        assert_eq!(item_embedding_text("devto", "Only title", ""), "Only title");
    }
}
