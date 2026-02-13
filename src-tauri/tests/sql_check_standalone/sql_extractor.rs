//! SQL Extractor: finds SQL queries in Rust source files.
//!
//! Walks `.rs` files under a directory, extracts SQL string literals
//! containing DML statements (SELECT, INSERT, UPDATE, DELETE, REPLACE),
//! and records their file paths and line numbers.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExtractedQuery {
    pub sql: String,
    pub file_path: String,
    pub line_number: usize,
    pub param_count: usize,
    pub is_dynamic: bool,
}

/// Extract SQL queries from all `.rs` files under a directory (recursive).
pub fn extract_queries_from_dir(dir: &Path) -> Vec<ExtractedQuery> {
    let mut queries = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        eprintln!("Warning: directory does not exist: {}", dir.display());
        return queries;
    }

    walk_rs_files(dir, &mut |path| {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: could not read {}: {}", path.to_string_lossy(), e);
                return;
            }
        };

        let file_path = path.to_string_lossy().to_string();
        let file_queries = extract_queries_from_file(&file_path, &source);
        queries.extend(file_queries);
    });

    queries
}

/// Recursively walk `.rs` files, skipping _future and test/bench dirs.
fn walk_rs_files(dir: &Path, callback: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip known non-source directories
            if dir_name.starts_with('.')
                || dir_name == "target"
                || dir_name == "_future"
                || dir_name == "node_modules"
                || dir_name == "sql_check"
            {
                continue;
            }

            walk_rs_files(&path, callback);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            // Skip bin directory test files (they have test-specific SQL)
            let path_str = path.to_string_lossy();
            if path_str.contains("\\bin\\") || path_str.contains("/bin/") {
                continue;
            }
            callback(&path);
        }
    }
}

/// Extract SQL queries from a single Rust source file.
pub fn extract_queries_from_file(file_path: &str, source_text: &str) -> Vec<ExtractedQuery> {
    let mut queries = Vec::new();
    let lines: Vec<&str> = source_text.lines().collect();

    // Find the line number where #[cfg(test)] blocks start.
    // SQL in test blocks often uses different schemas and shouldn't be validated.
    let test_block_start = find_test_block_start(&lines);

    // Track string literals and their locations
    let string_lits = extract_string_literals_with_lines(source_text);

    for (content, line_num) in &string_lits {
        let lower = content.to_lowercase();
        let trimmed = lower.trim();

        // Must contain a DML keyword with proper SQL structure
        if !is_dml_query(trimmed) {
            continue;
        }

        // Skip DDL statements
        if is_ddl_statement(trimmed) {
            continue;
        }

        // Skip PRAGMA statements
        if trimmed.starts_with("pragma") {
            continue;
        }

        // Determine if this is dynamic SQL (in a format! macro)
        let mut is_dynamic = is_in_format_macro(source_text, &lines, *line_num);

        // Mark queries in #[cfg(test)] blocks as dynamic (skip validation)
        if let Some(test_start) = test_block_start {
            if *line_num >= test_start {
                is_dynamic = true;
            }
        }

        // Count parameters
        let param_count = count_parameters(content);

        // Clean up the SQL for storage (normalize whitespace)
        let clean_sql = normalize_whitespace(content);

        if clean_sql.len() > 5 {
            queries.push(ExtractedQuery {
                sql: clean_sql,
                file_path: file_path.to_string(),
                line_number: *line_num,
                param_count,
                is_dynamic,
            });
        }
    }

    queries
}

/// Find the line number where #[cfg(test)] block starts in a file.
/// Returns None if there is no test block.
fn find_test_block_start(lines: &[&str]) -> Option<usize> {
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "#[cfg(test)]" {
            return Some(i + 1); // 1-based line number
        }
    }
    None
}

/// Check if a string looks like a DML SQL query.
///
/// Requires SQL keywords to be followed by appropriate SQL structure,
/// to avoid matching English sentences like "Update available" or
/// "Failed to update job status".
fn is_dml_query(lower: &str) -> bool {
    // SELECT ... FROM
    if lower.starts_with("select ") && (lower.contains(" from ") || lower.contains(" from\n")) {
        return true;
    }
    // INSERT INTO / INSERT OR IGNORE INTO / INSERT OR REPLACE INTO
    if lower.starts_with("insert ") && lower.contains(" into ") {
        return true;
    }
    // UPDATE ... SET
    if lower.starts_with("update ") && lower.contains(" set ") {
        return true;
    }
    // DELETE FROM
    if lower.starts_with("delete ") && lower.contains(" from ") {
        return true;
    }
    // REPLACE INTO
    if lower.starts_with("replace ") && lower.contains(" into ") {
        return true;
    }
    false
}

/// Check if a string is a DDL statement (should be skipped)
fn is_ddl_statement(lower: &str) -> bool {
    lower.starts_with("create")
        || lower.starts_with("alter")
        || lower.starts_with("drop")
        || lower.starts_with("--")
        || lower.starts_with("pragma")
        // Skip strings that are just table/column names or type references
        || (!lower.contains("select")
            && !lower.contains("insert")
            && !lower.contains("update")
            && !lower.contains("delete")
            && !lower.contains("replace"))
}

/// Check if a line is inside a format! macro (making it dynamic SQL)
fn is_in_format_macro(source: &str, lines: &[&str], line_num: usize) -> bool {
    // Check the line itself and a few lines before for format! macro
    let start = if line_num > 5 { line_num - 5 } else { 0 };
    let end = std::cmp::min(line_num + 1, lines.len());

    for i in start..end {
        let line = lines[i].trim();
        if line.contains("format!(") || line.contains("&format!(") {
            return true;
        }
    }

    // Also check if the string contains {} placeholders (format string markers)
    // But only curly braces that look like format placeholders, not JSON
    let line_content = if line_num > 0 && line_num <= lines.len() {
        lines[line_num - 1]
    } else {
        ""
    };

    // Check for common format patterns
    if line_content.contains("format!(") {
        return true;
    }

    // Check if the SQL itself contains {} placeholders (but not JSON like '{}')
    // A format string typically has {} or {0} or {name} outside of quotes
    let _ = source; // suppress unused warning
    false
}

