// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Search engine implementation — hybrid (exact-title lane + BM25 + vector KNN,
//! fused via RRF) retrieval for the natural language query pipeline.
//!
//! The legacy `execute_text_search` / `execute_vector_search` pair it replaced
//! was deleted 2026-08-12 after its removal deadline passed with zero callers.

use crate::db::hybrid_search::HybridSearchResult;
use crate::error::{FourDaError, Result};

use super::{ParsedQuery, QueryResultItem};

/// Run synchronous SQLite / CPU work off the async runtime's worker threads.
/// A command that holds a tokio worker for a multi-second DB scan stalls every
/// other async command scheduled on that worker.
pub(crate) async fn run_blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| FourDaError::Internal(format!("search task failed: {e}")))?
}

// ============================================================================
// Hybrid search (exact-title lane + BM25 + vector KNN fused via RRF)
// ============================================================================

/// Embed the query and nudge it toward the user's tech domain (ACE weighting).
/// Returns an empty vector when embeddings are unavailable (keyword-only search).
async fn embed_query(parsed: &ParsedQuery) -> Vec<f32> {
    let search_text = crate::utils::preprocess_content(&parsed.keywords.join(" "));
    if search_text.is_empty() {
        return Vec::new();
    }
    let mut embedding = match crate::embeddings::embed_texts(&[search_text]).await {
        Ok(embs) if !embs.is_empty() && embs[0].iter().any(|&v| v != 0.0) => embs[0].clone(),
        _ => return Vec::new(),
    };

    let ace_ctx = run_blocking(|| Ok(crate::scoring::get_ace_context()))
        .await
        .unwrap_or_default();
    let topic_embeddings = crate::scoring::get_topic_embeddings(&ace_ctx).await;
    if !topic_embeddings.is_empty() {
        let tech_embs: Vec<Vec<f32>> = topic_embeddings.into_values().collect();
        crate::scoring::query_weighting::apply_ace_weighting(&mut embedding, &tech_embs, 0.2);
    }
    embedding
}

pub(crate) async fn execute_hybrid_search(
    query_text: &str,
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>> {
    let weighted_embedding = embed_query(parsed).await;

    let query_owned = query_text.to_string();
    let results = run_blocking(move || {
        let db = crate::get_database().map_err(|e| FourDaError::Internal(format!("DB: {e}")))?;
        Ok(db.hybrid_search(&query_owned, &weighted_embedding, limit, 0.4, 0.6))
    })
    .await?;

    let mut items: Vec<QueryResultItem> = results
        .into_iter()
        .enumerate()
        .map(|(position, r)| to_query_item(position, r))
        .collect();
    sort_items(&mut items);
    Ok(items)
}

/// Compute an ABSOLUTE relevance per item.
///
/// Hybrid RRF still determines recall/ranking inside `hybrid_search`, but the
/// DISPLAYED relevance must be a real, query-stable measure — not the old
/// `rrf_score / max_score`, which pinned the top hit to exactly 1.00 on every
/// query. Exact-title matches are the one case that earns the top of the scale
/// (they are pinned first by `hybrid_search`, `position` keeps their order).
/// Vector matches → cosine similarity from the L2 distance (embeddings are
/// L2-normalized, so cos = 1 - d^2/2), clamped to [0,1]. Keyword-only matches
/// (no vector distance) → a gentle rank-based score so they read sensibly.
fn to_query_item(position: usize, r: HybridSearchResult) -> QueryResultItem {
    let relevance = if r.exact_title {
        1.0 - 0.001 * position as f64
    } else {
        match r.vec_distance {
            // 0.99 ceiling: only an exact-title match reads as 100%.
            Some(d) if d.is_finite() => (1.0 - d * d / 2.0).clamp(0.0, 0.99),
            _ => {
                let rank = r.bm25_rank.unwrap_or(20) as f64;
                (0.78 / (1.0 + 0.12 * (rank - 1.0))).clamp(0.0, 0.85)
            }
        }
    };
    let match_reason = match (r.exact_title, r.bm25_rank, r.vec_rank) {
        (true, _, _) => "exact title match".to_string(),
        (false, Some(_), Some(_)) => format!("keyword + semantic ({:.0}%)", relevance * 100.0),
        (false, Some(_), None) => "keyword match".to_string(),
        (false, None, Some(_)) => format!("semantic similarity ({:.0}%)", relevance * 100.0),
        (false, None, None) => "match".to_string(),
    };
    QueryResultItem {
        id: r.item_id,
        file_path: r.url,
        file_name: Some(r.title),
        preview: crate::utils::truncate_display(&r.content, 200),
        relevance,
        source_type: r.source_type,
        timestamp: r.created_at,
        match_reason,
        exact_match: r.exact_title,
    }
}

/// Exact-title matches first, then by displayed relevance so the percentages
/// read monotonically.
pub(crate) fn sort_items(items: &mut [QueryResultItem]) {
    items.sort_by(|a, b| {
        b.exact_match.cmp(&a.exact_match).then(
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
}
