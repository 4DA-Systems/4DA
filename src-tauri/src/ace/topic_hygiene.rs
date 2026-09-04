// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Which names may be minted as `active_topics` from file content, and the
//! purge that applies the same rule to rows minted before it existed.
//!
//! A topic mined from a source file is only evidence of the user's stack when
//! it names something the user actually depends on. `extract_rust_topics`
//! keeps every `use` first segment, so the user's OWN crate modules minted as
//! topics (live 2026-09-04: `grounding`, `briefing_prompt`, `briefing_slate`,
//! `content_dna_classifiers`, `fourda_lib`, `fourda_macros`); the generic
//! keyword scan minted `kubernetes` from a JSON fixture. The rule, applied at
//! mint time in `context::process_file_changes` and at startup/rescan by
//! [`purge_non_dependency_topics`]: a `file_content` topic must be a known
//! dependency name, a detected-tech name, or one of the pattern labels the
//! rich extractors emit. Nothing else.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::error::{Result, ResultExt};

/// The set of names a file-content topic may take, in normalized form.
pub(crate) struct KnownTopicNames {
    names: HashSet<String>,
    /// How many DEPENDENCY names were loaded. Zero means nothing has been
    /// scanned yet — callers must not purge against an empty vocabulary.
    dependency_count: usize,
}

impl KnownTopicNames {
    /// Load dependency names (`project_dependencies` — every manifest row —
    /// plus direct `user_dependencies`), `detected_tech` names, and the rich
    /// pattern labels. A missing table (a DB that predates one of them)
    /// contributes nothing rather than failing the load.
    pub(crate) fn load(conn: &Connection) -> Self {
        let mut names: HashSet<String> = HashSet::new();
        let mut dependency_count = 0usize;
        for sql in [
            "SELECT package_name FROM project_dependencies",
            "SELECT package_name FROM user_dependencies WHERE is_direct = 1",
        ] {
            for name in column_values(conn, sql) {
                dependency_count += 1;
                // `@tauri-apps/api` is also known by its scope root: that is
                // what the JS extractor emits for a scoped import.
                if let Some(scope) = name.strip_prefix('@').and_then(|s| s.split('/').next()) {
                    names.insert(Self::normalize(scope));
                }
                names.insert(Self::normalize(&name));
            }
        }
        for name in column_values(conn, "SELECT name FROM detected_tech") {
            names.insert(Self::normalize(&name));
        }
        for label in super::watcher::RICH_PATTERN_LABELS {
            names.insert(Self::normalize(label));
        }
        Self {
            names,
            dependency_count,
        }
    }

    /// `serde-json`, `serde_json` and `Serde_JSON` are one name.
    pub(crate) fn normalize(name: &str) -> String {
        name.trim().to_lowercase().replace('-', "_")
    }

    /// May this topic be minted from (or kept as) file content?
    pub(crate) fn allows(&self, topic: &str) -> bool {
        self.names.contains(&Self::normalize(topic))
    }

    /// True once at least one dependency has been scanned.
    pub(crate) fn has_dependencies(&self) -> bool {
        self.dependency_count > 0
    }
}

