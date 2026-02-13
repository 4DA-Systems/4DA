//! vec0 virtual table support for the SQL checker.
//!
//! Handles sqlite-vec virtual table specifics, including pseudo-columns
//! and MATCH syntax validation.

use super::schema_parser::{ColumnInfo, TableSchema};

/// Generate pseudo-schema for vec0 virtual tables.
///
/// vec0 tables in sqlite-vec support these pseudo-columns:
/// - `rowid`: INTEGER primary key
/// - `embedding`: The vector column (BLOB)
/// - `distance`: Result distance from MATCH queries (REAL)
pub fn vec0_pseudo_columns(table_name: &str) -> TableSchema {
    TableSchema {
        name: table_name.to_string(),
        columns: vec![
            ColumnInfo {
                name: "rowid".to_string(),
                col_type: "INTEGER".to_string(),
                nullable: false,
                has_default: false,
            },
            ColumnInfo {
                name: "embedding".to_string(),
                col_type: "BLOB".to_string(),
                nullable: false,
                has_default: false,
            },
            ColumnInfo {
                name: "distance".to_string(),
                col_type: "REAL".to_string(),
                nullable: true,
                has_default: false,
            },
        ],
        is_virtual: true,
        virtual_module: Some("vec0".to_string()),
    }
}

/// Check if a SQL query uses vec0 MATCH syntax
pub fn is_vec0_match_query(sql: &str, table_name: &str) -> bool {
    let sql_lower = sql.to_lowercase();
    sql_lower.contains(table_name) && sql_lower.contains("match")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pseudo_columns() {
        let schema = vec0_pseudo_columns("test_vec");
        assert_eq!(schema.name, "test_vec");
        assert!(schema.is_virtual);
        assert_eq!(schema.virtual_module, Some("vec0".to_string()));
        assert_eq!(schema.columns.len(), 3);
        assert!(schema.columns.iter().any(|c| c.name == "rowid"));
        assert!(schema.columns.iter().any(|c| c.name == "embedding"));
        assert!(schema.columns.iter().any(|c| c.name == "distance"));
    }

    #[test]
    fn test_is_vec0_match_query() {
        assert!(is_vec0_match_query(
            "SELECT rowid, distance FROM source_vec WHERE embedding MATCH ?",
            "source_vec"
        ));
        assert!(!is_vec0_match_query(
            "SELECT * FROM source_items WHERE id = 1",
            "source_vec"
        ));
    }
}
