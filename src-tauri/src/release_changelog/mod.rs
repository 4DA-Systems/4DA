// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The release-changelog lane: "what changed" for a registry release, read
//! from the changelog the package ships inside its own registry archive
//! (Decision 1, option A — no host beyond the registry that already sees the
//! package name; NETWORK.md §1e).
//!
//! A graded release row ("Breaking upgrade: fastembed 5.17.4 → 7.1.0") said
//! WHICH project a release concerns and that it is a major, never what the
//! major changed. This lane reads the new version's archive once, parses its
//! changelog (`parse.rs`, a port of the MCP server's tested parser), cuts the
//! sections in (installed, new] and classifies each entry (`classify.rs`).
//!
//! Flow:
//! - `spawn_backlog` (after every cache fill) walks recent registry rows,
//!   grades each with `scoring::release_grade`, and fetches the archive of
//!   every Breaking / Minor / Yanked row not yet cached — at most
//!   [`MAX_FETCHES_PER_CYCLE`] archives a cycle, spaced [`FETCH_SPACING`]
//!   apart, never on the scoring path.
//! - `get_release_changes` (IPC) answers the release card from the cache, and
//!   reads the archive on demand when the backlog has not reached it yet.
//!
//! The card never invents: no changelog in the package, a changelog this
//! parser cannot read, or a registry that could not be reached are each said
//! as such.

pub(crate) mod archive;
mod classify;
pub(crate) mod commands;
pub(crate) mod fetch;
pub(crate) mod parse;
pub(crate) mod store;

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use ts_rs::TS;

use crate::db::Database;
use crate::scoring::release_grade::{self, ReleaseClass};
use classify::ChangeKind;
use fetch::{Ecosystem, FetchError};
use parse::{find_changelog_file, is_changelog_name, parse_changelog, select_range};
use store::{ChangelogRecord, RecordStatus};

/// Most archives fetched by one backlog pass.
const MAX_FETCHES_PER_CYCLE: usize = 6;
/// Pause between two archive fetches in a backlog pass.
const FETCH_SPACING: Duration = Duration::from_millis(1100);
/// Registry rows the backlog looks at, newest first.
const BACKLOG_SCAN_ROWS: i64 = 300;
/// Breaking entries shown on the card.
const TOP_BREAKING: usize = 5;

/// What the release card says about one release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum ReleaseChangesStatus {
    /// Changelog sections in (from, to] were found.
    Found,
    /// The package's changelog has no section for any version in range.
    NoSectionsInRange,
    /// The package ships no changelog file.
    NoChangelog,
    /// The changelog has no version headings the parser recognises.
    Unparsed,
    /// The archive was refused (over the size cap, not gzip, not found).
    Refused,
    /// The registry could not be reached; retried on a later cycle.
    Unavailable,
}

/// One entry shown on the card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReleaseChangeEntry {
    /// The release the entry belongs to.
    pub version: String,
    /// Sanitised entry text (control / bidi characters stripped, ≤ 400 chars).
    pub text: String,
    /// The heading or parent bullet it sits under ("Removed").
    pub under: Option<String>,
}

/// "What changed" between the installed and the announced version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ReleaseChanges {
    pub ecosystem: String,
    pub package: String,
    pub from_version: String,
    pub to_version: String,
    pub status: ReleaseChangesStatus,
    /// The changelog's path inside the archive ("CHANGELOG.md").
    pub file: Option<String>,
    /// Release sections in (from, to], newest first as the changelog lists them.
    pub versions: Vec<String>,
    /// The changelog reaches both ends of the range; false = partial history.
    pub covers_range: bool,
    pub breaking: u32,
    pub features: u32,
    pub fixes: u32,
    pub security: u32,
    pub deprecations: u32,
    pub other: u32,
    pub top_breaking: Vec<ReleaseChangeEntry>,
    /// Where the user can read the changelog itself (a link, never fetched).
    pub source_url: Option<String>,
    /// Why there is nothing to show, in plain words.
    pub reason: Option<String>,
}

/// The release a graded row is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseTarget {
    pub eco: Ecosystem,
    pub package: String,
    pub from: String,
    pub to: String,
}

/// The release target of a registry row, when the grade says it is news
/// (Breaking, Minor, Yanked) and names the installed copy.
pub(crate) fn release_target(
    db: &Database,
    source_type: &str,
    title: &str,
    content: &str,
) -> Option<ReleaseTarget> {
    let eco = Ecosystem::from_source_type(source_type)?;
    let grade = release_grade::grade_registry_release(db, source_type, title, content)?;
    let news = matches!(
        grade.class()?,
        ReleaseClass::Breaking | ReleaseClass::Minor | ReleaseClass::Yanked
    );
    if !news {
        return None;
    }
    Some(ReleaseTarget {
        eco,
        package: grade.package.clone(),
        from: grade.headline_installed()?,
        to: grade.announced.to_string(),
    })
}

