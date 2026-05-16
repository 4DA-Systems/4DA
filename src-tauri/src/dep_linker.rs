// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Post-ingestion dependency linker for 4DA.
//!
//! After source items are ingested and stored, this module scans unlinked items
//! and creates `source_item_dependencies` rows linking them to known user
//! dependencies from `project_dependencies`. Three match tiers:
//!
//! - **Exact registry** (0.95): registry source whose source_id IS the package.
//! - **Advisory** (0.90): security advisory mentioning a known dep.
//! - **Title heuristic** (0.30–0.50): general item whose title mentions a dep.

use rusqlite::params;
use tracing::{debug, info};

use crate::blind_spots::is_ambiguous_package_name;
use crate::db::Database;
use crate::error::Result;

// ============================================================================
// Public API
// ============================================================================

/// Link recently ingested source items to known dependencies.
///
/// Runs after each fetch cycle. Only processes items that have no rows yet
/// in `source_item_dependencies`, limited to the last 7 days and at most 500
/// items per invocation to keep each pass cheap.
///
/// Returns the number of links created.
pub fn link_recent_items(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    let dep_names = load_dependency_names(&conn)?;
    if dep_names.is_empty() {
        return Ok(0);
    }

    let unlinked = load_unlinked_items(
        &conn,
        "AND si.created_at >= datetime('now', '-7 days')",
        500,
    )?;

    if unlinked.is_empty() {
        return Ok(0);
    }

    let total = link_items_to_deps(&conn, &unlinked, &dep_names)?;
    if total > 0 {
        info!(
            target: "4da::dep_linker",
            linked = total,
            items = unlinked.len(),
            deps = dep_names.len(),
            "Linked source items to dependencies"
        );
    }
    Ok(total)
}

/// One-time backfill: link ALL existing source items to dependencies.
///
/// Checks whether `source_item_dependencies` is empty. If so, processes
/// every source item in batches of 500 (no date filter). Safe to call on
/// every startup — it short-circuits immediately when the table already
/// has rows.
///
/// Returns the total number of links created.
pub fn backfill_if_empty(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    // Short-circuit: if the table already has rows, nothing to do.
    let has_rows: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM source_item_dependencies LIMIT 1)",
            [],
            |row| row.get(0),
        )
        .unwrap_or(true); // default to "has rows" on error so we don't blow up

    if has_rows {
        return Ok(0);
    }

    let dep_names = load_dependency_names(&conn)?;
    if dep_names.is_empty() {
        return Ok(0);
    }

    info!(
        target: "4da::dep_linker",
        deps = dep_names.len(),
        "Backfilling source_item_dependencies (table empty)"
    );

    let mut total_linked = 0usize;
    let mut offset = 0i64;
    let batch_size = 500;

    loop {
        let batch = load_unlinked_items_paged(&conn, batch_size, offset)?;
        if batch.is_empty() {
            break;
        }
        let batch_len = batch.len();
        total_linked += link_items_to_deps(&conn, &batch, &dep_names)?;
        offset += batch_len as i64;
    }

    info!(
        target: "4da::dep_linker",
        total_linked,
        "Backfill complete"
    );
    Ok(total_linked)
}

// ============================================================================
// Internal types
// ============================================================================

/// Lightweight projection of a source item — just what we need for matching.
struct UnlinkedItem {
    id: i64,
    title: String,
    source_type: String,
    content_type: Option<String>,
    source_id: String,
}

// ============================================================================
// Database helpers
// ============================================================================

/// Load all distinct dependency package names (lowercased, direct, non-dev).
fn load_dependency_names(conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT LOWER(package_name) FROM project_dependencies
         WHERE is_dev = 0 AND is_direct = 1 AND project_relevance >= 0.15",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut names = Vec::new();
    for name in rows {
        if let Ok(n) = name {
            names.push(n);
        }
    }
    Ok(names)
}

