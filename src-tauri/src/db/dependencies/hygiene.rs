// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency-table self-heal purges and filesystem reconciles.
//!
//! Write-time guards (`queries::is_excluded_project_path`, the ACE scanner
//! skip lists) stop NEW pollution; the functions here heal EXISTING installs
//! whose tables were written before a guard existed, plus drift no write
//! guard can catch (projects deleted/moved on disk). All are idempotent,
//! transactional, and called from startup cleanup (`app_setup`) or the
//! post-scan reconcile (`ace_commands::scanning`):
//!
//! - [`purge_agent_infra_dependencies`] — tier-1 `.claude`/`.codex` rows +
//!   duplicate-identity collapse (#212 precedent);
//! - [`purge_non_project_intelligence`] — full canonical inclusion policy
//!   (tiers 1+2) across every path-keyed intelligence table (Wave 6);
//! - [`purge_builtin_import_dependencies`] — import-scraped Node/Python
//!   builtin modules + the stale decision windows they minted (Wave 8a);
//! - [`prune_orphaned_project_dependencies`] — rows of projects that no
//!   longer exist on disk (Wave 8a).

use rusqlite::{params, Result as SqliteResult};

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

/// Counts from a [`purge_builtin_import_dependencies`] self-heal pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuiltinImportPurge {
    /// Builtin-module rows deleted from `user_dependencies`.
    pub user_dependencies: usize,
    /// Builtin-module rows deleted from `project_dependencies`.
    pub project_dependencies: usize,
    /// Builtin-module rows deleted from `dependency_snapshots`.
    pub dependency_snapshots: usize,
    /// Open decision windows closed because their dependency was a builtin
    /// with no surviving versioned dependency row.
    pub windows_closed: usize,
}

impl BuiltinImportPurge {
    pub fn total(&self) -> usize {
        self.user_dependencies + self.project_dependencies + self.dependency_snapshots
    }
}

/// The ecosystem labels that participate in the builtin purge. Rust/go/etc.
/// rows are NEVER candidates — `http`/`url` in `ecosystem='rust'` are real
/// crates.
const BUILTIN_PURGE_ECOSYSTEMS: &str =
    "('javascript', 'js', 'npm', 'node', 'python', 'py', 'pypi', 'pip')";

/// Delete import-scraped builtin-module rows from one dependency table.
/// The pollution signature is exactly: `version IS NULL AND is_direct = 1`
/// (import scrape can never resolve a version, and always writes direct) in a
/// javascript/python ecosystem row whose name is a language builtin. The
/// builtin predicate is evaluated in Rust against the canonical registry
/// (`ace::builtin_modules`) so SQL can never drift from it.
///
/// What this deliberately KEEPS:
/// - versioned rows: npm polyfill packages (`buffer@5.7.1`, `events@3.3.0`,
///   `string_decoder@1.3.0`) come from lockfiles WITH versions;
/// - `is_direct = 0` rows: lockfile transitives, even unversioned;
/// - every non-js/py ecosystem: the Rust `http`/`url` CRATES survive.
fn purge_builtin_rows(
    conn: &rusqlite::Connection,
    table: &str,
    ecosystem_col: &str,
) -> SqliteResult<usize> {
    if !table_exists(conn, table) {
        return Ok(0);
    }
    let rows: Vec<(i64, String, String)> = conn
        .prepare(&format!(
            "SELECT rowid, package_name, {ecosystem_col} FROM {table}
             WHERE version IS NULL AND is_direct = 1
               AND LOWER({ecosystem_col}) IN {BUILTIN_PURGE_ECOSYSTEMS}"
        ))?
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    let mut deleted = 0usize;
    for (rowid, name, ecosystem) in rows {
        if crate::ace::builtin_modules::is_builtin_for_ecosystem(&ecosystem, &name) {
            deleted += conn.execute(
                &format!("DELETE FROM {table} WHERE rowid = ?1"),
                params![rowid],
            )?;
        }
    }
    Ok(deleted)
}