/// Parse one archive's bytes into a cacheable record. Pure; CPU-bound.
pub(crate) fn record_from_archive(gz: &[u8]) -> ChangelogRecord {
    let files = match archive::read_root_text_files(gz, is_changelog_name) {
        Ok(files) => files,
        Err(e) => return refused(e.0),
    };
    let Some(file) = find_changelog_file(files.keys().map(String::as_str)).map(str::to_string)
    else {
        return ChangelogRecord {
            status: RecordStatus::NoChangelog,
            file: None,
            text: None,
            reason: Some("the package archive ships no changelog file".into()),
        };
    };
    let text = files.get(&file).cloned().unwrap_or_default();
    let readable = !parse_changelog(&text).is_empty();
    ChangelogRecord {
        status: if readable {
            RecordStatus::Found
        } else {
            RecordStatus::Unparsed
        },
        reason: (!readable)
            .then(|| format!("{file} has no version headings this parser recognises")),
        file: Some(file),
        text: Some(text),
    }
}

fn refused(reason: String) -> ChangelogRecord {
    ChangelogRecord {
        status: RecordStatus::Refused,
        file: None,
        text: None,
        reason: Some(reason),
    }
}

/// The cached record for `target`'s new version, fetching and caching it
/// when absent. `Err` = transient (network / registry) — not cached.
pub(crate) async fn changelog_for(
    db: &Database,
    t: &ReleaseTarget,
) -> Result<ChangelogRecord, String> {
    if let Ok(Some(hit)) = store::get(db, t.eco, &t.package, &t.to) {
        return Ok(hit);
    }
    let record = match fetch::download_archive(t.eco, &t.package, &t.to).await {
        Ok(bytes) => tokio::task::spawn_blocking(move || record_from_archive(&bytes))
            .await
            .map_err(|e| format!("archive parse task failed: {e}"))?,
        Err(FetchError::Refused(reason)) => refused(reason),
        Err(FetchError::Transient(reason)) => return Err(reason),
    };
    if let Err(e) = store::put(db, t.eco, &t.package, &t.to, &record) {
        warn!(target: "4da::release_changelog", package = %t.package, "cache write failed: {e}");
    }
    Ok(record)
}

/// Where the user can read the changelog (opened in their browser by a click;
/// the app itself never fetches it).
fn source_url(t: &ReleaseTarget, file: Option<&str>) -> String {
    match (t.eco, file) {
        (Ecosystem::Crates, Some(file)) => {
            let relative = file.split_once('/').map_or(file, |(_, rest)| rest);
            format!(
                "https://docs.rs/crate/{}/{}/source/{relative}",
                t.package, t.to
            )
        }
        (Ecosystem::Crates, None) => format!("https://crates.io/crates/{}/{}", t.package, t.to),
        (Ecosystem::Npm, _) => format!(
            "https://www.npmjs.com/package/{}/v/{}?activeTab=code",
            t.package, t.to
        ),
    }
}

fn base_changes(t: &ReleaseTarget, status: ReleaseChangesStatus) -> ReleaseChanges {
    ReleaseChanges {
        ecosystem: t.eco.as_str().to_string(),
        package: t.package.clone(),
        from_version: t.from.clone(),
        to_version: t.to.clone(),
        status,
        file: None,
        versions: Vec::new(),
        covers_range: false,
        breaking: 0,
        features: 0,
        fixes: 0,
        security: 0,
        deprecations: 0,
        other: 0,
        top_breaking: Vec::new(),
        source_url: None,
        reason: None,
    }
}

/// The card for a transient failure.
pub(crate) fn unavailable(t: &ReleaseTarget, reason: String) -> ReleaseChanges {
    let mut out = base_changes(t, ReleaseChangesStatus::Unavailable);
    out.reason = Some(reason);
    out
}

