// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Search engine implementations — text search and vector similarity search
//! for the natural language query pipeline.

use tracing::{debug, warn};

use crate::db::embedding_to_blob;
use crate::error::{Result, ResultExt};

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

    // 2. Call hybrid search
    let db = crate::get_database()
        .map_err(|e| crate::error::FourDaError::Internal(format!("DB: {e}")))?;
    let results = db.hybrid_search(query_text, &query_embedding, limit, 0.4, 0.6);

    if results.is_empty() {
        return Ok(Vec::new());
    }

    // 3. Normalize RRF scores to [0, 1] relative to top result
    let max_score = results[0].rrf_score;

    Ok(results
        .into_iter()
        .map(|r| {
            let relevance = if max_score > 0.0 {
                (r.rrf_score / max_score).clamp(0.0, 1.0)
            } else {
                0.0
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
        .collect())
}

// ============================================================================
// SQL text search
// ============================================================================

#[allow(dead_code)] // REMOVE BY 2026-08-01 — replaced by hybrid_search
pub(crate) fn execute_text_search(
    conn: &rusqlite::Connection,
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>> {
    if parsed.keywords.is_empty() {
        return Ok(Vec::new());
    }

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    let conditions: Vec<String> = parsed
        .keywords
        .iter()
        .map(|k| {
            let like_val = format!("%{k}%");
            params.push(Box::new(like_val.clone()));
            params.push(Box::new(like_val));
            "(LOWER(s.title) LIKE LOWER(?) OR LOWER(s.content) LIKE LOWER(?))".to_string()
        })
        .collect();

    let where_clause = conditions.join(" AND ");

    let type_filter = if parsed.file_types.is_empty() {
        String::new()
    } else {
        let placeholders: Vec<&str> = parsed
            .file_types
            .iter()
            .map(|t| {
                params.push(Box::new(t.clone()));
                "?"
            })
            .collect();
        format!(" AND s.source_type IN ({})", placeholders.join(","))
    };

    let time_filter = if let Some(ref tr) = parsed.time_range {
        params.push(Box::new(tr.start.clone()));
        " AND s.created_at >= ?".to_string()
    } else {
        String::new()
    };

    params.push(Box::new(limit as i64));

    let sql = format!(
        "SELECT s.id, s.source_type, s.url, s.title, s.content, s.created_at
         FROM source_items s
         WHERE ({where_clause}){type_filter}{time_filter}
         ORDER BY s.last_seen DESC
         LIMIT ?"
    );

    debug!(target: "4da::search", sql = %sql, "Executing text search");

    let mut stmt = conn.prepare(&sql).context("Query error")?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
            |row| {
                let id: i64 = row.get(0)?;
                let source_type: String = row.get(1)?;
                let url: Option<String> = row.get(2)?;
                let title: String = row.get(3)?;
                let content: String = row.get(4)?;
                let created_at: Option<String> = row.get(5)?;

                let preview = if content.len() > 200 {
                    format!("{}...", &content[..content.floor_char_boundary(200)])
                } else {
                    content
                };

                Ok(QueryResultItem {
                    id,
                    file_path: url.clone(),
                    file_name: Some(title),
                    preview,
                    relevance: 0.5,
                    source_type,
                    timestamp: created_at,
                    match_reason: "keyword match".to_string(),
                })
            },
        )
        .context("Query error")?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::search", error = %e, "Row parse error"),
        }
    }
    Ok(items)
}

// ============================================================================
// Vector similarity search
// ============================================================================

#[allow(dead_code)] // REMOVE BY 2026-08-01 — replaced by hybrid_search
pub(crate) async fn execute_vector_search(
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>> {
    let search_text = crate::utils::preprocess_content(&parsed.keywords.join(" "));
    if search_text.is_empty() {
        return Ok(Vec::new());
    }

    let embeddings = crate::embeddings::embed_texts(&[search_text]).await?;
    if embeddings.is_empty() || embeddings[0].iter().all(|&v| v == 0.0) {
        debug!(target: "4da::search", "No embedding available, skipping vector search");
        return Ok(Vec::new());
    }

    let query_embedding = &embeddings[0];
    let conn = crate::open_db_connection()?;
    let blob = embedding_to_blob(query_embedding);

    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.source_type, s.url, s.title, s.content, s.created_at, v.distance
             FROM source_vec v
             JOIN source_items s ON s.id = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )
        .context("Vector query error")?;

    let rows = stmt
        .query_map(rusqlite::params![blob, limit as i64], |row| {
            let id: i64 = row.get(0)?;
            let source_type: String = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let title: String = row.get(3)?;
            let content: String = row.get(4)?;
            let created_at: Option<String> = row.get(5)?;
            let distance: f64 = row.get(6)?;
            let relevance = (1.0 - distance).clamp(0.0, 1.0);

            let preview = if content.len() > 200 {
                format!("{}...", &content[..content.floor_char_boundary(200)])
            } else {
                content
            };

            Ok(QueryResultItem {
                id,
                file_path: url.clone(),
                file_name: Some(title),
                preview,
                relevance,
                source_type,
                timestamp: created_at,
                match_reason: format!("semantic similarity ({:.0}%)", relevance * 100.0),
            })
        })
        .context("Vector query error")?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::search", error = %e, "Row parse error"),
        }
    }
    Ok(items)
}
