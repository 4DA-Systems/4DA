// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cache for the opt-in GitHub release-notes fallback: one row per
//! (ecosystem, package, from, to, parser version).
//!
//! Like `store.rs`, the table is created on first use (`CREATE TABLE IF NOT
//! EXISTS`), not by a versioned migration, so the schema version is untouched
//! and an older binary can still open the database. The parser version is in
//! the key: a change to tag matching or body parsing makes old rows unread,
//! and the prune drops them. Only answers are written — a rate limit or a
//! network failure never reaches this table.
//!
//! Found notes are kept for [`RETENTION_DAYS`]; a "nothing found" row only
//! for [`MISS_TTL_HOURS`], because a release's notes are often published on
//! GitHub after the registry announces the version.

use std::io::{Read, Write};

use rusqlite::{params, Connection, OptionalExtension};

use super::archive::MAX_FILE_BYTES;
use super::github::{GithubNotes, ReleaseNote, GITHUB_PARSER_VERSION};
use super::store::package_key;
use super::ReleaseTarget;
use crate::db::Database;

const RETENTION_DAYS: i64 = 120;
const MISS_TTL_HOURS: i64 = 24;

const TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS release_notes_github (
    ecosystem TEXT NOT NULL,
    package TEXT NOT NULL,
    from_version TEXT NOT NULL,
    to_version TEXT NOT NULL,
    parser_version INTEGER NOT NULL,
    repo TEXT,
    notes_gz BLOB,
    reached_from INTEGER NOT NULL DEFAULT 0,
    reason TEXT,
    found INTEGER NOT NULL DEFAULT 0,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (ecosystem, package, from_version, to_version, parser_version)
)";

fn ensure_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(TABLE_SQL)
}

fn compress(notes: &[ReleaseNote]) -> Option<Vec<u8>> {
    let json = serde_json::to_vec(notes).ok()?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&json).ok()?;
    enc.finish().ok()
}

fn decompress(gz: &[u8]) -> Option<Vec<ReleaseNote>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(gz)
        .take(MAX_FILE_BYTES as u64 * 8)
        .read_to_end(&mut out)
        .ok()?;
    serde_json::from_slice(&out).ok()
}

/// The cached notes for `t` under the current parser, if still fresh.
pub(crate) fn get(db: &Database, t: &ReleaseTarget) -> rusqlite::Result<Option<GithubNotes>> {
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    type Row = (Option<String>, Option<Vec<u8>>, bool, Option<String>);
    let row: Option<Row> = conn
        .query_row(
            "SELECT repo, notes_gz, reached_from, reason FROM release_notes_github
             WHERE ecosystem = ?1 AND package = ?2 AND from_version = ?3
               AND to_version = ?4 AND parser_version = ?5
               AND (found = 1 OR fetched_at >= datetime('now', ?6))",
            params![
                t.eco.as_str(),
                package_key(t.eco, &t.package),
                t.from,
                t.to,
                GITHUB_PARSER_VERSION,
                format!("-{MISS_TTL_HOURS} hours"),
            ],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(row.map(|(repo, gz, reached_from, reason)| GithubNotes {
        repo,
        notes: gz.as_deref().and_then(decompress).unwrap_or_default(),
        reached_from,
        reason,
    }))
}

/// Cache one lookup's answer (replacing any earlier one).
pub(crate) fn put(db: &Database, t: &ReleaseTarget, notes: &GithubNotes) -> rusqlite::Result<()> {
    let gz = (!notes.notes.is_empty())
        .then(|| compress(&notes.notes))
        .flatten();
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO release_notes_github
            (ecosystem, package, from_version, to_version, parser_version,
             repo, notes_gz, reached_from, reason, found, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
        params![
            t.eco.as_str(),
            package_key(t.eco, &t.package),
            t.from,
            t.to,
            GITHUB_PARSER_VERSION,
            notes.repo,
            gz,
            notes.reached_from,
            notes.reason,
            !notes.notes.is_empty(),
        ],
    )?;
    Ok(())
}

/// Drop rows past retention, expired misses, and rows of an older parser.
pub(crate) fn prune(db: &Database) -> rusqlite::Result<usize> {
    let conn = db.conn.lock();
    ensure_table(&conn)?;
    conn.execute(
        "DELETE FROM release_notes_github
         WHERE parser_version != ?1
            OR fetched_at < datetime('now', ?2)
            OR (found = 0 AND fetched_at < datetime('now', ?3))",
        params![
            GITHUB_PARSER_VERSION,
            format!("-{RETENTION_DAYS} days"),
            format!("-{MISS_TTL_HOURS} hours"),
        ],
    )
}