/// Load source items that have no link rows yet, applying an optional
/// SQL fragment for date filtering and a hard row limit.
fn load_unlinked_items(
    conn: &rusqlite::Connection,
    date_filter: &str,
    limit: i64,
) -> Result<Vec<UnlinkedItem>> {
    let sql = format!(
        "SELECT si.id, si.title, si.source_type, si.content_type, si.source_id
         FROM source_items si
         LEFT JOIN source_item_dependencies sid ON sid.source_item_id = si.id
         WHERE sid.id IS NULL
           {date_filter}
         ORDER BY si.created_at DESC
         LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(UnlinkedItem {
            id: row.get(0)?,
            title: row.get(1)?,
            source_type: row.get(2)?,
            content_type: row.get(3)?,
            source_id: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        if let Ok(item) = row {
            items.push(item);
        }
    }
    Ok(items)
}

/// Paginated loader for the backfill path (no date filter).
fn load_unlinked_items_paged(
    conn: &rusqlite::Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<UnlinkedItem>> {
    let sql = "SELECT si.id, si.title, si.source_type, si.content_type, si.source_id
               FROM source_items si
               LEFT JOIN source_item_dependencies sid ON sid.source_item_id = si.id
               WHERE sid.id IS NULL
               ORDER BY si.id ASC
               LIMIT ?1 OFFSET ?2";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(UnlinkedItem {
            id: row.get(0)?,
            title: row.get(1)?,
            source_type: row.get(2)?,
            content_type: row.get(3)?,
            source_id: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        if let Ok(item) = row {
            items.push(item);
        }
    }
    Ok(items)
}

/// Core linking logic: for each item, check every dep for a match and
/// insert a `source_item_dependencies` row for each hit.
///
/// Returns the number of link rows created.
fn link_items_to_deps(
    conn: &rusqlite::Connection,
    items: &[UnlinkedItem],
    dep_names: &[String],
) -> Result<usize> {
    let insert_sql = "INSERT OR IGNORE INTO source_item_dependencies
                      (source_item_id, package_name, ecosystem, match_type, confidence)
                      VALUES (?1, ?2, ?3, ?4, ?5)";
    let mut stmt = conn.prepare(insert_sql)?;
    let mut count = 0usize;

    for item in items {
        for dep_name in dep_names {
            if let Some((match_type, confidence)) = classify_item_dep_match(item, dep_name) {
                let ecosystem = infer_ecosystem(item, dep_name);
                match stmt.execute(params![
                    item.id, dep_name, ecosystem, match_type, confidence
                ]) {
                    Ok(changed) => count += changed,
                    Err(e) => {
                        debug!(
                            target: "4da::dep_linker",
                            item_id = item.id,
                            dep = dep_name,
                            error = %e,
                            "Failed to insert dep link"
                        );
                    }
                }
            }
        }
    }
    Ok(count)
}

// ============================================================================
// Matching logic
// ============================================================================

/// Determine whether an item matches a dependency name and, if so, return
/// the match type and confidence.
fn classify_item_dep_match(item: &UnlinkedItem, dep_name: &str) -> Option<(&'static str, f64)> {
    let st = item.source_type.to_lowercase();

    // ---- Tier 1: exact registry match (0.95) ----
    if is_registry_source(&st) {
        if let Some(pkg) = extract_registry_package(&st, &item.source_id) {
            let pkg_lower = pkg.to_lowercase();
            let dep_lower = dep_name.to_lowercase();
            let dep_normalized = dep_lower.replace('-', "_");
            let dep_hyphen = dep_lower.replace('_', "-");
            if pkg_lower == dep_lower || pkg_lower == dep_normalized || pkg_lower == dep_hyphen {
                return Some(("exact_registry", 0.95));
            }
        }
    }

    // ---- Tier 2: advisory match (0.90) ----
    if is_advisory_source(&st, item.content_type.as_deref()) {
        // Advisory titles almost always contain the affected package name.
        if matches_dep_in_title(&item.title, dep_name).is_some() {
            return Some(("advisory", 0.90));
        }
    }

    // ---- Tier 3: title heuristic (0.30–0.50) ----
    // Skip very short names (too generic) and known-ambiguous names.
    if dep_name.len() < 4 {
        return None;
    }
    if is_ambiguous_package_name(dep_name) {
        return None;
    }
    if let Some(confidence) = matches_dep_in_title(&item.title, dep_name) {
        return Some(("title_heuristic", confidence));
    }

    None
}

/// Is this source_type a package registry?
fn is_registry_source(source_type: &str) -> bool {
    matches!(
        source_type,
        "npm"
            | "crates_io"
            | "crates"
            | "pypi"
            | "go"
            | "maven"
            | "nuget"
            | "packagist"
            | "rubygems"
            | "cocoapods"
    )
}

/// Is this source an advisory / security report?
fn is_advisory_source(source_type: &str, content_type: Option<&str>) -> bool {
    if matches!(source_type, "osv" | "cve") {
        return true;
    }
    matches!(
        content_type,
        Some("security_advisory") | Some("vulnerability_report") | Some("cve")
    )
}

/// Extract the package name from a registry source's source_id.
///
/// For most registries the source_id IS the package name (or `@scope/name`
/// for npm). Returns `None` for non-registry sources.
fn extract_registry_package(source_type: &str, source_id: &str) -> Option<String> {
    match source_type {
        "npm" | "crates_io" | "crates" | "pypi" | "go" | "maven" | "nuget" | "packagist"
        | "rubygems" | "cocoapods" => Some(source_id.to_string()),
        _ => None,
    }
}

/// Check if a dependency name appears in a title string.
///
/// Returns `Some(confidence)` where:
/// - 0.50 for a word-boundary match (preceded/followed by non-alphanumeric)
/// - 0.30 for a plain substring match
///
/// Normalizes both hyphens and underscores so "async-trait" and
/// "async_trait" compare equal.
fn matches_dep_in_title(title: &str, dep_name: &str) -> Option<f64> {
    let title_lower = title.to_lowercase();
    let dep_lower = dep_name.to_lowercase();
    let dep_normalized = dep_lower.replace('-', "_");
    let dep_hyphen = dep_lower.replace('_', "-");

    // Word-boundary match (higher confidence)
    for variant in [&dep_lower, &dep_normalized, &dep_hyphen] {
        if variant.is_empty() {
            continue;
        }
        if let Some(pos) = title_lower.find(variant.as_str()) {
            let before_ok = pos == 0 || !title_lower.as_bytes()[pos - 1].is_ascii_alphanumeric();
            let after_pos = pos + variant.len();
            let after_ok = after_pos >= title_lower.len()
                || !title_lower.as_bytes()[after_pos].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return Some(0.50);
            }
        }
    }

    // Substring match (lower confidence)
    for variant in [&dep_lower, &dep_normalized, &dep_hyphen] {
        if variant.is_empty() {
            continue;
        }
        if title_lower.contains(variant.as_str()) {
            return Some(0.30);
        }
    }

    None
}

/// Best-effort ecosystem inference from the source type.
fn infer_ecosystem(item: &UnlinkedItem, _dep_name: &str) -> Option<String> {
    match item.source_type.to_lowercase().as_str() {
        "npm" => Some("npm".into()),
        "crates_io" | "crates" => Some("crates.io".into()),
        "pypi" => Some("pypi".into()),
        "go" => Some("go".into()),
        "maven" => Some("maven".into()),
        "nuget" => Some("nuget".into()),
        "packagist" => Some("packagist".into()),
        "rubygems" => Some("rubygems".into()),
        "cocoapods" => Some("cocoapods".into()),
        "osv" | "cve" => Some("advisory".into()),
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_dep_in_title_word_boundary() {
        // "react" appears as a whole word — should return 0.50
        let result = matches_dep_in_title("react 19 released", "react");
        assert_eq!(result, Some(0.50));

        // At end of string
        let result = matches_dep_in_title("major update for react", "react");
        assert_eq!(result, Some(0.50));

        // Surrounded by punctuation (still word-boundary)
        let result = matches_dep_in_title("[react] v19 is out", "react");
        assert_eq!(result, Some(0.50));

        // Hyphen/underscore normalization
        let result = matches_dep_in_title("async-trait 0.2 released", "async_trait");
        assert_eq!(result, Some(0.50));
    }

    #[test]
    fn test_matches_dep_in_title_substring() {
        // "react" is embedded inside "react-query" — not a word boundary for "react"
        // but "react-query" as a whole IS a word boundary match.
        // For dep "react", "react-query" has 'react' followed by '-' which IS non-alphanumeric.
        // So "react" in "react-query" is actually a word-boundary match at 0.50.
        // Let's use a genuine substring case instead.
        let result = matches_dep_in_title("reactivity patterns in Vue", "react");
        assert_eq!(result, Some(0.30));
    }

    #[test]
    fn test_ambiguous_names_skipped() {
        // "image" is in the ambiguous list — classify_item_dep_match should return None
        // for title-heuristic tier (tier 3).
        let item = UnlinkedItem {
            id: 1,
            title: "image processing article".to_string(),
            source_type: "hn".to_string(),
            content_type: None,
            source_id: "12345".to_string(),
        };
        let result = classify_item_dep_match(&item, "image");
        assert!(result.is_none(), "Ambiguous dep 'image' should be skipped");
    }

    #[test]
    fn test_short_names_skipped() {
        // Dep names < 4 chars are too generic for title heuristic.
        let item = UnlinkedItem {
            id: 2,
            title: "all about the arc reactor".to_string(),
            source_type: "hn".to_string(),
            content_type: None,
            source_id: "99".to_string(),
        };
        let result = classify_item_dep_match(&item, "arc");
        assert!(result.is_none(), "3-char dep 'arc' should be skipped");
    }

    #[test]
    fn test_exact_registry_confidence() {
        // npm source_id matches dep name exactly — 0.95
        let item = UnlinkedItem {
            id: 3,
            title: "axios 1.7.0".to_string(),
            source_type: "npm".to_string(),
            content_type: None,
            source_id: "axios".to_string(),
        };
        let result = classify_item_dep_match(&item, "axios");
        assert_eq!(result, Some(("exact_registry", 0.95)));
    }

    #[test]
    fn test_exact_registry_hyphen_normalization() {
        // crates_io source with underscore vs hyphen in dep name
        let item = UnlinkedItem {
            id: 4,
            title: "async-trait update".to_string(),
            source_type: "crates_io".to_string(),
            content_type: None,
            source_id: "async-trait".to_string(),
        };
        let result = classify_item_dep_match(&item, "async_trait");
        assert_eq!(result, Some(("exact_registry", 0.95)));
    }

    #[test]
    fn test_advisory_match() {
        // OSV advisory mentioning tokio
        let item = UnlinkedItem {
            id: 5,
            title: "RUSTSEC-2023-0001: tokio race condition".to_string(),
            source_type: "osv".to_string(),
            content_type: Some("security_advisory".to_string()),
            source_id: "RUSTSEC-2023-0001".to_string(),
        };
        let result = classify_item_dep_match(&item, "tokio");
        assert_eq!(result, Some(("advisory", 0.90)));
    }

    #[test]
    fn test_advisory_via_content_type() {
        // Generic source but content_type marks it as advisory
        let item = UnlinkedItem {
            id: 6,
            title: "Critical vulnerability in serde_json".to_string(),
            source_type: "hn".to_string(),
            content_type: Some("security_advisory".to_string()),
            source_id: "40001".to_string(),
        };
        let result = classify_item_dep_match(&item, "serde_json");
        assert_eq!(result, Some(("advisory", 0.90)));
    }

    #[test]
    fn test_title_heuristic_general_source() {
        // HN article mentioning a dep by name (word boundary)
        let item = UnlinkedItem {
            id: 7,
            title: "Why we migrated from axios to fetch".to_string(),
            source_type: "hn".to_string(),
            content_type: None,
            source_id: "40002".to_string(),
        };
        let result = classify_item_dep_match(&item, "axios");
        assert_eq!(result, Some(("title_heuristic", 0.50)));
    }

    #[test]
    fn test_no_match_returns_none() {
        let item = UnlinkedItem {
            id: 8,
            title: "Introduction to quantum computing".to_string(),
            source_type: "hn".to_string(),
            content_type: None,
            source_id: "40003".to_string(),
        };
        let result = classify_item_dep_match(&item, "tokio");
        assert!(result.is_none());
    }

    #[test]
    fn test_registry_non_match() {
        // npm source but source_id doesn't match the dep we're checking
        let item = UnlinkedItem {
            id: 9,
            title: "lodash 5.0 released".to_string(),
            source_type: "npm".to_string(),
            content_type: None,
            source_id: "lodash".to_string(),
        };
        // Title heuristic for "axios" won't match, and registry source_id is "lodash"
        let result = classify_item_dep_match(&item, "axios");
        assert!(result.is_none());
    }

    /// Regression test: proves the INSERT into source_item_dependencies works
    /// against the real migrated schema. A previous bug used `dependency_name`
    /// instead of `package_name`, causing a column mismatch at runtime.
    #[test]
    fn test_link_items_inserts_into_real_schema() {
        use crate::test_utils::test_db;

        let db = test_db();
        let conn = db.conn.lock();

        // Insert a minimal source_items row
        conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, title, content, content_hash, embedding)
             VALUES (1, 'crates_io', 'serde', 'serde 1.0.200 released', '', 'hash1', zeroblob(1536))",
            [],
        )
        .expect("insert source_items");

        // Insert a minimal project_dependencies row so load_dependency_names can find it
        conn.execute(
            "INSERT INTO project_dependencies (package_name, project_path, manifest_type, language, is_dev, is_direct, project_relevance)
             VALUES ('serde', '/home/user/project', 'Cargo.toml', 'rust', 0, 1, 1.0)",
            [],
        )
        .expect("insert project_dependencies");

        // Build an UnlinkedItem that will match via exact_registry tier
        let items = vec![UnlinkedItem {
            id: 1,
            title: "serde 1.0.200 released".into(),
            source_type: "crates_io".into(),
            content_type: None,
            source_id: "serde".into(),
        }];
        let dep_names = vec!["serde".to_string()];

        // This is the call that would fail with "table source_item_dependencies
        // has no column named dependency_name" if the INSERT used the wrong column.
        let linked = link_items_to_deps(&conn, &items, &dep_names)
            .expect("link_items_to_deps should succeed against real schema");
        assert_eq!(linked, 1, "Expected exactly 1 link row");

        // Verify the row exists with the correct columns
        let (pkg, eco, mt, conf): (String, Option<String>, String, f64) = conn
            .query_row(
                "SELECT package_name, ecosystem, match_type, confidence
                 FROM source_item_dependencies
                 WHERE source_item_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("should find the inserted row");

        assert_eq!(pkg, "serde");
        assert_eq!(eco.as_deref(), Some("crates.io"));
        assert_eq!(mt, "exact_registry");
        assert!((conf - 0.95).abs() < f64::EPSILON);

        // Verify total row count is exactly 1
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_item_dependencies", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_is_registry_source() {
        assert!(is_registry_source("npm"));
        assert!(is_registry_source("crates_io"));
        assert!(is_registry_source("pypi"));
        assert!(!is_registry_source("hn"));
        assert!(!is_registry_source("reddit"));
    }

    #[test]
    fn test_is_advisory_source() {
        assert!(is_advisory_source("osv", None));
        assert!(is_advisory_source("cve", None));
        assert!(is_advisory_source("hn", Some("security_advisory")));
        assert!(is_advisory_source("reddit", Some("vulnerability_report")));
        assert!(!is_advisory_source("hn", None));
        assert!(!is_advisory_source("hn", Some("discussion")));
    }
}
