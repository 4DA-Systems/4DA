// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Incremental maintenance of the per-item context match.
//!
//! The database layer is [`crate::db::context_cache`]; this is the worker that
//! keeps it current and the pure function that makes doing so exact.
//!
//! ## The contract
//!
//! An entry stamped at generation `g` holds the true top-K grounding-eligible
//! chunks over the context corpus as it stood at `g`. To advance it to the
//! current generation we merge in only the chunks that changed in between. That
//! is EXACT, and the proof is short enough to state:
//!
//! - every chunk not in the delta is unchanged, so its distance to this item is
//!   unchanged;
//! - the cached list was the exact top-K at `g`, so every unchanged chunk
//!   outside it has a distance no better than the K-th cached distance;
//! - therefore nothing outside `cached âˆª delta` can enter the new top-K,
//!   *provided no cached entry left the corpus or changed* — which is exactly
//!   the case [`merge_context_delta`] refuses.
//!
//! It relies on the cached list being a true top-K rather than a truncation
//! artifact. Before schema 113 it was not: `find_similar_contexts` over-fetched
//! k=24 from an unpartitioned index and filtered in Rust, so an item whose 24
//! nearest chunks were all prose got a short list, and merging into a short list
//! is unsound. Moving the grounding filter into the vec0 partition key is what
//! makes this module correct, not merely faster.

use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::db::context_cache::{ContextDelta, CONTEXT_MATCH_K};
use crate::db::Database;

/// Chunks changed beyond which merging stops being cheaper than recomputing.
/// A full re-index blows past it, which is correct — everything really did
/// change, and `O(items x 15,599)` merges cost more than the KNN they replace.
const DELTA_MERGE_CAP: usize = 2_000;

/// Items per refresh chunk. Bounds both the writer-lock hold and the memory
/// held for one batch of 768-dim embeddings (2,000 x 3 KB = 6 MB).
const REFRESH_CHUNK: usize = 2_000;

/// Squared-free Euclidean distance, matching what vec0 reports for a
/// `float[N]` column.
///
/// Verified against the linked sqlite-vec 0.1.9 build rather than assumed: two
/// orthogonal unit vectors come back as 1.4142135, i.e. sqrt(2) — L2, not
/// squared L2. If that ever changes, the cache produces distances that disagree
/// with a fresh query and `live_context_cache_equivalence` fails loudly.
pub(crate) fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

/// Advance a cached top-K list across a context-corpus delta, exactly.
///
/// Returns `None` when the list cannot be maintained incrementally — a cached
/// chunk was deleted, reclassified, or re-embedded, so its own distance may have
/// moved in either direction and no amount of adding candidates can repair the
/// ordering. The caller recomputes those items from the index.
///
/// Replaying a delta that is already reflected in the list is a no-op (entries
/// are keyed by `context_id` and replaced, not appended), so a cache stamped at
/// a stale generation is repaired rather than corrupted.
pub(crate) fn merge_context_delta(
    cached: &[(i64, f32)],
    item_embedding: &[f32],
    delta: &[ContextDelta],
    k: usize,
) -> Option<Vec<(i64, f32)>> {
    if delta
        .iter()
        .any(|d| cached.iter().any(|(id, _)| *id == d.context_id))
    {
        return None;
    }

    let mut merged: Vec<(i64, f32)> = cached.to_vec();
    for d in delta {
        if d.deleted || !d.grounds || d.embedding.len() != item_embedding.len() {
            continue;
        }
        let distance = l2_distance(item_embedding, &d.embedding);
        merged.retain(|(id, _)| *id != d.context_id);
        merged.push((d.context_id, distance));
    }
    // Ties broken by context_id so the merge is deterministic. Exact float ties
    // across 768 real-valued dimensions do not occur in practice; determinism
    // here is about the verifier, not about correctness of the axis.
    merged.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    merged.truncate(k);
    Some(merged)
}

/// What one refresh pass achieved.
#[derive(Debug, Clone, Default)]
pub(crate) struct ContextCacheRefresh {
    /// Entries advanced by merging a delta — the cheap path.
    pub merged: usize,
    /// Entries computed from the vector index — the 52 ms path.
    pub recomputed: usize,
    /// Items still missing or stale after this pass.
    pub remaining: i64,
    pub elapsed_ms: u128,
}

