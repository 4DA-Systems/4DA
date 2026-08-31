// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Materialised per-item context match — the database layer.
//!
//! ## Why this exists
//!
//! Measured on the live corpus 2026-08-27, scoring one item costs 54.7 ms, of
//! which **52.0 ms (95.8%)** is a single sqlite-vec KNN over the context corpus.
//! Everything else in PASIFA — topic extraction, dependency matching across 143
//! packages, keyword and interest scoring, content DNA, the confirmation gate,
//! every boost and ceiling — is the remaining 2.7 ms.
//!
//! That KNN is a pure function of two inputs: the item's embedding, which never
//! changes, and the context corpus, which changed **145 of 15,599 chunks in
//! fifteen days**. Neither input moves when the SCORING pipeline changes: the
//! v25+v26 arc was 17 commits and 4,392 lines, and *zero* of them were in
//! `db/mod.rs`, `context_admission.rs`, `embeddings.rs` or `context_engine/`.
//! So a 47,000-item drain spent 96% of its wall clock recomputing a value that
//! provably could not have moved.
//!
//! ## The invalidation rule, and the one it is NOT
//!
//! A bare generation counter — "any context write invalidates every cache
//! entry" — would have invalidated 100% of the cache on those 145 writes. The
//! measurement that motivates this module is a *precise* one: of 600 sampled
//! items, **zero** had a top-3 that touched a changed chunk. So the cache is
//! maintained INCREMENTALLY: an entry stamped at generation `g` is advanced to
//! the current generation by merging only the chunks that changed in between
//! (`scoring::context_cache::merge_context_delta`), which is exact, and costs
//! `O(items x changed_chunks)` instead of `O(items x corpus)`.
//!
//! Exactness depends on [`Database::find_similar_contexts`] returning a true
//! top-K rather than a truncation artifact — which is why schema 113 moved the
//! grounding filter into the vec0 partition key. A list that might have been
//! cut short by an over-fetch cannot be repaired by adding candidates to it.
//!
//! ## What is stored
//!
//! Raw `(context_id, distance)` only. The boilerplate filter and the 100-char
//! truncation stay at the read site in `pipeline_v2`, so the cached value is
//! exactly what `find_similar_contexts` returns and the two paths cannot drift.

use rusqlite::{params, Result as SqliteResult};

use super::{embedding_to_blob, Database, SimilarityResult};

/// Semantics version of a cached entry. Bump when the meaning of a stored
/// `(context_id, distance)` pair changes — a different `k`, a different vec0
/// distance metric, a different eligibility rule. The generation counter tracks
/// the DATA; this tracks the CODE that read it.
pub(crate) const CONTEXT_CACHE_BUILDER: i64 = 1;

/// Context matches kept per item. Mirrors the `find_similar_contexts(.., 3)`
/// call in `pipeline_v2::score_item`; only the first is used by the context
/// axis, the rest are displayed as "similar to your code" evidence.
pub(crate) const CONTEXT_MATCH_K: usize = 3;

/// One context chunk that changed since a given generation, with everything the
/// incremental merge needs to place it.
#[derive(Debug, Clone)]
pub(crate) struct ContextDelta {
    pub context_id: i64,
    /// The chunk is gone. If it was in a cached top-K, that entry cannot be
    /// repaired by merging and must be recomputed.
    pub deleted: bool,
    /// Grounding-eligible under the CURRENT classification (code/config).
    pub grounds: bool,
    pub embedding: Vec<f32>,
}

impl Database {
    /// Current generation of the context corpus: the high-water mark of the
    /// trigger-maintained change log. `0` on a corpus that has not changed
    /// since migration 113 — which is a perfectly good generation, not a
    /// sentinel.
    pub(crate) fn context_generation(&self) -> SqliteResult<i64> {
        let conn = self.read_conn();
        conn.query_row(
            "SELECT COALESCE(MAX(gen), 0) FROM context_change_log",
            [],
            |r| r.get(0),
        )
    }

