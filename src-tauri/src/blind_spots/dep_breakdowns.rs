// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! One shared pass for every coverage gap's consequence breakdown.
//!
//! Measured 2026-10-10 on a 2 GB corpus: `count_signal_types_for_dep_conn`
//! cost ~0.3 s per dependency, because each call re-read the ~77k items of
//! the last 30 days with a title `LIKE` and two correlated linker lookups.
//! A report build asks for every gap row's breakdown three times (the
//! nothing-left-to-review filter, the recommendations, the tab items) — 8.8 to
//! 10.8 s in the filter alone.
//!
//! [`prime`] reads those recent rows and the linker's structured links ONCE,
//! then visits, for each dependency, exactly the rows the per-dependency query
//! selects (`title LIKE '%name%'` OR a registry/advisory link) and judges each
//! with the SAME [`DepTally`] that query uses. The results land in the memo
//! every later lookup reads, so the three consumers pay for one pass.
//!
//! Candidate selection reproduces SQLite exactly: `LIKE` folds ASCII letters
//! only and treats `_` as "any one character" and `%` as "any run";
//! `LOWER()` folds ASCII only. Proven against the per-dependency query by the
//! equivalence tests (a fixture, and a live-snapshot test).

use std::collections::{HashMap, HashSet};

use rusqlite::params;
use tracing::{debug, warn};

use super::{release_is_newer_than_installed, DepSignalBreakdown};
use crate::package_ambiguity::{has_word_boundary_match, is_ambiguous_package_name};

/// One dependency's view of the corpus while its breakdown is tallied: the
/// per-dependency facts every candidate row is judged against. Shared by the
/// per-dependency query ([`count_signal_types_for_dep_conn`]) and the one-pass
/// batch (`dep_breakdowns`), so the two can only differ in WHICH rows they
/// visit — never in how a row is judged.
pub(super) struct DepTally<'a> {
    dep_name: &'a str,
    dep_lower: String,
    ambiguous: bool,
    wanted_ecosystem: Option<&'static str>,
    installs: Vec<crate::osv::exposure::Install>,
    lowest_installed: Option<&'a str>,
    /// Whether some install is exposed to a stored advisory — one query, run
    /// only when a linked advisory row actually needs it.
    exposed: std::cell::OnceCell<bool>,
    b: DepSignalBreakdown,
}

/// One candidate row, as the breakdown reads it.
pub(super) struct CandidateRow<'r> {
    title: &'r str,
    content_type: Option<&'r str>,
    source_type: &'r str,
    linked: bool,
    source_id: &'r str,
    item_id: i64,
}

impl<'a> DepTally<'a> {
    pub(super) fn new(dep_name: &'a str, ecosystem: Option<&str>, installed: &'a [String]) -> Self {
        // A security signal counts only while SOME install is exposed to a
        // stored advisory. Live 2026-09-07: hono 4.13.3 (every advisory fixed
        // ≤ 4.12.34), lettre 0.11.22 (= the fix) and react 19.2.7 (OSV-clean)
        // all read "N security signals unreviewed" at HIGH, and the AI
        // assessment then told the user to review before upgrading.
        // Conservative: no known installed version stays exposed. `installed`
        // is lowest-first (see `installed_versions`), so a release is NEW when
        // the lowest install is below it — one project behind keeps it new.
        // Installs carry this row's ecosystem (AD-045): an npm `jsonwebtoken`
        // is never exposed by the crates.io advisory of the same name.
        let wanted_ecosystem = ecosystem.and_then(crate::osv::exposure::canonical);
        let installs = installed
            .iter()
            .map(|v| crate::osv::exposure::Install {
                ecosystem: wanted_ecosystem,
                version: v.clone(),
            })
            .collect();
        Self {
            dep_name,
            dep_lower: dep_name.to_lowercase(),
            ambiguous: is_ambiguous_package_name(dep_name),
            wanted_ecosystem,
            installs,
            lowest_installed: installed.first().map(String::as_str),
            exposed: std::cell::OnceCell::new(),
            b: DepSignalBreakdown::default(),
        }
    }

