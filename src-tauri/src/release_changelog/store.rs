// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The lane's cache: one row per (ecosystem, package, version) archive, so a
//! published archive — which never changes — is downloaded once.
//!
//! The table is owned by this module and created with `CREATE TABLE IF NOT
//! EXISTS` on first use (the `engine_runs` pattern), not by a versioned
//! migration: it is a pure cache nothing else reads, and an additive table
//! leaves the schema version alone, so a binary built before this lane can
//! still open a database a newer one has touched (CLAUDE.md: "Never let an OLD
//! binary open a NEWER database").
//!
//! The cache holds the changelog TEXT (gzip, ≤ 2 MB before compression), not
//! parsed sections: the installed version can move after the fetch, so the
//! range is cut at read time, and parsing on read means a first answer and a
//! cached answer can never differ (a section cap here once made `ai`'s 603
//! in-range sections read as 400 from cache). Parsing a changelog is
//! milliseconds. Transient failures are never written here.

use std::io::{Read, Write};

use rusqlite::{params, Connection, OptionalExtension};

use super::archive::MAX_FILE_BYTES;
use super::fetch::Ecosystem;
use crate::db::Database;

/// Rows not re-fetched for this long are dropped; a release row that old has
/// left the feed, and a card asked for again simply re-reads the archive.
const RETENTION_DAYS: i64 = 120;

const TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS release_changelogs (
    ecosystem TEXT NOT NULL,
    package TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    file TEXT,
    changelog_gz BLOB,
    reason TEXT,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (ecosystem, package, version)
)";

/// What reading one archive produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordStatus {
    /// A changelog with version sections.
    Found,
    /// The archive ships no changelog file.
    NoChangelog,
    /// A changelog file with no version headings this parser recognises.
    Unparsed,
    /// The archive was refused (cap, format, not on the registry, 404).
    Refused,
}

impl RecordStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::NoChangelog => "no_changelog",
            Self::Unparsed => "unparsed",
            Self::Refused => "refused",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "found" => Self::Found,
            "no_changelog" => Self::NoChangelog,
            "unparsed" => Self::Unparsed,
            "refused" => Self::Refused,
            _ => return None,
        })
    }
}

/// One archive's changelog, as cached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangelogRecord {
    pub status: RecordStatus,
    /// The changelog's path inside the archive.
    pub file: Option<String>,
    /// The changelog text (present for Found and Unparsed).
    pub text: Option<String>,
    pub reason: Option<String>,
}

/// The cache key for a package: crates.io folds `-`/`_` and case.
pub(crate) fn package_key(eco: Ecosystem, name: &str) -> String {
    match eco {
        Ecosystem::Crates => name.to_lowercase().replace('_', "-"),
        Ecosystem::Npm => name.to_lowercase(),
    }
}

pub(crate) fn ensure_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(TABLE_SQL)
}

fn compress(text: &str) -> Option<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(text.as_bytes()).ok()?;
    enc.finish().ok()
}

/// Inflate a stored changelog, bounded by the per-file cap it was read under.
fn decompress(gz: &[u8]) -> Option<String> {
    let mut out = String::new();
    flate2::read::GzDecoder::new(gz)
        .take(MAX_FILE_BYTES as u64 * 4)
        .read_to_string(&mut out)
        .ok()?;
    Some(out)
}

/// The cached record for one archive, if any.
pub(crate) fn get(
    db: &Database,
    eco: Ecosystem,
    name: &str,
    version: &str,
) -> rusqlite::Result<Option<ChangelogRecord>> {
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    let row: Option<(String, Option<String>, Option<Vec<u8>>, Option<String>)> = conn
        .query_row(
            "SELECT status, file, changelog_gz, reason FROM release_changelogs
             WHERE ecosystem = ?1 AND package = ?2 AND version = ?3",
            params![eco.as_str(), package_key(eco, name), version],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(row.and_then(|(status, file, gz, reason)| {
        Some(ChangelogRecord {
            status: RecordStatus::parse(&status)?,
            file,
            text: gz.as_deref().and_then(decompress),
            reason,
        })
    }))
}

/// True when the archive is already cached.
pub(crate) fn has(db: &Database, eco: Ecosystem, name: &str, version: &str) -> bool {
    get(db, eco, name, version).ok().flatten().is_some()
}

/// Cache one archive's record (replacing any earlier one).
pub(crate) fn put(
    db: &Database,
    eco: Ecosystem,
    name: &str,
    version: &str,
    record: &ChangelogRecord,
) -> rusqlite::Result<()> {
    let gz = record.text.as_deref().and_then(compress);
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO release_changelogs
            (ecosystem, package, version, status, file, changelog_gz, reason, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
        params![
            eco.as_str(),
            package_key(eco, name),
            version,
            record.status.as_str(),
            record.file,
            gz,
            record.reason,
        ],
    )?;
    Ok(())
}

/// Drop rows older than [`RETENTION_DAYS`]. Returns how many went.
pub(crate) fn prune(db: &Database) -> rusqlite::Result<usize> {
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    conn.execute(
        "DELETE FROM release_changelogs WHERE fetched_at < datetime('now', ?1)",
        params![format!("-{RETENTION_DAYS} days")],
    )
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
