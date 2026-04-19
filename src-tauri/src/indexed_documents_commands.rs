// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Indexed Documents Tauri commands.
//!
//! Expose the local document index to the frontend for browsing and search.
//! Uses `open_db_connection()` for ad-hoc queries against the `indexed_documents`
//! and `document_chunks` tables created by the ACE extractor pipeline.

use crate::error::{Result, ResultExt};
use crate::open_db_connection;
use rusqlite::params;

/// List indexed documents with pagination and optional file type filter.
#[tauri::command]
pub async fn get_indexed_documents(
    limit: i64,
    offset: i64,
    file_type: Option<String>,
) -> Result<serde_json::Value> {
    let conn = open_db_connection()?;

    let total: i64 = if let Some(ref ft) = file_type {
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_documents WHERE file_type = ?1",
            params![ft],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
        .unwrap_or(0)
    };

    let docs: Vec<serde_json::Value> = if let Some(ref ft) = file_type {
        let mut stmt = conn.prepare(
            "SELECT id, file_path, file_name, file_type, file_size, word_count,
                    extraction_confidence, indexed_at
             FROM indexed_documents WHERE file_type = ?1
             ORDER BY indexed_at DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![ft, limit, offset], row_to_doc)?;
        rows.filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in indexed_documents_commands: {e}");
                None
            }
        })
        .collect()
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, file_path, file_name, file_type, file_size, word_count,
                    extraction_confidence, indexed_at
             FROM indexed_documents
             ORDER BY indexed_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], row_to_doc)?;
        rows.filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in indexed_documents_commands: {e}");
                None
            }
        })
        .collect()
    };

    Ok(serde_json::json!({
        "documents": docs,
        "total": total,
        "limit": limit,
        "offset": offset
    }))
}

/// Get aggregate statistics about the indexed document corpus.
#[tauri::command]
pub async fn get_indexed_stats() -> Result<serde_json::Value> {
    let conn = open_db_connection()?;

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM indexed_documents", [], |r| r.get(0))
        .unwrap_or(0);
    let total_chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let total_size: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM indexed_documents",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT file_type, COUNT(*) FROM indexed_documents
         GROUP BY file_type ORDER BY COUNT(*) DESC",
    )?;

    let types: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "file_type": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?
            }))
        })?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in indexed_documents_commands: {e}");
                None
            }
        })
        .collect();

    Ok(serde_json::json!({
        "total_documents": total,
        "total_chunks": total_chunks,
        "total_size_bytes": total_size,
        "file_types": types
    }))
}

/// Search document chunks by text content (LIKE query).
#[tauri::command]
pub async fn search_documents(query: String, limit: i64) -> Result<serde_json::Value> {
    let query =
        crate::ipc_guard::validate_length("query", &query, crate::ipc_guard::MAX_INPUT_LENGTH)?;
    let conn = open_db_connection()?;
    let search_pattern = format!("%{query}%");

    let mut stmt = conn.prepare(
        "SELECT d.id, d.file_path, d.file_name, d.file_type, c.content, c.chunk_index
         FROM document_chunks c
         JOIN indexed_documents d ON d.id = c.document_id
         WHERE c.content LIKE ?1
         LIMIT ?2",
    )?;

    let results: Vec<serde_json::Value> = stmt
        .query_map(params![search_pattern, limit], |row| {
            Ok(serde_json::json!({
                "document_id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
                "file_name": row.get::<_, String>(2)?,
                "file_type": row.get::<_, String>(3)?,
                "content_preview": row.get::<_, String>(4)?,
                "chunk_index": row.get::<_, i64>(5)?
            }))
        })?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in indexed_documents_commands: {e}");
                None
            }
        })
        .collect();

    Ok(serde_json::json!({ "results": results }))
}

/// Get full content of a single indexed document by reassembling its chunks.
#[tauri::command]
pub async fn get_document_content(document_id: i64) -> Result<serde_json::Value> {
    let conn = open_db_connection()?;

    let doc = conn
        .query_row(
            "SELECT id, file_path, file_name, file_type, file_size, word_count
             FROM indexed_documents WHERE id = ?1",
            [document_id],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "file_path": row.get::<_, String>(1)?,
                    "file_name": row.get::<_, String>(2)?,
                    "file_type": row.get::<_, String>(3)?,
                    "file_size": row.get::<_, i64>(4)?,
                    "word_count": row.get::<_, i64>(5)?
                }))
            },
        )
        .context("Document not found")?;

    let mut stmt = conn.prepare(
        "SELECT content, chunk_index FROM document_chunks
         WHERE document_id = ?1 ORDER BY chunk_index",
    )?;

    let chunks: Vec<String> = stmt
        .query_map([document_id], |row| row.get::<_, String>(0))?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in indexed_documents_commands: {e}");
                None
            }
        })
        .collect();

    let chunk_count = chunks.len();
    let full_content = chunks.join("\n");

    Ok(serde_json::json!({
        "document": doc,
        "content": full_content,
        "chunk_count": chunk_count
    }))
}

// ============================================================================
// Helpers
// ============================================================================

fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "file_path": row.get::<_, String>(1)?,
        "file_name": row.get::<_, String>(2)?,
        "file_type": row.get::<_, String>(3)?,
        "file_size": row.get::<_, i64>(4)?,
        "word_count": row.get::<_, i64>(5)?,
        "extraction_confidence": row.get::<_, f64>(6)?,
        "indexed_at": row.get::<_, String>(7)?
    }))
}
