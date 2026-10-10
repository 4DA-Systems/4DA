// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Hybrid search combining BM25 (FTS5) and vector similarity (sqlite-vec) via
//! Reciprocal Rank Fusion (RRF). Provides better recall than either method alone,
//! especially for developer content with exact technical terms.
//!
//! An **exact-title lane** runs ahead of fusion. RRF alone cannot rank a keyword-only
//! hit: with weights 0.4 (FTS) / 0.6 (vector) and k = 60, FTS rank 1 scores
//! 0.4/61 = 0.00656 while vector rank 30 scores 0.6/90 = 0.00667, so an item that only
//! the keyword leg found was always cut by the final truncate. Measured 2026-10-07:
//! searching "rusqlite" returned rumdl / rsconstruct / rusdu while
//! `crates.io: rusqlite v0.40.2` sat at FTS rank 6. Items whose title contains the
//! query as a whole word are now pinned first, and the top keyword hits are
//! guaranteed a slot in what remains.

use std::collections::HashMap;

use rusqlite::{params, Connection, Result as SqliteResult};
use tracing::debug;

use super::{embedding_to_blob, Database};

#[path = "hybrid_search_exact.rs"]
mod exact;
use exact::{exact_title_queries, sanitize_fts5_query, title_has_exact_phrase};

#[cfg(test)]
#[path = "hybrid_search_tests.rs"]
mod tests;

/// Result from hybrid search combining keyword and semantic signals.
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub item_id: i64,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub rrf_score: f64,
    pub bm25_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    /// Raw L2 distance from the vector KNN leg (None for keyword-only matches).
    /// Used to compute an absolute semantic-relevance score instead of a
    /// rank-ratio that always saturates the top hit at 1.0.
    pub vec_distance: Option<f64>,
    /// The item's title contains the query as a whole word or phrase. These are
    /// pinned ahead of the fused ranking (see the module docs).
    pub exact_title: bool,
}

/// RRF smoothing constant (Cormack et al. 2009). Higher values dampen rank differences.
const RRF_K: f64 = 60.0;

/// At most this many exact-title matches are pinned ahead of the fused ranking.
const MAX_EXACT_PINS: usize = 5;

/// Exact-lane FTS candidates fetched before the word-boundary filter.
const EXACT_LANE_CANDIDATES: i64 = 50;

/// The top N keyword (BM25) hits are guaranteed a place in the final results even
/// when their fused RRF score falls below the truncation line.
const GUARANTEED_FTS_SLOTS: usize = 3;

/// One row from a single retrieval leg; its rank is its 1-based position in the leg.
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub distance: Option<f64>,
}

impl Candidate {
    fn to_result(&self) -> HybridSearchResult {
        HybridSearchResult {
            item_id: self.id,
            title: self.title.clone(),
            content: self.content.clone(),
            source_type: self.source_type.clone(),
            url: self.url.clone(),
            created_at: self.created_at.clone(),
            rrf_score: 0.0,
            bm25_rank: None,
            vec_rank: None,
            vec_distance: self.distance,
            exact_title: false,
        }
    }
}

fn candidate_from_row(row: &rusqlite::Row<'_>, with_distance: bool) -> SqliteResult<Candidate> {
    Ok(Candidate {
        id: row.get(0)?,
        title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        content: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        source_type: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        url: row.get(4)?,
        created_at: row.get(5)?,
        distance: if with_distance {
            Some(row.get::<_, Option<f64>>(6)?.unwrap_or(f64::MAX))
        } else {
            None
        },
    })
}

/// Run one retrieval leg. Any SQL error yields an empty leg, so search degrades to
/// the remaining legs instead of failing.
fn run_leg<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    with_distance: bool,
) -> Vec<Candidate> {
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params, |row| candidate_from_row(row, with_distance)) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

/// Exact-title lane: FTS5 column-filtered phrase match on `title`, ranked by BM25 with
/// the title weighted 10x, then filtered to whole-word title matches in Rust (FTS5's
/// porter stemmer and tokenizer are looser than "exact": `rusqlite-migration` and
/// `runs`→`running` both match at the FTS layer).
fn exact_title_lane(conn: &Connection, query_text: &str) -> Vec<Candidate> {
    let mut pinned: Vec<Candidate> = Vec::new();
    for (fts_query, terms) in exact_title_queries(query_text) {
        let rows = run_leg(
            conn,
            "SELECT si.id, si.title, si.content, si.source_type, si.url, si.created_at
             FROM source_items_fts fts
             JOIN source_items si ON si.id = fts.rowid
             WHERE source_items_fts MATCH ?1
               AND si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)
             ORDER BY bm25(source_items_fts, 10.0, 1.0)
             LIMIT ?2",
            params![fts_query, EXACT_LANE_CANDIDATES],
            false,
        );
        for cand in rows {
            if pinned.len() >= MAX_EXACT_PINS {
                return pinned;
            }
            if title_has_exact_phrase(&cand.title, &terms)
                && !pinned.iter().any(|p| p.id == cand.id)
            {
                pinned.push(cand);
            }
        }
    }
    pinned
}