    /// Chunks that changed after `since`, collapsed to their FINAL state (a
    /// chunk written three times contributes one delta).
    ///
    /// Returns `None` when more than `cap` chunks changed — at that point the
    /// merge is no longer cheaper than a fresh KNN, and the caller recomputes.
    /// A full re-index lands here, which is correct: everything really did
    /// change.
    pub(crate) fn context_delta_since(
        &self,
        since: i64,
        cap: usize,
    ) -> SqliteResult<Option<Vec<ContextDelta>>> {
        let conn = self.read_conn();
        let changed: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT context_id) FROM context_change_log WHERE gen > ?1",
            params![since],
            |r| r.get(0),
        )?;
        if changed as usize > cap {
            return Ok(None);
        }
        if changed == 0 {
            return Ok(Some(Vec::new()));
        }
        let mut stmt = conn.prepare_cached(
            "SELECT l.context_id,
                    MAX(l.gen) AS latest,
                    c.id IS NULL AS gone,
                    COALESCE(c.source_type, '') AS source_type,
                    c.embedding
             FROM context_change_log l
             LEFT JOIN context_chunks c ON c.id = l.context_id
             WHERE l.gen > ?1
             GROUP BY l.context_id",
        )?;
        let rows = stmt.query_map(params![since], |row| {
            let gone: bool = row.get(2)?;
            let source_type: String = row.get(3)?;
            let blob: Option<Vec<u8>> = row.get(4)?;
            Ok(ContextDelta {
                context_id: row.get(0)?,
                // A row whose chunk no longer exists is a deletion regardless of
                // what the log said — the log records intent, the join records
                // truth, and truth wins.
                deleted: gone || blob.is_none(),
                grounds: Self::grounds_partition(&source_type) == 1,
                embedding: blob
                    .map(|b| super::blob_to_embedding(&b))
                    .unwrap_or_default(),
            })
        })?;
        rows.collect::<SqliteResult<Vec<_>>>().map(Some)
    }

    /// The cached context match for one item, or `None` on a miss.
    ///
    /// A HIT with zero matches (the item genuinely has no grounding-eligible
    /// neighbour) is distinct from a MISS and returns `Some(vec![])` — the
    /// difference is the whole point, since re-running a 52 ms KNN to rediscover
    /// "still nothing" is exactly the waste this module removes.
    pub(crate) fn cached_context_matches(
        &self,
        item_id: i64,
        generation: i64,
    ) -> SqliteResult<Option<Vec<SimilarityResult>>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT m.context_id, m.distance, c.source_file, c.text
             FROM item_context_cache k
             LEFT JOIN item_context_match m ON m.item_id = k.item_id
             LEFT JOIN context_chunks c ON c.id = m.context_id
             WHERE k.item_id = ?1 AND k.generation = ?2 AND k.builder = ?3
             ORDER BY m.rank",
        )?;
        let rows = stmt.query_map(params![item_id, generation, CONTEXT_CACHE_BUILDER], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<f32>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut out = Vec::with_capacity(CONTEXT_MATCH_K);
        let mut saw_entry = false;
        for row in rows {
            saw_entry = true;
            let (context_id, distance, source_file, text) = row?;
            let Some(context_id) = context_id else {
                continue; // valid entry, no matches
            };
            // A match row whose chunk vanished without bumping the generation
            // should be impossible (the delete trigger bumps it). If it happens
            // anyway, treat the entry as a MISS rather than serving a partial
            // list — a wrong context axis is worse than a slow one.
            let (Some(distance), Some(source_file), Some(text)) = (distance, source_file, text)
            else {
                return Ok(None);
            };
            out.push(SimilarityResult {
                context_id,
                source_file,
                text,
                distance,
            });
        }
        Ok(saw_entry.then_some(out))
    }

    /// The raw cached `(context_id, distance)` lists for a batch of items, for
    /// the incremental merge. Only entries at `generation` or older are useful
    /// to the merge, so the caller passes the generation each was stamped at.
    pub(crate) fn cached_context_pairs(
        &self,
        item_ids: &[i64],
    ) -> SqliteResult<std::collections::HashMap<i64, Vec<(i64, f32)>>> {
        let mut out: std::collections::HashMap<i64, Vec<(i64, f32)>> =
            std::collections::HashMap::new();
        if item_ids.is_empty() {
            return Ok(out);
        }
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT context_id, distance FROM item_context_match
             WHERE item_id = ?1 ORDER BY rank",
        )?;
        for id in item_ids {
            let pairs: Vec<(i64, f32)> = stmt
                .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<SqliteResult<Vec<_>>>()?;
            out.insert(*id, pairs);
        }
        Ok(out)
    }

    /// Write a batch of computed matches, stamping them at `generation`.
    ///
    /// One transaction for the whole batch: the refresh pass is the only writer
    /// here and it works in chunks, so the writer lock is held in short bursts
    /// rather than for the length of a corpus sweep.
    pub(crate) fn store_context_matches(
        &self,
        entries: &[(i64, Vec<(i64, f32)>)],
        generation: i64,
    ) -> SqliteResult<usize> {
        if entries.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        {
            let mut del = tx.prepare_cached("DELETE FROM item_context_match WHERE item_id = ?1")?;
            let mut ins_m = tx.prepare_cached(
                "INSERT INTO item_context_match (item_id, rank, context_id, distance)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut ins_k = tx.prepare_cached(
                "INSERT INTO item_context_cache (item_id, generation, builder)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(item_id) DO UPDATE SET generation = ?2, builder = ?3",
            )?;
            for (item_id, matches) in entries {
                del.execute(params![item_id])?;
                for (rank, (context_id, distance)) in matches.iter().enumerate() {
                    ins_m.execute(params![item_id, rank as i64, context_id, distance])?;
                }
                ins_k.execute(params![item_id, generation, CONTEXT_CACHE_BUILDER])?;
            }
        }
        tx.commit()?;
        Ok(entries.len())
    }

    /// Items whose cached context match is missing or stamped at a superseded
    /// generation. Ids and their cached generation only — deliberately NOT the
    /// embedding blob, because once the cache is warm this query runs on every
    /// cycle and returns nothing, and reading 53,000 x 3 KB of overflow pages to
    /// discover that would cost more than the work it is looking for.
    pub(crate) fn items_needing_context_cache(
        &self,
        generation: i64,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, Option<i64>)>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached(
            "SELECT si.id, k.generation
             FROM source_items si
             LEFT JOIN item_context_cache k ON k.item_id = si.id
             WHERE k.item_id IS NULL OR k.generation <> ?1 OR k.builder <> ?2
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![generation, CONTEXT_CACHE_BUILDER, limit as i64],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows.collect()
    }

    /// Embeddings for a batch of item ids, skipping absent or zero vectors —
    /// those have no meaningful KNN and are cached as "no matches".
    pub(crate) fn embeddings_for_items(
        &self,
        item_ids: &[i64],
    ) -> SqliteResult<std::collections::HashMap<i64, Vec<f32>>> {
        let mut out = std::collections::HashMap::with_capacity(item_ids.len());
        if item_ids.is_empty() {
            return Ok(out);
        }
        let conn = self.read_conn();
        let mut stmt = conn.prepare_cached("SELECT embedding FROM source_items WHERE id = ?1")?;
        for id in item_ids {
            let blob: Option<Vec<u8>> = stmt.query_row(params![id], |r| r.get(0)).ok();
            let emb = blob
                .map(|b| super::blob_to_embedding(&b))
                .unwrap_or_default();
            out.insert(*id, emb);
        }
        Ok(out)
    }

    /// How much of the corpus is cached at the current generation, and how much
    /// of it there is. The honest health metric for this subsystem.
    pub(crate) fn context_cache_coverage(&self, generation: i64) -> SqliteResult<(i64, i64)> {
        let conn = self.read_conn();
        let cached: i64 = conn.query_row(
            "SELECT COUNT(*) FROM item_context_cache WHERE generation = ?1 AND builder = ?2",
            params![generation, CONTEXT_CACHE_BUILDER],
            |r| r.get(0),
        )?;
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM source_items", [], |r| r.get(0))?;
        Ok((cached, total))
    }

    /// Drop change-log rows no cache entry can still need.
    ///
    /// Safe because the log only exists to answer "what changed since generation
    /// g" for the OLDEST surviving cache entry. Rows at or below that generation
    /// are unreachable. Never prunes the high-water row, so the generation
    /// counter itself cannot go backwards.
    pub(crate) fn prune_context_change_log(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let floor: Option<i64> =
            conn.query_row("SELECT MIN(generation) FROM item_context_cache", [], |r| {
                r.get(0)
            })?;
        let Some(floor) = floor else {
            return Ok(0);
        };
        conn.execute(
            "DELETE FROM context_change_log
             WHERE gen <= ?1 AND gen < (SELECT MAX(gen) FROM context_change_log)",
            params![floor],
        )
    }
}

