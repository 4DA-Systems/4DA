// Query executor - Hybrid keyword + vector search
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

use super::{ParsedQuery, QueryIntent};

/// A single result item from a query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultItem {
    /// Document/chunk ID
    pub id: i64,
    /// File path (if from indexed documents)
    pub file_path: Option<String>,
    /// File name
    pub file_name: Option<String>,
    /// Content preview
    pub preview: String,
    /// Relevance score (0.0-1.0)
    pub relevance: f32,
    /// Source type (e.g., "pdf", "docx", "context")
    pub source_type: String,
    /// Timestamp (indexed_at or created_at)
    pub timestamp: Option<String>,
    /// Why this result matched
    pub match_reason: String,
}

/// Result of executing a query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Original query
    pub query: String,
    /// Parsed intent
    pub intent: QueryIntent,
    /// Result items
    pub items: Vec<QueryResultItem>,
    /// Total count (may be more than returned items)
    pub total_count: i64,
    /// Execution time in milliseconds
    pub execution_ms: u64,
    /// Summary (for summarize intent)
    pub summary: Option<String>,
}

impl QueryResult {
    pub fn empty(query: &str, intent: QueryIntent) -> Self {
        Self {
            query: query.to_string(),
            intent,
            items: Vec::new(),
            total_count: 0,
            execution_ms: 0,
            summary: None,
        }
    }
}

/// Query executor that performs hybrid keyword + vector search
pub struct QueryExecutor {
    conn: Arc<Mutex<Connection>>,
}

