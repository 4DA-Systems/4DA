//! Document search and statistics commands.
//!
//! Search document content and query indexed document statistics.
//! Extracted from document_index.rs to reduce file size.

use crate::get_ace_engine;

// ============================================================================
// Search Commands
// ============================================================================

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
    crate::settings::require_pro_feature("natural_language_query")?;
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    /// Create an in-memory database with indexed_documents and document_chunks tables.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS indexed_documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                file_type TEXT NOT NULL,
                file_size INTEGER,
                content_hash TEXT,
                word_count INTEGER DEFAULT 0,
                page_count INTEGER DEFAULT 0,
                extraction_confidence REAL DEFAULT 0.0,
                extracted_topics TEXT,
                last_modified TEXT,
                indexed_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_indexed_documents_path ON indexed_documents(file_path);
            CREATE INDEX IF NOT EXISTS idx_indexed_documents_type ON indexed_documents(file_type);
            CREATE INDEX IF NOT EXISTS idx_indexed_documents_indexed ON indexed_documents(indexed_at);

            CREATE TABLE IF NOT EXISTS document_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                word_count INTEGER DEFAULT 0,
                embedding BLOB,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (document_id) REFERENCES indexed_documents(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_document_chunks_doc ON document_chunks(document_id);",
        )
        .expect("create tables");
        conn
    }

    /// Insert a test document and return its id.
    fn insert_doc(
        conn: &Connection,
        path: &str,
        name: &str,
        file_type: &str,
        file_size: i64,
        word_count: i64,
        confidence: f64,
    ) -> i64 {
        conn.execute(
            "INSERT INTO indexed_documents (file_path, file_name, file_type, file_size, word_count, extraction_confidence)
             VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![path, name, file_type, file_size, word_count, confidence],
        )
        .expect("insert doc");
        conn.last_insert_rowid()
    }

    /// Insert a test chunk for a given document.
    fn insert_chunk(conn: &Connection, document_id: i64, chunk_index: i64, content: &str) {
        let wc = content.split_whitespace().count() as i64;
        conn.execute(
            "INSERT INTO document_chunks (document_id, chunk_index, content, word_count) VALUES (?, ?, ?, ?)",
            rusqlite::params![document_id, chunk_index, content, wc],
        )
        .expect("insert chunk");
    }

    // ------------------------------------------------------------------
    // 1. Search documents (LIKE pattern matching)
    // ------------------------------------------------------------------

    #[test]
    fn test_search_documents_query() {
        let conn = setup_test_db();
        let doc1 = insert_doc(
            &conn,
            "/rust_guide.pdf",
            "rust_guide.pdf",
            "pdf",
            2000,
            500,
            0.9,
        );
        insert_chunk(&conn, doc1, 0, "Introduction to Rust programming language");
        insert_chunk(&conn, doc1, 1, "Ownership and borrowing in Rust");

        let doc2 = insert_doc(
            &conn,
            "/python_intro.pdf",
            "python_intro.pdf",
            "pdf",
            1500,
            400,
            0.85,
        );
        insert_chunk(
            &conn,
            doc2,
            0,
            "Python is a high-level programming language",
        );

        let doc3 = insert_doc(
            &conn,
            "/recipes.docx",
            "recipes.docx",
            "docx",
            500,
            100,
            0.7,
        );
        insert_chunk(&conn, doc3, 0, "Best chocolate cake recipe");

        let search_pattern = format!("%{}%", "Rust");
        let limit: i64 = 20;

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
            .unwrap();

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
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Two matching chunks produce two DISTINCT rows (different preview text)
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r["file_name"] == "rust_guide.pdf"));
    }

    // ------------------------------------------------------------------
    // 2. Search with no matches returns empty
    // ------------------------------------------------------------------

    #[test]
    fn test_search_documents_no_matches() {
        let conn = setup_test_db();
        let doc1 = insert_doc(&conn, "/notes.pdf", "notes.pdf", "pdf", 500, 100, 0.9);
        insert_chunk(&conn, doc1, 0, "Some general notes about the project");

        let search_pattern = format!("%{}%", "quantum_physics_xyz");
        let limit: i64 = 20;

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
            .unwrap();

        let results: Vec<serde_json::Value> = stmt
            .query_map([&search_pattern, &limit.to_string()], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "file_path": row.get::<_, String>(1)?,
                }))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(results.is_empty());
    }

    // ------------------------------------------------------------------
    // 3. Stats query (totals and by-type breakdown)
    // ------------------------------------------------------------------

    #[test]
    fn test_indexed_stats_query() {
        let conn = setup_test_db();
        let d1 = insert_doc(&conn, "/a.pdf", "a.pdf", "pdf", 500, 100, 0.9);
        let d2 = insert_doc(&conn, "/b.pdf", "b.pdf", "pdf", 600, 200, 0.85);
        let d3 = insert_doc(&conn, "/c.docx", "c.docx", "docx", 400, 50, 0.7);

        insert_chunk(&conn, d1, 0, "chunk one");
        insert_chunk(&conn, d2, 0, "chunk two");
        insert_chunk(&conn, d2, 1, "chunk three");
        insert_chunk(&conn, d3, 0, "chunk four");

        let total_docs: i64 = conn
            .query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);
        assert_eq!(total_docs, 3);

        let total_chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(total_chunks, 4);

        let total_words: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(word_count), 0) FROM indexed_documents",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(total_words, 350);

        let mut stmt = conn
            .prepare("SELECT file_type, COUNT(*) FROM indexed_documents GROUP BY file_type")
            .unwrap();
        let by_type: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(by_type.len(), 2);
        let pdf_entry = by_type.iter().find(|(t, _)| t == "pdf").unwrap();
        assert_eq!(pdf_entry.1, 2);
        let docx_entry = by_type.iter().find(|(t, _)| t == "docx").unwrap();
        assert_eq!(docx_entry.1, 1);
    }

    // ------------------------------------------------------------------
    // 4. Empty database stats returns zeros
    // ------------------------------------------------------------------

    #[test]
    fn test_stats_empty_database() {
        let conn = setup_test_db();

        let total_docs: i64 = conn
            .query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);
        assert_eq!(total_docs, 0);

        let total_chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(total_chunks, 0);

        let total_words: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(word_count), 0) FROM indexed_documents",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(total_words, 0);
    }

    // ------------------------------------------------------------------
    // 5. Pagination (offset skips records, limit caps results)
    // ------------------------------------------------------------------

    #[test]
    fn test_pagination_with_offset_and_limit() {
        let conn = setup_test_db();
        for i in 0..5 {
            insert_doc(
                &conn,
                &format!("/doc_{}.pdf", i),
                &format!("doc_{}.pdf", i),
                "pdf",
                100 * (i + 1),
                10 * (i + 1),
                0.5 + (i as f64) * 0.1,
            );
        }

        let query = "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
                     FROM indexed_documents ORDER BY indexed_at DESC LIMIT 2 OFFSET 0";
        let mut stmt = conn.prepare(query).unwrap();
        let page1: Vec<i64> = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(page1.len(), 2);

        let query = "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
                     FROM indexed_documents ORDER BY indexed_at DESC LIMIT 2 OFFSET 2";
        let mut stmt = conn.prepare(query).unwrap();
        let page2: Vec<i64> = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(page2.len(), 2);

        let query = "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
                     FROM indexed_documents ORDER BY indexed_at DESC LIMIT 2 OFFSET 4";
        let mut stmt = conn.prepare(query).unwrap();
        let page3: Vec<i64> = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(page3.len(), 1);

        assert!(!page1.iter().any(|id| page2.contains(id)));
        assert!(!page2.iter().any(|id| page3.contains(id)));
    }
}