/// Bring the context-match cache up to the current generation, bounded by
/// `budget`.
///
/// Runs BEFORE any drain: `score_item` only ever READS this cache and falls back
/// to a live KNN on a miss, so a drain against a cold cache is exactly as slow as
/// it always was. Warming it first is what turns a 22-minute corpus re-score into
/// a ~1-minute one.
pub(crate) fn refresh_context_cache(db: &Database, budget: Duration) -> ContextCacheRefresh {
    let started = Instant::now();
    let mut out = ContextCacheRefresh::default();

    let generation = match db.context_generation() {
        Ok(g) => g,
        Err(e) => {
            warn!(target: "4da::ctxcache", error = %e, "Context generation unreadable — cache refresh skipped");
            return out;
        }
    };

    // Nothing to ground against: an empty context corpus means score_item never
    // calls the KNN at all, so a cache would be caching nothing.
    if db.context_count().unwrap_or(0) == 0 {
        return out;
    }

    while started.elapsed() < budget {
        let batch = match db.items_needing_context_cache(generation, REFRESH_CHUNK) {
            Ok(b) => b,
            Err(e) => {
                warn!(target: "4da::ctxcache", error = %e, "Cache selection failed");
                break;
            }
        };
        if batch.is_empty() {
            break;
        }

        let ids: Vec<i64> = batch.iter().map(|(id, _)| *id).collect();
        let embeddings = db.embeddings_for_items(&ids).unwrap_or_default();

        // Deltas are loaded relative to the OLDEST cached generation in the
        // batch, so one load serves every item in it.
        let oldest = batch.iter().filter_map(|(_, g)| *g).min();
        let delta = oldest
            .and_then(|g| db.context_delta_since(g, DELTA_MERGE_CAP).ok().flatten())
            .unwrap_or_default();
        let cached_pairs = if delta.is_empty() && oldest.is_none() {
            std::collections::HashMap::new()
        } else {
            db.cached_context_pairs(&ids).unwrap_or_default()
        };

        let computed = compute_batch(db, &batch, &embeddings, &cached_pairs, &delta);
        out.merged += computed.iter().filter(|(_, _, m)| *m).count();
        out.recomputed += computed.iter().filter(|(_, _, m)| !*m).count();

        let to_store: Vec<(i64, Vec<(i64, f32)>)> =
            computed.into_iter().map(|(id, v, _)| (id, v)).collect();
        if let Err(e) = db.store_context_matches(&to_store, generation) {
            warn!(target: "4da::ctxcache", error = %e, "Cache write failed — next pass retries");
            break;
        }
    }

    // The log only answers "what changed since the oldest surviving entry", so
    // everything below that is unreachable. Left unpruned it grows without
    // bound on an actively-edited codebase.
    if let Err(e) = db.prune_context_change_log() {
        warn!(target: "4da::ctxcache", error = %e, "Change-log prune failed (non-fatal)");
    }

    out.remaining = db
        .items_needing_context_cache(generation, 1)
        .map(|v| v.len() as i64)
        .unwrap_or(0);
    out.elapsed_ms = started.elapsed().as_millis();
    if out.merged + out.recomputed > 0 {
        let (cached, total) = db.context_cache_coverage(generation).unwrap_or((0, 0));
        info!(
            target: "4da::ctxcache",
            merged = out.merged,
            recomputed = out.recomputed,
            more_remaining = out.remaining > 0,
            generation,
            coverage = format!("{cached}/{total}"),
            elapsed_ms = out.elapsed_ms,
            "Context-match cache refreshed"
        );
    }
    out
}

/// Compute one batch, in parallel across the read pool.
///
/// Same threading contract as `analysis_backfill::score_chunk`: one thread per
/// pooled reader, each borrowing its own connection for the KNN, and no DB
/// WRITES inside the parallel region — the caller persists the whole batch in a
/// single transaction afterwards. Returns `(item_id, matches, was_merged)`.
fn compute_batch(
    db: &Database,
    batch: &[(i64, Option<i64>)],
    embeddings: &std::collections::HashMap<i64, Vec<f32>>,
    cached_pairs: &std::collections::HashMap<i64, Vec<(i64, f32)>>,
    delta: &[ContextDelta],
) -> Vec<(i64, Vec<(i64, f32)>, bool)> {
    let one = |(item_id, cached_gen): &(i64, Option<i64>)| -> (i64, Vec<(i64, f32)>, bool) {
        let Some(embedding) = embeddings.get(item_id) else {
            return (*item_id, Vec::new(), false);
        };
        // A zero or absent vector produces uniform distances against every chunk
        // — score_item refuses to ground on it, so cache the empty answer rather
        // than a meaningless one.
        if embedding.is_empty() || !embedding.iter().any(|v| *v != 0.0) {
            return (*item_id, Vec::new(), false);
        }
        if cached_gen.is_some() {
            if let Some(cached) = cached_pairs.get(item_id) {
                if let Some(merged) = merge_context_delta(cached, embedding, delta, CONTEXT_MATCH_K)
                {
                    return (*item_id, merged, true);
                }
            }
        }
        let fresh = db
            .find_similar_contexts(embedding, CONTEXT_MATCH_K)
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.context_id, m.distance))
            .collect();
        (*item_id, fresh, false)
    };

    let threads = db
        .read_pool_len()
        .min(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    if threads <= 1 || batch.len() < 64 {
        return batch.iter().map(one).collect();
    }
    let chunk = batch.len().div_ceil(threads);
    let one_ref = &one;
    std::thread::scope(|s| {
        let handles: Vec<_> = batch
            .chunks(chunk)
            .map(|c| s.spawn(move || c.iter().map(one_ref).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

#[cfg(test)]
#[path = "context_cache_tests.rs"]
mod context_cache_tests;
