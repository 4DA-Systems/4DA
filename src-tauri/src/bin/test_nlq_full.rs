//! Test Natural Language Query with Database Executor
//!
//! Run with: cargo run --bin test_nlq_full --release

use fourda_lib::query::{parse_simple, QueryExecutor};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;

fn main() {
    println!("=== Full NLQ Executor Test ===\n");

    // Open database
    let db_path =
        std::env::var("FOURDA_DB").unwrap_or_else(|_| "D:/4da-v3/data/4da.db".to_string());

    println!("Opening database: {}", db_path);

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to open database: {}", e);
            return;
        }
    };

    // Check what's in the database
    let doc_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    let chunk_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
        .unwrap_or(0);
    let context_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
        .unwrap_or(0);

    println!("Database stats:");
    println!("  - Indexed documents: {}", doc_count);
    println!("  - Document chunks: {}", chunk_count);
    println!("  - Context chunks: {}", context_count);
    println!();

    if context_count == 0 && chunk_count == 0 {
        println!("⚠️  No indexed content found. Please index some files first.");
        println!("   Run the app and let ACE scan your projects.");
        return;
    }

    // Create executor
    let conn = Arc::new(Mutex::new(conn));
    let executor = QueryExecutor::new(conn);

    // Test queries
    let test_queries = [
        "files about rust",
        "show me tauri",
        "authentication code",
        "what did I work on",
    ];

    for query_text in &test_queries {
        println!("─────────────────────────────────────────");
        println!("Query: \"{}\"", query_text);

        let parsed = parse_simple(query_text);
        println!(
            "Parsed: {:?} intent, keywords: {:?}",
            parsed.intent, parsed.keywords
        );

        match executor.execute(&parsed) {
            Ok(result) => {
                println!(
                    "Results: {} items in {}ms",
                    result.total_count, result.execution_ms
                );

                if result.items.is_empty() {
                    println!("  (no matches found)");
                } else {
                    for (i, item) in result.items.iter().take(3).enumerate() {
                        println!(
                            "  {}. [{}] {} (relevance: {:.0}%)",
                            i + 1,
                            item.source_type,
                            item.file_name.as_deref().unwrap_or("unknown"),
                            item.relevance * 100.0
                        );
                        println!("     → {}", item.match_reason);
                        // Show preview truncated
                        let preview: String = item.preview.chars().take(80).collect();
                        println!("     \"{}...\"", preview.replace('\n', " "));
                    }
                    if result.items.len() > 3 {
                        println!("  ... and {} more", result.items.len() - 3);
                    }
                }

                if let Some(ref summary) = result.summary {
                    println!("Summary: {}", summary);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
        println!();
    }

    println!("=== Test Complete ===");
}
