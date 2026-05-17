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
//! - **Title heuristic** (0.50): general item whose title mentions a dep.

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
/// Runs after each fetch cycle. Processes recent items even when they already
/// have dependency rows, because the upsert path can upgrade weak matches and
/// repair missing evidence/source URLs. Limited to the last 7 days and at most
/// 500 items per invocation to keep each pass cheap.
///
/// Returns the number of links created or materially upgraded.
pub fn link_recent_items(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    let dep_names = load_dependency_names(&conn)?;
    if dep_names.is_empty() {
        return Ok(0);
    }

    let recent = load_recent_items(&conn, 500)?;

    if recent.is_empty() {
        return Ok(0);
    }

    let total = link_items_to_deps(&conn, &recent, &dep_names)?;
    if total > 0 {
        info!(
            target: "4da::dep_linker",
            linked = total,
            items = recent.len(),
            deps = dep_names.len(),
            "Linked source items to dependencies"
        );
    }
    Ok(total)
}

/// Backfill: link source items to known dependencies.
///
/// Processes source items in batches of 500 (no date filter). The insert path
/// is an idempotent upsert, so this safely revisits partially-linked rows and
/// can upgrade weak/incomplete links when stronger evidence appears.
///
/// Safe to call on every startup; no-op conflicts are filtered by the upsert.
///
/// Returns the total number of rows inserted or materially upgraded.
pub fn backfill_if_empty(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    let dep_names = load_dependency_names(&conn)?;
    if dep_names.is_empty() {
        return Ok(0);
    }

    let mut total_linked = 0usize;
    let mut last_id = 0i64;
    let batch_size = 500;

    loop {
        let batch = load_items_after(&conn, batch_size, last_id)?;
        if batch.is_empty() {
            break;
        }
        if let Some(last) = batch.last() {
            last_id = last.id;
        }
        total_linked += link_items_to_deps(&conn, &batch, &dep_names)?;
    }

    if total_linked > 0 {
        info!(
            target: "4da::dep_linker",
            total_linked,
            deps = dep_names.len(),
            "Backfill linked new items to dependencies"
        );
    }
    Ok(total_linked)
}

