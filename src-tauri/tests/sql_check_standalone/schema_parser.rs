//! Schema parser: extracts CREATE TABLE/VIRTUAL TABLE from Rust source files.
//!
//! Parses string literals in Rust code to find SQL DDL statements and
//! extract table names, column definitions, and constraints.

use std::collections::HashMap;

use super::vec0_support;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub col_type: String,
    pub nullable: bool,
    pub has_default: bool,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub is_virtual: bool,
    pub virtual_module: Option<String>,
}

#[derive(Debug, Default)]
pub struct SchemaInfo {
    pub tables: HashMap<String, TableSchema>,
}

impl SchemaInfo {
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.contains_key(&name.to_lowercase())
    }

    pub fn has_column(&self, table: &str, column: &str) -> bool {
        self.tables
            .get(&table.to_lowercase())
            .map(|t| t.columns.iter().any(|c| c.name == column.to_lowercase()))
            .unwrap_or(false)
    }

    /// Merge another SchemaInfo into this one.
    /// For duplicate tables, combines columns from both definitions.
    /// This handles cases where the same table is defined in multiple modules
    /// with slightly different schemas (e.g., context_engine.rs vs ace/db.rs).
    pub fn merge(&mut self, other: SchemaInfo) {
        for (name, table) in other.tables {
            if let Some(existing) = self.tables.get_mut(&name) {
                // Merge columns: add any new columns from the other definition
                for col in &table.columns {
                    if !existing.columns.iter().any(|c| c.name == col.name) {
                        existing.columns.push(col.clone());
                    }
                }
            } else {
                self.tables.insert(name, table);
            }
        }
    }
}

/// Parse all CREATE TABLE and CREATE VIRTUAL TABLE statements from Rust source text.
///
/// This extracts SQL DDL from Rust string literals (both regular and raw strings)
/// and parses the table schemas from them.
pub fn parse_schema_from_source(source_text: &str) -> SchemaInfo {
    let mut schema = SchemaInfo::default();

    // Extract all string literal contents from Rust source
    let string_contents = extract_string_literals(source_text);

    for content in &string_contents {
        // Look for CREATE TABLE statements
        parse_create_tables(content, &mut schema);
        // Look for CREATE VIRTUAL TABLE statements
        parse_create_virtual_tables(content, &mut schema);
        // Look for ALTER TABLE ... ADD COLUMN statements
        parse_alter_table_add_column(content, &mut schema);
    }

    schema
}

/// Extract contents of string literals from Rust source code.
///
/// Handles:
/// - Regular strings: "..."
/// - Raw strings: r#"..."#, r##"..."##, etc.
/// - Multi-line strings (both types)
fn extract_string_literals(source: &str) -> Vec<String> {
    let mut results = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip line comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Skip block comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Raw string: r#"..."#
        if chars[i] == 'r' && i + 1 < len && (chars[i + 1] == '#' || chars[i + 1] == '"') {
            let start = i;
            i += 1; // skip 'r'

            // Count hashes
            let mut hash_count = 0;
            while i < len && chars[i] == '#' {
                hash_count += 1;
                i += 1;
            }

            if i < len && chars[i] == '"' {
                i += 1; // skip opening quote
                let content_start = i;

                // Find closing: "###
                'outer: while i < len {
                    if chars[i] == '"' {
                        let quote_pos = i;
                        i += 1;
                        let mut matched_hashes = 0;
                        while i < len && chars[i] == '#' && matched_hashes < hash_count {
                            matched_hashes += 1;
                            i += 1;
                        }
                        if matched_hashes == hash_count {
                            let content: String = chars[content_start..quote_pos].iter().collect();
                            results.push(content);
                            break 'outer;
                        }
                        // Not the end, continue searching
                    } else {
                        i += 1;
                    }
                }
                continue;
            } else {
                // Not a raw string, backtrack
                i = start + 1;
                continue;
            }
        }

        // Regular string: "..."
        if chars[i] == '"' {
            i += 1; // skip opening quote
            let mut content = String::new();
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    // Escaped character - include the actual character
                    match chars[i + 1] {
                        'n' => content.push('\n'),
                        't' => content.push('\t'),
                        'r' => content.push('\r'),
                        '\\' => content.push('\\'),
                        '"' => content.push('"'),
                        _ => {
                            content.push(chars[i]);
                            content.push(chars[i + 1]);
                        }
                    }
                    i += 2;
                } else {
                    content.push(chars[i]);
                    i += 1;
                }
            }
            if i < len {
                i += 1; // skip closing quote
            }
            if content.len() > 10 {
                // Only consider non-trivial strings
                results.push(content);
            }
            continue;
        }

        i += 1;
    }

    results
}

