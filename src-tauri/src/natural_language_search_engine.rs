// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Search engine implementation — hybrid (BM25 + vector KNN, fused via RRF)
//! retrieval for the natural language query pipeline.
//!
//! The legacy `execute_text_search` / `execute_vector_search` pair it replaced
//! was deleted 2026-08-12 after its removal deadline passed with zero callers.

use crate::error::Result;

use super::{ParsedQuery, QueryResultItem};

// ============================================================================
// Hybrid search (BM25 + vector KNN fused via RRF)
// ============================================================================

pub(crate) async fn execute_hybrid_search(
    query_text: &str,
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>> {
    // 1. Embed the query
    let search_text = crate::utils::preprocess_content(&parsed.keywords.join(" "));
    let query_embedding = if !search_text.is_empty() {
        match crate::embeddings::embed_texts(&[search_text]).await {
            Ok(embs) if !embs.is_empty() && embs[0].iter().any(|&v| v != 0.0) => embs[0].clone(),
            _ => vec![],
        }
    } else {
        vec![]
    };

    // 2. Apply ACE context weighting — nudge embedding toward user's tech domain
    let mut weighted_embedding = query_embedding;
    if !weighted_embedding.is_empty() {
        let ace_ctx = crate::scoring::get_ace_context();
        let topic_embeddings = crate::scoring::get_topic_embeddings(&ace_ctx).await;
        if !topic_embeddings.is_empty() {
            let tech_embs: Vec<Vec<f32>> = topic_embeddings.into_values().collect();
            crate::scoring::query_weighting::apply_ace_weighting(
                &mut weighted_embedding,
                &tech_embs,
                0.2,
            );
        }
    }

    // 3. Call hybrid search
    let db = crate::get_database()
        .map_err(|e| crate::error::FourDaError::Internal(format!("DB: {e}")))?;
    let results = db.hybrid_search(query_text, &weighted_embedding, limit, 0.4, 0.6);

    if results.is_empty() {
        return Ok(Vec::new());
    }

    // 4. Compute an ABSOLUTE relevance per item.
    //    Hybrid RRF still determines recall/ranking inside `hybrid_search`, but the
    //    DISPLAYED relevance must be a real, query-stable measure — not the old
    //    `rrf_score / max_score`, which pinned the top hit to exactly 1.00 on every
    //    query and rounded the tightly-clustered runners-up to 1.00 as well.
    //    Vector matches → cosine similarity from the L2 distance (embeddings are
    //    L2-normalized, so cos = 1 - d^2/2), clamped to [0,1]. Keyword-only matches
    //    (no vector distance) → a gentle rank-based score so they read sensibly.
    let mut items: Vec<QueryResultItem> = results
        .into_iter()
        .map(|r| {
            let relevance = match r.vec_distance {
                Some(d) if d.is_finite() => (1.0 - d * d / 2.0).clamp(0.0, 1.0),
                _ => {
                    let rank = r.bm25_rank.unwrap_or(20) as f64;
                    (0.78 / (1.0 + 0.12 * (rank - 1.0))).clamp(0.0, 0.85)
                }
            };
            let match_reason = match (r.bm25_rank, r.vec_rank) {
                (Some(_), Some(_)) => format!("keyword + semantic ({:.0}%)", relevance * 100.0),
                (Some(_), None) => "keyword match".to_string(),
                (None, Some(_)) => format!("semantic similarity ({:.0}%)", relevance * 100.0),
                (None, None) => "match".to_string(),
            };
            let preview = if r.content.len() > 200 {
                format!("{}...", &r.content[..r.content.floor_char_boundary(200)])
            } else {
                r.content
            };
            QueryResultItem {
                id: r.item_id,
                file_path: r.url,
                file_name: Some(r.title),
                preview,
                relevance,
                source_type: r.source_type,
                timestamp: r.created_at,
                match_reason,
            }
        })
        .collect();

    // Order by the displayed relevance so the percentages read monotonically.
    items.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(items)
}
