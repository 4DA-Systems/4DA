// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency query methods on `impl Database`: storage, retrieval, scanned deps,
//! relevant deps, and cross-project queries.

use rusqlite::{params, Result as SqliteResult};

use crate::db::Database;

use crate::ace::scanner::DependencyEdge;

use super::mappers::map_dependency_row;
use super::types::{
    CrossProjectPackage, DependencyEdgeRow, DependencyInstanceInput, DependencyInstanceRow,
    StoredDependency,
};

/// Hard project exclusion, applied at WRITE time to every dependency table
/// and mirrored by the `get_auditable_*` read filters.
///
/// Delegates to the canonical policy (`crate::project_inclusion`): tier 1 is
/// agent infrastructure / ephemeral paths (the ENTIRE `.claude/` and `.codex/`
/// trees — worktrees AND scratch fixtures like `.claude/plans/ledger-fixtures/`
/// whose Gemfile.lock / composer.lock surfaced nokogiri + symfony as the
/// user's stack, producing phantom Ruby/PHP CVE alerts on the Preemption
/// Radar — plus temp dirs); tier 2 is generic non-project scaffolding
/// (fixture-tree segments, `-placeholder` dirs), waived only in ledger runs
/// (strict-manifest mode + isolated data dir). Case-insensitive, both slash
/// styles (paths reach here in raw `D:\...` and canonicalized `d:/...`
/// forms). Tier-2 rejections log once per path (accurate-first — a silent
/// permanent exclusion is not acceptable); this fn is only called at write
/// sites, so the log marks real ingestion refusals.
pub(crate) fn is_excluded_project_path(project_path: &str) -> bool {
    let excluded = crate::project_inclusion::is_hard_excluded(project_path);
    if excluded {
        crate::project_inclusion::log_tier2_exclusion(project_path, "write");
    }
    excluded
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
    /// Store (upsert) a dependency confirmed by a LOCKFILE walk (a direct dep
    /// whose resolved version the lockfile provides). Kept name for history;
    /// the manifest-scan sync uses [`Self::store_manifest_dependency`].
    ///
    /// Provenance: inserts as 'lockfile'; on conflict only UPGRADES a legacy
    /// 'unknown' to 'lockfile' — a 'manifest'/'import_scrape' label from the
    /// scan sync is more specific about declaration and must not be
    /// overwritten by a version update.
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
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, license, detected_from, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 'lockfile', datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = ?5,
                license = COALESCE(?6, user_dependencies.license),
                detected_from = CASE WHEN user_dependencies.detected_from = 'unknown'
                                     THEN 'lockfile' ELSE user_dependencies.detected_from END,
                last_seen_at = datetime('now')",
            params![project_path, package_name, version, ecosystem, is_dev as i32, license],
        )?;
        Ok(())
    }

    /// Store (upsert) a dependency observed by the MANIFEST scan sync
    /// (`store_direct_dependencies` copying `project_dependencies` rows).
    ///
    /// The manifest scan is AUTHORITATIVE for `is_direct` in BOTH directions:
    /// a module that moved to go.mod's `// indirect` set is downgraded here
    /// (the old sync routed indirect rows through
    /// [`Self::store_transitive_dependency`], whose conflict path preserves
    /// `is_direct`, so pre-fix rows stored direct could never heal).
    /// `detected_from` is propagated from the `project_dependencies` row
    /// ('manifest' | 'import_scrape' | legacy 'unknown').
    #[allow(clippy::too_many_arguments)]
    pub fn store_manifest_dependency(
        &self,
        project_path: &str,
        package_name: &str,
        version: Option<&str>,
        ecosystem: &str,
        is_dev: bool,
        is_direct: bool,
        detected_from: &str,
    ) -> SqliteResult<()> {
        if is_excluded_project_path(project_path) {
            return Ok(());
        }
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_from, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = ?5,
                is_direct = ?6,
                detected_from = ?7,
                last_seen_at = datetime('now')",
            params![
                project_path,
                package_name,
                version,
                ecosystem,
                is_dev as i32,
                is_direct as i32,
                detected_from
            ],
        )?;
        Ok(())
    }

    /// Store (upsert) a transitive dependency discovered from a lockfile.
    /// Sets `is_direct = 0` for new entries. On conflict, preserves existing
    /// `is_direct` value (so direct deps from manifests are not downgraded).
    /// Lockfile version is preferred (it's the actual resolved/installed version).
    /// Provenance mirrors [`Self::store_dependency`]: insert as 'lockfile',
    /// upgrade only legacy 'unknown' on conflict.
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
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_from, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 'lockfile', datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = MIN(user_dependencies.is_dev, ?5),
                detected_from = CASE WHEN user_dependencies.detected_from = 'unknown'
                                     THEN 'lockfile' ELSE user_dependencies.detected_from END,
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

    /// Bulk-replace the installed-instance inventory for one
    /// `(project_path, ecosystem)` (`dependency_instances`, Phase 92).
    ///
    /// DELETE-then-insert in a single transaction so a rescan REFRESHES the set
    /// rather than accumulating stale versions (a package upgraded away must not
    /// linger as a phantom vulnerable instance — the opposite failure to the
    /// collapse this table fixes). The `UNIQUE(project, ecosystem, package,
    /// version)` key permits the same package at multiple versions;
    /// `INSERT OR IGNORE` dedups an exact `(package, version)` repeated within a
    /// lockfile. Skips excluded paths and canonicalizes exactly like
    /// [`Self::store_dependency`], so instances land on the same project key as
    /// the collapsed tables. Returns the number of rows written.
    ///
    /// A directory holding two lockfiles of the SAME ecosystem (e.g. both
    /// `package-lock.json` and `pnpm-lock.yaml`) resolves to the last
    /// processor's set — matching the existing last-write-wins semantics of the
    /// collapsed tables; well-formed projects have one lockfile per ecosystem.
    pub(crate) fn store_dependency_instances(
        &self,
        project_path: &str,
        ecosystem: &str,
        instances: &[DependencyInstanceInput],
    ) -> SqliteResult<usize> {
        if is_excluded_project_path(project_path) {
            return Ok(0);
        }
        let project_path = canonicalize_project_path(project_path);
        // Store the OSV-canonical ecosystem name (npm / crates.io / PyPI / Go /
        // ...), not the ACE language string ("rust"/"javascript"), so the
        // version-confirmed matcher joins advisories (keyed on OSV names)
        // directly. Normalized once here → DELETE and INSERT stay consistent.
        let ecosystem =
            crate::ecosystem::Ecosystem::parse(ecosystem).map_or(ecosystem, |e| e.osv_name());
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM dependency_instances WHERE project_path = ?1 AND ecosystem = ?2",
            params![project_path, ecosystem],
        )?;
        let mut inserted = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO dependency_instances
                     (project_path, ecosystem, package_name, version, is_direct, is_dev, scope, detected_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            )?;
            for inst in instances {
                inserted += stmt.execute(params![
                    project_path,
                    ecosystem,
                    inst.package_name,
                    inst.version,
                    inst.is_direct as i32,
                    inst.is_dev as i32,
                    inst.scope,
                ])?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// All installed instances for a project (every version of every package),
    /// ordered for stable output. Reads the raw inventory; because writes are
    /// path-guarded and canonicalized and this table is new (no pre-guard stale
    /// rows), no post-filter is applied here — a consumer feeding an
    /// intelligence surface applies the canonical inclusion policy at the
    /// match layer, as the OSV matcher already does.
    pub fn get_dependency_instances(
        &self,
        project_path: &str,
    ) -> SqliteResult<Vec<DependencyInstanceRow>> {
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, ecosystem, package_name, version,
                    is_direct, is_dev, scope, detected_at
             FROM dependency_instances
             WHERE project_path = ?1
             ORDER BY ecosystem, package_name, version",
        )?;
        let rows = stmt.query_map(params![project_path], map_instance_row)?;
        Ok(collect_instance_rows(rows))
    }

    /// Every installed instance across ALL projects — the bulk read the OSV
    /// matcher and alert auto-resolver index once per pass (cheaper than a
    /// per-advisory/per-alert query). Ecosystem is already stored OSV-canonical.
    pub fn get_all_dependency_instances(&self) -> SqliteResult<Vec<DependencyInstanceRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, ecosystem, package_name, version,
                    is_direct, is_dev, scope, detected_at
             FROM dependency_instances
             ORDER BY project_path, ecosystem, package_name, version",
        )?;
        let rows = stmt.query_map([], map_instance_row)?;
        Ok(collect_instance_rows(rows))
    }

    /// Every installed instance of one package across ALL projects, matched by
    /// OSV-normalized ecosystem so callers pass either the ACE language string
    /// (`"rust"`) or the OSV name (`"crates.io"`). This is the read the
    /// version-confirmed matcher uses to prove an advisory against EVERY
    /// installed version — the whole reason the table exists.
    pub fn get_package_instances(
        &self,
        ecosystem: &str,
        package_name: &str,
    ) -> SqliteResult<Vec<DependencyInstanceRow>> {
        let osv = crate::ecosystem::Ecosystem::parse(ecosystem).map_or(ecosystem, |e| e.osv_name());
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, ecosystem, package_name, version,
                    is_direct, is_dev, scope, detected_at
             FROM dependency_instances
             WHERE package_name = ?1
               AND (ecosystem = ?2 OR ecosystem = ?3)
             ORDER BY project_path, version",
        )?;
        let rows = stmt.query_map(params![package_name, ecosystem, osv], map_instance_row)?;
        Ok(collect_instance_rows(rows))
    }

    /// Whether the multi-version inventory has any rows for a project — the
    /// D-3 coverage gate. A negative verdict (`not_affected` / safe-to-close /
    /// quiet-week) is only honest for a project whose instances were captured;
    /// this returns false when the inventory is absent (fail closed).
    pub fn project_has_dependency_instances(&self, project_path: &str) -> SqliteResult<bool> {
        let project_path = canonicalize_project_path(project_path);
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM dependency_instances WHERE project_path = ?1)",
            params![project_path],
            |row| row.get::<_, i64>(0).map(|n| n > 0),
        )
    }
}

fn map_instance_row(row: &rusqlite::Row<'_>) -> SqliteResult<DependencyInstanceRow> {
    Ok(DependencyInstanceRow {
        id: row.get(0)?,
        project_path: row.get(1)?,
        ecosystem: row.get(2)?,
        package_name: row.get(3)?,
        version: row.get(4)?,
        is_direct: row.get::<_, i64>(5)? != 0,
        is_dev: row.get::<_, i64>(6)? != 0,
        scope: row.get(7)?,
        detected_at: row.get(8)?,
    })
}

fn collect_instance_rows<I>(rows: I) -> Vec<DependencyInstanceRow>
where
    I: Iterator<Item = SqliteResult<DependencyInstanceRow>>,
{
    rows.filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in dependency instances: {e}");
            None
        }
    })
    .collect()
}
