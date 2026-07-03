// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency query methods on `impl Database`: storage, retrieval, scanned deps,
//! relevant deps, and cross-project queries.

use rusqlite::{params, Result as SqliteResult};

use crate::db::Database;

use crate::ace::scanner::DependencyEdge;

use super::mappers::map_dependency_row;
use super::types::{CrossProjectPackage, DependencyEdgeRow, StoredDependency};

/// Hard project exclusion, applied at WRITE time to every dependency table
/// and mirrored by the `get_auditable_*` read filters.
///
/// Delegates to the canonical policy (`crate::project_inclusion`): tier 1 is
/// agent infrastructure / ephemeral paths (the ENTIRE `.claude/` and `.codex/`
/// trees — worktrees AND scratch fixtures like `.claude/plans/ledger-fixtures/`
/// whose Gemfile.lock / composer.lock surfaced nokogiri + symfony as the
/// user's stack, producing phantom Ruby/PHP CVE alerts on the Preemption
/// Radar — plus temp dirs); tier 2 is generic non-project scaffolding
/// (fixture-tree segments, `-placeholder` dirs), waived in strict-manifest
/// (ledger) mode. Case-insensitive, both slash styles (paths reach here in
/// raw `D:\...` and canonicalized `d:/...` forms).
pub(crate) fn is_excluded_project_path(project_path: &str) -> bool {
    crate::project_inclusion::is_hard_excluded(project_path)
}

/// Counts from a [`purge_agent_infra_dependencies`] self-heal pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgentInfraPurge {
    /// `.claude`/`.codex`/temp rows deleted from `user_dependencies`.
    pub user_dependencies: usize,
    /// `.claude`/`.codex`/temp rows deleted from `dependency_snapshots`.
    pub dependency_snapshots: usize,
    /// Duplicate-identity rows collapsed (slash-style/case path variants).
    pub duplicates: usize,
    /// Residual rows whose `project_path` was rewritten to the canonical form.
    pub canonicalized: usize,
}

impl AgentInfraPurge {
    pub fn total(&self) -> usize {
        self.user_dependencies + self.dependency_snapshots + self.duplicates
    }
}

