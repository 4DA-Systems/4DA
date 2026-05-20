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
    pub rrf_score: f64,
    pub bm25_rank: Option<usize>,
    pub vec_rank: Option<usize>,
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
        let mut bm25_results: Vec<(i64, String, String, usize)> = Vec::new();
        if !fts_query.is_empty() {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT si.id, si.title, si.content
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
                    ))
                }) {
                    for (rank, row) in rows.flatten().enumerate() {
                        bm25_results.push((row.0, row.1, row.2, rank + 1));
                    }
                }
            }
        }

        // Stage 2: Vector KNN search via sqlite-vec
        let embedding_blob = embedding_to_blob(query_embedding);
        let has_real_embedding = query_embedding.iter().any(|&v| v != 0.0);
        let mut vec_results: Vec<(i64, String, String, usize)> = Vec::new();
        if has_real_embedding {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT sv.rowid, si.title, si.content
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
                    ))
                }) {
                    for (rank, row) in rows.flatten().enumerate() {
                        vec_results.push((row.0, row.1, row.2, rank + 1));
                    }
                }
            }
        }

        // Stage 3: Reciprocal Rank Fusion
        use std::collections::HashMap;
        let mut scores: HashMap<i64, (f64, Option<usize>, Option<usize>, String, String)> =
            HashMap::new();

        for (id, title, content, rank) in &bm25_results {
            let rrf = fts_weight / (RRF_K + *rank as f64);
            let entry =
                scores
                    .entry(*id)
                    .or_insert((0.0, None, None, title.clone(), content.clone()));
            entry.0 += rrf;
            entry.1 = Some(*rank);
        }

        for (id, title, content, rank) in &vec_results {
            let rrf = vec_weight / (RRF_K + *rank as f64);
            let entry =
                scores
                    .entry(*id)
                    .or_insert((0.0, None, None, title.clone(), content.clone()));
            entry.0 += rrf;
            entry.2 = Some(*rank);
        }

        let mut results: Vec<HybridSearchResult> = scores
            .into_iter()
            .map(
                |(id, (score, bm25_rank, vec_rank, title, content))| HybridSearchResult {
                    item_id: id,
                    title,
                    content,
                    rrf_score: score,
                    bm25_rank,
                    vec_rank,
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