/// Parse CREATE TABLE IF NOT EXISTS statements from SQL text
fn parse_create_tables(sql_text: &str, schema: &mut SchemaInfo) {
    let lower = sql_text.to_lowercase();

    // Find all CREATE TABLE occurrences
    let pattern = "create table if not exists";
    let mut search_start = 0;

    while let Some(pos) = lower[search_start..].find(pattern) {
        let abs_pos = search_start + pos + pattern.len();
        search_start = abs_pos;

        // Also support "CREATE TABLE" without IF NOT EXISTS
        let remaining = sql_text[abs_pos..].trim_start();
        let remaining_lower = remaining.to_lowercase();

        // Extract table name (first identifier after the keywords)
        let table_name = match extract_identifier(&remaining_lower) {
            Some(name) => name,
            None => continue,
        };

        // Find the opening paren for column definitions
        let after_name = &remaining_lower[table_name.len()..];
        let paren_pos = match after_name.find('(') {
            Some(p) => p,
            None => continue,
        };

        // Find the matching closing paren
        let body_start = table_name.len() + paren_pos;
        let body_text = &remaining[body_start..];
        let body = match extract_balanced_parens(body_text) {
            Some(b) => b,
            None => continue,
        };

        // Parse column definitions from the body
        let columns = parse_column_definitions(&body);

        let table = TableSchema {
            name: table_name.clone(),
            columns,
            is_virtual: false,
            virtual_module: None,
        };

        schema.tables.insert(table_name, table);
    }

    // Also handle "CREATE TABLE" without "IF NOT EXISTS"
    let pattern2 = "create table";
    search_start = 0;

    while let Some(pos) = lower[search_start..].find(pattern2) {
        let abs_pos = search_start + pos + pattern2.len();
        search_start = abs_pos;

        let remaining = sql_text[abs_pos..].trim_start();
        let remaining_lower = remaining.to_lowercase();

        // Skip if this is actually a "CREATE TABLE IF NOT EXISTS" (already handled)
        if remaining_lower.starts_with("if") {
            continue;
        }

        // Skip if this is a virtual table
        if lower[search_start - pattern2.len()..search_start]
            .trim()
            .ends_with("virtual")
        {
            continue;
        }

        // Also check if "virtual" appears before "table" in this statement
        let stmt_start = if pos >= 10 {
            search_start - pattern2.len() - 10
        } else {
            0
        };
        let before = &lower[stmt_start..search_start - pattern2.len()];
        if before.contains("virtual") {
            continue;
        }

        let table_name = match extract_identifier(&remaining_lower) {
            Some(name) => name,
            None => continue,
        };

        // Skip if already parsed
        if schema.tables.contains_key(&table_name) {
            continue;
        }

        let after_name = &remaining_lower[table_name.len()..];
        let paren_pos = match after_name.find('(') {
            Some(p) => p,
            None => continue,
        };

        let body_start = table_name.len() + paren_pos;
        let body_text = &remaining[body_start..];
        let body = match extract_balanced_parens(body_text) {
            Some(b) => b,
            None => continue,
        };

        let columns = parse_column_definitions(&body);

        let table = TableSchema {
            name: table_name.clone(),
            columns,
            is_virtual: false,
            virtual_module: None,
        };

        schema.tables.insert(table_name, table);
    }
}

