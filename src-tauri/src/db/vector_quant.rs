// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Compact copies of every item vector, for the search KNN (Phase 127).
//!
//! The vector leg of hybrid search used to run sqlite-vec's brute-force KNN
//! over `source_vec`: 169,495 × 768 f32 = 498 MB read on every query. On a
//! machine whose file cache could not keep that resident (the founder's,
//! 2026-10-11: 0.43 GB of cache under fleet load), the first search after a
//! few idle minutes spent 3.9-4.8 s in that scan (`knn_ms` in the search log)
//! while the embedding took 16-360 ms. Since Phase 126 most of those vectors
//! also belong to turned-off interests, so the scan read 498 MB to keep ~8%
//! of it, and the leg often came back with fewer than `k` neighbours.
//!
//! Two plain tables now carry what the search needs:
//! - `item_vec_bits` — one sign bit per dimension (96 bytes) plus the
//!   item's `source_type`, so the scan skips turned-off sources. ~20 MB.
//! - `item_vec_i8`   — the vector at 8 bits per dimension with one f32 scale
//!   per vector (772 bytes, stored inline in its b-tree page), read by id for
//!   the candidates only.
//!
//! A query takes the `k × OVERSAMPLE` nearest by Hamming distance over the
//! bits, re-ranks those by L2 against the int8 vectors and keeps `k`.
//! Measured on the live corpus (40 queries, 2026-10-11) against the exact
//! float KNN: recall@10 0.992, recall@30 0.986; the cosine shown to the user
//! differs by at most 0.0013.
//!
//! `source_vec` stays the canonical store (`item_embeddings`); these tables
//! are derived from it. Every vector write goes through
//! `item_embeddings::write_item_vector`, which calls [`write`]. Deleting an
//! item removes its rows by trigger (these are ordinary tables, unlike vec0).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};

use super::blob_to_embedding;

const DIMS: usize = crate::EMBEDDING_DIMS;
const VECTOR_BYTES: usize = DIMS * 4;
const BIT_BYTES: usize = DIMS.div_ceil(8);
const WORDS: usize = DIMS.div_ceil(64);
/// Packed int8 vector: f32 scale (little-endian) then one i8 per dimension.
const I8_BYTES: usize = 4 + DIMS;

/// Hamming candidates read per neighbour kept. 16 gave recall@30 0.986 on the
/// live corpus; 8 gave 0.971.
pub(crate) const OVERSAMPLE: usize = 16;

/// Ids per `IN (...)` lookup — well under SQLite's bound-parameter limit.
const LOOKUP_BATCH: usize = 900;

/// Ids per vector read while backfilling.
const BACKFILL_BATCH: usize = 2_000;

pub(crate) const SCHEMA_SQL: &str = "
    CREATE TABLE IF NOT EXISTS item_vec_bits (
        id INTEGER PRIMARY KEY,
        source_type TEXT NOT NULL,
        bits BLOB NOT NULL
    );
    CREATE TABLE IF NOT EXISTS item_vec_i8 (
        id INTEGER PRIMARY KEY,
        v BLOB NOT NULL
    );
    CREATE TRIGGER IF NOT EXISTS item_vec_quant_item_delete
    AFTER DELETE ON source_items BEGIN
        DELETE FROM item_vec_bits WHERE id = OLD.id;
        DELETE FROM item_vec_i8 WHERE id = OLD.id;
    END;
";

