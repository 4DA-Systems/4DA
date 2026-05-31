// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Hybrid search combining BM25 (FTS5) and vector similarity (sqlite-vec) via
//! Reciprocal Rank Fusion (RRF). Provides better recall than either method alone,
//! especially for developer content with exact technical terms.

use rusqlite::params;
use tracing::debug;

use super::{embedding_to_blob, Database};

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
}

/// RRF smoothing constant (Cormack et al. 2009). Higher values dampen rank differences.
const RRF_K: f64 = 60.0;

impl Database {
    /// Hybrid search: BM25 keyword matching + vector KNN, fused via RRF.
    ///
    /// `query_text` is the raw user query (for BM25).
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
        let conn = self.read_conn();
        let k = (limit * 3).max(50); // fetch 3x candidates from each method

        // Stage 1: BM25 keyword search via FTS5
        let fts_query = sanitize_fts5_query(query_text);
        let mut bm25_results: Vec<(
            i64,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            usize,
        )> = Vec::new();
        if !fts_query.is_empty() {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT si.id, si.title, si.content, si.source_type, si.url, si.created_at
                 FROM source_items_fts fts
                 JOIN source_items si ON si.id = fts.rowid
                 WHERE source_items_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_query, k as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1).unwrap_or_default(),
                        row.get::<_, String>(2).unwrap_or_default(),
                        row.get::<_, String>(3).unwrap_or_default(),
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                }) {
                    for (rank, row) in rows.flatten().enumerate() {
                        bm25_results.push((row.0, row.1, row.2, row.3, row.4, row.5, rank + 1));
                    }
                }
            }
        }

        // Stage 2: Vector KNN search via sqlite-vec
        let embedding_blob = embedding_to_blob(query_embedding);
        let has_real_embedding = query_embedding.iter().any(|&v| v != 0.0);
        let mut vec_results: Vec<(
            i64,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            usize,
        )> = Vec::new();
        // item_id -> raw L2 distance from the KNN leg, for absolute relevance scoring.
        let mut vec_distances: std::collections::HashMap<i64, f64> =
            std::collections::HashMap::new();
        if has_real_embedding {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT sv.rowid, si.title, si.content, si.source_type, si.url, si.created_at, sv.distance
                 FROM source_vec sv
                 JOIN source_items si ON si.id = sv.rowid
                 WHERE sv.embedding MATCH ?1 AND k = ?2
                 ORDER BY sv.distance",
            ) {
                if let Ok(rows) = stmt.query_map(params![embedding_blob, k as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1).unwrap_or_default(),
                        row.get::<_, String>(2).unwrap_or_default(),
                        row.get::<_, String>(3).unwrap_or_default(),
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, f64>(6).unwrap_or(f64::MAX),
                    ))
                }) {
                    for (rank, row) in rows.flatten().enumerate() {
                        vec_distances.insert(row.0, row.6);
                        vec_results.push((row.0, row.1, row.2, row.3, row.4, row.5, rank + 1));
                    }
                }
            }
        }

        // Stage 3: Reciprocal Rank Fusion
        // Tuple: (rrf_score, bm25_rank, vec_rank, title, content, source_type, url, created_at)
        use std::collections::HashMap;
        type FusionEntry = (
            f64,
            Option<usize>,
            Option<usize>,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        );
        let mut scores: HashMap<i64, FusionEntry> = HashMap::new();

        for (id, title, content, source_type, url, created_at, rank) in &bm25_results {
            let rrf = fts_weight / (RRF_K + *rank as f64);
            let entry = scores.entry(*id).or_insert((
                0.0,
                None,
                None,
                title.clone(),
                content.clone(),
                source_type.clone(),
                url.clone(),
                created_at.clone(),
            ));
            entry.0 += rrf;
            entry.1 = Some(*rank);
        }

        for (id, title, content, source_type, url, created_at, rank) in &vec_results {
            let rrf = vec_weight / (RRF_K + *rank as f64);
            let entry = scores.entry(*id).or_insert((
                0.0,
                None,
                None,
                title.clone(),
                content.clone(),
                source_type.clone(),
                url.clone(),
                created_at.clone(),
            ));
            entry.0 += rrf;
            entry.2 = Some(*rank);
        }

        let mut results: Vec<HybridSearchResult> = scores
            .into_iter()
            .map(
                |(
                    id,
                    (score, bm25_rank, vec_rank, title, content, source_type, url, created_at),
                )| {
                    HybridSearchResult {
                        item_id: id,
                        title,
                        content,
                        source_type,
                        url,
                        created_at,
                        rrf_score: score,
                        bm25_rank,
                        vec_rank,
                        vec_distance: vec_distances.get(&id).copied(),
                    }
                },
            )
            .collect();

        results.sort_by(|a, b| {
            b.rrf_score
                .partial_cmp(&a.rrf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        debug!(
            target: "4da::hybrid_search",
            bm25_count = bm25_results.len(),
            vec_count = vec_results.len(),
            fused_count = results.len(),
            "Hybrid search: BM25 + vector fused via RRF"
        );

        results
    }
}

/// Sanitize a query string for FTS5 MATCH syntax.
/// Escapes special characters and wraps each token for prefix matching.
fn sanitize_fts5_query(input: &str) -> String {
    let tokens: Vec<String> = input
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|token| {
            // Remove FTS5 operators and special chars
            let cleaned: String = token
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                .collect();
            if cleaned.is_empty() {
                String::new()
            } else {
                // Quote to prevent operator interpretation, add * for prefix match
                format!("\"{}\"*", cleaned)
            }
        })
        .filter(|t| !t.is_empty())
        .collect();

    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts5_query_sanitization() {
        assert_eq!(sanitize_fts5_query("tokio async"), "\"tokio\"* \"async\"*");
        assert_eq!(sanitize_fts5_query("react-dom"), "\"react-dom\"*");
        assert_eq!(sanitize_fts5_query(""), "");
        assert_eq!(
            sanitize_fts5_query("OR AND NOT"),
            "\"OR\"* \"AND\"* \"NOT\"*"
        );
    }

    #[test]
    fn rrf_fusion_logic() {
        // Verify RRF constant produces expected score range
        let score_rank1 = 1.0 / (RRF_K + 1.0);
        let score_rank10 = 1.0 / (RRF_K + 10.0);
        assert!(score_rank1 > score_rank10);
        assert!(score_rank1 < 0.02); // rank 1 ~= 0.0164
    }
}
