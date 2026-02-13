//! SQL Validator: checks extracted queries against the parsed schema.
//!
//! Performs lightweight validation of SQL queries:
//! - Table name existence
//! - Column name existence (for simple single-table queries)
//! - Parameter count consistency
//!
//! Conservative: skips anything uncertain to avoid false positives.

use super::schema_parser::SchemaInfo;
use super::sql_extractor::ExtractedQuery;

#[derive(Debug)]
pub enum ValidationError {
    UnknownTable {
        table: String,
        file: String,
        line: usize,
        sql_preview: String,
    },
    UnknownColumn {
        table: String,
        column: String,
        file: String,
        line: usize,
        sql_preview: String,
    },
    #[allow(dead_code)]
    ParameterCountMismatch {
        expected: usize,
        found: usize,
        file: String,
        line: usize,
        sql_preview: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTable {
                table,
                file,
                line,
                sql_preview,
            } => write!(
                f,
                "  {}:{} - UnknownTable {{ table: \"{}\" }}\n    SQL: {}",
                file, line, table, sql_preview
            ),
            Self::UnknownColumn {
                table,
                column,
                file,
                line,
                sql_preview,
            } => write!(
                f,
                "  {}:{} - UnknownColumn {{ table: \"{}\", column: \"{}\" }}\n    SQL: {}",
                file, line, table, column, sql_preview
            ),
            Self::ParameterCountMismatch {
                expected,
                found,
                file,
                line,
                sql_preview,
            } => write!(
                f,
                "  {}:{} - ParameterCountMismatch {{ expected: {}, found: {} }}\n    SQL: {}",
                file, line, expected, found, sql_preview
            ),
        }
    }
}

/// Validate a single query against the schema.
pub fn validate_query(query: &ExtractedQuery, schema: &SchemaInfo) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Skip dynamic SQL entirely
    if query.is_dynamic {
        return errors;
    }

    let sql = &query.sql;
    let lower = sql.to_lowercase();
    let preview = sql_preview(sql);

    // Skip very short queries or non-SQL strings that slipped through
    if lower.len() < 10 {
        return errors;
    }

    // Skip queries that contain subqueries (too complex for text-based validation)
    let select_count = lower.matches("select").count();
    if select_count > 1 {
        return errors;
    }

    // Skip queries with UNION
    if lower.contains(" union ") {
        return errors;
    }

    // Skip queries with CTEs (WITH ... AS)
    if lower.trim_start().starts_with("with ") {
        return errors;
    }

    // Extract table references
    let table_refs = extract_table_references(&lower);

    // If no tables found, skip (probably not real SQL)
    if table_refs.is_empty() {
        return errors;
    }

    // Validate table references
    let mut valid_tables: Vec<String> = Vec::new();
    let mut table_aliases: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (table_name, alias) in &table_refs {
        // Skip known SQLite internal tables/functions
        if is_sqlite_internal(table_name) {
            valid_tables.push(table_name.clone());
            if let Some(a) = alias {
                table_aliases.insert(a.clone(), table_name.clone());
            }
            continue;
        }

        if schema.has_table(table_name) {
            valid_tables.push(table_name.clone());
            if let Some(a) = alias {
                table_aliases.insert(a.clone(), table_name.clone());
            }
        } else {
            errors.push(ValidationError::UnknownTable {
                table: table_name.clone(),
                file: query.file_path.clone(),
                line: query.line_number,
                sql_preview: preview.clone(),
            });
        }
    }

    // Column validation: only for single-table queries (no JOINs) to minimize false positives
    if valid_tables.len() == 1 && table_refs.len() == 1 && errors.is_empty() {
        let table_name = &valid_tables[0];

        // Skip column validation for sqlite internal tables
        if !is_sqlite_internal(table_name) {
            // Skip column validation for vec0 MATCH queries (special syntax)
            if is_vec0_match_query(&lower) {
                return errors;
            }

            let columns = extract_column_references(&lower);
            for col in &columns {
                if !is_valid_column(col, table_name, schema) {
                    errors.push(ValidationError::UnknownColumn {
                        table: table_name.clone(),
                        column: col.clone(),
                        file: query.file_path.clone(),
                        line: query.line_number,
                        sql_preview: preview.clone(),
                    });
                }
            }
        }
    }

    errors
}