/// Parse CREATE VIRTUAL TABLE statements
fn parse_create_virtual_tables(sql_text: &str, schema: &mut SchemaInfo) {
    let lower = sql_text.to_lowercase();

    // Pattern: CREATE VIRTUAL TABLE IF NOT EXISTS <name> USING <module>(...)
    let pattern = "create virtual table if not exists";
    let mut search_start = 0;

    while let Some(pos) = lower[search_start..].find(pattern) {
        let abs_pos = search_start + pos + pattern.len();
        search_start = abs_pos;

        let remaining_lower = lower[abs_pos..].trim_start();

        // Extract table name
        let table_name = match extract_identifier(remaining_lower) {
            Some(name) => name,
            None => continue,
        };

        // Find USING keyword
        let after_name = remaining_lower[table_name.len()..].trim_start();
        if !after_name.starts_with("using") {
            continue;
        }

        let after_using = after_name["using".len()..].trim_start();

        // Extract module name
        let module_name = match extract_identifier(after_using) {
            Some(name) => name,
            None => continue,
        };

        // Generate pseudo-schema based on module
        let table = if module_name == "vec0" {
            // Parse the vec0 column definitions to get actual column names
            let after_module = after_using[module_name.len()..].trim_start();
            let mut vec_table = vec0_support::vec0_pseudo_columns(&table_name);

            // Try to extract actual column definitions from vec0(...)
            if after_module.starts_with('(') {
                if let Some(body) = extract_balanced_parens(after_module) {
                    // Parse vec0 columns like "embedding float[384]"
                    for part in body.split(',') {
                        let part = part.trim().to_lowercase();
                        if part.is_empty() {
                            continue;
                        }
                        // Extract the column name (first word)
                        if let Some(col_name) = extract_identifier(&part) {
                            // Add it if not already present
                            if !vec_table.columns.iter().any(|c| c.name == col_name) {
                                vec_table.columns.push(ColumnInfo {
                                    name: col_name,
                                    col_type: "BLOB".to_string(),
                                    nullable: false,
                                    has_default: false,
                                });
                            }
                        }
                    }
                }
            }

            vec_table
        } else {
            // Unknown virtual table module - create minimal schema
            TableSchema {
                name: table_name.clone(),
                columns: vec![ColumnInfo {
                    name: "rowid".to_string(),
                    col_type: "INTEGER".to_string(),
                    nullable: false,
                    has_default: false,
                }],
                is_virtual: true,
                virtual_module: Some(module_name),
            }
        };

        schema.tables.insert(table_name, table);
    }

    // Also handle without IF NOT EXISTS
    let pattern2 = "create virtual table";
    search_start = 0;

    while let Some(pos) = lower[search_start..].find(pattern2) {
        let abs_pos = search_start + pos + pattern2.len();
        search_start = abs_pos;

        let remaining_lower = lower[abs_pos..].trim_start();

        // Skip "IF NOT EXISTS" variant (already handled above)
        if remaining_lower.starts_with("if") {
            continue;
        }

        let table_name = match extract_identifier(remaining_lower) {
            Some(name) => name,
            None => continue,
        };

        if schema.tables.contains_key(&table_name) {
            continue;
        }

        let after_name = remaining_lower[table_name.len()..].trim_start();
        if !after_name.starts_with("using") {
            continue;
        }

        let after_using = after_name["using".len()..].trim_start();
        let module_name = match extract_identifier(after_using) {
            Some(name) => name,
            None => continue,
        };

        let table = if module_name == "vec0" {
            let after_module = after_using[module_name.len()..].trim_start();
            let mut vec_table = vec0_support::vec0_pseudo_columns(&table_name);

            if after_module.starts_with('(') {
                if let Some(body) = extract_balanced_parens(after_module) {
                    for part in body.split(',') {
                        let part = part.trim().to_lowercase();
                        if part.is_empty() {
                            continue;
                        }
                        if let Some(col_name) = extract_identifier(&part) {
                            if !vec_table.columns.iter().any(|c| c.name == col_name) {
                                vec_table.columns.push(ColumnInfo {
                                    name: col_name,
                                    col_type: "BLOB".to_string(),
                                    nullable: false,
                                    has_default: false,
                                });
                            }
                        }
                    }
                }
            }

            vec_table
        } else {
            TableSchema {
                name: table_name.clone(),
                columns: vec![ColumnInfo {
                    name: "rowid".to_string(),
                    col_type: "INTEGER".to_string(),
                    nullable: false,
                    has_default: false,
                }],
                is_virtual: true,
                virtual_module: Some(module_name),
            }
        };

        schema.tables.insert(table_name, table);
    }
}

