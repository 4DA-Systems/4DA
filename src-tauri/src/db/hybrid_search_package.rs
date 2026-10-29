// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Package lane for hybrid search.
//!
//! A query such as "sharp vulnerability" names a package the user has installed.
//! Neither retrieval leg treats that name as the subject: the BM25 leg needs every
//! token ("vulnerability" is absent from many advisories) and the vector leg ranks
//! by topic, so two Next.js advisories that merely mention `sharp` outranked
//! sharp's own advisories and .NET advisories filled the tail (audit L8,
//! 2026-10-12).
//!
//! The CVE and OSV adapters write the advisory's affected packages as structured
//! text, `Affected: sharp (npm), other (crates.io)`. This lane reads that line:
//!
//! - a query term that is an installed package name is a **named package**;
//! - an advisory whose `Affected:` list names it is pinned into the results and
//!   marked, so the ranker can place it above advisories that only mention it;
//! - while a named package is present, an advisory whose affected ecosystems are
//!   all ones the user has no dependencies in is marked as foreign.
//!
//! The lane only runs when the query also asks about security (one of
//! [`ADVISORY_INTENT`]): "react server components" names the installed `react`
//! but is a topic query, and pinning React's XSS advisories there would be wrong.
//! A query without both a named package and that intent passes through untouched.

use std::collections::HashSet;

use rusqlite::{params_from_iter, Connection};

use super::exact::exact_title_terms;
use super::{run_leg, Candidate, HybridSearchResult};
use crate::ecosystem::Ecosystem;

#[cfg(test)]
#[path = "hybrid_search_package_tests.rs"]
mod tests;

/// At most this many package advisories are pinned into the results.
pub(crate) const MAX_PACKAGE_PINS: usize = 15;

/// Query words that ask about a package's advisories rather than its topic.
const ADVISORY_INTENT: &[&str] = &[
    "vulnerability",
    "vulnerabilities",
    "vulnerable",
    "vuln",
    "vulns",
    "cve",
    "cves",
    "advisory",
    "advisories",
    "security",
    "exploit",
    "ghsa",
    "rustsec",
    "osv",
];

fn has_advisory_intent(terms: &[String]) -> bool {
    terms.iter().any(|t| ADVISORY_INTENT.contains(&t.as_str()))
}

/// FTS candidates read per named package before the `Affected:` filter.
const PACKAGE_LANE_CANDIDATES: i64 = 200;

/// What the query says about the user's own dependencies.
#[derive(Debug, Default, Clone)]
pub(crate) struct PackageContext {
    /// Query terms that are installed package names, lowercased, in query order.
    pub named: Vec<String>,
    /// Ecosystems the user has at least one dependency in.
    pub installed_ecosystems: Vec<Ecosystem>,
}

impl PackageContext {
    pub(crate) fn is_active(&self) -> bool {
        !self.named.is_empty()
    }
}

/// Lowercase, strip a leading `@` scope marker and fold `_` into `-`, the same
/// comparison the dependency linker uses for advisory package names.
fn normalize_package(name: &str) -> String {
    name.trim()
        .trim_start_matches('@')
        .replace('_', "-")
        .to_lowercase()
}

/// `(package, ecosystem)` pairs from an advisory's `Affected:` line. Empty when the
/// item has no such line or it reads `Unknown`.
pub(crate) fn parse_affected(content: &str) -> Vec<(String, String)> {
    let Some(line) = content
        .lines()
        .find_map(|l| l.trim().strip_prefix("Affected:").map(str::trim))
    else {
        return Vec::new();
    };
    line.split(',')
        .filter_map(|entry| {
            let (name, rest) = entry.trim().split_once('(')?;
            let (eco, _) = rest.split_once(')')?;
            let name = name.trim();
            let eco = eco.trim();
            (!name.is_empty() && !eco.is_empty())
                .then(|| (normalize_package(name), eco.to_lowercase()))
        })
        .collect()
}

/// The first named package the advisory's `Affected:` list contains.
pub(crate) fn affected_named_package(
    affected: &[(String, String)],
    named: &[String],
) -> Option<String> {
    named
        .iter()
        .find(|n| {
            let n = normalize_package(n);
            affected.iter().any(|(pkg, _)| *pkg == n)
        })
        .cloned()
}

/// True when the advisory lists affected packages and none of them is in an
/// ecosystem the user has dependencies in. An ecosystem 4DA does not read
/// (`Ecosystem::parse` returns `None`) can never hold one of the user's deps.
pub(crate) fn is_foreign_ecosystem(affected: &[(String, String)], installed: &[Ecosystem]) -> bool {
    !affected.is_empty()
        && !affected
            .iter()
            .any(|(_, eco)| Ecosystem::parse(eco).is_some_and(|e| installed.contains(&e)))
}