    fn exposed(&self, conn: &rusqlite::Connection) -> bool {
        *self.exposed.get_or_init(|| {
            self.installs.is_empty()
                || crate::knowledge_decay::installs_still_vulnerable(
                    conn,
                    self.dep_name,
                    &self.installs,
                )
        })
    }

    /// Judge one candidate row and count it in at most one bucket.
    pub(super) fn tally(&mut self, conn: &rusqlite::Connection, row: &CandidateRow<'_>) {
        // A REGISTRY row is a release of its SUBJECT crate, nothing else: the
        // subject must be this dependency (axum-stack, axum-serde-boundary
        // and tauri-plugin-* are not releases of axum or tauri — live
        // 2026-09-07 "axum — 32 new releases in 30 days" for a crate that
        // shipped once, in April), and a version the user already runs is
        // not a NEW release ("sha2 — 2 new releases" for the installed 0.11.0).
        if crate::dep_linker::is_registry_source(row.source_type) {
            // A registry row speaks for ITS registry's package: a crates.io
            // `jsonwebtoken` release is no news about the npm one (AD-045).
            if let (Some(want), Some(row_ecosystem)) = (
                self.wanted_ecosystem,
                crate::osv::exposure::registry_source_ecosystem(row.source_type),
            ) {
                if want != row_ecosystem {
                    return;
                }
            }
            let Some((subject, version)) = crate::dep_linker::registry_title_subject(row.title)
            else {
                return;
            };
            if !crate::dep_linker::registry_names_equal(&subject, self.dep_name) {
                return;
            }
            if release_is_newer_than_installed(version.as_deref(), self.lowest_installed) {
                self.b.releases += 1;
            }
            return;
        }
        // An ADVISORY row counts only through the linker's `Affected:` proof
        // and only while the install is exposed; its title is never the link.
        if matches!(row.source_type, "osv" | "cve") {
            // Judged per ADVISORY where the mirror can resolve the row — its
            // own ecosystem, its own range, against these installs (AD-045) —
            // else by whether any install is exposed to anything at all.
            if row.linked
                && crate::osv::exposure::advisory_row_reaches(
                    conn,
                    row.source_id,
                    self.dep_name,
                    &self.installs,
                )
                .unwrap_or_else(|| self.exposed(conn))
            {
                self.b.add_security(row.item_id, true);
            }
            return;
        }
        // Editorial rows: a linker row is proof; otherwise an ambiguous name
        // never counts on its title and anything else needs the dependency
        // name as a WHOLE word — "silverstripe" is not stripe, "honors" is
        // not hono, "reacted" is not react. An editorial security story is a
        // citation about the package, never proof of exposure — it is
        // discussion here (the Shai-Hulud codegen story is not a react bug).
        let qualifies = row.linked
            || (!self.ambiguous
                && has_word_boundary_match(&row.title.to_lowercase(), &self.dep_lower));
        if !qualifies {
            return;
        }
        match row.content_type {
            Some("release_notes") | Some("platform_update") => self.b.releases += 1,
            Some("expert_analysis") | Some("deep_dive") => self.b.analyses += 1,
            Some("breaking_change") => self.b.add_security(row.item_id, false),
            _ => self.b.other += 1,
        }
    }
}

