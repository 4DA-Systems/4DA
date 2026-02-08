//! Check database content
//!
//! Run with: cargo run --bin test_db_content --release

use rusqlite::Connection;

fn main() {
    println!("=== Database Content Check ===\n");

    let db_path = "D:/4da-v3/data/4da.db";
    let conn = Connection::open(db_path).expect("Failed to open database");

    // Check context_chunks schema
    println!("=== context_chunks schema ===");
    let mut stmt = conn
        .prepare("PRAGMA table_info(context_chunks)")
        .expect("Failed to prepare");

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?, // column name
                row.get::<_, String>(2)?, // type
            ))
        })
        .expect("Failed to query");

    for row in rows.flatten() {
        let (name, typ) = row;
        println!("  {} ({})", name, typ);
    }

    // Check context_chunks content using the right column
    println!("\n=== Context Chunks (first 5) ===\n");

    let mut stmt = conn
        .prepare("SELECT id, source_file, SUBSTR(text, 1, 200) FROM context_chunks LIMIT 5")
        .expect("Failed to prepare");

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("Failed to query");

    for row in rows.flatten() {
        let (id, source, preview) = row;
        println!("ID {}: {}", id, source);
        println!(
            "  Preview: {}...",
            preview
                .replace('\n', " ")
                .chars()
                .take(150)
                .collect::<String>()
        );
        println!();
    }

    // Check active_topics
    println!("=== Active Topics (top 10) ===\n");

    let mut stmt = conn
        .prepare("SELECT topic, weight FROM active_topics ORDER BY weight DESC LIMIT 10")
        .expect("Failed to prepare");

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .expect("Failed to query");

    for row in rows.flatten() {
        let (topic, weight) = row;
        println!("  {} (weight: {:.2})", topic, weight);
    }

    println!("\n=== Done ===");
}