/// Count SQL parameter placeholders (? and ?N patterns)
fn count_parameters(sql: &str) -> usize {
    let mut count = 0;
    let mut max_numbered = 0;
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_single_quote = false;

    while i < len {
        if chars[i] == '\'' {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }

        if !in_single_quote && chars[i] == '?' {
            // Check for numbered parameter ?1, ?2, etc.
            let mut num = String::new();
            let mut j = i + 1;
            while j < len && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }

            if !num.is_empty() {
                if let Ok(n) = num.parse::<usize>() {
                    max_numbered = max_numbered.max(n);
                }
                i = j;
            } else {
                count += 1;
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    // If numbered parameters are used, use the max number
    if max_numbered > 0 {
        max_numbered
    } else {
        count
    }
}

/// Normalize whitespace in SQL: collapse multiple spaces/newlines into single spaces
fn normalize_whitespace(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut last_was_space = false;

    for ch in sql.chars() {
        if ch.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

/// Extract string literals with their approximate line numbers.
///
/// Returns (content, line_number) pairs.
fn extract_string_literals_with_lines(source: &str) -> Vec<(String, usize)> {
    let mut results = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut current_line = 1;

    while i < len {
        // Track line numbers
        if chars[i] == '\n' {
            current_line += 1;
            i += 1;
            continue;
        }

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
                if chars[i] == '\n' {
                    current_line += 1;
                }
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

        // Raw string: r#"..."# or r"..."
        if chars[i] == 'r' && i + 1 < len && (chars[i + 1] == '#' || chars[i + 1] == '"') {
            let string_line = current_line;
            let start = i;
            i += 1; // skip 'r'

            let mut hash_count = 0;
            while i < len && chars[i] == '#' {
                hash_count += 1;
                i += 1;
            }

            if i < len && chars[i] == '"' {
                i += 1; // skip opening quote
                let content_start = i;

                'raw_outer: while i < len {
                    if chars[i] == '\n' {
                        current_line += 1;
                    }
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
                            if content.len() > 10 {
                                results.push((content, string_line));
                            }
                            break 'raw_outer;
                        }
                    } else {
                        i += 1;
                    }
                }
                continue;
            } else {
                i = start + 1;
                continue;
            }
        }

        // Regular string: "..."
        if chars[i] == '"' {
            let string_line = current_line;
            i += 1; // skip opening quote
            let mut content = String::new();

            while i < len && chars[i] != '"' {
                if chars[i] == '\n' {
                    current_line += 1;
                }
                if chars[i] == '\\' && i + 1 < len {
                    match chars[i + 1] {
                        'n' => content.push('\n'),
                        't' => content.push('\t'),
                        'r' => content.push('\r'),
                        '\\' => content.push('\\'),
                        '"' => content.push('"'),
                        '\n' => {
                            current_line += 1;
                            // Line continuation - skip
                        }
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
                results.push((content, string_line));
            }
            continue;
        }

        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_parameters_simple() {
        assert_eq!(count_parameters("SELECT * FROM t WHERE id = ?"), 1);
        assert_eq!(count_parameters("INSERT INTO t VALUES (?, ?, ?)"), 3);
        assert_eq!(count_parameters("SELECT * FROM t"), 0);
    }

    #[test]
    fn test_count_parameters_numbered() {
        assert_eq!(
            count_parameters("SELECT * FROM t WHERE a = ?1 AND b = ?2"),
            2
        );
        assert_eq!(count_parameters("INSERT INTO t VALUES (?1, ?2, ?3)"), 3);
    }

    #[test]
    fn test_count_parameters_in_string() {
        // ? inside SQL string literals should NOT be counted
        assert_eq!(count_parameters("SELECT * FROM t WHERE name = 'what?'"), 0);
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(
            normalize_whitespace("  SELECT  *\n  FROM  table  "),
            "SELECT * FROM table"
        );
    }

    #[test]
    fn test_is_dml_query() {
        assert!(is_dml_query("select * from users"));
        assert!(is_dml_query("insert into users values (1)"));
        assert!(is_dml_query("update users set name = 'x'"));
        assert!(is_dml_query("delete from users"));
        assert!(!is_dml_query("create table users (id int)"));
        assert!(!is_dml_query("this is just text"));
    }

    #[test]
    fn test_extract_from_file() {
        let source = r#"
            fn do_query(conn: &Connection) {
                conn.execute("INSERT INTO users (name, email) VALUES (?1, ?2)", params![name, email]);
                let rows = conn.prepare("SELECT id, name FROM users WHERE active = 1");
            }
        "#;

        let queries = extract_queries_from_file("test.rs", source);
        assert_eq!(queries.len(), 2);
        assert!(queries[0].sql.contains("INSERT INTO users"));
        assert_eq!(queries[0].param_count, 2);
        assert!(queries[1].sql.contains("SELECT id, name FROM users"));
    }

    #[test]
    fn test_skip_ddl() {
        let source = r#"
            fn migrate(conn: &Connection) {
                conn.execute_batch("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY)");
                conn.execute("ALTER TABLE users ADD COLUMN age INTEGER");
                conn.execute("SELECT * FROM users WHERE id = ?1", params![1]);
            }
        "#;

        let queries = extract_queries_from_file("test.rs", source);
        assert_eq!(queries.len(), 1);
        assert!(queries[0].sql.contains("SELECT"));
    }
}