/// Reconcile dependency links against the current classifier.
///
/// - Deletes rows where the classifier no longer produces any match.
/// - Updates rows where the stored match_type/confidence differs from
///   what the current classifier would produce (e.g. an old "advisory"
///   that now classifies as "title_heuristic" gets corrected).
///
/// This ensures stale links created by earlier, noisier matching logic
/// are either corrected or removed on startup.
pub fn prune_invalid_links(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    let mut stmt = conn.prepare(
        "SELECT sid.id, sid.package_name, sid.match_type, sid.confidence,
                si.id, si.title, si.content, si.source_type, si.content_type, si.source_id, si.url
         FROM source_item_dependencies sid
         JOIN source_items si ON si.id = sid.source_item_id",
    )?;

    struct LinkRow {
        id: i64,
        package_name: String,
        match_type: String,
        confidence: f64,
        item: UnlinkedItem,
    }

    let rows: Vec<LinkRow> = stmt
        .query_map([], |row| {
            Ok(LinkRow {
                id: row.get(0)?,
                package_name: row.get(1)?,
                match_type: row.get(2)?,
                confidence: row.get(3)?,
                item: UnlinkedItem {
                    id: row.get(4)?,
                    title: row.get(5)?,
                    content: row.get(6)?,
                    source_type: row.get(7)?,
                    content_type: row.get(8)?,
                    source_id: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    url: row.get(10)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut delete_ids: Vec<i64> = Vec::new();
    let mut updates: Vec<(i64, &'static str, f64, String)> = Vec::new();

    for row in &rows {
        match classify_item_dep_match(&row.item, &row.package_name) {
            None => delete_ids.push(row.id),
            Some((new_type, new_conf)) => {
                if new_type != row.match_type || (new_conf - row.confidence).abs() > 0.001 {
                    let evidence = build_evidence_text(new_type, &row.item, &row.package_name);
                    updates.push((row.id, new_type, new_conf, evidence));
                }
            }
        }
    }

    let mut deleted = 0usize;
    if !delete_ids.is_empty() {
        let mut del = conn.prepare("DELETE FROM source_item_dependencies WHERE id = ?1")?;
        for id in &delete_ids {
            deleted += del.execute(params![id])?;
        }
    }

    let mut updated = 0usize;
    if !updates.is_empty() {
        let mut upd = conn.prepare(
            "UPDATE source_item_dependencies
             SET match_type = ?2, confidence = ?3, evidence_text = ?4
             WHERE id = ?1",
        )?;
        for (id, mt, conf, evidence) in &updates {
            updated += upd.execute(params![id, mt, conf, evidence])?;
        }
    }

    if deleted > 0 || updated > 0 {
        info!(
            target: "4da::dep_linker",
            deleted,
            updated,
            "Reconciled dependency links against current classifier"
        );
    }
    Ok(deleted + updated)
}

/// Repair existing `source_item_dependencies` rows that have NULL or empty
/// `evidence_text` / `source_url`. Joins back to `source_items` to regenerate
/// the evidence using the same `build_evidence_text()` logic used during linking.
///
/// Returns the number of rows repaired.
pub fn repair_evidence(db: &Database) -> Result<usize> {
    let conn = db.conn.lock();

    let mut select_stmt = conn.prepare(
        "SELECT sid.id, sid.source_item_id, sid.package_name, sid.ecosystem, sid.match_type,
                si.title, si.content, si.source_type, si.source_id, si.url
         FROM source_item_dependencies sid
         JOIN source_items si ON si.id = sid.source_item_id
         WHERE sid.evidence_text IS NULL OR sid.evidence_text = ''
            OR sid.source_url IS NULL OR sid.source_url = ''",
    )?;

    struct RepairRow {
        id: i64,
        source_item_id: i64,
        package_name: String,
        match_type: String,
        title: String,
        content: String,
        source_type: String,
        source_id: String,
        url: Option<String>,
    }

    let rows: Vec<RepairRow> = select_stmt
        .query_map([], |row| {
            Ok(RepairRow {
                id: row.get(0)?,
                source_item_id: row.get(1)?,
                package_name: row.get(2)?,
                match_type: row.get(4)?,
                title: row.get(5)?,
                content: row.get(6)?,
                source_type: row.get(7)?,
                source_id: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                url: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut update_stmt = conn.prepare(
        "UPDATE source_item_dependencies SET evidence_text = ?1, source_url = ?2
         WHERE id = ?3",
    )?;

    let mut repaired = 0usize;
    for row in &rows {
        // Reconstruct a minimal UnlinkedItem for build_evidence_text
        let item = UnlinkedItem {
            id: row.source_item_id,
            title: row.title.clone(),
            content: row.content.clone(),
            source_type: row.source_type.clone(),
            content_type: None,
            source_id: row.source_id.clone(),
            url: row.url.clone(),
        };
        let evidence = build_evidence_text(&row.match_type, &item, &row.package_name);
        let source_url = row.url.as_deref();

        match update_stmt.execute(params![evidence, source_url, row.id,]) {
            Ok(changed) => repaired += changed,
            Err(e) => {
                debug!(
                    target: "4da::dep_linker",
                    item_id = row.source_item_id,
                    dep = row.package_name,
                    error = %e,
                    "Failed to repair evidence for dep link"
                );
            }
        }
    }

    if repaired > 0 {
        info!(
            target: "4da::dep_linker",
            repaired,
            "Repaired NULL evidence_text/source_url in source_item_dependencies"
        );
    }
    Ok(repaired)
}

// ============================================================================
// Internal types
// ============================================================================

/// Lightweight projection of a source item — just what we need for matching.
struct UnlinkedItem {
    id: i64,
    title: String,
    content: String,
    source_type: String,
    content_type: Option<String>,
    source_id: String,
    url: Option<String>,
}

// ============================================================================
// Database helpers
// ============================================================================

/// Load all distinct dependency package names (lowercased, direct, non-dev)
/// from both the ACE-populated project_dependencies table and the
/// dependency_snapshots table (if populated).
fn load_dependency_names(conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT LOWER(package_name) FROM (
             SELECT package_name FROM project_dependencies
             WHERE is_dev = 0 AND is_direct = 1 AND project_relevance >= 0.15
             UNION
             SELECT package_name FROM dependency_snapshots
             WHERE is_dev = 0 AND is_direct = 1
         )",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut names = Vec::new();
    for name in rows {
        names.push(name?);
    }
    Ok(names)
}

/// Load recent source items for the post-fetch reconciliation path.
fn load_recent_items(conn: &rusqlite::Connection, limit: i64) -> Result<Vec<UnlinkedItem>> {
    let sql =
        "SELECT si.id, si.title, si.content, si.source_type, si.content_type, si.source_id, si.url
         FROM source_items si
         WHERE si.created_at >= datetime('now', '-7 days')
         ORDER BY si.created_at DESC
         LIMIT ?1";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(UnlinkedItem {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            source_type: row.get(3)?,
            content_type: row.get(4)?,
            source_id: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            url: row.get(6)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

/// Keyset-paginated loader for the backfill path (no date filter).
/// Uses WHERE si.id > last_id instead of OFFSET to avoid phantom reads.
fn load_items_after(
    conn: &rusqlite::Connection,
    limit: i64,
    after_id: i64,
) -> Result<Vec<UnlinkedItem>> {
    let sql =
        "SELECT si.id, si.title, si.content, si.source_type, si.content_type, si.source_id, si.url
               FROM source_items si
               WHERE si.id > ?2
               ORDER BY si.id ASC
               LIMIT ?1";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![limit, after_id], |row| {
        Ok(UnlinkedItem {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            source_type: row.get(3)?,
            content_type: row.get(4)?,
            source_id: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            url: row.get(6)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

/// Core linking logic: for each item, check every dep for a match and
/// upsert a `source_item_dependencies` row for each hit.
///
/// Returns the number of link rows inserted or materially upgraded.
fn link_items_to_deps(
    conn: &rusqlite::Connection,
    items: &[UnlinkedItem],
    dep_names: &[String],
) -> Result<usize> {
    let insert_sql = "INSERT INTO source_item_dependencies
                      (source_item_id, package_name, ecosystem, match_type, confidence, evidence_text, source_url)
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                      ON CONFLICT(source_item_id, package_name)
                      DO UPDATE SET
                         ecosystem = COALESCE(NULLIF(excluded.ecosystem, ''), source_item_dependencies.ecosystem),
                         match_type = CASE
                             WHEN excluded.confidence > source_item_dependencies.confidence
                             THEN excluded.match_type
                             ELSE source_item_dependencies.match_type
                         END,
                         confidence = MAX(source_item_dependencies.confidence, excluded.confidence),
                         evidence_text = CASE
                             WHEN excluded.confidence > source_item_dependencies.confidence
                             THEN COALESCE(NULLIF(excluded.evidence_text, ''), source_item_dependencies.evidence_text)
                             ELSE COALESCE(NULLIF(source_item_dependencies.evidence_text, ''), NULLIF(excluded.evidence_text, ''))
                         END,
                         source_url = COALESCE(NULLIF(source_item_dependencies.source_url, ''), NULLIF(excluded.source_url, ''))
                      WHERE excluded.confidence > source_item_dependencies.confidence
                         OR ((source_item_dependencies.evidence_text IS NULL OR source_item_dependencies.evidence_text = '')
                             AND excluded.evidence_text IS NOT NULL AND excluded.evidence_text <> '')
                         OR ((source_item_dependencies.source_url IS NULL OR source_item_dependencies.source_url = '')
                             AND excluded.source_url IS NOT NULL AND excluded.source_url <> '')";
    let mut stmt = conn.prepare(insert_sql)?;
    let mut count = 0usize;

    for item in items {
        for dep_name in dep_names {
            if let Some((match_type, confidence)) = classify_item_dep_match(item, dep_name) {
                let ecosystem = infer_ecosystem(item, dep_name);
                let evidence = build_evidence_text(match_type, item, dep_name);
                let source_url = item.url.as_deref();
                match stmt.execute(params![
                    item.id, dep_name, ecosystem, match_type, confidence, evidence, source_url
                ]) {
                    Ok(changed) => count += changed,
                    Err(e) => {
                        debug!(
                            target: "4da::dep_linker",
                            item_id = item.id,
                            dep = dep_name,
                            error = %e,
                            "Failed to upsert dep link"
                        );
                    }
                }
            }
        }
    }
    Ok(count)
}

fn build_evidence_text(match_type: &str, item: &UnlinkedItem, dep_name: &str) -> String {
    match match_type {
        "exact_registry" => format!(
            "Registry source '{}' published item with source_id matching '{}'",
            item.source_type, dep_name
        ),
        "advisory" => {
            if advisory_affected_package_match(&item.content, dep_name) {
                format!(
                    "Security advisory from '{}' lists '{}' in Affected packages",
                    item.source_type, dep_name
                )
            } else {
                format!(
                    "Security advisory from '{}' references '{}' in title: \"{}\"",
                    item.source_type,
                    dep_name,
                    truncate_title(&item.title, 80)
                )
            }
        }
        _ => format!(
            "Title heuristic: '{}' found in \"{}\" (source: {})",
            dep_name,
            truncate_title(&item.title, 80),
            item.source_type
        ),
    }
}

fn truncate_title(title: &str, max_len: usize) -> &str {
    if title.len() <= max_len {
        title
    } else {
        &title[..title.floor_char_boundary(max_len)]
    }
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
        match advisory_affected_status(&item.content, dep_name) {
            AffectedStatus::Matched => return Some(("advisory", 0.90)),
            // Structured metadata exists but names a DIFFERENT package —
            // title fallback would produce a false positive.
            AffectedStatus::MetadataExistsNoMatch => return None,
            // No structured metadata at all — allow title fallback for
            // RSS/security posts that lack affected-package fields.
            AffectedStatus::NoMetadata => {
                if is_specific_title_match_candidate(dep_name)
                    && matches_dep_in_title(&item.title, dep_name) == Some(0.50)
                {
                    return Some(("advisory", 0.75));
                }
            }
        }
    }

    // ---- Tier 3: title heuristic (0.30–0.50) ----
    // Skip very short/common/ambiguous names and require a whole-token match.
    if !is_specific_title_match_candidate(dep_name) {
        return None;
    }
    if let Some(confidence @ 0.50) = matches_dep_in_title(&item.title, dep_name) {
        return Some(("title_heuristic", confidence));
    }

    None
}

/// Is this source_type a package registry?
fn is_registry_source(source_type: &str) -> bool {
    matches!(
        source_type,
        "npm_registry"
            | "npm"
            | "crates_io"
            | "crates"
            | "pypi"
            | "go_modules"
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

/// Extract the bare package name from a registry source's source_id.
///
/// Handles adapter-specific formats:
/// - npm_registry: `react@19.2.5` → `react`, `@tanstack/react-query@5.0.0` → `@tanstack/react-query`
/// - crates_io: `crate-serde` → `serde`
/// - Others: source_id is the bare package name.
fn extract_registry_package(source_type: &str, source_id: &str) -> Option<String> {
    match source_type {
        "npm_registry" | "npm" => {
            // npm source_id format: `name@version` or `@scope/name@version`
            // For scoped packages, the first `@` is the scope prefix.
            let name = if source_id.starts_with('@') {
                // Scoped: `@scope/name@version` — find the LAST `@`
                source_id.rfind('@').map(|pos| {
                    if pos == 0 {
                        source_id
                    } else {
                        &source_id[..pos]
                    }
                })
            } else {
                // Unscoped: `name@version` — split at first `@`
                Some(source_id.split_once('@').map_or(source_id, |(n, _)| n))
            };
            name.map(|n| n.to_string())
        }
        "crates_io" | "crates" => {
            // crates_io source_id format: `crate-{name}`
            Some(
                source_id
                    .strip_prefix("crate-")
                    .unwrap_or(source_id)
                    .to_string(),
            )
        }
        "pypi" | "go_modules" | "go" | "maven" | "nuget" | "packagist" | "rubygems"
        | "cocoapods" => Some(source_id.to_string()),
        _ => None,
    }
}

/// Check if a dependency name appears in a title string.
///
/// Returns `Some(confidence)` where:
/// - 0.50 for a whole-token match (preceded/followed by a package-name boundary)
///
/// Normalizes both hyphens and underscores so "async-trait" and
/// "async_trait" compare equal.
///
/// A "package-name boundary" is a character that is NOT alphanumeric, hyphen,
/// underscore, dot, or `@` — those characters are package-name-internal and
/// must NOT be treated as word separators. This prevents "react" from matching
/// inside "react-native" or "react_query".
fn matches_dep_in_title(title: &str, dep_name: &str) -> Option<f64> {
    let title_lower = title.to_lowercase();
    let dep_lower = dep_name.to_lowercase();
    let dep_normalized = dep_lower.replace('-', "_");
    let dep_hyphen = dep_lower.replace('_', "-");

    for variant in [&dep_lower, &dep_normalized, &dep_hyphen] {
        if variant.is_empty() {
            continue;
        }
        let hay = title_lower.as_bytes();
        let needle = variant.as_bytes();
        let mut start = 0;
        while let Some(rel) = title_lower[start..].find(variant.as_str()) {
            let pos = start + rel;
            let before_ok = pos == 0 || is_package_boundary(hay[pos - 1]);
            let after_pos = pos + needle.len();
            let after_ok = after_pos >= hay.len() || is_package_boundary(hay[after_pos]);
            if before_ok && after_ok {
                return Some(0.50);
            }
            start = pos + 1;
        }
    }

    None
}

fn is_package_boundary(b: u8) -> bool {
    !b.is_ascii_alphanumeric() && b != b'-' && b != b'_' && b != b'.' && b != b'@'
}

/// Generic/common dependency names are too noisy for title-only matching.
///
/// Registry and structured advisory matches can still use these names because
/// those paths have ecosystem/package proof. Title-only paths cannot.
fn is_specific_title_match_candidate(dep_name: &str) -> bool {
    let normalized = dep_name
        .trim()
        .trim_start_matches('@')
        .replace(['/', '_'], "-")
        .to_lowercase();

    !is_ambiguous_package_name(&normalized) && !crate::scoring::is_ambiguous_dep_name(&normalized)
}

enum AffectedStatus {
    Matched,
    MetadataExistsNoMatch,
    NoMetadata,
}

/// Check structured advisory content for affected-package evidence.
///
/// Both CVE/OSV adapters include a line like:
/// `Affected: package-name (ecosystem), other-package (ecosystem)`.
///
/// Returns tri-state: the dep IS in the affected list, the affected list
/// exists but names different packages, or no structured metadata at all.
fn advisory_affected_status(content: &str, dep_name: &str) -> AffectedStatus {
    let Some(affected) = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("Affected:").map(str::trim))
    else {
        return AffectedStatus::NoMetadata;
    };

    if affected.eq_ignore_ascii_case("unknown") || affected.is_empty() {
        return AffectedStatus::NoMetadata;
    }

    if affected
        .split(',')
        .map(extract_affected_package_name)
        .any(|pkg| exact_package_name_match(&pkg, dep_name))
    {
        AffectedStatus::Matched
    } else {
        AffectedStatus::MetadataExistsNoMatch
    }
}

/// Bool convenience for evidence text generation.
fn advisory_affected_package_match(content: &str, dep_name: &str) -> bool {
    matches!(
        advisory_affected_status(content, dep_name),
        AffectedStatus::Matched
    )
}

fn extract_affected_package_name(token: &str) -> String {
    token
        .trim()
        .split_once(" (")
        .map_or_else(|| token.trim(), |(name, _)| name.trim())
        .to_string()
}

fn exact_package_name_match(candidate: &str, dep_name: &str) -> bool {
    let candidate = normalize_package_for_exact_match(candidate);
    let dep = normalize_package_for_exact_match(dep_name);
    candidate == dep
}

fn normalize_package_for_exact_match(name: &str) -> String {
    name.trim()
        .trim_start_matches('@')
        .replace('_', "-")
        .to_lowercase()
}

/// Best-effort ecosystem inference from the source type.
fn infer_ecosystem(item: &UnlinkedItem, _dep_name: &str) -> Option<String> {
    match item.source_type.to_lowercase().as_str() {
        "npm_registry" | "npm" => Some("npm".into()),
        "crates_io" | "crates" => Some("crates.io".into()),
        "pypi" => Some("pypi".into()),
        "go_modules" | "go" => Some("go".into()),
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
#[path = "dep_linker_tests.rs"]
mod tests;