/// The breakdown for ONE dependency, by its own query. The report build
/// primes every gap row's breakdown in one shared pass ([`prime`]);
/// this stays the path for a single lookup and the reference the batch is
/// held to by the equivalence tests.
pub(super) fn count_signal_types_for_dep_conn(
    conn: &rusqlite::Connection,
    dep_name: &str,
    ecosystem: Option<&str>,
    installed: &[String],
) -> DepSignalBreakdown {
    let mut tally = DepTally::new(dep_name, ecosystem, installed);
    // Candidates: everything the linker bound to this package with
    // registry/advisory proof, plus title substring hits — re-checked in
    // `DepTally::tally`. The bare `title LIKE '%name%'` this replaced counted
    // five Next.js advisories, "how Google reacted" and a post about
    // neoliberalism as ten react security signals, Electron's "honors" as a
    // hono advisory and four silverstripe CVEs as stripe's (2026-09-06).
    let sql = "SELECT si.title, si.content_type, si.source_type,
                      EXISTS(SELECT 1 FROM source_item_dependencies sid
                              WHERE sid.source_item_id = si.id
                                AND LOWER(sid.package_name) = LOWER(?1)
                                AND sid.match_type IN ('exact_registry', 'advisory')) AS linked,
                      si.source_id, si.id
               FROM source_items si
               WHERE si.created_at >= datetime('now', '-30 days')
                 AND (si.title LIKE '%' || ?1 || '%'
                      OR EXISTS(SELECT 1 FROM source_item_dependencies sid2
                                 WHERE sid2.source_item_id = si.id
                                   AND LOWER(sid2.package_name) = LOWER(?1)
                                   AND sid2.match_type IN ('exact_registry', 'advisory')))";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return tally.b;
    };
    let Ok(rows) = stmt.query_map(params![dep_name], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            row.get::<_, i64>(5)?,
        ))
    }) else {
        return tally.b;
    };
    for (title, content_type, source_type, linked, source_id, item_id) in rows.flatten() {
        tally.tally(
            conn,
            &CandidateRow {
                title: &title,
                content_type: content_type.as_deref(),
                source_type: &source_type,
                linked,
                source_id: &source_id,
                item_id,
            },
        );
    }
    tally.b
}

/// One breakdown to compute: the dependency's bare name, the ecosystem its
/// gap row names, and its installed versions (lowest first).
pub(super) struct BreakdownRequest {
    pub dep_name: String,
    pub ecosystem: Option<String>,
    pub installed: Vec<String>,
}

/// One recent `source_items` row, as the breakdown reads it.
struct RecentRow {
    item_id: i64,
    title: String,
    /// The title with ASCII letters lowercased — what SQLite's `LIKE`
    /// compares (it does not fold non-ASCII letters).
    title_fold: String,
    content_type: Option<String>,
    source_type: String,
    source_id: String,
}

/// The last 30 days of items plus the linker's structured links, read once.
pub(super) struct RecentCorpus {
    rows: Vec<RecentRow>,
    /// Item id → position in `rows`.
    by_id: HashMap<i64, usize>,
    /// `LOWER(package_name)` (ASCII fold) → the recent items the linker bound
    /// to it with registry/advisory proof.
    linked: HashMap<String, HashSet<i64>>,
}

/// The window and link kinds the per-dependency query uses, verbatim.
const RECENT_ROWS_SQL: &str =
    "SELECT si.id, si.title, si.content_type, si.source_type, si.source_id
     FROM source_items si
     WHERE si.created_at >= datetime('now', '-30 days')";
const RECENT_LINKS_SQL: &str = "SELECT sid.source_item_id, sid.package_name
     FROM source_item_dependencies sid
     JOIN source_items si ON si.id = sid.source_item_id
     WHERE si.created_at >= datetime('now', '-30 days')
       AND sid.match_type IN ('exact_registry', 'advisory')";

