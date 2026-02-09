//! Indexed document commands for querying and searching local documents.
//!
//! Extracted from lib.rs to reduce file size. These are Tauri command wrappers
//! around the ACE engine's database for document indexing features.

use serde::Serialize;

use crate::get_ace_engine;

// ============================================================================
// Indexed Documents Commands
// ============================================================================

/// Indexed document summary for UI
#[derive(Debug, Clone, Serialize)]
pub struct IndexedDocumentSummary {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub file_size: i64,
    pub word_count: i64,
    pub extraction_confidence: f64,
    pub indexed_at: String,
}

/// Get list of indexed documents
#[tauri::command]
pub async fn get_indexed_documents(
    limit: Option<i64>,
    offset: Option<i64>,
    file_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let query = if file_type.is_some() {
        format!(
            "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
             FROM indexed_documents
             WHERE file_type = ?
             ORDER BY indexed_at DESC
             LIMIT {} OFFSET {}",
            limit, offset
        )
    } else {
        format!(
            "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
             FROM indexed_documents
             ORDER BY indexed_at DESC
             LIMIT {} OFFSET {}",
            limit, offset
        )
    };

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<IndexedDocumentSummary> {
        Ok(IndexedDocumentSummary {
            id: row.get(0)?,
            file_path: row.get(1)?,
            file_name: row.get(2)?,
            file_type: row.get(3)?,
            file_size: row.get(4)?,
            word_count: row.get(5)?,
            extraction_confidence: row.get(6)?,
            indexed_at: row.get(7)?,
        })
    };

    let docs: Vec<IndexedDocumentSummary> = if let Some(ref ft) = file_type {
        stmt.query_map([ft], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map([], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect()
    };

    // Get total count
    let total: i64 = if let Some(ref ft) = file_type {
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_documents WHERE file_type = ?",
            [ft],
            |row| row.get(0),
        )
    } else {
        conn.query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
    }
    .unwrap_or(0);

    Ok(serde_json::json!({
        "documents": docs,
        "total": total,
        "limit": limit,
        "offset": offset
    }))
}

/// Get document content (chunks) by document ID
#[tauri::command]
pub async fn get_document_content(document_id: i64) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    // Get document metadata
    let doc: Option<IndexedDocumentSummary> = conn.query_row(
        "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
         FROM indexed_documents WHERE id = ?",
        [document_id],
        |row| {
            Ok(IndexedDocumentSummary {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_name: row.get(2)?,
                file_type: row.get(3)?,
                file_size: row.get(4)?,
                word_count: row.get(5)?,
                extraction_confidence: row.get(6)?,
                indexed_at: row.get(7)?,
            })
        },
    ).ok();

    let doc = doc.ok_or("Document not found")?;

    // Get chunks
    let mut stmt = conn
        .prepare(
            "SELECT chunk_index, content, word_count FROM document_chunks
         WHERE document_id = ? ORDER BY chunk_index",
        )
        .map_err(|e| e.to_string())?;

    let chunks: Vec<serde_json::Value> = stmt
        .query_map([document_id], |row| {
            Ok(serde_json::json!({
                "index": row.get::<_, i64>(0)?,
                "content": row.get::<_, String>(1)?,
                "word_count": row.get::<_, i64>(2)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "document": doc,
        "chunks": chunks
    }))
}

/// Search document content
#[tauri::command]
pub async fn search_documents(
    query: String,
    limit: Option<i64>,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let limit = limit.unwrap_or(20);
    let search_pattern = format!("%{}%", query);

    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT d.id, d.file_path, d.file_name, d.file_type, d.word_count, d.indexed_at,
                substr(c.content, 1, 200) as preview
         FROM indexed_documents d
         JOIN document_chunks c ON c.document_id = d.id
         WHERE c.content LIKE ?
         ORDER BY d.indexed_at DESC
         LIMIT ?",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<serde_json::Value> = stmt
        .query_map([&search_pattern, &limit.to_string()], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
                "file_name": row.get::<_, String>(2)?,
                "file_type": row.get::<_, String>(3)?,
                "word_count": row.get::<_, i64>(4)?,
                "indexed_at": row.get::<_, String>(5)?,
                "preview": row.get::<_, String>(6)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "query": query,
        "results": results,
        "count": results.len()
    }))
}

/// Natural language query for documents and context
/// Phase 2 feature - supports queries like "show me files about rust from last week"
#[tauri::command]
pub async fn natural_language_query(query_text: String) -> Result<serde_json::Value, String> {
    use crate::query::{parse_simple, QueryExecutor};

    let ace = get_ace_engine()?;
    let conn = ace.get_conn().clone();

    // Parse the natural language query
    let parsed = parse_simple(&query_text);

    // Execute the query
    let executor = QueryExecutor::new(conn);
    let result = executor.execute(&parsed).map_err(|e| e.to_string())?;

    // Convert to JSON
    Ok(serde_json::json!({
        "query": result.query,
        "intent": format!("{:?}", result.intent),
        "items": result.items,
        "total_count": result.total_count,
        "execution_ms": result.execution_ms,
        "summary": result.summary,
        "parsed": {
            "keywords": parsed.keywords,
            "entities": parsed.entities,
            "time_range": parsed.time_range.map(|tr| serde_json::json!({
                "start": tr.start.to_rfc3339(),
                "end": tr.end.to_rfc3339(),
                "relative": tr.relative
            })),
            "file_types": parsed.file_types,
            "sentiment": parsed.sentiment.map(|s| format!("{:?}", s)),
            "confidence": parsed.confidence
        }
    }))
}

/// Get indexed documents statistics
#[tauri::command]
pub async fn get_indexed_stats() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let total_docs: i64 = conn
        .query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let total_chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
        .unwrap_or(0);

    let total_words: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(word_count), 0) FROM indexed_documents",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Get counts by file type
    let mut stmt = conn
        .prepare("SELECT file_type, COUNT(*) FROM indexed_documents GROUP BY file_type")
        .map_err(|e| e.to_string())?;

    let by_type: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "file_type": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "total_documents": total_docs,
        "total_chunks": total_chunks,
        "total_words": total_words,
        "by_type": by_type
    }))
}
