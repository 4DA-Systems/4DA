// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which sources the Brief's "worth knowing" section may draw on (AD-054
//! rule 3): social and editorial reading is an opt-in INTEREST, so the
//! section reads only sources that are interests AND enabled
//! (`sources.enabled = 1`). Registries (crates.io, npm, PyPI, Go) and
//! advisories (OSV, CVE) are always on and feed the stack facts sections,
//! never this one.
//!
//! Interim, private classification: AD-054 Wave 2-1 owns the writer for
//! `sources.enabled` and the shared interests classification. Until that
//! lands this list mirrors AD-054's own enumeration; unify the two when it
//! does. A source in neither list (today: `github`) is not an interest, so it
//! is not "worth knowing" material either.

/// AD-054 rule 3's interests, by `source_items.source_type`.
const INTEREST_SOURCE_TYPES: &[&str] = &[
    "hackernews",
    "lobsters",
    "mastodon",
    "devto",
    "dev_to",
    "reddit",
    "lemmy",
    "bluesky",
    "youtube",
    "huggingface",
    "arxiv",
    "papers_with_code",
    "stackoverflow",
    "producthunt",
    "twitter",
    "x",
    "rss",
];

/// Is `source_type` a social/editorial interest (as opposed to a registry,
/// an advisory feed, or an unclassified source)?
pub(crate) fn is_interest_source(source_type: &str) -> bool {
    INTEREST_SOURCE_TYPES.contains(&source_type)
}

/// The interest sources the user has enabled. A source with no `sources`
/// row is not enabled, and a failed read enables nothing: an opt-in must
/// fail toward "off", never toward reading everything.
pub(crate) fn enabled_interest_source_types(conn: &rusqlite::Connection) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare("SELECT source_type FROM sources WHERE enabled = 1") else {
        return Vec::new();
    };
    let mut types: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map(|rows| rows.flatten().filter(|t| is_interest_source(t)).collect())
        .unwrap_or_default();
    types.sort();
    types.dedup();
    types
}

/// The SQL list literal (`'hackernews','rss'`) for an `IN (...)` clause.
/// Every entry is one of [`INTEREST_SOURCE_TYPES`], so no user text reaches
/// the statement.
pub(crate) fn sql_in_list(types: &[String]) -> String {
    types
        .iter()
        .filter(|t| is_interest_source(t))
        .map(|t| format!("'{t}'"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn_with(rows: &[(&str, i64)]) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("memory db");
        conn.execute_batch(
            "CREATE TABLE sources (id INTEGER PRIMARY KEY, source_type TEXT NOT NULL, \
             name TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1)",
        )
        .expect("schema");
        for (t, enabled) in rows {
            conn.execute(
                "INSERT INTO sources (source_type, name, enabled) VALUES (?1, ?1, ?2)",
                rusqlite::params![t, enabled],
            )
            .expect("row");
        }
        conn
    }

    #[test]
    fn registries_and_advisories_are_never_interests() {
        for t in [
            "crates_io",
            "npm_registry",
            "pypi",
            "go_modules",
            "osv",
            "cve",
        ] {
            assert!(!is_interest_source(t), "{t}");
        }
        for t in ["hackernews", "devto", "mastodon", "rss", "arxiv"] {
            assert!(is_interest_source(t), "{t}");
        }
        assert!(
            !is_interest_source("github"),
            "unclassified is not an interest"
        );
    }

    #[test]
    fn only_enabled_interests_are_read() {
        let conn = conn_with(&[
            ("hackernews", 1),
            ("mastodon", 0),
            ("crates_io", 1),
            ("devto", 1),
        ]);
        assert_eq!(
            enabled_interest_source_types(&conn),
            vec!["devto".to_string(), "hackernews".to_string()]
        );
    }

    #[test]
    fn a_missing_table_enables_nothing() {
        let conn = rusqlite::Connection::open_in_memory().expect("memory db");
        assert!(enabled_interest_source_types(&conn).is_empty());
    }

    #[test]
    fn the_sql_list_holds_only_known_interests() {
        let list = sql_in_list(&["hackernews".into(), "x'); DROP TABLE t;--".into()]);
        assert_eq!(list, "'hackernews'");
    }
}