/// One bit per dimension, set where the component is positive.
pub(crate) fn sign_bits(v: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; BIT_BYTES];
    for (i, &x) in v.iter().take(DIMS).enumerate() {
        if x > 0.0 {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

/// The vector at 8 bits per dimension, scaled by its own largest component.
pub(crate) fn quantize_i8(v: &[f32]) -> Vec<u8> {
    let max = v.iter().take(DIMS).fold(0f32, |m, x| m.max(x.abs()));
    let scale = if max > 0.0 { max / 127.0 } else { 0.0 };
    let mut out = Vec::with_capacity(I8_BYTES);
    out.extend_from_slice(&scale.to_le_bytes());
    for i in 0..DIMS {
        let x = v.get(i).copied().unwrap_or(0.0);
        let q = if scale > 0.0 {
            (x / scale).round().clamp(-127.0, 127.0)
        } else {
            0.0
        };
        out.push(q as i8 as u8);
    }
    out
}

/// L2 distance from `query` to a packed int8 vector, or None if it is malformed.
pub(crate) fn approx_l2(query: &[f32], packed: &[u8]) -> Option<f64> {
    if packed.len() != I8_BYTES || query.len() != DIMS {
        return None;
    }
    let scale = f32::from_le_bytes([packed[0], packed[1], packed[2], packed[3]]);
    let sum: f32 = query
        .iter()
        .zip(&packed[4..])
        .map(|(&q, &b)| {
            let d = q - f32::from(b as i8) * scale;
            d * d
        })
        .sum();
    Some(f64::from(sum).sqrt())
}

fn words(bits: &[u8]) -> Option<[u64; WORDS]> {
    if bits.len() != BIT_BYTES {
        return None;
    }
    let mut w = [0u64; WORDS];
    for (i, chunk) in bits.chunks(8).enumerate() {
        let mut b = [0u8; 8];
        b[..chunk.len()].copy_from_slice(chunk);
        w[i] = u64::from_le_bytes(b);
    }
    Some(w)
}

fn hamming(a: &[u64; WORDS], b: &[u64; WORDS]) -> u32 {
    a.iter().zip(b).map(|(x, y)| (x ^ y).count_ones()).sum()
}

/// Store the compact copies of item `id`'s vector. A blob that is not a full
/// vector removes them instead. Nothing is written for an id with no item row.
pub(crate) fn write(conn: &Connection, id: i64, blob: &[u8]) -> SqliteResult<()> {
    if blob.len() != VECTOR_BYTES {
        return delete(conn, id);
    }
    let v = blob_to_embedding(blob);
    let stored = conn
        .prepare_cached(
            "INSERT OR REPLACE INTO item_vec_bits (id, source_type, bits)
             SELECT id, source_type, ?2 FROM source_items WHERE id = ?1",
        )?
        .execute(params![id, sign_bits(&v)])?;
    if stored == 1 {
        conn.prepare_cached("INSERT OR REPLACE INTO item_vec_i8 (id, v) VALUES (?1, ?2)")?
            .execute(params![id, quantize_i8(&v)])?;
    }
    Ok(())
}

/// Remove item `id`'s compact copies (its vector was dropped, not its item).
pub(crate) fn delete(conn: &Connection, id: i64) -> SqliteResult<()> {
    conn.prepare_cached("DELETE FROM item_vec_bits WHERE id = ?1")?
        .execute(params![id])?;
    conn.prepare_cached("DELETE FROM item_vec_i8 WHERE id = ?1")?
        .execute(params![id])?;
    Ok(())
}

/// The `k` enabled items nearest to `query`, as `(id, L2 distance)` nearest
/// first. `None` when the compact index is empty, so the caller can fall back
/// to the float KNN (a database written before any vector, or a fixture that
/// writes `source_vec` directly).
pub(crate) fn knn(
    conn: &Connection,
    query: &[f32],
    k: usize,
) -> SqliteResult<Option<Vec<(i64, f64)>>> {
    if query.len() != DIMS || k == 0 {
        return Ok(None);
    }
    let any: Option<i64> = conn
        .query_row("SELECT id FROM item_vec_bits LIMIT 1", [], |r| r.get(0))
        .optional()?;
    if any.is_none() {
        return Ok(None);
    }
    let Some(q) = words(&sign_bits(query)) else {
        return Ok(None);
    };

    let want = k.saturating_mul(OVERSAMPLE);
    // Max-heap on (distance, id): the root is the worst candidate kept.
    let mut heap: BinaryHeap<(u32, i64)> = BinaryHeap::with_capacity(want + 1);
    let mut stmt = conn.prepare_cached(
        "SELECT id, bits FROM item_vec_bits
         WHERE source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let Some(w) = words(row.get_ref(1)?.as_blob()?) else {
            continue;
        };
        let d = hamming(&q, &w);
        if heap.len() < want {
            heap.push((d, id));
        } else if heap.peek().is_some_and(|&(worst, _)| d < worst) {
            heap.pop();
            heap.push((d, id));
        }
    }

    let mut ids: Vec<i64> = heap.into_iter().map(|(_, id)| id).collect();
    ids.sort_unstable(); // rowid order reads the int8 table front to back
    let mut scored: Vec<(i64, f64)> = Vec::with_capacity(ids.len());
    for batch in ids.chunks(LOOKUP_BATCH) {
        let placeholders = vec!["?"; batch.len()].join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT id, v FROM item_vec_i8 WHERE id IN ({placeholders})"
        ))?;
        let mut rows = stmt.query(rusqlite::params_from_iter(batch.iter()))?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            if let Some(d) = approx_l2(query, row.get_ref(1)?.as_blob()?) {
                scored.push((id, d));
            }
        }
    }
    scored.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(k);
    Ok(Some(scored))
}

/// Phase 127: create the tables and fill them from `source_vec`. Returns
/// `(indexed, without_vector)`.
pub(crate) fn create_and_backfill(conn: &Connection) -> SqliteResult<(usize, usize)> {
    conn.execute_batch(SCHEMA_SQL)?;
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM source_items ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<SqliteResult<_>>()?;
    let (mut indexed, mut without) = (0usize, 0usize);
    for batch in ids.chunks(BACKFILL_BATCH) {
        let vectors = super::item_embeddings::load_item_vector_blobs(conn, batch)?;
        for &id in batch {
            match vectors.get(&id) {
                Some(blob) if blob.len() == VECTOR_BYTES => {
                    write(conn, id, blob)?;
                    indexed += 1;
                }
                _ => without += 1,
            }
        }
    }
    Ok((indexed, without))
}

#[cfg(test)]
#[path = "vector_quant_tests.rs"]
mod tests;