impl RecentCorpus {
    pub(super) fn load(conn: &rusqlite::Connection) -> rusqlite::Result<Self> {
        let mut stmt = conn.prepare(RECENT_ROWS_SQL)?;
        // A NULL title or source type fails the per-dependency query's row
        // read, which drops the row; skipping it here is the same outcome.
        let rows: Vec<RecentRow> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .flatten()
            .filter_map(|(item_id, title, content_type, source_type, source_id)| {
                let (title, source_type) = (title?, source_type?);
                Some(RecentRow {
                    item_id,
                    title_fold: title.to_ascii_lowercase(),
                    title,
                    content_type,
                    source_type,
                    source_id: source_id.unwrap_or_default(),
                })
            })
            .collect();
        let mut linked: HashMap<String, HashSet<i64>> = HashMap::new();
        let mut stmt = conn.prepare(RECENT_LINKS_SQL)?;
        let links = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        for (item_id, package) in links.flatten() {
            if let Some(package) = package {
                linked
                    .entry(package.to_ascii_lowercase())
                    .or_default()
                    .insert(item_id);
            }
        }
        let by_id = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.item_id, i))
            .collect();
        Ok(Self {
            rows,
            by_id,
            linked,
        })
    }

    /// The breakdown [`count_signal_types_for_dep_conn`] would return, from the
    /// rows already in memory.
    pub(super) fn breakdown(
        &self,
        conn: &rusqlite::Connection,
        dep_name: &str,
        ecosystem: Option<&str>,
        installed: &[String],
    ) -> DepSignalBreakdown {
        let fold = dep_name.to_ascii_lowercase();
        let linked_ids = self.linked.get(&fold);
        let pattern = LikeContains::new(&fold);
        let mut tally = DepTally::new(dep_name, ecosystem, installed);
        // Rows the title names (checking the link only for those)...
        for row in self.rows.iter().filter(|r| pattern.matches(&r.title_fold)) {
            let linked = linked_ids.is_some_and(|ids| ids.contains(&row.item_id));
            tally.tally(conn, &row.candidate(linked));
        }
        // ...then the linked rows the title does not name. Every row is
        // visited at most once; the tally is order-independent.
        for id in linked_ids.into_iter().flatten() {
            let Some(row) = self.by_id.get(id).and_then(|&i| self.rows.get(i)) else {
                continue;
            };
            if !pattern.matches(&row.title_fold) {
                tally.tally(conn, &row.candidate(true));
            }
        }
        tally.b
    }
}

impl RecentRow {
    fn candidate(&self, linked: bool) -> CandidateRow<'_> {
        CandidateRow {
            title: &self.title,
            content_type: self.content_type.as_deref(),
            source_type: &self.source_type,
            linked,
            source_id: &self.source_id,
            item_id: self.item_id,
        }
    }
}

/// SQLite's `haystack LIKE '%' || needle || '%'`, both sides already
/// ASCII-folded. A needle without wildcards is a plain substring test (byte
/// substring of valid UTF-8 is char-aligned); `_` and `%` inside a package
/// name (`serial_test`) keep their `LIKE` meaning.
struct LikeContains {
    needle: String,
    /// `%needle%` as chars when the needle carries a wildcard.
    glob: Option<Vec<char>>,
}

impl LikeContains {
    fn new(needle_fold: &str) -> Self {
        let glob = needle_fold.contains(['%', '_']).then(|| {
            std::iter::once('%')
                .chain(needle_fold.chars())
                .chain(std::iter::once('%'))
                .collect()
        });
        Self {
            needle: needle_fold.to_string(),
            glob,
        }
    }

    fn matches(&self, haystack_fold: &str) -> bool {
        match &self.glob {
            None => haystack_fold.contains(self.needle.as_str()),
            Some(glob) => like_glob(&haystack_fold.chars().collect::<Vec<_>>(), glob),
        }
    }
}

/// Whole-string `LIKE` match: `%` any run (including empty), `_` exactly one
/// character, anything else itself. Greedy with single backtrack point — the
/// standard linear-space wildcard algorithm.
fn like_glob(hay: &[char], pat: &[char]) -> bool {
    let (mut h, mut p) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while h < hay.len() {
        match pat.get(p) {
            Some('%') => {
                star = Some((p, h));
                p += 1;
            }
            Some(&c) if c == '_' || c == hay[h] => {
                h += 1;
                p += 1;
            }
            _ => match star {
                Some((sp, sh)) => {
                    p = sp + 1;
                    h = sh + 1;
                    star = Some((sp, sh + 1));
                }
                None => return false,
            },
        }
    }
    pat[p..].iter().all(|&c| c == '%')
}