/// BM25 keyword leg over `source_items_fts` (prefix-matched tokens).
fn bm25_leg(conn: &Connection, query_text: &str, k: usize) -> Vec<Candidate> {
    let fts_query = sanitize_fts5_query(query_text);
    if fts_query.is_empty() {
        return Vec::new();
    }
    run_leg(
        conn,
        "SELECT si.id, si.title, si.content, si.source_type, si.url, si.created_at
         FROM source_items_fts fts
         JOIN source_items si ON si.id = fts.rowid
         WHERE source_items_fts MATCH ?1
           AND si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)
         ORDER BY rank
         LIMIT ?2",
        params![fts_query, k as i64],
        false,
    )
}

/// Vector KNN leg via sqlite-vec. KNN needs `k = ?` in WHERE, never a trailing LIMIT.
fn vector_leg(conn: &Connection, query_embedding: &[f32], k: usize) -> Vec<Candidate> {
    if !query_embedding.iter().any(|&v| v != 0.0) {
        return Vec::new();
    }
    let embedding_blob = embedding_to_blob(query_embedding);
    // A source the user turned off is not searched (AD-054). The KNN runs
    // before that filter, so it over-fetches and the leg keeps the best `k`
    // that survive — otherwise a corpus that is mostly turned-off interests
    // would leave this leg nearly empty.
    let mut rows = run_leg(
        conn,
        "SELECT sv.rowid, si.title, si.content, si.source_type, si.url, si.created_at, sv.distance
         FROM source_vec sv
         JOIN source_items si ON si.id = sv.rowid
         WHERE sv.embedding MATCH ?1 AND k = ?2
           AND si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)
         ORDER BY sv.distance",
        params![embedding_blob, (k * VECTOR_OVERFETCH) as i64],
        true,
    );
    rows.truncate(k);
    rows
}

/// How many KNN neighbours the vector leg reads per result it keeps.
const VECTOR_OVERFETCH: usize = 4;

/// Reciprocal Rank Fusion of the two legs, sorted by descending fused score.
pub(crate) fn fuse_rrf(
    bm25: &[Candidate],
    vector: &[Candidate],
    fts_weight: f64,
    vec_weight: f64,
) -> Vec<HybridSearchResult> {
    let mut scores: HashMap<i64, HybridSearchResult> = HashMap::new();
    for (i, cand) in bm25.iter().enumerate() {
        let rank = i + 1;
        let entry = scores.entry(cand.id).or_insert_with(|| cand.to_result());
        entry.rrf_score += fts_weight / (RRF_K + rank as f64);
        entry.bm25_rank = Some(rank);
    }
    for (i, cand) in vector.iter().enumerate() {
        let rank = i + 1;
        let entry = scores.entry(cand.id).or_insert_with(|| cand.to_result());
        entry.rrf_score += vec_weight / (RRF_K + rank as f64);
        entry.vec_rank = Some(rank);
        entry.vec_distance = cand.distance;
    }
    let mut fused: Vec<HybridSearchResult> = scores.into_values().collect();
    sort_by_rrf(&mut fused);
    fused
}

fn sort_by_rrf(results: &mut [HybridSearchResult]) {
    results.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.item_id.cmp(&b.item_id))
    });
}

/// Pin up to [`MAX_EXACT_PINS`] exact-title matches first (keeping their fused ranks
/// when the legs also found them), fill the remaining budget from the fused ranking,
/// and guarantee the top [`GUARANTEED_FTS_SLOTS`] keyword hits a place.
pub(crate) fn merge_with_exact_lane(
    exact: &[Candidate],
    mut fused: Vec<HybridSearchResult>,
    limit: usize,
) -> Vec<HybridSearchResult> {
    let mut out: Vec<HybridSearchResult> = Vec::with_capacity(limit);
    for cand in exact.iter().take(MAX_EXACT_PINS) {
        if out.len() >= limit || out.iter().any(|r| r.item_id == cand.id) {
            continue;
        }
        let mut pinned = match fused.iter().position(|r| r.item_id == cand.id) {
            Some(pos) => fused.remove(pos),
            None => cand.to_result(),
        };
        pinned.exact_title = true;
        out.push(pinned);
    }
    let budget = limit.saturating_sub(out.len());
    let tail = fused.split_off(budget.min(fused.len()));
    let mut head = fused;
    guarantee_fts_slots(&mut head, tail, budget);
    out.extend(head);
    out
}

fn is_guaranteed_fts(r: &HybridSearchResult) -> bool {
    r.bm25_rank.is_some_and(|rank| rank <= GUARANTEED_FTS_SLOTS)
}

