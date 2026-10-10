// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! An item's embedding lives in ONE place: the `source_vec` vec0 index
//! (Phase 126).
//!
//! Until schema 126 every vector was written twice — into the
//! `source_items.embedding` BLOB and into `source_vec` — and read back from
//! the BLOB everywhere except the KNN search. On the founder snapshot
//! (2026-10-09) that was 163,816 vectors × 3,072 bytes ≈ 503 MB held twice.
//! `source_vec` is the store that can't go (it is what `MATCH` searches), so
//! it is canonical: `source_items.embedding` is kept as an always-empty
//! column (`X''`; the column is `NOT NULL` and dropping it would rewrite the
//! whole table on a user's startup) and every reader loads vectors from here.
//!
//! Bulk reads go through vec0's own storage: `source_vec_rowids` maps a rowid
//! to `(chunk_id, chunk_offset)` and `source_vec_vector_chunks00.vectors`
//! holds 1,024 packed f32 vectors per row. Reading one slice per item through
//! incremental blob I/O costs the same as the old BLOB column (5,000 vectors
//! in ~65 ms warm on the snapshot), where a vec0 point query per row costs
//! ~0.9 ms each. If that layout is ever not there (a sqlite-vec that stores
//! vectors differently), every read falls back to the point query, which is
//! the public interface and always right.

use std::collections::HashMap;

use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Result as SqliteResult};

use super::{blob_to_embedding, parse_datetime, StoredSourceItem};

/// Bytes of one stored vector (`float[EMBEDDING_DIMS]`).
const VECTOR_BYTES: usize = crate::EMBEDDING_DIMS * 4;

/// Ids per `IN (...)` lookup — well under SQLite's bound-parameter limit.
const LOOKUP_BATCH: usize = 900;

/// Column list matching [`stored_item_from_row`], for `SELECT {cols} FROM
/// source_items {prefix}`. `prefix` is the table alias with its dot (`"si."`)
/// or empty. The embedding is NOT in it: call [`attach_embeddings`] after.
pub(crate) fn stored_item_columns(prefix: &str) -> String {
    format!(
        "{p}id, {p}source_type, {p}source_id, {p}url, {p}title, {p}content, {p}content_hash, \
         {p}created_at, {p}last_seen, COALESCE({p}detected_lang, 'en'), {p}feed_origin, {p}tags, \
         {p}published_at",
        p = prefix
    )
}

/// Map a row selected with [`stored_item_columns`]. `embedding` is empty
/// until [`attach_embeddings`] fills it.
pub(crate) fn stored_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSourceItem> {
    Ok(StoredSourceItem {
        id: row.get(0)?,
        source_type: row.get(1)?,
        source_id: row.get(2)?,
        url: row.get(3)?,
        title: row.get(4)?,
        content: row.get(5)?,
        content_hash: row.get(6)?,
        embedding: Vec::new(),
        created_at: parse_datetime(row.get::<_, String>(7)?),
        last_seen: parse_datetime(row.get::<_, String>(8)?),
        detected_lang: row.get::<_, String>(9).unwrap_or_else(|_| "en".to_string()),
        feed_origin: row.get(10).ok().flatten(),
        tags: row.get(11).ok().flatten(),
        published_at: crate::db::parse_datetime_opt(
            row.get::<_, Option<String>>(12).ok().flatten(),
        ),
    })
}

/// Fill `embedding` on every item from `source_vec`. An item with no vector
/// (a pending row) keeps an empty one, exactly what the old empty BLOB gave.
pub(crate) fn attach_embeddings(
    conn: &Connection,
    items: &mut [StoredSourceItem],
) -> SqliteResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    let mut vectors = load_item_vectors(conn, &ids)?;
    for item in items.iter_mut() {
        if let Some(v) = vectors.remove(&item.id) {
            item.embedding = v;
        }
    }
    Ok(())
}

/// Vectors for `ids`, keyed by item id. Ids with no vector are absent.
pub(crate) fn load_item_vectors(
    conn: &Connection,
    ids: &[i64],
) -> SqliteResult<HashMap<i64, Vec<f32>>> {
    Ok(load_item_vector_blobs(conn, ids)?
        .into_iter()
        .map(|(id, blob)| (id, blob_to_embedding(&blob)))
        .collect())
}