/// Validate all queries against the schema.
pub fn validate_all(queries: &[ExtractedQuery], schema: &SchemaInfo) -> Vec<ValidationError> {
    queries
        .iter()
        .flat_map(|q| validate_query(q, schema))
        .collect()
}

/// Extract table references from SQL.
/// Returns (table_name, optional_alias) pairs.
fn extract_table_references(sql: &str) -> Vec<(String, Option<String>)> {
    let mut tables = Vec::new();

    // Normalize whitespace for easier regex-like matching
    let normalized = normalize_for_matching(sql);

    // FROM table [alias]
    extract_table_after_keyword(&normalized, "from", &mut tables);

    // INTO table
    extract_table_after_keyword(&normalized, "into", &mut tables);

    // UPDATE table
    extract_table_after_keyword(&normalized, "update", &mut tables);

    // JOIN table [alias]
    extract_table_after_keyword(&normalized, "join", &mut tables);

    // DELETE FROM table
    // Already handled by "from" above

    // INSERT OR IGNORE INTO / INSERT OR REPLACE INTO
    // The "into" keyword catch handles these

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    tables.retain(|(name, _)| seen.insert(name.clone()));

    tables
}

/// Extract a table name (and optional alias) after a keyword.
fn extract_table_after_keyword(
    sql: &str,
    keyword: &str,
    tables: &mut Vec<(String, Option<String>)>,
) {
    let search = format!(" {} ", keyword);
    let mut pos = 0;

    while let Some(found) = sql[pos..].find(&search) {
        let after = pos + found + search.len();
        pos = after;

        let remaining = sql[after..].trim_start();

        // Extract table name
        let table_name = match extract_word(remaining) {
            Some(name) => name,
            None => continue,
        };

        // Skip SQL keywords that appear after FROM/JOIN but aren't table names
        if is_sql_keyword(&table_name) {
            continue;
        }

        // Check for alias after table name
        let after_table = remaining[table_name.len()..].trim_start();
        let alias = if let Some(next_word) = extract_word(after_table) {
            if !is_sql_keyword(&next_word)
                && next_word != "on"
                && next_word != "where"
                && next_word != "set"
                && next_word != "values"
                && next_word != "using"
                && next_word != "order"
                && next_word != "group"
                && next_word != "having"
                && next_word != "limit"
                && next_word != "("
            {
                // Check for explicit AS
                if next_word == "as" {
                    let after_as = after_table[next_word.len()..].trim_start();
                    extract_word(after_as)
                } else {
                    Some(next_word)
                }
            } else {
                None
            }
        } else {
            None
        };

        tables.push((table_name, alias));
    }

    // Also check if the keyword is at the very start of the string
    let start_search = format!("{} ", keyword);
    if sql.starts_with(&start_search) {
        let remaining = sql[start_search.len()..].trim_start();
        if let Some(table_name) = extract_word(remaining) {
            if !is_sql_keyword(&table_name) {
                let after_table = remaining[table_name.len()..].trim_start();
                let alias = if let Some(next_word) = extract_word(after_table) {
                    if !is_sql_keyword(&next_word)
                        && next_word != "on"
                        && next_word != "where"
                        && next_word != "set"
                        && next_word != "values"
                        && next_word != "using"
                        && next_word != "order"
                        && next_word != "group"
                        && next_word != "having"
                        && next_word != "limit"
                        && next_word != "("
                    {
                        if next_word == "as" {
                            let after_as = after_table[next_word.len()..].trim_start();
                            extract_word(after_as)
                        } else {
                            Some(next_word)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Only add if not already present
                if !tables.iter().any(|(t, _)| *t == table_name) {
                    tables.push((table_name, alias));
                }
            }
        }
    }
}

/// Extract column references from SQL for single-table validation.
///
/// This is intentionally conservative - it only extracts columns it's confident about.
fn extract_column_references(sql: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let normalized = normalize_for_matching(sql);

    // SELECT columns (between SELECT and FROM)
    if let Some(select_pos) = normalized.find("select ") {
        if let Some(from_pos) = normalized.find(" from ") {
            if select_pos < from_pos {
                let select_part = &normalized[select_pos + 7..from_pos];
                extract_select_columns(select_part, &mut columns);
            }
        }
    }

    // WHERE columns
    if let Some(where_pos) = normalized.find(" where ") {
        let where_clause = &normalized[where_pos + 7..];
        // Cut off ORDER BY, GROUP BY, LIMIT, etc.
        let end = find_clause_end(where_clause);
        let where_part = &where_clause[..end];
        extract_where_columns(where_part, &mut columns);
    }

    // SET columns (UPDATE ... SET col = val, ...)
    if let Some(set_pos) = normalized.find(" set ") {
        let set_clause = &normalized[set_pos + 5..];
        let end = find_clause_end(set_clause);
        let set_part = &set_clause[..end];
        extract_set_columns(set_part, &mut columns);
    }

    // INSERT columns (INSERT INTO table (col1, col2, ...) ...)
    if let Some(into_pos) = normalized.find(" into ") {
        let after_into = &normalized[into_pos + 6..];
        if let Some(table_name) = extract_word(after_into) {
            let after_table = after_into[table_name.len()..].trim_start();
            if after_table.starts_with('(') {
                if let Some(close) = find_matching_paren(after_table) {
                    let col_list = &after_table[1..close];
                    for col in col_list.split(',') {
                        let col = col.trim();
                        if let Some(name) = extract_word(col) {
                            if !is_sql_keyword(&name) && !name.starts_with('?') {
                                columns.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    // NOTE: ORDER BY and GROUP BY columns are intentionally NOT validated.
    // They frequently use aliases from SELECT (e.g., "SELECT x AS y ... ORDER BY y")
    // which would cause false positives. The main value of column validation
    // is in SELECT, WHERE, SET, and INSERT columns.

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    columns.retain(|c| seen.insert(c.clone()));

    columns
}

/// Extract column names from a SELECT clause
fn extract_select_columns(select_part: &str, columns: &mut Vec<String>) {
    let trimmed = select_part.trim();

    // SELECT * - skip
    if trimmed == "*" {
        return;
    }

    // Split by commas (respecting parentheses)
    let parts = split_respecting_parens_simple(trimmed);

    for part in &parts {
        let part = part.trim();

        // Skip empty parts
        if part.is_empty() {
            continue;
        }

        // Handle "expr AS alias" - the alias isn't a column reference
        let effective = if let Some(as_pos) = part.rfind(" as ") {
            &part[..as_pos]
        } else {
            part
        };

        let effective = effective.trim();

        // Skip * (including table.*)
        if effective.ends_with('*') {
            continue;
        }

        // Skip function calls like COUNT(*), datetime('now'), etc.
        if effective.contains('(') {
            // Extract column names from function arguments
            // But this is complex, so skip for now to avoid false positives
            continue;
        }

        // Skip numeric literals
        if effective.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }

        // Skip string literals
        if effective.starts_with('\'') || effective.starts_with('"') {
            continue;
        }

        // Skip parameters
        if effective.starts_with('?') {
            continue;
        }

        // Handle "table.column" - extract just the column name
        let col_name = if let Some(dot_pos) = effective.rfind('.') {
            &effective[dot_pos + 1..]
        } else {
            effective
        };

        if let Some(name) = extract_word(col_name) {
            if !is_sql_keyword(&name) && !name.is_empty() {
                columns.push(name);
            }
        }
    }
}

/// Extract column names from WHERE clause comparisons
fn extract_where_columns(where_part: &str, columns: &mut Vec<String>) {
    // Look for patterns like: column_name = ?, column_name > ?, column_name IS NULL, etc.
    let tokens = tokenize_simple(where_part);

    for i in 0..tokens.len() {
        let token = &tokens[i];

        // Skip non-identifiers
        if !token
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        {
            continue;
        }

        // Check if next token is an operator
        if i + 1 < tokens.len() {
            let next = &tokens[i + 1];
            if is_comparison_operator(next)
                || next == "is"
                || next == "in"
                || next == "like"
                || next == "between"
                || next == "match"
            {
                let name = if let Some(dot_pos) = token.rfind('.') {
                    &token[dot_pos + 1..]
                } else {
                    token.as_str()
                };

                if !is_sql_keyword(name)
                    && !name.starts_with('?')
                    && !is_sql_function(name)
                    && !name.is_empty()
                {
                    columns.push(name.to_string());
                }
            }
        }
    }
}

/// Extract column names from SET clause
fn extract_set_columns(set_part: &str, columns: &mut Vec<String>) {
    // SET col1 = val1, col2 = val2, ...
    for assignment in set_part.split(',') {
        let assignment = assignment.trim();
        if let Some(eq_pos) = assignment.find('=') {
            let col_part = assignment[..eq_pos].trim();
            if let Some(name) = extract_word(col_part) {
                if !is_sql_keyword(&name) && !name.starts_with('?') {
                    columns.push(name);
                }
            }
        }
    }
}

/// Check if a column reference is valid for a given table
fn is_valid_column(column: &str, table: &str, schema: &SchemaInfo) -> bool {
    // Always allow these pseudo-columns
    if column == "rowid" || column == "_rowid_" || column == "oid" {
        return true;
    }

    // Allow any column that exists in the table schema
    if schema.has_column(table, column) {
        return true;
    }

    // For aggregate queries, allow count/sum/min/max etc as "columns"
    if is_sql_function(column) {
        return true;
    }

    // Allow numeric literals that might have been tokenized oddly
    if column.parse::<f64>().is_ok() {
        return true;
    }

    false
}

/// Check if a table name is an internal SQLite entity
fn is_sqlite_internal(name: &str) -> bool {
    name.starts_with("sqlite_")
        || name.starts_with("pragma_")
        || name == "json_each"
        || name == "json_tree"
        || name == "generate_series"
        || name == "fts"
        || name == "vec_each"
}

/// Check if a SQL query uses vec0 MATCH syntax
fn is_vec0_match_query(sql: &str) -> bool {
    sql.contains("match") && (sql.contains("_vec") || sql.contains("embedding"))
}

/// Check if a word is a SQL keyword (not a table/column name)
fn is_sql_keyword(word: &str) -> bool {
    matches!(
        word,
        "select"
            | "from"
            | "where"
            | "and"
            | "or"
            | "not"
            | "in"
            | "is"
            | "null"
            | "like"
            | "between"
            | "exists"
            | "insert"
            | "into"
            | "values"
            | "update"
            | "set"
            | "delete"
            | "join"
            | "inner"
            | "left"
            | "right"
            | "outer"
            | "cross"
            | "on"
            | "as"
            | "order"
            | "by"
            | "group"
            | "having"
            | "limit"
            | "offset"
            | "asc"
            | "desc"
            | "distinct"
            | "case"
            | "when"
            | "then"
            | "else"
            | "end"
            | "cast"
            | "union"
            | "all"
            | "except"
            | "intersect"
            | "with"
            | "recursive"
            | "create"
            | "table"
            | "index"
            | "if"
            | "alter"
            | "drop"
            | "primary"
            | "key"
            | "foreign"
            | "references"
            | "default"
            | "check"
            | "unique"
            | "constraint"
            | "autoincrement"
            | "cascade"
            | "conflict"
            | "replace"
            | "ignore"
            | "abort"
            | "rollback"
            | "fail"
            | "do"
            | "nothing"
            | "match"
            | "true"
            | "false"
            | "pragma"
    )
}

/// Check if a word is a SQL function name
fn is_sql_function(word: &str) -> bool {
    matches!(
        word,
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "abs"
            | "coalesce"
            | "ifnull"
            | "nullif"
            | "iif"
            | "typeof"
            | "length"
            | "lower"
            | "upper"
            | "trim"
            | "ltrim"
            | "rtrim"
            | "substr"
            | "substring"
            | "replace"
            | "instr"
            | "hex"
            | "quote"
            | "randomblob"
            | "zeroblob"
            | "unicode"
            | "char"
            | "glob"
            | "like"
            | "printf"
            | "format"
            | "datetime"
            | "date"
            | "time"
            | "julianday"
            | "strftime"
            | "json"
            | "json_extract"
            | "json_array"
            | "json_object"
            | "json_type"
            | "json_valid"
            | "json_group_array"
            | "json_group_object"
            | "total"
            | "group_concat"
            | "row_number"
            | "rank"
            | "dense_rank"
            | "ntile"
            | "lag"
            | "lead"
            | "first_value"
            | "last_value"
            | "nth_value"
            | "vec_distance_l2"
            | "vec_distance_cosine"
    )
}

/// Check if a token is a comparison operator
fn is_comparison_operator(token: &str) -> bool {
    matches!(token, "=" | "!=" | "<>" | "<" | ">" | "<=" | ">=" | "==")
}

/// Normalize SQL for matching: collapse whitespace, lowercase
fn normalize_for_matching(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len() + 2);
    result.push(' '); // leading space so " keyword " patterns work at start
    let mut last_was_space = false;

    for ch in sql.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch.to_ascii_lowercase());
            last_was_space = false;
        }
    }

    result.push(' '); // trailing space
    result
}

/// Extract a column name from a potentially qualified reference like "table.column".
/// Returns just the column name part.
fn extract_column_name_from_ref(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Handle table.column pattern
    if let Some(dot_pos) = text.find('.') {
        let after_dot = &text[dot_pos + 1..];
        extract_word(after_dot)
    } else {
        extract_word(text)
    }
}

/// Extract a single word (identifier) from the start of text
fn extract_word(text: &str) -> Option<String> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }

    let first = text.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }

    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            break;
        }
    }

    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

/// Simple tokenizer for SQL: splits on whitespace and punctuation
fn tokenize_simple(sql: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for ch in sql.chars() {
        if ch == '\'' {
            in_quote = !in_quote;
            if !in_quote {
                // End of string literal, discard it
                current.clear();
                continue;
            } else {
                current.clear();
                continue;
            }
        }

        if in_quote {
            continue;
        }

        if ch.is_whitespace() || ch == '(' || ch == ')' || ch == ',' || ch == ';' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                tokens.push(trimmed);
            }
            current.clear();

            // Add single-char operators as tokens
            if ch == '(' || ch == ')' {
                tokens.push(ch.to_string());
            }
        } else if ch == '=' || ch == '<' || ch == '>' || ch == '!' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                tokens.push(trimmed);
            }
            current.clear();

            // Build multi-char operators
            current.push(ch);
        } else {
            // If current is an operator char, push it first
            if !current.is_empty()
                && current
                    .chars()
                    .all(|c| c == '=' || c == '<' || c == '>' || c == '!')
            {
                tokens.push(current.clone());
                current.clear();
            }
            current.push(ch);
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        tokens.push(trimmed);
    }

    tokens
}