/// Read the named packages and installed ecosystems for `query_text`.
pub(crate) fn package_context(conn: &Connection, query_text: &str) -> PackageContext {
    let mut terms: Vec<String> = Vec::new();
    for term in exact_title_terms(query_text) {
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    if terms.is_empty() || !has_advisory_intent(&terms) {
        return PackageContext::default();
    }
    let installed = installed_names(conn, &terms);
    let named: Vec<String> = terms
        .into_iter()
        .filter(|t| installed.contains(&normalize_package(t)))
        .collect();
    if named.is_empty() {
        return PackageContext::default();
    }
    PackageContext {
        named,
        installed_ecosystems: installed_ecosystems(conn),
    }
}

/// Which of `terms` are installed, read from the lockfile inventory and the
/// manifest inventory. Returned normalized.
fn installed_names(conn: &Connection, terms: &[String]) -> HashSet<String> {
    let normalized: Vec<String> = terms.iter().map(|t| normalize_package(t)).collect();
    let placeholders = vec!["?"; normalized.len()].join(",");
    let mut found = HashSet::new();
    for table in ["dependency_instances", "user_dependencies"] {
        let sql = format!(
            "SELECT DISTINCT lower(replace(ltrim(package_name, '@'), '_', '-')) FROM {table}
             WHERE lower(replace(ltrim(package_name, '@'), '_', '-')) IN ({placeholders})"
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map(params_from_iter(normalized.iter()), |r| {
            r.get::<_, String>(0)
        }) else {
            continue;
        };
        found.extend(rows.flatten());
    }
    found
}

fn installed_ecosystems(conn: &Connection) -> Vec<Ecosystem> {
    let mut out: Vec<Ecosystem> = Vec::new();
    for sql in [
        "SELECT DISTINCT ecosystem FROM dependency_instances",
        "SELECT DISTINCT ecosystem FROM user_dependencies",
    ] {
        let Ok(mut stmt) = conn.prepare(sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) else {
            continue;
        };
        for eco in rows.flatten().filter_map(|e| Ecosystem::parse(&e)) {
            if !out.contains(&eco) {
                out.push(eco);
            }
        }
    }
    out
}

/// Advisories whose `Affected:` list names one of the query's named packages,
/// best BM25 match first, at most [`MAX_PACKAGE_PINS`].
pub(crate) fn package_lane(conn: &Connection, ctx: &PackageContext) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for name in &ctx.named {
        let fts_query = format!("\"{}\"", name.replace('"', "\"\""));
        let rows = run_leg(
            conn,
            "SELECT si.id, si.title, si.content, si.source_type, si.url, si.created_at
             FROM source_items_fts fts
             JOIN source_items si ON si.id = fts.rowid
             WHERE source_items_fts MATCH ?1
               AND si.content LIKE '%Affected:%'
               AND si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)
             ORDER BY rank
             LIMIT ?2",
            rusqlite::params![fts_query, PACKAGE_LANE_CANDIDATES],
            false,
        );
        for cand in rows {
            if out.len() >= MAX_PACKAGE_PINS {
                return out;
            }
            let affected = parse_affected(&cand.content);
            let names_it = affected_named_package(&affected, std::slice::from_ref(name)).is_some();
            if names_it && !out.iter().any(|c| c.id == cand.id) {
                out.push(cand);
            }
        }
    }
    out
}

/// Pin the package advisories behind the exact-title pins: an advisory already in
/// the results keeps its place and fused ranks; a missing one replaces the lowest
/// unpinned result, so the result count never exceeds `limit`.
pub(crate) fn merge_package_lane(
    mut results: Vec<HybridSearchResult>,
    lane: &[Candidate],
    limit: usize,
) -> Vec<HybridSearchResult> {
    let mut pinned: HashSet<i64> = HashSet::new();
    for cand in lane {
        if results.iter().any(|r| r.item_id == cand.id) {
            pinned.insert(cand.id);
            continue;
        }
        if results.len() < limit {
            results.push(cand.to_result());
        } else if let Some(pos) = results
            .iter()
            .rposition(|r| !r.exact_title && !pinned.contains(&r.item_id))
        {
            results[pos] = cand.to_result();
        } else {
            continue;
        }
        pinned.insert(cand.id);
    }
    results
}

/// Mark every result that is an advisory for a named package, or an advisory for
/// an ecosystem the user has no dependencies in. No-op without a named package.
pub(crate) fn annotate(results: &mut [HybridSearchResult], ctx: &PackageContext) {
    if !ctx.is_active() {
        return;
    }
    for r in results.iter_mut() {
        let affected = parse_affected(&r.content);
        if affected.is_empty() {
            continue;
        }
        r.query_package = affected_named_package(&affected, &ctx.named);
        r.foreign_ecosystem =
            r.query_package.is_none() && is_foreign_ecosystem(&affected, &ctx.installed_ecosystems);
    }
}