/// Parse ALTER TABLE ... ADD COLUMN statements to augment existing table schemas.
///
/// Handles patterns like:
/// - `ALTER TABLE source_items ADD COLUMN embedding_status TEXT DEFAULT 'complete'`
fn parse_alter_table_add_column(sql_text: &str, schema: &mut SchemaInfo) {
    let lower = sql_text.to_lowercase();

    // Pattern: ALTER TABLE <name> ADD COLUMN <column_def>
    // Also: ALTER TABLE <name> ADD <column_def> (without COLUMN keyword)
    let pattern = "alter table";
    let mut search_start = 0;

    while let Some(pos) = lower[search_start..].find(pattern) {
        let abs_pos = search_start + pos + pattern.len();
        search_start = abs_pos;

        let remaining_lower = lower[abs_pos..].trim_start();

        // Extract table name
        let table_name = match extract_identifier(remaining_lower) {
            Some(name) => name,
            None => continue,
        };

        // Look for ADD [COLUMN]
        let after_name = remaining_lower[table_name.len()..].trim_start();
        if !after_name.starts_with("add") {
            continue;
        }

        let after_add = after_name["add".len()..].trim_start();
        let col_def_start = if after_add.starts_with("column") {
            after_add["column".len()..].trim_start()
        } else {
            after_add
        };

        // Extract the column name and type
        // The column definition continues until ; or end of string
        let col_def_end = col_def_start.find(';').unwrap_or(col_def_start.len());
        let col_def = col_def_start[..col_def_end].trim();

        if col_def.is_empty() {
            continue;
        }

        // Parse the column definition
        if let Some(col_info) = parse_single_column(col_def) {
            // Add to existing table if it exists
            if let Some(table) = schema.tables.get_mut(&table_name) {
                if !table.columns.iter().any(|c| c.name == col_info.name) {
                    table.columns.push(col_info);
                }
            }
        }
    }
}

/// Extract a SQL identifier (table name, column name) from the start of text.
/// Returns the identifier lowercased.
fn extract_identifier(text: &str) -> Option<String> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }

    let mut chars = text.chars().peekable();
    let first = *chars.peek()?;

    // Identifiers start with a letter or underscore
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }

    let mut ident = String::new();
    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            ident.push(ch);
        } else {
            break;
        }
    }

    if ident.is_empty() {
        None
    } else {
        Some(ident.to_lowercase())
    }
}

/// Extract the contents between balanced parentheses.
/// Input should start with '('. Returns the content between '(' and matching ')'.
fn extract_balanced_parens(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || chars[0] != '(' {
        return None;
    }

    let mut depth = 0;
    let mut start = 0;

    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            '(' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(chars[start..i].iter().collect());
                }
            }
            _ => {}
        }
    }

    None
}

/// Strip SQL single-line comments (-- ...) from text.
/// This is needed because CREATE TABLE definitions in the codebase often have
/// inline comments like `column_name TEXT, -- description` which would cause
/// the next column's definition to be merged with the comment text.
fn strip_sql_comments(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_single_quote = false;

    for line in text.lines() {
        let mut cleaned_line = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '\'' && !in_single_quote {
                in_single_quote = true;
                cleaned_line.push(chars[i]);
            } else if chars[i] == '\'' && in_single_quote {
                in_single_quote = false;
                cleaned_line.push(chars[i]);
            } else if !in_single_quote
                && i + 1 < chars.len()
                && chars[i] == '-'
                && chars[i + 1] == '-'
            {
                // Found a comment outside of quotes, skip the rest of the line
                break;
            } else {
                cleaned_line.push(chars[i]);
            }
            i += 1;
        }

        result.push_str(&cleaned_line);
        result.push('\n');
    }

    result
}