fn column_values(conn: &Connection, sql: &str) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    stmt.query_map([], |row| row.get::<_, String>(0))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// Startup / post-scan self-heal: delete `active_topics` rows minted from
/// file content whose topic is not a known dependency, detected tech, or
/// rich pattern label. Each deletion is written to the identity ledger with
/// reason `purge_non_dependency_topic`. Returns rows deleted.
///
/// Skipped (returns 0) while NO dependency has been scanned: on a fresh
/// install the vocabulary is empty and every topic would be "unknown" — the
/// scan that follows populates the tables and this runs again after it.
pub fn purge_non_dependency_topics(conn: &Connection) -> Result<usize> {
    let known = KnownTopicNames::load(conn);
    if !known.has_dependencies() {
        tracing::debug!(
            target: "ace::topics",
            "non-dependency topic purge skipped — no dependencies scanned yet"
        );
        return Ok(0);
    }

    let doomed: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, topic FROM active_topics WHERE source = 'file_content'")
            .context("select file_content active_topics")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .context("read active_topics")?;
        rows.flatten()
            .filter(|(_, topic)| !known.allows(topic))
            .collect()
    };

    for (id, topic) in &doomed {
        super::db::record_identity_change(
            conn,
            "topic",
            topic,
            "purge",
            "purge_non_dependency_topic",
            Some(&format!("active_topics.id={id}")),
        );
    }

    let mut deleted = 0usize;
    let ids: Vec<i64> = doomed.iter().map(|(id, _)| *id).collect();
    for chunk in ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!("DELETE FROM active_topics WHERE id IN ({placeholders})");
        deleted += conn
            .execute(&sql, rusqlite::params_from_iter(chunk.iter()))
            .context("delete non-dependency active_topics")?;
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ace::{FileChange, FileChangeType};
    use parking_lot::Mutex;
    use std::sync::Arc;

    /// ACE tables via the real ACE migration, plus the two dependency tables
    /// the main schema owns and the identity ledger.
    fn conn() -> Arc<Mutex<Connection>> {
        crate::register_sqlite_vec_extension();
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        crate::ace::db::migrate(&conn).unwrap();
        conn.lock()
            .execute_batch(
                "CREATE TABLE project_dependencies (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    project_path TEXT NOT NULL,
                    package_name TEXT NOT NULL,
                    language TEXT NOT NULL DEFAULT 'rust'
                );
                CREATE TABLE user_dependencies (
                    id INTEGER PRIMARY KEY,
                    project_path TEXT NOT NULL,
                    package_name TEXT NOT NULL,
                    ecosystem TEXT NOT NULL,
                    is_direct INTEGER DEFAULT 1
                );
                CREATE TABLE identity_ledger (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    entity_kind TEXT NOT NULL,
                    entity_key TEXT NOT NULL,
                    change TEXT NOT NULL,
                    reason TEXT,
                    evidence TEXT,
                    at TEXT NOT NULL DEFAULT (datetime('now'))
                );",
            )
            .unwrap();
        conn
    }

    fn manifest_dep(conn: &Connection, name: &str) {
        conn.execute(
            "INSERT INTO project_dependencies (project_path, package_name) VALUES ('d:/app', ?1)",
            [name],
        )
        .unwrap();
    }

    fn topic(conn: &Connection, name: &str, source: &str) {
        conn.execute(
            "INSERT INTO active_topics (topic, source) VALUES (?1, ?2)",
            [name, source],
        )
        .unwrap();
    }

    fn topics(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT topic FROM active_topics ORDER BY topic")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .flatten()
            .collect()
    }

    #[test]
    fn own_crate_modules_are_not_known_names_but_dependencies_are() {
        let shared = conn();
        let c = shared.lock();
        manifest_dep(&c, "tokio");
        manifest_dep(&c, "serde-json");
        c.execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct)
             VALUES ('d:/app', '@tauri-apps/api', 'javascript', 1),
                    ('d:/app', 'transitive_only', 'rust', 0)",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO detected_tech (name, category, source) VALUES ('Tauri', 'framework', 'manifest')",
            [],
        )
        .unwrap();

        let known = KnownTopicNames::load(&c);
        assert!(known.has_dependencies());
        assert!(known.allows("tokio"));
        assert!(known.allows("serde_json"), "hyphen/underscore fold");
        assert!(known.allows("tauri"), "detected tech, case-folded");
        assert!(known.allows("tauri-apps"), "scoped package root");
        assert!(known.allows("error_handling"), "rich pattern label");
        for own_module in [
            "briefing_prompt",
            "grounding",
            "fourda_lib",
            "fourda_macros",
        ] {
            assert!(
                !known.allows(own_module),
                "{own_module} is the user's own code"
            );
        }
        assert!(
            !known.allows("transitive_only"),
            "only DIRECT user_dependencies count"
        );
        assert!(!known.allows("kubernetes"));
    }

    /// End-to-end through `process_file_changes`: a Rust file importing a
    /// real dependency and a sibling module of its own crate.
    #[test]
    fn process_file_changes_mints_dependencies_only() {
        let shared = conn();
        manifest_dep(&shared.lock(), "tokio");
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(
            &file,
            "use tokio::runtime::Runtime;\nuse briefing_prompt::build;\nuse crate::grounding;\n",
        )
        .unwrap();

        crate::ace::context::process_file_changes(
            &shared,
            &[FileChange {
                path: file,
                change_type: FileChangeType::Modified,
            }],
        )
        .unwrap();

        let c = shared.lock();
        let minted = topics(&c);
        assert!(minted.contains(&"tokio".to_string()), "got {minted:?}");
        assert!(
            !minted.contains(&"briefing_prompt".to_string()),
            "got {minted:?}"
        );
        assert!(!minted.contains(&"grounding".to_string()), "got {minted:?}");
        // The file itself is still recorded as a signal.
        let signals: i64 = c
            .query_row("SELECT COUNT(*) FROM file_signals", [], |r| r.get(0))
            .unwrap();
        assert_eq!(signals, 1);
    }

    #[test]
    fn purge_removes_fixture_minted_topics_and_keeps_dependencies() {
        let shared = conn();
        let c = shared.lock();
        manifest_dep(&c, "serde");
        c.execute(
            "INSERT INTO detected_tech (name, category, source) VALUES ('docker', 'tool', 'manifest')",
            [],
        )
        .unwrap();
        // Minted from benchmark_scenarios.json by the old generic scan.
        topic(&c, "kubernetes", "file_content");
        topic(&c, "grpc", "file_content");
        // Own-crate module minted by extract_rust_topics.
        topic(&c, "briefing_slate", "file_content");
        // Legitimate: a dependency, a detected tech, a rich pattern label.
        topic(&c, "serde", "file_content");
        topic(&c, "docker", "file_content");
        topic(&c, "async_runtime", "file_content");
        // Other sources are never this purge's business.
        topic(&c, "rust", "git_commit");
        topic(&c, "machine-learning", "learning_trajectory");

        let deleted = purge_non_dependency_topics(&c).unwrap();
        assert_eq!(deleted, 3, "kubernetes + grpc + briefing_slate");
        assert_eq!(
            topics(&c),
            vec![
                "async_runtime",
                "docker",
                "machine-learning",
                "rust",
                "serde"
            ]
        );

        let ledger: Vec<(String, String)> = c
            .prepare(
                "SELECT entity_key, reason FROM identity_ledger WHERE change = 'purge' ORDER BY entity_key",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(
            ledger,
            vec![
                (
                    "briefing_slate".to_string(),
                    "purge_non_dependency_topic".to_string()
                ),
                ("grpc".to_string(), "purge_non_dependency_topic".to_string()),
                (
                    "kubernetes".to_string(),
                    "purge_non_dependency_topic".to_string()
                ),
            ]
        );
    }

    /// A fresh install has scanned nothing; purging against an empty
    /// vocabulary would delete every topic. Skip until the scan lands.
    #[test]
    fn purge_is_skipped_until_dependencies_exist() {
        let shared = conn();
        let c = shared.lock();
        topic(&c, "kubernetes", "file_content");
        assert_eq!(purge_non_dependency_topics(&c).unwrap(), 0);
        assert_eq!(topics(&c), vec!["kubernetes"]);
    }
}