/// Self-heal purge for dependency tables polluted by agent infrastructure.
///
/// Mirrors the `project_dependencies` startup purge precedent (app_setup):
/// existing installs carry rows scanned before the write-time exclusion
/// existed, so deleting at startup heals the live DB without manual surgery.
///
/// Three passes (single transaction — a startup crash mid-purge must not
/// leave a half-deduped table):
/// 1. Delete `.claude/` + `.codex/` rows from `user_dependencies` and
///    `dependency_snapshots` (SQLite `LIKE` is ASCII case-insensitive, so the
///    two slash-style patterns cover all four case/slash variants). The old
///    `%worktrees%agent-%` pattern was DROPPED: as a substring match it
///    deleted legitimate projects (`/home/u/worktrees/reagent-app`,
///    `D:\worktrees\agent-ui`) every startup while scans re-added them
///    (flip-flop churn) — and agent worktrees always live under
///    `.claude/worktrees/`, already covered by the segment patterns.
/// 2. Collapse duplicate identities: migration 67 canonicalized
///    `project_dependencies` paths (lowercase, forward slashes) but never
///    backfilled `user_dependencies`, so when write-time canonicalization
///    landed, every re-scanned project gained a second row set
///    (`D:\4DA\...` from before, `d:/4da/...` after). The group key MUST
///    mirror `canonicalize_project_path`: lowercasing is Windows-only — on
///    Linux `/home/u/App` and `/home/u/app` are genuinely distinct projects
///    and must NOT collapse. The hyphen->underscore package-name fold is only
///    valid for ecosystems that treat them as equivalent (crates.io, PyPI);
///    npm's `left-pad` and `left_pad` are DIFFERENT packages, so the fold is
///    restricted to rust/python ecosystem labels.
/// 3. Rewrite residual non-canonical paths to the canonical form (matches
///    `canonicalize_project_path`; lowercasing is Windows-only). Safe after
///    pass 2: each normalized identity now has exactly one row, so the
///    UNIQUE(project_path, package_name, ecosystem) key cannot collide.
pub fn purge_agent_infra_dependencies(
    conn: &rusqlite::Connection,
) -> SqliteResult<AgentInfraPurge> {
    const INFRA_PREDICATE: &str = "project_path LIKE '%/.claude/%' \
         OR project_path LIKE '%\\.claude\\%' \
         OR project_path LIKE '%/.codex/%' \
         OR project_path LIKE '%\\.codex\\%'";

    let tx = conn.unchecked_transaction()?;

    let user_dependencies = tx.execute(
        &format!("DELETE FROM user_dependencies WHERE {INFRA_PREDICATE}"),
        [],
    )?;
    let dependency_snapshots = tx.execute(
        &format!("DELETE FROM dependency_snapshots WHERE {INFRA_PREDICATE}"),
        [],
    )?;

    // Collapse slash-style (and on Windows, case) duplicates, keeping the
    // most recent row. Group key mirrors canonicalize_project_path exactly.
    let path_key = if cfg!(windows) {
        "LOWER(REPLACE(project_path, '\\', '/'))"
    } else {
        "REPLACE(project_path, '\\', '/')"
    };
    // Hyphen/underscore are interchangeable on crates.io and PyPI only.
    const NAME_KEY: &str = "CASE WHEN LOWER(ecosystem) IN \
            ('rust', 'cargo', 'crates.io', 'python', 'pypi', 'pip') \
         THEN LOWER(REPLACE(package_name, '-', '_')) \
         ELSE LOWER(package_name) END";
    let duplicates = tx.execute(
        &format!(
            "DELETE FROM user_dependencies WHERE rowid NOT IN (
                SELECT MAX(rowid) FROM user_dependencies
                GROUP BY {NAME_KEY}, {path_key}, LOWER(ecosystem)
            )"
        ),
        [],
    )?;

    // Rewrite survivors onto the canonical key so path-scoped readers (which
    // canonicalize their query path) can see them again.
    let canonicalized = tx.execute(
        &format!(
            "UPDATE user_dependencies SET project_path = {path_key}
             WHERE project_path <> {path_key}"
        ),
        [],
    )?;

    tx.commit()?;

    Ok(AgentInfraPurge {
        user_dependencies,
        dependency_snapshots,
        duplicates,
        canonicalized,
    })
}

/// Counts from a [`purge_non_project_intelligence`] self-heal pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct NonProjectPurge {
    /// Hard-excluded rows deleted from `detected_projects`.
    pub detected_projects: usize,
    /// Hard-excluded rows deleted from `project_dependencies`.
    pub project_dependencies: usize,
    /// Hard-excluded rows deleted from `user_dependencies` (tier-2 sweep; the
    /// tier-1 SQL pass in [`purge_agent_infra_dependencies`] runs first).
    pub user_dependencies: usize,
    /// Hard-excluded rows deleted from `dependency_snapshots`.
    pub dependency_snapshots: usize,
    /// `detected_tech` rows deleted because EVERY evidence entry referenced a
    /// hard-excluded path.
    pub detected_tech_deleted: usize,
    /// `detected_tech` rows whose evidence was rewritten to drop entries
    /// referencing hard-excluded paths (real-project evidence kept).
    pub detected_tech_rewritten: usize,
}

impl NonProjectPurge {
    pub fn total(&self) -> usize {
        self.detected_projects
            + self.project_dependencies
            + self.user_dependencies
            + self.dependency_snapshots
            + self.detected_tech_deleted
    }
}

/// True when `table` exists in the connected database. The ACE tables
/// (`detected_projects`, `detected_tech`) are created by the ACE migration,
/// which may not have run yet on a brand-new install when startup cleanup
/// fires — skip gracefully instead of erroring.
fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