/// Parse column definitions from the body of a CREATE TABLE statement.
///
/// Handles:
/// - Standard columns: `name TYPE [constraints]`
/// - DEFAULT expressions with parens: `DEFAULT (datetime('now'))`
/// - CHECK constraints
/// - FOREIGN KEY constraints (skipped)
/// - Table-level PRIMARY KEY, UNIQUE constraints (skipped)
fn parse_column_definitions(body: &str) -> Vec<ColumnInfo> {
    let mut columns = Vec::new();

    // Strip SQL inline comments (-- ...) before splitting
    let cleaned_body = strip_sql_comments(body);

    // Split by commas, but respect parentheses nesting
    let parts = split_respecting_parens(&cleaned_body);

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();

        // Skip table-level constraints
        // Use word boundary checks to avoid matching column names like "checked_at"
        if lower.starts_with("primary key")
            || lower.starts_with("foreign key")
            || lower.starts_with("unique")
            || lower.starts_with("check(")
            || lower.starts_with("check ")
            || lower == "check"
            || lower.starts_with("constraint")
        {
            continue;
        }

        // Skip CREATE INDEX that might be in the same batch
        // Use word boundary check to avoid matching column names like "created_at"
        if lower.starts_with("create ") || lower.starts_with("create\t") || lower == "create" {
            continue;
        }

        // Skip INSERT statements that might be in the same batch
        // Use word boundary check to avoid false matches
        if lower.starts_with("insert ") || lower.starts_with("insert\t") || lower == "insert" {
            continue;
        }

        // Parse as column definition: name type [constraints...]
        let col = parse_single_column(trimmed);
        if let Some(col) = col {
            columns.push(col);
        }
    }

    // Always add implicit 'rowid' column for non-WITHOUT ROWID tables
    // SQLite tables always have rowid accessible
    if !columns.iter().any(|c| c.name == "rowid") {
        columns.push(ColumnInfo {
            name: "rowid".to_string(),
            col_type: "INTEGER".to_string(),
            nullable: false,
            has_default: true, // auto-generated
        });
    }

    columns
}

/// Split text by commas, respecting parentheses nesting.
fn split_respecting_parens(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_single_quote = false;

    for ch in text.chars() {
        match ch {
            '\'' if depth == 0 => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            }
            '(' if !in_single_quote => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_single_quote => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 && !in_single_quote => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }

    parts
}

/// Parse a single column definition like "name TEXT NOT NULL DEFAULT 'foo'"
fn parse_single_column(definition: &str) -> Option<ColumnInfo> {
    let lower = definition.to_lowercase();
    let tokens = tokenize_sql_definition(&lower);

    if tokens.is_empty() {
        return None;
    }

    let name = &tokens[0];

    // Skip SQL keywords that aren't column names
    let sql_keywords = [
        "primary",
        "foreign",
        "unique",
        "check",
        "constraint",
        "create",
        "insert",
        "index",
        "on",
        "references",
        "--",
    ];
    if sql_keywords.contains(&name.as_str()) {
        return None;
    }

    // Extract type (second token if it looks like a type)
    let col_type = if tokens.len() > 1 {
        let t = &tokens[1];
        // Check if it looks like a SQL type
        if is_sql_type(t) {
            t.to_uppercase()
        } else {
            // Could be a constraint-only column (e.g., "id INTEGER PRIMARY KEY")
            // or the type might be part of the name, treat as TEXT
            "TEXT".to_string()
        }
    } else {
        "TEXT".to_string()
    };

    // Check for NOT NULL
    let nullable = !lower.contains("not null");

    // Check for DEFAULT
    let has_default = lower.contains("default")
        || lower.contains("autoincrement")
        || lower.contains("primary key");

    Some(ColumnInfo {
        name: name.to_string(),
        col_type,
        nullable,
        has_default,
    })
}

/// Tokenize a SQL column definition, treating parenthesized expressions as single tokens.
fn tokenize_sql_definition(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_single_quote = false;

    for ch in text.chars() {
        match ch {
            '\'' => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            }
            '(' if !in_single_quote => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_single_quote => {
                depth -= 1;
                current.push(ch);
            }
            ' ' | '\t' | '\n' | '\r' if depth == 0 && !in_single_quote => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    tokens.push(trimmed);
                }
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        tokens.push(trimmed);
    }

    tokens
}