/// Raw little-endian f32 blobs for `ids` (for writers that compare bytes).
pub(crate) fn load_item_vector_blobs(
    conn: &Connection,
    ids: &[i64],
) -> SqliteResult<HashMap<i64, Vec<u8>>> {
    match load_chunked(conn, ids) {
        Ok(found) => Ok(found),
        Err(e) => {
            tracing::warn!(target: "4da::db", error = %e,
                "source_vec chunk read unavailable; falling back to point queries");
            load_point(conn, ids)
        }
    }
}

/// One item's vector blob through the vec0 interface.
pub(crate) fn load_item_vector_blob(conn: &Connection, id: i64) -> SqliteResult<Option<Vec<u8>>> {
    conn.query_row(
        "SELECT embedding FROM source_vec WHERE rowid = ?1",
        params![id],
        |row| row.get::<_, Vec<u8>>(0),
    )
    .optional()
}

fn load_point(conn: &Connection, ids: &[i64]) -> SqliteResult<HashMap<i64, Vec<u8>>> {
    let mut out = HashMap::with_capacity(ids.len());
    for &id in ids {
        if let Some(blob) = load_item_vector_blob(conn, id)? {
            out.insert(id, blob);
        }
    }
    Ok(out)
}

fn load_chunked(conn: &Connection, ids: &[i64]) -> SqliteResult<HashMap<i64, Vec<u8>>> {
    // chunk_id -> [(item id, slot)]
    let mut by_chunk: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    for batch in ids.chunks(LOOKUP_BATCH) {
        let placeholders = vec!["?"; batch.len()].join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT rowid, chunk_id, chunk_offset FROM source_vec_rowids WHERE rowid IN ({placeholders})"
        ))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(batch.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (id, chunk, slot) = row?;
            by_chunk.entry(chunk).or_default().push((id, slot));
        }
    }
    let mut out = HashMap::with_capacity(ids.len());
    for (chunk, slots) in by_chunk {
        let blob = conn.blob_open(
            DatabaseName::Main,
            "source_vec_vector_chunks00",
            "vectors",
            chunk,
            true,
        )?;
        for (id, slot) in slots {
            let offset = usize::try_from(slot)
                .ok()
                .and_then(|s| s.checked_mul(VECTOR_BYTES))
                .ok_or(rusqlite::Error::InvalidQuery)?;
            let mut buf = vec![0u8; VECTOR_BYTES];
            let read = blob.read_at(&mut buf, offset)?;
            if read != VECTOR_BYTES {
                // Not the layout we expect — let the caller fall back.
                return Err(rusqlite::Error::InvalidQuery);
            }
            out.insert(id, buf);
        }
    }
    Ok(out)
}

/// Write `blob` as item `id`'s vector. Returns whether the stored vector
/// changed. A change invalidates the item's materialised context match
/// (`item_context_cache` / `item_context_match`), the job the
/// `item_context_cache_reembed` trigger did while the BLOB column was the
/// store (vec0 tables cannot carry triggers, so every vector write comes
/// through here instead).
pub(crate) fn write_item_vector(conn: &Connection, id: i64, blob: &[u8]) -> SqliteResult<bool> {
    let existing = load_item_vector_blob(conn, id)?;
    match existing {
        Some(old) if old == blob => Ok(false),
        Some(_) => {
            conn.execute(
                "UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2",
                params![blob, id],
            )?;
            conn.execute(
                "DELETE FROM item_context_cache WHERE item_id = ?1",
                params![id],
            )?;
            conn.execute(
                "DELETE FROM item_context_match WHERE item_id = ?1",
                params![id],
            )?;
            Ok(true)
        }
        None => {
            conn.execute(
                "INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
                params![id, blob],
            )?;
            Ok(true)
        }
    }
}

/// Test fixtures: store a (possibly low-dimensional) vector for `id`,
/// zero-padded to the index width. Padding changes no cosine, so a fixture
/// written in 8 or 32 dimensions keeps its geometry.
#[cfg(test)]
pub(crate) fn put_test_vector(conn: &Connection, id: i64, v: &[f32]) {
    let mut padded = v.to_vec();
    padded.resize(crate::EMBEDDING_DIMS, 0.0);
    write_item_vector(conn, id, &super::embedding_to_blob(&padded)).expect("test vector");
}

#[cfg(test)]
#[path = "item_embeddings_tests.rs"]
mod tests;