/// Delete all rows of `table` whose `path_col` is hard-excluded (tiers 1+2 of
/// the canonical project-inclusion policy). Predicate evaluated in Rust so the
/// SQL can never drift from `project_inclusion` — the June 2026 pollution fix
/// purged `project_dependencies` but missed `detected_projects` precisely
/// because each purge hand-rolled its own SQL patterns.
fn purge_hard_excluded_rows(
    conn: &rusqlite::Connection,
    table: &str,
    path_col: &str,
) -> SqliteResult<usize> {
    if !table_exists(conn, table) {
        return Ok(0);
    }
    let paths: Vec<String> = conn
        .prepare(&format!("SELECT DISTINCT {path_col} FROM {table}"))?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(std::result::Result::ok)
        .collect();
    let mut deleted = 0usize;
    for path in paths
        .iter()
        .filter(|p| crate::project_inclusion::is_hard_excluded(p))
    {
        deleted += conn.execute(
            &format!("DELETE FROM {table} WHERE {path_col} = ?1"),
            params![path],
        )?;
    }
    Ok(deleted)
}

/// Self-heal purge for EVERY intelligence table that keys rows by project
/// path, against the canonical hard-exclusion policy (tiers 1+2:
/// agent infra / temp + non-project scaffolding; strict-manifest mode waives
/// tier 2 so the receipts ledger's fixture stacks survive in ledger data
/// dirs). Extends the #212 precedent to `detected_projects` — which the June
/// 2026 pollution fix never covered, leaving five `.claude/plans/
/// ledger-fixtures/*` rows surfacing in the "Your Stack" list — plus
/// `project_dependencies`, the tier-2 sweep of `user_dependencies` /
/// `dependency_snapshots`, and `detected_tech` evidence hygiene. Idempotent;
/// single transaction.
pub fn purge_non_project_intelligence(
    conn: &rusqlite::Connection,
) -> SqliteResult<NonProjectPurge> {
    let tx = conn.unchecked_transaction()?;
    let mut counts = NonProjectPurge {
        detected_projects: purge_hard_excluded_rows(&tx, "detected_projects", "path")?,
        project_dependencies: purge_hard_excluded_rows(
            &tx,
            "project_dependencies",
            "project_path",
        )?,
        user_dependencies: purge_hard_excluded_rows(&tx, "user_dependencies", "project_path")?,
        dependency_snapshots: purge_hard_excluded_rows(
            &tx,
            "dependency_snapshots",
            "project_path",
        )?,
        ..Default::default()
    };

    // detected_tech accumulates manifest-path evidence strings ("Found in
    // <path>"; "; "-joined). Drop entries that reference hard-excluded paths;
    // delete the row outright when nothing legitimate remains (its tech was
    // only ever evidenced by scaffolding).
    if table_exists(&tx, "detected_tech") {
        let rows: Vec<(i64, String)> = tx
            .prepare("SELECT id, evidence FROM detected_tech WHERE evidence IS NOT NULL")?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(std::result::Result::ok)
            .collect();
        for (id, evidence) in rows {
            let parts: Vec<&str> = evidence.split("; ").collect();
            let kept: Vec<&str> = parts
                .iter()
                .filter(|e| !crate::project_inclusion::is_hard_excluded(e))
                .copied()
                .collect();
            if kept.len() == parts.len() {
                continue;
            }
            if kept.is_empty() {
                tx.execute("DELETE FROM detected_tech WHERE id = ?1", params![id])?;
                counts.detected_tech_deleted += 1;
            } else {
                tx.execute(
                    "UPDATE detected_tech SET evidence = ?1 WHERE id = ?2",
                    params![kept.join("; "), id],
                )?;
                counts.detected_tech_rewritten += 1;
            }
        }
    }

    tx.commit()?;
    Ok(counts)
}

/// Canonicalize a project path for storage + the `ON CONFLICT` key. MUST match
/// `temporal::canonicalize_project_path` so `user_dependencies` rows land on the SAME
/// key as the `project_dependencies` rows written there. Without this, the manifest
/// scan (which stores the canonical path via `store_direct_dependencies`) and the
/// lockfile processors (which pass the RAW `dir.to_string_lossy()` scan path) write TWO
/// rows for one dependency — a null-version row on the canonical path and a versioned
/// row on the raw path — across every ecosystem. Pure string normalization (no fs
/// access), so it is deterministic on synthetic/test paths.
fn canonicalize_project_path(project_path: &str) -> String {
    crate::project_inclusion::canonical_storage_path(project_path)
}