/// Check if a token looks like a SQL column type
fn is_sql_type(token: &str) -> bool {
    let types = [
        "integer",
        "int",
        "text",
        "real",
        "blob",
        "numeric",
        "boolean",
        "json",
        "float",
        "varchar",
        "char",
        "double",
        "bigint",
        "smallint",
        "tinyint",
        "decimal",
        "date",
        "datetime",
        "timestamp",
    ];

    // Direct match or starts with a known type (handles "float[384]", "varchar(255)")
    let lower = token.to_lowercase();
    types.iter().any(|t| lower.starts_with(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let source = r#"
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    email TEXT UNIQUE,
                    age INTEGER DEFAULT 0
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("users"));
        assert!(schema.has_column("users", "id"));
        assert!(schema.has_column("users", "name"));
        assert!(schema.has_column("users", "email"));
        assert!(schema.has_column("users", "age"));
    }

    #[test]
    fn test_parse_raw_string() {
        let source = r##"
            conn.execute_batch(r#"
                CREATE TABLE IF NOT EXISTS items (
                    id INTEGER PRIMARY KEY,
                    title TEXT NOT NULL,
                    created_at TEXT DEFAULT (datetime('now'))
                );
            "#);
        "##;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("items"));
        assert!(schema.has_column("items", "id"));
        assert!(schema.has_column("items", "title"));
        assert!(schema.has_column("items", "created_at"));
    }

    #[test]
    fn test_parse_virtual_table() {
        let source = r#"
            conn.execute_batch("
                CREATE VIRTUAL TABLE IF NOT EXISTS my_vec USING vec0(
                    embedding float[384]
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("my_vec"));
        assert!(schema.has_column("my_vec", "rowid"));
        assert!(schema.has_column("my_vec", "embedding"));
        assert!(schema.has_column("my_vec", "distance"));
    }

    #[test]
    fn test_parse_default_with_parens() {
        let source = r#"
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS logs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    message TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT DEFAULT (datetime('now'))
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("logs"));
        assert!(schema.has_column("logs", "id"));
        assert!(schema.has_column("logs", "message"));
        assert!(schema.has_column("logs", "created_at"));
        assert!(schema.has_column("logs", "updated_at"));
    }

    #[test]
    fn test_parse_foreign_key() {
        let source = r#"
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS comments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    post_id INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("comments"));
        assert!(schema.has_column("comments", "id"));
        assert!(schema.has_column("comments", "post_id"));
        assert!(schema.has_column("comments", "text"));
        // FOREIGN KEY should not be parsed as a column
        assert!(!schema.has_column("comments", "foreign"));
    }

    #[test]
    fn test_case_insensitive() {
        let source = r#"
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS MyTable (
                    ID INTEGER PRIMARY KEY,
                    Name TEXT NOT NULL
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("mytable"));
        assert!(schema.has_table("MyTable"));
        assert!(schema.has_column("MyTable", "id"));
        assert!(schema.has_column("mytable", "name"));
    }

    #[test]
    fn test_check_constraint_in_column() {
        let source = r#"
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS jobs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed')),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            ");
        "#;

        let schema = parse_schema_from_source(source);
        assert!(schema.has_table("jobs"));
        assert!(schema.has_column("jobs", "id"));
        assert!(schema.has_column("jobs", "status"));
        assert!(schema.has_column("jobs", "created_at"));
    }

    #[test]
    fn test_extract_identifier() {
        assert_eq!(extract_identifier("users"), Some("users".to_string()));
        assert_eq!(
            extract_identifier("  my_table "),
            Some("my_table".to_string())
        );
        assert_eq!(
            extract_identifier("Table123("),
            Some("table123".to_string())
        );
        assert_eq!(extract_identifier("123bad"), None);
        assert_eq!(extract_identifier(""), None);
    }

    #[test]
    fn test_split_respecting_parens() {
        let parts = split_respecting_parens("a, b, c");
        assert_eq!(parts, vec!["a", "b", "c"]);

        let parts = split_respecting_parens("a DEFAULT (1,2), b, c");
        assert_eq!(parts, vec!["a DEFAULT (1,2)", "b", "c"]);
    }
}