impl Database {
    /// Grounding matches for one item: the cached value when it is current for
    /// this context generation, otherwise a live KNN.
    ///
    /// NEVER writes. Population belongs to `scoring::context_cache`'s refresh
    /// pass, which batches its writes into one transaction — keeping this path
    /// read-only is what lets `analysis_backfill::score_chunk` run it across
    /// eight threads without any of them touching the writer lock.
    ///
    /// `item_id` must be a real `source_items.id`. The deep-scan path scores
    /// pre-persist items whose id is a hash of `source_type:source_id`, so it
    /// can in principle collide with a live row and be served that row's
    /// matches; at ~53k live ids against a 64-bit hash that is a one-in-3e14
    /// event per deep-scan item, bounded to three evidence chunks, and
    /// self-correcting on the next cycle. Caching those items would be pointless
    /// anyway — they are not in the corpus yet.
    pub(crate) fn context_matches_for_scoring(
        &self,
        item_id: i64,
        embedding: &[f32],
        limit: usize,
        generation: i64,
    ) -> SqliteResult<Vec<SimilarityResult>> {
        if limit == CONTEXT_MATCH_K {
            if let Some(hit) = self.cached_context_matches(item_id, generation)? {
                return Ok(hit);
            }
        }
        self.find_similar_contexts(embedding, limit)
    }
}