/// Self-heal purge for import-scraped BUILTIN modules persisted as user
/// dependencies (Node builtins `fs`/`path`/`http`/..., Python stdlib
/// `os`/`json`/...). The scanner no longer persists these (see
/// `ace::builtin_modules`), but existing installs carry rows written before
/// the fix — one of which ("http" from an import scrape) minted a phantom
/// "Security: http" decision window. Sweeps `user_dependencies`,
/// `project_dependencies` (which would otherwise re-seed `user_dependencies`
/// on the next sync), and `dependency_snapshots`; then closes open decision
/// windows whose `dependency` is a builtin name with no surviving VERSIONED
/// `user_dependencies` row of the same name (a window on "http" backed by the
/// real Rust `http` crate at 1.4.x stays open). Idempotent; single
/// transaction. Mirrors the #212/#214 startup self-heal precedent.
pub fn purge_builtin_import_dependencies(
    conn: &rusqlite::Connection,
) -> SqliteResult<BuiltinImportPurge> {
    let tx = conn.unchecked_transaction()?;
    let mut counts = BuiltinImportPurge {
        user_dependencies: purge_builtin_rows(&tx, "user_dependencies", "ecosystem")?,
        project_dependencies: purge_builtin_rows(&tx, "project_dependencies", "language")?,
        dependency_snapshots: purge_builtin_rows(&tx, "dependency_snapshots", "ecosystem")?,
        ..Default::default()
    };

    // Close the stale windows this pollution class minted. Predicated on the
    // builtin REGISTRY (not on what this run deleted) so a window that
    // outlived an earlier partial cleanup is still healed.
    if table_exists(&tx, "decision_windows") && table_exists(&tx, "user_dependencies") {
        let open: Vec<(i64, String)> = tx
            .prepare(
                "SELECT id, dependency FROM decision_windows
                 WHERE status = 'open' AND dependency IS NOT NULL AND dependency <> ''",
            )?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(std::result::Result::ok)
            .collect();
        for (id, dep) in open {
            let name = dep.to_lowercase();
            let is_builtin = crate::ace::builtin_modules::is_node_builtin(&name)
                || crate::ace::builtin_modules::is_python_stdlib(&name);
            if !is_builtin {
                continue;
            }
            let survivors: i64 = tx.query_row(
                "SELECT COUNT(*) FROM user_dependencies
                 WHERE LOWER(package_name) = ?1 AND version IS NOT NULL",
                params![name],
                |row| row.get(0),
            )?;
            if survivors == 0 {
                counts.windows_closed += tx.execute(
                    "UPDATE decision_windows
                     SET status = 'closed', closed_at = datetime('now'),
                         outcome = 'invalidated'
                     WHERE id = ?1 AND status = 'open'",
                    params![id],
                )?;
            }
        }
    }

    tx.commit()?;
    Ok(counts)
}

/// Counts from a [`prune_orphaned_project_dependencies`] reconcile pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct OrphanedProjectPurge {
    /// Distinct project paths found missing on disk.
    pub orphaned_paths: usize,
    /// Rows deleted from `user_dependencies`.
    pub user_dependencies: usize,
    /// Rows deleted from `project_dependencies`.
    pub project_dependencies: usize,
    /// Rows deleted from `dependency_snapshots`.
    pub dependency_snapshots: usize,
}

impl OrphanedProjectPurge {
    pub fn total(&self) -> usize {
        self.user_dependencies + self.project_dependencies + self.dependency_snapshots
    }
}

/// Production "is this project gone?" probe for the orphan reconcile.
/// Deliberately conservative — pruning a live project is worse than keeping a
/// dead row for one more cycle:
/// - relative / empty paths: never missing (nothing sane to probe);
/// - UNC / network shares: never missing (an offline share is not a deleted
///   project, and probing a dead share can block for the SMB timeout);
/// - a path whose volume ROOT is also gone (unplugged external drive): never
///   missing — the project may come back with the drive.
pub fn project_path_missing_on_disk(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.starts_with("\\\\") || trimmed.starts_with("//") {
        return false;
    }
    let p = std::path::Path::new(trimmed);
    if !p.is_absolute() {
        return false;
    }
    if p.exists() {
        return false;
    }
    if let Some(root) = p.ancestors().last() {
        if !root.as_os_str().is_empty() && !root.exists() {
            return false;
        }
    }
    true
}

/// Reconcile dependency tables against the filesystem: `prune_removed_dependencies`
/// only runs for manifests that are RE-scanned, so the rows of a project that
/// was deleted or moved persist forever (its manifest never scans again) —
/// stale deps kept grounding alerts for projects that no longer exist. Called
/// after each full ACE scan.
///
/// `path_missing` is injected (production: [`project_path_missing_on_disk`])
/// so tests can simulate deletion. Tier-waived ledger fixture paths (canonical
/// policy: `project_inclusion` tier-2 + active waiver) are skipped — the
/// receipts ledger scans fixture stacks on purpose. Single transaction.
pub fn prune_orphaned_project_dependencies(
    conn: &rusqlite::Connection,
    path_missing: &dyn Fn(&str) -> bool,
) -> SqliteResult<OrphanedProjectPurge> {
    const TABLES: &[&str] = &[
        "user_dependencies",
        "project_dependencies",
        "dependency_snapshots",
    ];

    let tx = conn.unchecked_transaction()?;

    let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for table in TABLES {
        if !table_exists(&tx, table) {
            continue;
        }
        let mut stmt = tx.prepare(&format!("SELECT DISTINCT project_path FROM {table}"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        paths.extend(rows.filter_map(std::result::Result::ok));
    }

    let orphaned: Vec<String> = paths
        .into_iter()
        .filter(|p| {
            // Tier-waived ledger fixtures are legitimate scan roots.
            if crate::project_inclusion::is_non_project_path(p)
                && crate::project_inclusion::tier2_waiver_active()
            {
                return false;
            }
            path_missing(p)
        })
        .collect();

    let mut counts = OrphanedProjectPurge {
        orphaned_paths: orphaned.len(),
        ..Default::default()
    };
    for path in &orphaned {
        for table in TABLES {
            if !table_exists(&tx, table) {
                continue;
            }
            let deleted = tx.execute(
                &format!("DELETE FROM {table} WHERE project_path = ?1"),
                params![path],
            )?;
            match *table {
                "user_dependencies" => counts.user_dependencies += deleted,
                "project_dependencies" => counts.project_dependencies += deleted,
                _ => counts.dependency_snapshots += deleted,
            }
        }
    }

    tx.commit()?;
    Ok(counts)
}