/// Post-query filter applying the FULL canonical inclusion policy (tiers
/// 1+2 hard exclusion + tier-3 "Your Stack" user exclusions) to dependency
/// rows headed for intelligence surfaces (OSV matching/sync/cache, local
/// audit, dependency health). The SQL `NOT LIKE` clauses in the queries only
/// cover tier-1 patterns; this closes tier 2 and tier 3 — including stale
/// rows written before the write-time guards existed.
fn retain_included(deps: Vec<StoredDependency>) -> Vec<StoredDependency> {
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    deps.into_iter()
        .filter(|d| {
            !crate::project_inclusion::is_excluded_from_intelligence(
                &d.project_path,
                &user_excluded,
            )
        })
        .collect()
}

impl Database {
    /// Store (upsert) a dependency discovered by ACE scanner.
    pub fn store_dependency(
        &self,
        project_path: &str,
        package_name: &str,
        version: Option<&str>,
        ecosystem: &str,
        is_dev: bool,
        license: Option<&str>,
    ) -> SqliteResult<()> {
        // Agent worktrees / scratch fixtures / temp clones are not the user's
        // projects; storing them creates phantom CVE alerts (see
        // is_excluded_project_path). Silent no-op, mirroring store_dependency_edges.
        if is_excluded_project_path(project_path) {
            return Ok(());
        }
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, license, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = ?5,
                license = COALESCE(?6, user_dependencies.license),
                last_seen_at = datetime('now')",
            params![project_path, package_name, version, ecosystem, is_dev as i32, license],
        )?;
        Ok(())
    }

    /// Store (upsert) a transitive dependency discovered from a lockfile.
    /// Sets `is_direct = 0` for new entries. On conflict, preserves existing
    /// `is_direct` value (so direct deps from manifests are not downgraded).
    /// Lockfile version is preferred (it's the actual resolved/installed version).
    pub fn store_transitive_dependency(
        &self,
        project_path: &str,
        package_name: &str,
        version: Option<&str>,
        ecosystem: &str,
        is_dev: bool,
    ) -> SqliteResult<()> {
        // Same agent-infra guard as store_dependency: the lockfile walk feeds
        // this with raw scan-dir paths, including `.claude/` fixtures/worktrees.
        if is_excluded_project_path(project_path) {
            return Ok(());
        }
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = MIN(user_dependencies.is_dev, ?5),
                last_seen_at = datetime('now')",
            params![project_path, package_name, version, ecosystem, is_dev as i32],
        )?;
        Ok(())
    }

    /// Timestamp of the most recent ACE scan, as the freshness signal for the headless
    /// dep-scan gate. `detected_projects.updated_at` is DO-UPDATEd on every scan (by
    /// `upsert_detected_project`) for every detected project — including ones with no
    /// recognized dependencies — so it advances each scan where `project_dependencies`
    /// would stay null. Returns `None` when nothing has ever been scanned (cold start).
    pub fn last_ace_scan_time(&self) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row("SELECT MAX(updated_at) FROM detected_projects", [], |row| {
            row.get::<_, Option<String>>(0)
        })
    }

    /// Get all dependencies for a specific project.
    pub fn get_project_dependencies(
        &self,
        project_path: &str,
    ) -> SqliteResult<Vec<StoredDependency>> {
        // Canonicalize the query path to match the canonical key stored by
        // store_dependency / store_transitive_dependency (a UI/raw caller path like
        // `D:\proj` must still find the canonical `d:/proj` rows).
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )?;

        let rows = stmt.query_map(params![project_path], map_dependency_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get all tracked dependencies across all projects.
    pub fn get_all_user_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             ORDER BY ecosystem, package_name",
        )?;

        let rows = stmt.query_map([], map_dependency_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get dependencies suitable for security auditing.
    ///
    /// Includes direct, transitive, runtime, and dev dependencies, while excluding
    /// ephemeral worktrees and temp clones that would duplicate findings.
    pub fn get_auditable_user_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             WHERE project_path NOT LIKE '%/.claude/%'
               AND project_path NOT LIKE '%\\.claude\\%'
               AND project_path NOT LIKE '%/.codex/%'
               AND project_path NOT LIKE '%\\.codex\\%'
               AND project_path NOT LIKE '%/tmp/%'
               AND project_path NOT LIKE '%\\tmp\\%'
               AND project_path NOT LIKE '%AppData%Local%Temp%'
             ORDER BY ecosystem, package_name",
        )?;

        let rows = stmt.query_map([], map_dependency_row)?;
        Ok(retain_included(
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in auditable dependencies: {e}");
                    None
                }
            })
            .collect(),
        ))
    }

    /// Get user dependencies filtered to relevant runtime deps only.
    ///
    /// Excludes dev deps, transitive deps, and worktree paths to prevent
    /// inflated advisory matches from agent-generated worktree copies.
    pub fn get_relevant_user_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             WHERE is_dev = 0 AND is_direct = 1
               AND project_path NOT LIKE '%/.claude/%'
               AND project_path NOT LIKE '%\\.claude\\%'
               AND project_path NOT LIKE '%/.codex/%'
               AND project_path NOT LIKE '%\\.codex\\%'
             ORDER BY ecosystem, package_name",
        )?;

        let rows = stmt.query_map([], map_dependency_row)?;
        Ok(retain_included(
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect(),
        ))
    }

    /// Get all ACE-scanned dependencies from `project_dependencies`.
    /// Returns them as `StoredDependency` for compatibility with OSV matching.
    /// Maps `language` -> `ecosystem` and `last_scanned` -> `detected_at`/`last_seen_at`.
    pub fn get_all_scanned_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, language, is_dev, is_direct, last_scanned
             FROM project_dependencies
             ORDER BY language, package_name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(StoredDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                package_name: row.get(2)?,
                version: row.get(3)?,
                ecosystem: row.get::<_, String>(4)?,
                is_dev: row.get::<_, bool>(5)?,
                is_direct: row.get::<_, bool>(6)?,
                detected_at: row.get::<_, String>(7)?,
                last_seen_at: row.get::<_, String>(7)?,
                license: None,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in scanned dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get ACE-scanned dependencies suitable for security auditing.
    ///
    /// Includes all dependency scopes, but keeps the project-hygiene and
    /// relevance filters used by user-facing intelligence.
    pub fn get_auditable_scanned_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();

        let has_direct = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'is_direct'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        let has_relevance = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'project_relevance'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let direct_col = if has_direct {
            "is_direct"
        } else {
            "1 as is_direct"
        };
        let relevance_clause = if has_relevance {
            "AND project_relevance >= 0.15"
        } else {
            ""
        };
        let sql = format!(
            "SELECT id, project_path, package_name, version, language, is_dev, {direct_col}, last_scanned
             FROM project_dependencies
             WHERE project_path NOT LIKE '%/.claude/%'
               AND project_path NOT LIKE '%\\.claude\\%'
               AND project_path NOT LIKE '%/.codex/%'
               AND project_path NOT LIKE '%\\.codex\\%'
               AND project_path NOT LIKE '%/tmp/%'
               AND project_path NOT LIKE '%\\tmp\\%'
               AND project_path NOT LIKE '%AppData%Local%Temp%'
               {relevance_clause}
             ORDER BY language, package_name"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                package_name: row.get(2)?,
                version: row.get(3)?,
                ecosystem: row.get::<_, String>(4)?,
                is_dev: row.get::<_, bool>(5)?,
                is_direct: row.get::<_, bool>(6)?,
                detected_at: row.get::<_, String>(7)?,
                last_seen_at: row.get::<_, String>(7)?,
                license: None,
            })
        })?;

        Ok(retain_included(
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in auditable scanned dependencies: {e}");
                    None
                }
            })
            .collect(),
        ))
    }

    /// Get ACE-scanned dependencies filtered to relevant runtime deps only.
    ///
    /// Excludes dev deps, transitive deps, and low-relevance projects
    /// (example/demo/test directories). Falls back gracefully if `is_direct`
    /// or `project_relevance` columns don't exist in older databases.
    pub fn get_relevant_scanned_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();

        // Check which filter columns exist (Phase 53 added is_direct, Phase 55 added project_relevance)
        let has_direct = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'is_direct'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let has_relevance = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'project_relevance'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let direct_clause = if has_direct { "AND is_direct = 1" } else { "" };
        let relevance_clause = if has_relevance {
            "AND project_relevance >= 0.15"
        } else {
            ""
        };
        let direct_col = if has_direct {
            "is_direct"
        } else {
            "1 as is_direct"
        };

        let sql = format!(
            "SELECT id, project_path, package_name, version, language, is_dev, {direct_col}, last_scanned
             FROM project_dependencies
             WHERE is_dev = 0
               {direct_clause}
               {relevance_clause}
             ORDER BY language, package_name"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                package_name: row.get(2)?,
                version: row.get(3)?,
                ecosystem: row.get::<_, String>(4)?,
                is_dev: false,
                is_direct: row.get::<_, bool>(6)?,
                detected_at: row.get::<_, String>(7)?,
                last_seen_at: row.get::<_, String>(7)?,
                license: None,
            })
        })?;

        Ok(retain_included(
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in relevant scanned dependencies: {e}");
                    None
                }
            })
            .collect(),
        ))
    }

    /// Get packages that appear in multiple projects (cross-project insight).
    pub fn get_cross_project_packages(&self) -> SqliteResult<Vec<CrossProjectPackage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT package_name, ecosystem, COUNT(DISTINCT project_path) as project_count,
                    GROUP_CONCAT(DISTINCT project_path) as projects
             FROM user_dependencies
             GROUP BY package_name, ecosystem
             HAVING project_count > 1
             ORDER BY project_count DESC, package_name",
        )?;

        let rows = stmt.query_map([], |row| {
            let projects_str: String = row.get(3)?;
            let projects: Vec<String> = projects_str
                .split(',')
                .map(std::string::ToString::to_string)
                .collect();
            Ok(CrossProjectPackage {
                package_name: row.get(0)?,
                ecosystem: row.get(1)?,
                project_count: row.get(2)?,
                projects,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Bulk-insert parent->child dependency edges for a project (Step 1:
    /// reachability foundation). Runs in a single transaction. Skips ephemeral
    /// worktree/temp paths (same exclusion as the auditable-dependency queries).
    /// Returns the number of edges inserted (0 for excluded/empty input).
    pub(crate) fn store_dependency_edges(
        &self,
        project_path: &str,
        ecosystem: &str,
        edges: &[DependencyEdge],
    ) -> SqliteResult<usize> {
        if edges.is_empty() || is_excluded_project_path(project_path) {
            return Ok(0);
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut inserted = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO dependency_edges
                     (project_path, ecosystem, parent_package, parent_version,
                      child_package, child_version, scope, detected_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            )?;
            for edge in edges {
                stmt.execute(params![
                    project_path,
                    ecosystem,
                    edge.parent,
                    edge.parent_version,
                    edge.child,
                    edge.child_version,
                    edge.scope.as_str(),
                ])?;
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Get all stored dependency edges for a project.
    pub fn get_dependency_edges(&self, project_path: &str) -> SqliteResult<Vec<DependencyEdgeRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, ecosystem, parent_package, parent_version,
                    child_package, child_version, scope, detected_at
             FROM dependency_edges
             WHERE project_path = ?1
             ORDER BY parent_package, child_package",
        )?;

        let rows = stmt.query_map(params![project_path], |row| {
            Ok(DependencyEdgeRow {
                id: row.get(0)?,
                project_path: row.get(1)?,
                ecosystem: row.get(2)?,
                parent_package: row.get(3)?,
                parent_version: row.get(4)?,
                child_package: row.get(5)?,
                child_version: row.get(6)?,
                scope: row.get(7)?,
                detected_at: row.get(8)?,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependency edges: {e}");
                    None
                }
            })
            .collect())
    }
}