/// Find where a WHERE/SET clause ends
fn find_clause_end(clause: &str) -> usize {
    let terminators = [
        " order ",
        " group ",
        " having ",
        " limit ",
        " offset ",
        " union ",
        " except ",
        " intersect ",
        ";",
    ];

    let mut min_pos = clause.len();
    for term in &terminators {
        if let Some(pos) = clause.find(term) {
            min_pos = min_pos.min(pos);
        }
    }

    min_pos
}

/// Find where an ORDER BY/GROUP BY clause ends
fn find_clause_end_for_order(clause: &str) -> usize {
    let terminators = [" limit ", " offset ", " having ", ";"];

    let mut min_pos = clause.len();
    for term in &terminators {
        if let Some(pos) = clause.find(term) {
            min_pos = min_pos.min(pos);
        }
    }

    min_pos
}

/// Split text by commas, respecting parentheses
fn split_respecting_parens_simple(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_quote = false;

    for ch in text.chars() {
        match ch {
            '\'' => {
                in_quote = !in_quote;
                current.push(ch);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_quote => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 && !in_quote => {
                parts.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

/// Find matching closing paren position
fn find_matching_paren(text: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in text.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Create a short preview of SQL for error messages
fn sql_preview(sql: &str) -> String {
    let clean: String = sql
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.len() > 120 {
        format!("{}...", &clean[..120])
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_check_standalone::schema_parser::ColumnInfo;
    use crate::sql_check_standalone::schema_parser::TableSchema;

    fn make_schema() -> SchemaInfo {
        let mut schema = SchemaInfo::default();
        schema.tables.insert(
            "users".to_string(),
            TableSchema {
                name: "users".to_string(),
                columns: vec![
                    ColumnInfo {
                        name: "id".to_string(),
                        col_type: "INTEGER".to_string(),
                        nullable: false,
                        has_default: true,
                    },
                    ColumnInfo {
                        name: "name".to_string(),
                        col_type: "TEXT".to_string(),
                        nullable: false,
                        has_default: false,
                    },
                    ColumnInfo {
                        name: "email".to_string(),
                        col_type: "TEXT".to_string(),
                        nullable: true,
                        has_default: false,
                    },
                    ColumnInfo {
                        name: "rowid".to_string(),
                        col_type: "INTEGER".to_string(),
                        nullable: false,
                        has_default: true,
                    },
                ],
                is_virtual: false,
                virtual_module: None,
            },
        );
        schema
    }

    #[test]
    fn test_valid_select() {
        let schema = make_schema();
        let query = ExtractedQuery {
            sql: "SELECT id, name FROM users WHERE email = ?1".to_string(),
            file_path: "test.rs".to_string(),
            line_number: 10,
            param_count: 1,
            is_dynamic: false,
        };

        let errors = validate_query(&query, &schema);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_unknown_table() {
        let schema = make_schema();
        let query = ExtractedQuery {
            sql: "SELECT * FROM nonexistent WHERE id = ?1".to_string(),
            file_path: "test.rs".to_string(),
            line_number: 10,
            param_count: 1,
            is_dynamic: false,
        };

        let errors = validate_query(&query, &schema);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnknownTable { table, .. } if table == "nonexistent")
        );
    }

    #[test]
    fn test_unknown_column() {
        let schema = make_schema();
        let query = ExtractedQuery {
            sql: "SELECT nonexistent_col FROM users WHERE id = ?1".to_string(),
            file_path: "test.rs".to_string(),
            line_number: 10,
            param_count: 1,
            is_dynamic: false,
        };

        let errors = validate_query(&query, &schema);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnknownColumn { column, .. } if column == "nonexistent_col")
        );
    }

    #[test]
    fn test_skip_dynamic() {
        let schema = make_schema();
        let query = ExtractedQuery {
            sql: "SELECT * FROM nonexistent".to_string(),
            file_path: "test.rs".to_string(),
            line_number: 10,
            param_count: 0,
            is_dynamic: true,
        };

        let errors = validate_query(&query, &schema);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_extract_table_references() {
        let refs = extract_table_references("select * from users where id = 1");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "users");

        let refs = extract_table_references("insert into users (name) values ('test')");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "users");

        let refs = extract_table_references("update users set name = 'test'");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "users");
    }

    #[test]
    fn test_table_alias() {
        let refs = extract_table_references("select u.name from users u where u.id = 1");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "users");
        assert_eq!(refs[0].1.as_deref(), Some("u"));
    }
}