impl QueryExecutor {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Try to get an embedding for the query text
    /// Returns None if embedding fails (API not configured, etc.)
    /// Uses block_in_place to bridge sync QueryExecutor to async embed_texts
    fn try_embed_query(&self, query_text: &str) -> Option<Vec<f32>> {
        // Use the crate-level embed_texts function (async) via block_in_place bridge
        let texts = vec![query_text.to_string()];
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::embed_texts(&texts))
        });
        match result {
            Ok(embeddings) if !embeddings.is_empty() => {
                debug!(target: "query::executor", "Generated embedding for query");
                embeddings.into_iter().next()
            }
            Ok(_) => None,
            Err(e) => {
                debug!(target: "query::executor", error = %e, "Embedding not available, using keyword search only");
                None
            }
        }
    }

    /// Perform vector similarity search on context_vec
    fn vector_search(&self, embedding: &[f32], limit: usize) -> Vec<(i64, f32, String, String)> {
        let conn = self.conn.lock();

        // Convert embedding to blob for sqlite-vec
        let embedding_blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // KNN search requires k = ? in WHERE clause for sqlite-vec
        let sql = r#"
            SELECT
                c.id,
                v.distance,
                c.source_file,
                SUBSTR(c.content, 1, 200) as preview
            FROM context_vec v
            JOIN context_chunks c ON c.id = v.rowid
            WHERE v.embedding MATCH ?1 AND k = ?2
            ORDER BY v.distance
        "#;

        let mut results = Vec::new();
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) =
                stmt.query_map(rusqlite::params![embedding_blob, limit as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, f32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
            {
                for row in rows.flatten() {
                    results.push(row);
                }
            }
        }

        results
    }

    /// Execute a parsed query
    pub fn execute(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();

        let result = match query.intent {
            QueryIntent::Find => self.execute_find(query)?,
            QueryIntent::Summarize => self.execute_summarize(query)?,
            QueryIntent::Compare => self.execute_compare(query)?,
            QueryIntent::Timeline => self.execute_timeline(query)?,
            QueryIntent::Count => self.execute_count(query)?,
        };

        let mut result = result;
        result.execution_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }

    /// Execute a Find query - search for matching content
    /// Uses hybrid search: vector similarity + keyword matching
    fn execute_find(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        let mut items = Vec::new();
        let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

        // Build search terms from keywords and entities
        let search_terms: Vec<&str> = query
            .keywords
            .iter()
            .map(|s| s.as_str())
            .chain(query.entities.iter().map(|s| s.as_str()))
            .collect();

        if search_terms.is_empty() {
            return Ok(QueryResult::empty(&query.original, QueryIntent::Find));
        }

        // PHASE 1: Vector search (if embedding available)
        // Try to get an embedding for semantic search
        if let Some(query_embedding) = self.try_embed_query(&query.original) {
            let vector_results = self.vector_search(&query_embedding, 20);
            debug!(target: "query::executor", count = vector_results.len(), "Vector search results");

            for (id, distance, source_file, preview) in vector_results {
                if seen_ids.contains(&id) {
                    continue;
                }
                seen_ids.insert(id);

                // Convert distance to relevance (smaller distance = higher relevance)
                // sqlite-vec returns L2 distance, typically 0-2 range
                let relevance = (1.0 - (distance / 2.0).min(1.0)).max(0.0);

                let file_name = std::path::Path::new(&source_file)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string());

                items.push(QueryResultItem {
                    id,
                    file_path: Some(source_file),
                    file_name,
                    preview,
                    relevance,
                    source_type: "context".to_string(),
                    timestamp: None,
                    match_reason: format!("Semantic match (similarity: {:.0}%)", relevance * 100.0),
                });
            }
        }

        // PHASE 2: Keyword search
        let conn = self.conn.lock();
        let search_pattern = format!("%{}%", search_terms.join("%"));

        // Build the query with optional filters
        let mut sql = String::from(
            r#"
            SELECT DISTINCT
                d.id,
                d.file_path,
                d.file_name,
                SUBSTR(c.content, 1, 200) as preview,
                d.indexed_at,
                d.file_type
            FROM indexed_documents d
            JOIN document_chunks c ON c.document_id = d.id
            WHERE c.content LIKE ?1
            "#,
        );

        // Add file type filter
        if !query.file_types.is_empty() {
            let types: Vec<String> = query
                .file_types
                .iter()
                .map(|t| format!("'{}'", t))
                .collect();
            sql.push_str(&format!(" AND d.file_type IN ({})", types.join(",")));
        }

        // Add time filter
        if let Some(ref tr) = query.time_range {
            sql.push_str(&format!(" AND {}", tr.to_sql_clause("d.indexed_at")));
        }

        sql.push_str(" ORDER BY d.indexed_at DESC LIMIT 50");

        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&search_pattern], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows.flatten() {
            let (id, file_path, file_name, preview, indexed_at, file_type) = row;

            // Calculate relevance based on keyword matches
            let relevance = calculate_relevance(&preview, &search_terms);

            // Generate match reason
            let matched_terms: Vec<&str> = search_terms
                .iter()
                .filter(|t| preview.to_lowercase().contains(&t.to_lowercase()))
                .copied()
                .collect();

            let match_reason = if matched_terms.is_empty() {
                "Content match".to_string()
            } else {
                format!("Matched: {}", matched_terms.join(", "))
            };

            items.push(QueryResultItem {
                id,
                file_path: Some(file_path),
                file_name: Some(file_name),
                preview,
                relevance,
                source_type: file_type,
                timestamp: indexed_at,
                match_reason,
            });
        }

        // Also search context_chunks (from ACE) via keyword
        // Skip items already found by vector search
        // Note: context_chunks uses 'text' column, not 'content'
        let context_sql = r#"
            SELECT id, source_file, text, created_at
            FROM context_chunks
            WHERE text LIKE ?1
            ORDER BY created_at DESC
            LIMIT 20
        "#;

        if let Ok(mut stmt) = conn.prepare(context_sql) {
            if let Ok(rows) = stmt.query_map([&search_pattern], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (id, source_file, content, created_at) = row;

                    // Skip if already found by vector search
                    if seen_ids.contains(&id) {
                        continue;
                    }

                    let preview = if content.len() > 200 {
                        format!("{}...", &content[..200])
                    } else {
                        content.clone()
                    };

                    let relevance = calculate_relevance(&content, &search_terms);
                    let file_name = std::path::Path::new(&source_file)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string());

                    items.push(QueryResultItem {
                        id,
                        file_path: Some(source_file),
                        file_name,
                        preview,
                        relevance,
                        source_type: "context".to_string(),
                        timestamp: created_at,
                        match_reason: format!("Keyword match: {}", search_terms.join(", ")),
                    });
                }
            }
        }

        // Sort by relevance
        items.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by file path
        let mut seen = std::collections::HashSet::new();
        items.retain(|item| {
            if let Some(ref path) = item.file_path {
                seen.insert(path.clone())
            } else {
                true
            }
        });

        let total_count = items.len() as i64;

        Ok(QueryResult {
            query: query.original.clone(),
            intent: QueryIntent::Find,
            items,
            total_count,
            execution_ms: 0,
            summary: None,
        })
    }

    /// Execute a Summarize query - find content and generate summary
    fn execute_summarize(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        // First, find relevant content
        let find_result = self.execute_find(query)?;

        // For now, just return the find results with a placeholder summary
        // Full summarization would require LLM integration
        let summary = if find_result.items.is_empty() {
            Some("No content found matching the query.".to_string())
        } else {
            Some(format!(
                "Found {} items related to '{}'. Top results include: {}",
                find_result.total_count,
                query.keywords.join(", "),
                find_result
                    .items
                    .iter()
                    .take(3)
                    .filter_map(|i| i.file_name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        };

        Ok(QueryResult {
            summary,
            intent: QueryIntent::Summarize,
            ..find_result
        })
    }

    /// Execute a Compare query - find multiple items and compare
    fn execute_compare(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        // For compare queries, we search for all entities separately
        // and return them grouped
        let mut all_items = Vec::new();

        if query.entities.len() >= 2 {
            for entity in &query.entities {
                let entity_query = ParsedQuery {
                    keywords: vec![entity.clone()],
                    ..query.clone()
                };
                let result = self.execute_find(&entity_query)?;
                all_items.extend(result.items);
            }
        } else {
            // Fall back to regular find
            return self.execute_find(query);
        }

        let total_count = all_items.len() as i64;

        Ok(QueryResult {
            query: query.original.clone(),
            intent: QueryIntent::Compare,
            items: all_items,
            total_count,
            execution_ms: 0,
            summary: Some(format!("Comparing {} entities", query.entities.len())),
        })
    }

    /// Execute a Timeline query - find content ordered by time
    fn execute_timeline(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        let mut find_result = self.execute_find(query)?;

        // Re-sort by timestamp instead of relevance
        find_result.items.sort_by(|a, b| {
            let a_time = a.timestamp.as_ref().unwrap_or(&String::new()).clone();
            let b_time = b.timestamp.as_ref().unwrap_or(&String::new()).clone();
            b_time.cmp(&a_time) // Most recent first
        });

        Ok(QueryResult {
            intent: QueryIntent::Timeline,
            ..find_result
        })
    }

    /// Execute a Count query - count matching items
    fn execute_count(&self, query: &ParsedQuery) -> Result<QueryResult, String> {
        let find_result = self.execute_find(query)?;

        Ok(QueryResult {
            summary: Some(format!(
                "Found {} items matching '{}'",
                find_result.total_count,
                query.keywords.join(", ")
            )),
            intent: QueryIntent::Count,
            items: Vec::new(), // Don't return items for count queries
            ..find_result
        })
    }
}

/// Calculate relevance score based on keyword matches
fn calculate_relevance(text: &str, search_terms: &[&str]) -> f32 {
    if search_terms.is_empty() {
        return 0.0;
    }

    let text_lower = text.to_lowercase();
    let mut matches = 0;
    let mut total_weight = 0.0;

    for term in search_terms {
        let term_lower = term.to_lowercase();
        if text_lower.contains(&term_lower) {
            matches += 1;
            // Weight by term length (longer terms are more specific)
            total_weight += (term.len() as f32).sqrt();
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Normalize: ratio of matches * weight factor
    let match_ratio = matches as f32 / search_terms.len() as f32;
    let weight_factor = (total_weight / (search_terms.len() as f32 * 3.0)).min(1.0);

    (match_ratio * 0.6 + weight_factor * 0.4).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_relevance() {
        // All terms match
        let rel = calculate_relevance("rust programming language", &["rust", "programming"]);
        assert!(rel > 0.5);

        // Partial match
        let rel = calculate_relevance("rust is great", &["rust", "python"]);
        assert!(rel > 0.0 && rel < 1.0);

        // No match
        let rel = calculate_relevance("hello world", &["rust", "python"]);
        assert_eq!(rel, 0.0);
    }

    #[test]
    fn test_query_result_empty() {
        let result = QueryResult::empty("test query", QueryIntent::Find);
        assert_eq!(result.items.len(), 0);
        assert_eq!(result.total_count, 0);
    }
}