/// The card for `target` from its archive's record: the sections in
/// (from, to], counted by kind, with the first breaking entries.
pub(crate) fn summarize(t: &ReleaseTarget, record: &ChangelogRecord) -> ReleaseChanges {
    let status = match record.status {
        RecordStatus::Found => ReleaseChangesStatus::Found,
        RecordStatus::NoChangelog => ReleaseChangesStatus::NoChangelog,
        RecordStatus::Unparsed => ReleaseChangesStatus::Unparsed,
        RecordStatus::Refused => ReleaseChangesStatus::Refused,
    };
    let mut out = base_changes(t, status);
    out.file = record.file.clone();
    out.reason = record.reason.clone();
    out.source_url = Some(source_url(t, record.file.as_deref()));
    if record.status != RecordStatus::Found {
        return out;
    }
    // The installed version bounds the bottom: a changelog whose oldest
    // section is past it is reported as partial, never as the whole story.
    let all = parse_changelog(record.text.as_deref().unwrap_or(""));
    let (sections, covers) = select_range(&all, &t.from, &t.to, &t.from);
    if sections.is_empty() {
        out.status = ReleaseChangesStatus::NoSectionsInRange;
        out.reason = Some(format!(
            "{} has no section for a version after {} up to {}",
            record.file.as_deref().unwrap_or("the changelog"),
            t.from,
            t.to
        ));
        return out;
    }
    out.covers_range = covers;
    for section in &sections {
        out.versions.push(section.version.clone());
        for entry in &section.entries {
            match entry.kind {
                ChangeKind::Breaking => {
                    out.breaking += 1;
                    if out.top_breaking.len() < TOP_BREAKING {
                        out.top_breaking.push(ReleaseChangeEntry {
                            version: section.version.clone(),
                            text: entry.text.clone(),
                            under: entry.under.clone(),
                        });
                    }
                }
                ChangeKind::Feature => out.features += 1,
                ChangeKind::Fix => out.fixes += 1,
                ChangeKind::Security => out.security += 1,
                ChangeKind::Deprecation => out.deprecations += 1,
                ChangeKind::Other => out.other += 1,
            }
        }
    }
    out
}

/// Registry rows the backlog considers: recent, newest first.
fn recent_registry_rows(db: &Database) -> Vec<(String, String, String)> {
    let conn = db.conn.lock();
    let Ok(mut stmt) = conn.prepare(
        "SELECT source_type, title, COALESCE(content, '') FROM source_items
         WHERE source_type IN ('crates_io', 'crates', 'npm_registry', 'npm')
           AND created_at >= datetime('now', '-45 days')
         ORDER BY id DESC LIMIT ?1",
    ) else {
        return Vec::new();
    };
    stmt.query_map([BACKLOG_SCAN_ROWS], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// The newest graded news release per package among recent rows, minus the
/// ones already cached. One target per package: `ai` announced twelve 7.0.x
/// patches in a fortnight, and fetching each archive would spend a whole
/// cycle's budget on superseded versions (live 2026-10-04). An older row's
/// card still reads its own archive on demand.
pub(crate) fn backlog_targets(db: &Database) -> Vec<ReleaseTarget> {
    let mut newest: Vec<ReleaseTarget> = Vec::new();
    for (source_type, title, content) in recent_registry_rows(db) {
        let Some(t) = release_target(db, &source_type, &title, &content) else {
            continue;
        };
        let key = store::package_key(t.eco, &t.package);
        match newest
            .iter_mut()
            .find(|o| o.eco == t.eco && store::package_key(o.eco, &o.package) == key)
        {
            Some(o) => {
                if parse::compare_versions(&t.to, &o.to) == Some(std::cmp::Ordering::Greater) {
                    *o = t;
                }
            }
            None => newest.push(t),
        }
    }
    newest.retain(|t| !store::has(db, t.eco, &t.package, &t.to));
    newest
}

/// One backlog pass: fetch up to [`MAX_FETCHES_PER_CYCLE`] uncached archives.
pub(crate) async fn run_backlog(db: &Database) -> usize {
    if let Err(e) = store::prune(db) {
        warn!(target: "4da::release_changelog", "cache prune failed: {e}");
    }
    let targets = backlog_targets(db);
    let mut fetched = 0;
    for t in targets.iter().take(MAX_FETCHES_PER_CYCLE) {
        if fetched > 0 {
            tokio::time::sleep(FETCH_SPACING).await;
        }
        match changelog_for(db, t).await {
            Ok(r) => {
                debug!(target: "4da::release_changelog", package = %t.package, version = %t.to, status = ?r.status, "changelog cached")
            }
            Err(e) => {
                debug!(target: "4da::release_changelog", package = %t.package, version = %t.to, "changelog fetch deferred: {e}")
            }
        }
        fetched += 1;
    }
    if fetched > 0 {
        info!(target: "4da::release_changelog", fetched, pending = targets.len().saturating_sub(fetched), "release changelog backlog pass");
    }
    fetched
}

static BACKLOG_RUNNING: AtomicBool = AtomicBool::new(false);

/// Clears [`BACKLOG_RUNNING`] when a pass ends, including by panic.
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        BACKLOG_RUNNING.store(false, AtomicOrdering::SeqCst);
    }
}

/// Start a backlog pass in the background unless one is running. Called at
/// the end of every cache fill; never awaited by the scoring path.
pub(crate) fn spawn_backlog() {
    if BACKLOG_RUNNING.swap(true, AtomicOrdering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        let _guard = RunningGuard;
        if let Ok(db) = crate::get_database() {
            run_backlog(db).await;
        }
    });
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
