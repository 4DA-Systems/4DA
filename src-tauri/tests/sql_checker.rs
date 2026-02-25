//! SQL Compile-Time Checker Integration Test
//!
//! Validates all SQL queries in the codebase against the database schema.
//! Run: cargo test --test sql_checker
//!
//! This is a static analysis tool that:
//! 1. Parses CREATE TABLE/VIRTUAL TABLE statements from Rust source files
//! 2. Extracts all DML queries (SELECT, INSERT, UPDATE, DELETE)
//! 3. Validates table names and column names against the schema
//! 4. Reports any mismatches as test failures

use std::path::Path;

mod sql_check_standalone;

#[test]
fn validate_all_sql_queries() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    println!("=== SQL Compile-Time Checker ===\n");
    println!("Source directory: {}\n", src_dir.display());

    // =========================================================================
    // Phase 1: Parse schema from source files containing CREATE TABLE
    // =========================================================================

    let schema_files = vec![
        src_dir.join("db.rs"),
        src_dir.join("db").join("migrations.rs"),
        src_dir.join("ace").join("db.rs"),
        src_dir.join("ace").join("embedding.rs"),
        src_dir.join("ace").join("watcher.rs"),
        src_dir.join("context_engine.rs"),
    ];

    let mut schema = sql_check_standalone::schema_parser::SchemaInfo::default();

    for file_path in &schema_files {
        if !file_path.exists() {
            println!("  [SKIP] Schema file not found: {}", file_path.display());
            continue;
        }

        let source = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file_path.display(), e));

        let file_schema = sql_check_standalone::schema_parser::parse_schema_from_source(&source);
        let count = file_schema.tables.len();

        if count > 0 {
            println!(
                "  [OK] {} - {} tables",
                file_path.file_name().unwrap().to_string_lossy(),
                count
            );
        }

        schema.merge(file_schema);
    }

    println!("\nParsed {} tables from schema files:", schema.tables.len());
    let mut table_names: Vec<&String> = schema.tables.keys().collect();
    table_names.sort();
    for name in &table_names {
        let table = &schema.tables[*name];
        let col_names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        println!(
            "  {} ({} cols{}) -> [{}]",
            name,
            table.columns.len(),
            if table.is_virtual { ", virtual" } else { "" },
            col_names.join(", ")
        );
    }

    // Sanity check: we should have found a meaningful number of tables
    assert!(
        schema.tables.len() >= 10,
        "Expected at least 10 tables from schema parsing, found {}. Schema parsing may be broken.",
        schema.tables.len()
    );

    // =========================================================================
    // Phase 2: Extract SQL queries from all source files
    // =========================================================================

    let queries = sql_check_standalone::sql_extractor::extract_queries_from_dir(&src_dir);
    let total = queries.len();
    let dynamic_count = queries.iter().filter(|q| q.is_dynamic).count();
    let static_count = total - dynamic_count;

    println!(
        "\nExtracted {} SQL queries ({} dynamic, {} static)",
        total, dynamic_count, static_count
    );

    // Print some stats by file
    let mut by_file: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for q in &queries {
        // Get just the filename
        let file_name = Path::new(&q.file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        *by_file.entry(file_name).or_insert(0) += 1;
    }
    let mut file_counts: Vec<_> = by_file.into_iter().collect();
    file_counts.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nQueries per file (top 10):");
    for (file, count) in file_counts.iter().take(10) {
        println!("  {} - {} queries", file, count);
    }

    // Sanity check: we should find a reasonable number of queries
    assert!(
        total >= 20,
        "Expected at least 20 SQL queries, found {}. Extractor may be broken.",
        total
    );

    // =========================================================================
    // Phase 3: Validate queries against schema
    // =========================================================================

    let errors = sql_check_standalone::sql_validator::validate_all(&queries, &schema);

    println!("\n=== Validation Results ===\n");

    if errors.is_empty() {
        println!(
            "All {} static SQL queries validated successfully!",
            static_count
        );
        println!("({} dynamic queries skipped)", dynamic_count);
    } else {
        let error_msg = format!(
            "SQL validation found {} error(s):\n\n{}",
            errors.len(),
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        panic!("{}", error_msg);
    }
}

/// Verify that the schema parser can find key tables from each schema file.
#[test]
fn schema_parser_finds_key_tables() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    // Test db.rs tables
    let db_source = std::fs::read_to_string(src_dir.join("db.rs")).expect("read db.rs");
    let db_schema = sql_check_standalone::schema_parser::parse_schema_from_source(&db_source);

    let expected_main_tables = vec![
        "context_chunks",
        "source_items",
        "sources",
        "feedback",
        "schema_version",
        "extraction_jobs",
        "file_metadata_cache",
        "query_cache",
        "query_history",
        "chunk_sentiment",
        "void_positions",
        "temporal_events",
        "project_dependencies",
        "item_relationships",
        "source_health",
    ];

    for table in &expected_main_tables {
        assert!(
            db_schema.has_table(table),
            "db.rs: missing expected table '{}'. Found tables: {:?}",
            table,
            db_schema.tables.keys().collect::<Vec<_>>()
        );
    }

    // Check virtual tables
    let expected_virtual = vec!["context_vec", "source_vec"];
    for table in &expected_virtual {
        assert!(
            db_schema.has_table(table),
            "db.rs: missing expected virtual table '{}'. Found tables: {:?}",
            table,
            db_schema.tables.keys().collect::<Vec<_>>()
        );
    }

    // Test ace/db.rs tables
    let ace_source =
        std::fs::read_to_string(src_dir.join("ace").join("db.rs")).expect("read ace/db.rs");
    let ace_schema = sql_check_standalone::schema_parser::parse_schema_from_source(&ace_source);

    let expected_ace_tables = vec![
        "detected_projects",
        "detected_tech",
        "file_signals",
        "git_signals",
        "active_topics",
        "anti_topics",
        "interactions",
        "topic_affinities",
        "source_preferences",
        "activity_patterns",
        "validated_signals",
        "audit_log",
        "accuracy_metrics",
        "system_health",
        "bootstrap_paths",
        "indexed_documents",
        "document_chunks",
        "anomalies",
        "kv_store",
    ];

    for table in &expected_ace_tables {
        assert!(
            ace_schema.has_table(table),
            "ace/db.rs: missing expected table '{}'. Found tables: {:?}",
            table,
            ace_schema.tables.keys().collect::<Vec<_>>()
        );
    }

    // Check ACE virtual tables
    let expected_ace_virtual = vec!["topic_vec", "affinity_vec", "document_vec"];
    for table in &expected_ace_virtual {
        assert!(
            ace_schema.has_table(table),
            "ace/db.rs: missing expected virtual table '{}'. Found tables: {:?}",
            table,
            ace_schema.tables.keys().collect::<Vec<_>>()
        );
    }

    println!(
        "Schema parser found {} main tables, {} ACE tables",
        db_schema.tables.len(),
        ace_schema.tables.len()
    );
}