/// Compute every request in one pass over the recent corpus and memoise the
/// results for the lookups that follow. Returns how many were primed (0 when
/// the corpus could not be read — the lookups then fall back to their own
/// per-dependency queries, so nothing is lost but time).
pub(super) fn prime(conn: &rusqlite::Connection, requests: &[BreakdownRequest]) -> usize {
    if requests.is_empty() {
        return 0;
    }
    let started = std::time::Instant::now();
    let corpus = match RecentCorpus::load(conn) {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::blind_spots", error = %e, "breakdown pre-pass could not read the corpus");
            return 0;
        }
    };
    let loaded_ms = started.elapsed().as_millis() as u64;
    for r in requests {
        let b = corpus.breakdown(conn, &r.dep_name, r.ecosystem.as_deref(), &r.installed);
        memo_put(
            memo_key(&r.dep_name, r.ecosystem.as_deref(), &r.installed),
            b,
        );
    }
    debug!(
        target: "4da::blind_spots::profile",
        deps = requests.len(),
        rows = corpus.rows.len(),
        loaded_ms,
        total_ms = started.elapsed().as_millis() as u64,
        "breakdown pre-pass"
    );
    requests.len()
}

/// The memo key: one breakdown per (dependency, ecosystem, installs).
pub(super) fn memo_key(dep_name: &str, ecosystem: Option<&str>, installed: &[String]) -> String {
    format!(
        "{dep_name}\u{0}{}\u{0}{}",
        ecosystem.unwrap_or_default(),
        installed.join(",")
    )
}

/// Production memo: process-wide, each entry good for one report's lifetime.
#[cfg(not(test))]
mod memo {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use parking_lot::Mutex;

    use super::DepSignalBreakdown;

    /// A report build plus its items finishes well inside this; a lookup
    /// after it (the AI assessment) recomputes against current rows.
    const TTL: Duration = Duration::from_mins(2);

    static MEMO: Mutex<Option<HashMap<String, (Instant, DepSignalBreakdown)>>> = Mutex::new(None);

    pub(super) fn get(key: &str) -> Option<DepSignalBreakdown> {
        MEMO.lock()
            .as_ref()
            .and_then(|m| m.get(key))
            .filter(|(at, _)| at.elapsed() < TTL)
            .map(|(_, b)| *b)
    }

    pub(super) fn put(key: String, b: DepSignalBreakdown) {
        let mut guard = MEMO.lock();
        let memo = guard.get_or_insert_with(HashMap::new);
        memo.retain(|_, (at, _)| at.elapsed() < TTL);
        memo.insert(key, (Instant::now(), b));
    }
}

/// Test memo: per test thread, filled only by [`prime`], so a test that
/// never builds a report reads its corpus stand-in exactly as before.
#[cfg(test)]
mod memo {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::DepSignalBreakdown;

    thread_local! {
        static MEMO: RefCell<HashMap<String, DepSignalBreakdown>> = RefCell::new(HashMap::new());
    }

    pub(super) fn get(key: &str) -> Option<DepSignalBreakdown> {
        MEMO.with(|m| m.borrow().get(key).copied())
    }

    pub(super) fn put(key: String, b: DepSignalBreakdown) {
        MEMO.with(|m| m.borrow_mut().insert(key, b));
    }
}

pub(super) fn memo_get(key: &str) -> Option<DepSignalBreakdown> {
    memo::get(key)
}

pub(super) fn memo_put(key: String, b: DepSignalBreakdown) {
    memo::put(key, b);
}

#[cfg(test)]
#[path = "dep_breakdowns_tests.rs"]
mod tests;