/// Swap any top keyword hit that fell below the cut in for the lowest-scoring head
/// entry that is not itself a guaranteed keyword hit.
fn guarantee_fts_slots(
    head: &mut Vec<HybridSearchResult>,
    tail: Vec<HybridSearchResult>,
    budget: usize,
) {
    if budget == 0 {
        return;
    }
    let mut promoted: Vec<HybridSearchResult> =
        tail.into_iter().filter(is_guaranteed_fts).collect();
    promoted.sort_by_key(|r| r.bm25_rank);
    for hit in promoted {
        if head.len() < budget {
            head.push(hit);
        } else if let Some(pos) = head.iter().rposition(|r| !is_guaranteed_fts(r)) {
            head[pos] = hit;
        }
    }
    sort_by_rrf(head);
}

/// Wall-clock per hybrid-search leg, in milliseconds. `conn_wait_ms` is the time
/// spent getting a connection: near zero on an idle pool, and the whole stall when a
/// background cycle holds every reader and the search falls back to the writer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HybridSearchTimings {
    pub conn_wait_ms: u64,
    pub exact_ms: u64,
    pub fts_ms: u64,
    pub knn_ms: u64,
    pub fuse_ms: u64,
}

fn lap_ms(clock: &mut std::time::Instant) -> u64 {
    let ms = clock.elapsed().as_millis() as u64;
    *clock = std::time::Instant::now();
    ms
}

impl Database {
    /// Hybrid search: exact-title lane, then BM25 keyword matching + vector KNN fused
    /// via RRF.
    ///
    /// `query_text` is the raw user query (for BM25 and the exact lane).
    /// `query_embedding` is the embedded query vector (for KNN).
    /// `limit` is the final number of results to return.
    /// `fts_weight` and `vec_weight` control the blend (should sum to ~1.0).
    pub fn hybrid_search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        fts_weight: f64,
        vec_weight: f64,
    ) -> Vec<HybridSearchResult> {
        self.hybrid_search_timed(query_text, query_embedding, limit, fts_weight, vec_weight)
            .0
    }

    /// [`Self::hybrid_search`] plus the time each leg took. Runs on the interactive
    /// reader, so a background cycle that holds the pool and the writer cannot make
    /// a search wait (see [`Database::interactive_conn`]).
    pub fn hybrid_search_timed(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        fts_weight: f64,
        vec_weight: f64,
    ) -> (Vec<HybridSearchResult>, HybridSearchTimings) {
        let mut t = HybridSearchTimings::default();
        let mut clock = std::time::Instant::now();
        let conn = self.interactive_conn();
        t.conn_wait_ms = lap_ms(&mut clock);
        let k = (limit * 3).max(50); // fetch 3x candidates from each method
        let exact = exact_title_lane(&conn, query_text);
        t.exact_ms = lap_ms(&mut clock);
        let bm25 = bm25_leg(&conn, query_text, k);
        t.fts_ms = lap_ms(&mut clock);
        let vector = vector_leg(&conn, query_embedding, k);
        t.knn_ms = lap_ms(&mut clock);
        drop(conn);

        let fused = fuse_rrf(&bm25, &vector, fts_weight, vec_weight);
        let results = merge_with_exact_lane(&exact, fused, limit);
        t.fuse_ms = lap_ms(&mut clock);

        debug!(
            target: "4da::hybrid_search",
            exact_count = exact.len(),
            bm25_count = bm25.len(),
            vec_count = vector.len(),
            fused_count = results.len(),
            "Hybrid search: exact-title lane + BM25 + vector fused via RRF"
        );
        (results, t)
    }

    /// Verify that `source_items_fts` still agrees with `source_items`.
    ///
    /// `PRAGMA integrity_check` and `PRAGMA quick_check` do **not** cover this: they
    /// validate the b-trees FTS5 stores its index in, which stay perfectly well-formed
    /// while the postings inside them describe text the content table no longer holds.
    /// The same trap exists inside FTS5's own command — for an external-content table
    /// `('integrity-check', 0)` checks only internal consistency, and **only**
    /// `('integrity-check', 1)` recomputes the index checksum from `source_items` and
    /// compares. Rank 0 passed on the founder's corpus while rank 1 failed, so this
    /// deliberately asks for rank 1.
    ///
    /// Cost is a full tokenizing scan of the content table, so this is a diagnostic and
    /// a test assertion — never a startup or per-cycle check.
    ///
    /// Takes the writer connection: the pool is opened `SQLITE_OPEN_READ_ONLY` with
    /// `query_only=ON`, and FTS5 spells its commands as INSERTs, which SQLite refuses
    /// on a read-only connection whether or not they modify anything.
    pub fn fts_integrity_check(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO source_items_fts(source_items_fts, rank) VALUES('integrity-check', 1)",
            [],
        )?;
        Ok(())
    }

    /// Discard the FTS5 index and regenerate it from `source_items`.
    ///
    /// The schema-104 migration runs this once. It stays available because it is the only
    /// repair for a diverged external-content index — `'delete'` commands carrying values
    /// that were never indexed make the divergence worse, not better.
    pub fn rebuild_fts_index(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch("INSERT INTO source_items_fts(source_items_fts) VALUES('rebuild');")?;
        tracing::info!(target: "4da::hybrid_search", "FTS5 search index rebuilt from source_items");
        Ok(())
    }
}