/// Verify that key columns are parsed correctly.
#[test]
fn schema_parser_finds_key_columns() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let db_source = std::fs::read_to_string(src_dir.join("db.rs")).expect("read db.rs");
    let schema = sql_check_standalone::schema_parser::parse_schema_from_source(&db_source);

    // source_items should have these columns
    let expected_cols = vec![
        "id",
        "source_type",
        "source_id",
        "url",
        "title",
        "content",
        "content_hash",
        "embedding",
        "created_at",
        "last_seen",
    ];

    for col in &expected_cols {
        assert!(
            schema.has_column("source_items", col),
            "source_items: missing expected column '{}'. Found: {:?}",
            col,
            schema.tables["source_items"]
                .columns
                .iter()
                .map(|c| &c.name)
                .collect::<Vec<_>>()
        );
    }

    // context_chunks should have these columns
    let expected_context_cols = vec![
        "id",
        "source_file",
        "content_hash",
        "text",
        "embedding",
        "weight",
        "created_at",
        "updated_at",
    ];

    for col in &expected_context_cols {
        assert!(
            schema.has_column("context_chunks", col),
            "context_chunks: missing expected column '{}'. Found: {:?}",
            col,
            schema.tables["context_chunks"]
                .columns
                .iter()
                .map(|c| &c.name)
                .collect::<Vec<_>>()
        );
    }

    // vec0 tables should have pseudo-columns
    assert!(schema.has_column("context_vec", "rowid"));
    assert!(schema.has_column("context_vec", "embedding"));
    assert!(schema.has_column("context_vec", "distance"));
    assert!(schema.has_column("source_vec", "rowid"));
    assert!(schema.has_column("source_vec", "embedding"));
    assert!(schema.has_column("source_vec", "distance"));
}

/// Verify that the SQL extractor finds a reasonable number of queries.
#[test]
fn sql_extractor_finds_queries() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let queries = sql_check_standalone::sql_extractor::extract_queries_from_dir(&src_dir);

    println!("Found {} total queries", queries.len());

    // We should find queries
    assert!(!queries.is_empty(), "No SQL queries found!");

    // Check that we found different types
    let selects = queries
        .iter()
        .filter(|q| q.sql.to_lowercase().starts_with("select"))
        .count();
    let inserts = queries
        .iter()
        .filter(|q| {
            let l = q.sql.to_lowercase();
            l.starts_with("insert") || l.starts_with("replace")
        })
        .count();
    let updates = queries
        .iter()
        .filter(|q| q.sql.to_lowercase().starts_with("update"))
        .count();
    let deletes = queries
        .iter()
        .filter(|q| q.sql.to_lowercase().starts_with("delete"))
        .count();

    println!(
        "  SELECT: {}, INSERT: {}, UPDATE: {}, DELETE: {}",
        selects, inserts, updates, deletes
    );

    assert!(selects > 5, "Expected more than 5 SELECT queries");
    assert!(inserts > 3, "Expected more than 3 INSERT queries");

    // Check parameter counting
    let with_params = queries.iter().filter(|q| q.param_count > 0).count();
    println!(
        "  With parameters: {}, Without: {}",
        with_params,
        queries.len() - with_params
    );
    assert!(with_params > 5, "Expected more queries with parameters");
}