// ============================================================================
// Context vector index — the grounding read path and its partition key
// ============================================================================
//
// Extracted from `db/mod.rs` (2026-08-27) when the grounding partition and this
// cache pushed that file past the 1,000-line error threshold. It belongs here on
// the merits too: these two functions and the cache above are one subsystem —
// what "similar to your code" means, and how it is kept current.

impl Database {
    /// The `grounds` partition value for a chunk's persisted `source_type`.
    ///
    /// ONE definition, shared by every `context_vec` write path and mirrored by
    /// the CASE in migration 113. [`ContextClass::grounding_eligible`] stays the
    /// single source of truth for what may ground the feed; this only projects
    /// it onto the partition key. An unrecognised `source_type` (the legacy
    /// 'text' default) is not grounding-eligible, so it partitions to 0 —
    /// exactly what the old Rust-side filter did with it.
    pub(crate) fn grounds_partition(source_type: &str) -> i64 {
        i64::from(
            crate::context_admission::ContextClass::from_source_type(source_type)
                .is_some_and(crate::context_admission::ContextClass::grounding_eligible),
        )
    }

    /// KNN search for similar contexts using sqlite-vec (O(log n) instead of O(n)).
    ///
    /// Grounding is CODE-ONLY: results are restricted to grounding-eligible
    /// provenance (`code`/`config` — see [`crate::context_admission`]). Prose /
    /// doc embeddings are semantic wildcards that once surfaced a Spanish
    /// business course as "Similar to your code" on a Docker tool; they must
    /// never ground the feed nor move the context score. Both scoring pipelines
    /// read from here, so this one filter fixes evidence AND score at once.
    ///
    /// The filter lives INSIDE the index, as a vec0 partition key (schema 113).
    /// It used to be an over-fetch — take k=24 and keep the first 3 eligible in
    /// Rust — because vec0 selects `k` rows FIRST and applies join predicates
    /// after. That was inexact in the direction that matters: an item whose 24
    /// nearest chunks were all prose got fewer than three matches, or none, and
    /// then scored as though the user's codebase held nothing like it. With the
    /// partition, `k = limit` returns the exact top-`limit` eligible rows and
    /// scans only the eligible partition (20% of this corpus is doc/test_code).
    ///
    /// Exactness is load-bearing beyond accuracy: `scoring::context_cache`
    /// maintains this result incrementally, and a top-K that might be a
    /// truncation artifact cannot be maintained by merging a delta into it.
    pub fn find_similar_contexts(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> SqliteResult<Vec<SimilarityResult>> {
        let conn = self.read_conn();
        let embedding_blob = embedding_to_blob(query_embedding);

        let mut stmt = conn.prepare_cached(
            "SELECT v.id, v.distance, c.source_file, c.text, c.source_type
             FROM context_vec v
             JOIN context_chunks c ON c.id = v.id
             WHERE v.embedding MATCH ?1 AND k = ?2 AND v.grounds = 1
             ORDER BY v.distance",
        )?;

        let rows = stmt.query_map(params![embedding_blob, limit as i64], |row| {
            Ok((
                SimilarityResult {
                    context_id: row.get(0)?,
                    distance: row.get(1)?,
                    source_file: row.get(2)?,
                    text: row.get(3)?,
                },
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            ))
        })?;

        // Belt and braces. The partition key already guarantees eligibility, so
        // this can only fire if a write path moved a chunk's `source_type`
        // without moving its vector — the one way the two can drift. Dropping
        // the row keeps grounding correct; the warning says the index needs a
        // rebuild rather than letting the drift stay silent.
        let mut out = Vec::with_capacity(limit);
        let mut drifted = 0usize;
        for row in rows {
            let (res, source_type) = row?;
            let grounds = crate::context_admission::ContextClass::from_source_type(&source_type)
                .is_some_and(crate::context_admission::ContextClass::grounding_eligible);
            if grounds {
                out.push(res);
            } else {
                drifted += 1;
            }
        }
        if drifted > 0 {
            tracing::warn!(
                target: "4da::db",
                drifted,
                "context_vec grounding partition disagrees with context_chunks.source_type — re-index to repair"
            );
        }
        Ok(out)
    }
}
