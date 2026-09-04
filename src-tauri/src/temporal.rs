// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Temporal Event Store for 4DA Innovation Features
//!
//! Provides recording and querying of temporal events, project dependencies,
//! and cross-item relationships used by predictive context, semantic diff,
//! signal chains, knowledge decay, and attention tracking.

use crate::error::{Result, ResultExt};
use rusqlite::params;
use serde::{Deserialize, Serialize};

// ============================================================================
// Dependency scope health
// ============================================================================

/// Set when the git-recency scope filter in [`get_all_dependencies`] matched
/// NOTHING and the read fell back to a wider set than it was asked for.
///
/// Before 2026-08-26 that fallback was silent AND total: the filter compared a
/// raw `git_signals.repo_path` (`D:\4DA`, backslashes) against a canonicalized
/// `project_dependencies.project_path` (`d:/4da/src-tauri`, forward slashes),
/// so on Windows it matched zero of 245 rows on every run and
/// `filtered.is_empty()` re-admitted every dependency the filter existed to
/// exclude — including a dormant side project whose `axios` advisories then
/// held nine of the top-45 feed slots. A broken filter was indistinguishable
/// from a correctly-empty one. It is now distinguishable: this flag rides onto
/// every breakdown scored under it as `dep_scope_degraded`.
static DEP_SCOPE_DEGRADED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// True when the dependency set backing the current run is WIDER than the
/// git-recency scope asked for (see [`DEP_SCOPE_DEGRADED`]).
pub(crate) fn dep_scope_degraded() -> bool {
    DEP_SCOPE_DEGRADED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Test hook: force the flag so pipeline tests can assert the marker reaches
/// the breakdown without staging a real scope failure.
#[cfg(test)]
pub(crate) fn set_dep_scope_degraded_for_test(value: bool) {
    DEP_SCOPE_DEGRADED.store(value, std::sync::atomic::Ordering::Relaxed);
}

// ============================================================================
// Path Canonicalization
// ============================================================================

/// Normalize a project path for consistent DB storage on Windows.
/// Lowercases the drive letter and path segments so `Documents` and `documents`
/// resolve to the same UNIQUE key. Uses forward slashes for uniformity.
fn canonicalize_project_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.replace('\\', "/").to_string()
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEvent {
    pub id: i64,
    pub event_type: String,
    pub subject: String,
    pub data: serde_json::Value,
    pub source_item_id: Option<i64>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDependency {
    pub id: i64,
    pub project_path: String,
    pub manifest_type: String,
    pub package_name: String,
    pub version: Option<String>,
    pub is_dev: bool,
    /// Whether this is a direct dependency (listed in manifest) vs transitive
    /// (pulled in via lockfile). Direct deps default to true for backwards
    /// compatibility — existing rows without the column get is_direct=1.
    pub is_direct: bool,
    pub language: String,
    pub last_scanned: String,
    /// Provenance: HOW this row was discovered — 'manifest' (declared in a
    /// manifest file), 'import_scrape' (inferred from source import lines),
    /// 'lockfile', or 'unknown' (legacy rows written before migration 87).
    /// The builtin-module self-heal purge keys on this.
    pub detected_from: String,
    /// Scan-time confidence that this project is one the user actually works
    /// in: `path_score * git_recency` from
    /// [`crate::ace::scanner::compute_project_relevance`] — 1.0 for a real,
    /// recently-committed repo, 0.5 when no git repo is found, 0.1 for a stale
    /// (>90d) repo or a fixture/example/benchmark path. Persisted since
    /// migration 55 but never read by the scoring path until the 2026-08-26
    /// audit: `axios` (a dependency of a non-git side project, relevance 0.5)
    /// held nine of the top-45 feed slots at full dependency weight.
    pub project_relevance: f32,
}

// ============================================================================
// Project Dependencies
// ============================================================================

/// Upsert a project dependency.
///
/// `is_direct` indicates whether this dependency is declared directly in a
/// manifest file (`true`) or is a transitive dependency discovered from a
/// lockfile (`false`). On conflict the `is_direct` value is only *upgraded*
/// (transitive -> direct) but never downgraded, so a lockfile upsert cannot
/// demote a previously-seen direct dep.
///
/// `project_relevance` is a 0.0..1.0 score from ACE path/git analysis.
/// Example/demo/test directories get low scores (0.1x). The column defaults
/// to 1.0 in the schema, so existing rows and callers passing 1.0 are unaffected.
///
/// `detected_from` is the provenance label ('manifest' | 'import_scrape' |
/// 'lockfile'); each write is authoritative about how THIS observation was
/// made, so the conflict path assigns it.
#[allow(clippy::too_many_arguments)]
pub fn upsert_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    is_direct: bool,
    language: &str,
    project_relevance: f32,
    detected_from: &str,
) -> Result<()> {
    upsert_dependency_with_platform(
        conn,
        project_path,
        manifest_type,
        package_name,
        version,
        is_dev,
        is_direct,
        language,
        project_relevance,
        None,
        true,
        detected_from,
    )
}

/// Like [`upsert_dependency`], but records platform relevance. `target_cfg` is the
/// gating spec (e.g. `cfg(windows)`) or `None` for unconditional deps;
/// `platform_active` is `false` when the dep is not built on the host. These feed
/// the relevance gate so platform-irrelevant advisories can be de-emphasised. The
/// columns default to (NULL, 1) so callers that don't care stay unaffected.
#[allow(clippy::too_many_arguments)]
pub fn upsert_dependency_with_platform(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    is_direct: bool,
    language: &str,
    project_relevance: f32,
    target_cfg: Option<&str>,
    platform_active: bool,
    detected_from: &str,
) -> Result<()> {
    // Canonical write guard (tiers 1+2): agent-infra / temp paths and
    // non-project scaffolding (fixture trees, -placeholder dirs) must never
    // enter project_dependencies. No-op, mirroring
    // db::dependencies::store_dependency; tier-2 refusals log once per path.
    if crate::project_inclusion::is_hard_excluded(project_path) {
        crate::project_inclusion::log_tier2_exclusion(project_path, "write");
        return Ok(());
    }
    let canonical_path = canonicalize_project_path(project_path);
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language, project_relevance, target_cfg, platform_active, detected_from, last_scanned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET version = ?4, is_dev = ?5, is_direct = MAX(project_dependencies.is_direct, ?6), project_relevance = ?8, target_cfg = ?9, platform_active = ?10, detected_from = ?11, last_scanned = datetime('now')",
        params![canonical_path, manifest_type, package_name, version, is_dev as i32, is_direct as i32, language, project_relevance, target_cfg, platform_active as i32, detected_from],
    )
    .context("Failed to upsert dependency")?;
    Ok(())
}

/// Upsert a dependency the MANIFEST ITSELF declares as transitive (go.mod
/// `// indirect`). Unlike [`upsert_dependency`], the conflict path ASSIGNS
/// `is_direct = 0` instead of MAX-upgrading: the manifest is authoritative
/// about the directness of its own modules, so a module that moved from the
/// direct set to `// indirect` is downgraded on the next scan (the MAX guard
/// exists to stop LOCKFILE writes demoting manifest rows, which doesn't apply
/// here — lockfile transitives go to `user_dependencies`, not this table).
pub fn upsert_manifest_indirect_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    language: &str,
    project_relevance: f32,
) -> Result<()> {
    // Same canonical write guard as upsert_dependency_with_platform.
    if crate::project_inclusion::is_hard_excluded(project_path) {
        crate::project_inclusion::log_tier2_exclusion(project_path, "write");
        return Ok(());
    }
    let canonical_path = canonicalize_project_path(project_path);
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language, project_relevance, detected_from, last_scanned)
         VALUES (?1, ?2, ?3, NULL, 0, 0, ?4, ?5, 'manifest', datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET is_direct = 0, project_relevance = ?5, detected_from = 'manifest', last_scanned = datetime('now')",
        params![canonical_path, manifest_type, package_name, language, project_relevance],
    )
    .context("Failed to upsert manifest-indirect dependency")?;
    Ok(())
}

/// Remove dependencies for a (project, language) that were NOT present in
/// the latest manifest scan.
///
/// Called right after the freshly-parsed manifest deps are upserted, so any dep
/// dropped from the manifest — or now intentionally skipped (local `path`/`git`
/// crates, `file:`/`workspace:` npm specs) — stops lingering as a stale row.
/// Stale rows otherwise surface as bogus "unmonitored" blind spots (e.g. an
/// internal workspace crate the user removed weeks ago).
///
/// Covers BOTH direct and indirect rows: every `project_dependencies` row is
/// written by the manifest scan (lockfile transitives go to
/// `user_dependencies`), and `current_names` includes the manifest's indirect
/// set — so an `// indirect` module removed from go.mod is pruned too (it was
/// previously immortal: the old `is_direct = 1` scope never saw it).
/// No-op when `current_names` is empty, so a parse that yields nothing (parse
/// failure, or a genuinely dep-less manifest) cannot wipe a project's deps.
///
/// Returns the number of stale rows removed.
pub fn prune_removed_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
    language: &str,
    current_names: &[String],
) -> Result<usize> {
    if current_names.is_empty() {
        return Ok(0);
    }
    let canonical_path = canonicalize_project_path(project_path);
    let placeholders = vec!["?"; current_names.len()].join(", ");
    let sql = format!(
        "DELETE FROM project_dependencies
         WHERE project_path = ? AND language = ?
           AND package_name NOT IN ({placeholders})"
    );
    // Bind params in positional order: project_path, language, then keep-list.
    let mut values: Vec<String> = Vec::with_capacity(current_names.len() + 2);
    values.push(canonical_path);
    values.push(language.to_string());
    values.extend(current_names.iter().cloned());
    let removed = conn
        .execute(&sql, rusqlite::params_from_iter(values.iter()))
        .context("Failed to prune removed dependencies")?;
    Ok(removed)
}

/// Map a row from the project_dependencies table to a `ProjectDependency`.
/// Column order must be: id, project_path, manifest_type, package_name,
///                        version, is_dev, is_direct, language, last_scanned,
///                        detected_from.
fn map_project_dependency_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDependency> {
    Ok(ProjectDependency {
        id: row.get(0)?,
        project_path: row.get(1)?,
        manifest_type: row.get(2)?,
        package_name: row.get(3)?,
        version: row.get(4)?,
        is_dev: row.get::<_, i32>(5)? != 0,
        is_direct: row.get::<_, i32>(6).unwrap_or(1) != 0,
        language: row.get(7)?,
        last_scanned: row.get(8)?,
        detected_from: row
            .get::<_, String>(9)
            .unwrap_or_else(|_| "unknown".to_string()),
        // Legacy rows predating migration 55 have no value — 1.0 keeps them
        // scoring exactly as they did before this column was read.
        project_relevance: row.get::<_, f32>(10).unwrap_or(1.0),
    })
}

/// Get all tracked dependencies, scoped to projects with recent git activity.
/// Only includes deps from project trees that have commits in the last 60 days.
/// Falls back to all deps if no git signals exist (first run).
///
/// This is the CENTRAL read funnel for dependency intelligence (scoring's
/// `load_dependency_intelligence`, `store_direct_dependencies` →
/// `user_dependencies`, registry monitoring sets, knowledge decay), so the
/// canonical project-inclusion policy is enforced HERE: hard-excluded paths
/// (tiers 1+2 — agent infra, temp, fixture/placeholder scaffolding) and the
/// user's "Your Stack" exclusions (tier 3) never leave this function, even if
/// stale rows exist in the table. Tier-3 rows deliberately REMAIN in
/// `project_dependencies` (the Your Stack list reads the table directly so the
/// user can toggle projects back on).
pub fn get_all_dependencies(conn: &rusqlite::Connection) -> Result<Vec<ProjectDependency>> {
    let active_roots = active_repo_roots(conn);

    // The manifest parsers extract dependency NAMES only — a manifest
    // carries a RANGE ("^2.0.0"), not an installed version — so
    // `project_dependencies.version` has been NULL for every row since
    // the column existed. Consequence (2026-08-26 audit, A6): the
    // SameMajor x1.2, NewerMajor x1.1 and OlderMajor x0.5 multipliers in
    // `match_dependencies` have NEVER fired in production, including the
    // one documented in-code as the fix for "just because it's Tauri
    // doesn't mean it's relevant".
    //
    // The resolved version was one JOIN away the whole time: the LOCKFILE
    // parsers do capture (name, version) and write it to
    // `user_dependencies`. Resolve it at READ time rather than backfilling
    // once, so it tracks lockfile changes instead of going stale.
    //
    // Tier (i), here in SQL: the SAME project path and a congruent
    // ecosystem. The first cut of this join matched on package name alone,
    // across every project and ecosystem (2026-09-04 audit): 40 of 184 d:/4da
    // deps resolved from a FOREIGN lockfile — `vite` took navcal's 7.2.2 over
    // 4DA's own 8.1.3, the Rust `jsonwebtoken` crate took an npm 9.0.2, and
    // src-tauri's `axum` resolved to relay's 0.7.9. Tier (ii), in Rust below,
    // widens only to lockfiles under the SAME active repo root (a workspace
    // member resolving from the workspace lockfile). Never across roots or
    // ecosystems. Direct rows win over transitive, then most-recently-seen.
    let sql = format!(
        "SELECT pd.id, pd.project_path, pd.manifest_type, pd.package_name,
                    COALESCE(
                        pd.version,
                        (SELECT ud.version FROM user_dependencies ud
                          WHERE ud.project_path = pd.project_path
                            AND LOWER(ud.package_name) = LOWER(pd.package_name)
                            AND {ud_family} = {pd_family}
                            AND ud.version IS NOT NULL AND ud.version <> ''
                          ORDER BY ud.is_direct DESC, ud.last_seen_at DESC
                          LIMIT 1)
                    ) AS version,
                    pd.is_dev, pd.is_direct, pd.language, pd.last_scanned,
                    pd.detected_from, pd.project_relevance
             FROM project_dependencies pd
             ORDER BY pd.project_path, pd.package_name",
        ud_family = ecosystem_family_sql("ud.ecosystem"),
        pd_family = ecosystem_family_sql("pd.language"),
    );
    let mut stmt = conn.prepare(&sql)?;

    let user_excluded = crate::project_inclusion::user_excluded_paths();
    let mut all_deps: Vec<ProjectDependency> = stmt
        .query_map([], map_project_dependency_row)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .filter(|dep| {
            !crate::project_inclusion::is_excluded_from_intelligence(
                &dep.project_path,
                &user_excluded,
            )
        })
        .collect();
    resolve_versions_within_roots(conn, &mut all_deps, &active_roots);

    // Filter to deps from active project trees only
    if active_roots.is_empty() {
        // No git signals yet — return all included deps (first run fallback).
        // Genuinely unscoped, so it degrades loudly like every other widening.
        DEP_SCOPE_DEGRADED.store(true, std::sync::atomic::Ordering::Relaxed);
        return Ok(all_deps);
    }

    let filtered: Vec<ProjectDependency> = all_deps
        .iter()
        .filter(|dep| dep_within_active_root(&dep.project_path, &active_roots))
        .cloned()
        .collect();

    if !filtered.is_empty() {
        DEP_SCOPE_DEGRADED.store(false, std::sync::atomic::Ordering::Relaxed);
        return Ok(filtered);
    }

    // Nothing matched. Every dependency-bearing project is either gone or has
    // had no commit in 60 days. Widening to EVERYTHING was the old behaviour
    // and is exactly what let a dormant project's deps score at full weight;
    // widen instead to the highest-relevance projects present (ACE's scan-time
    // path/git score), and say so at ERROR.
    let max_relevance = all_deps
        .iter()
        .map(|d| d.project_relevance)
        .fold(f32::MIN, f32::max);
    let primary: Vec<ProjectDependency> = all_deps
        .iter()
        .filter(|d| (d.project_relevance - max_relevance).abs() < f32::EPSILON)
        .cloned()
        .collect();

    DEP_SCOPE_DEGRADED.store(true, std::sync::atomic::Ordering::Relaxed);
    tracing::error!(
        target: "4da::scoring",
        active_roots = active_roots.len(),
        total_deps = all_deps.len(),
        fallback_deps = primary.len(),
        max_relevance,
        "dependency scope filter matched NOTHING — falling back to the highest-relevance projects; every breakdown scored under this run carries dep_scope_degraded"
    );

    if primary.is_empty() {
        return Ok(all_deps);
    }
    Ok(primary)
}

/// Repo roots with a commit in the last 60 days — the scope every
/// dependency reader shares (this module's manifest funnel and the
/// `user_dependencies` audit/OSV readers in `db::dependencies::queries`).
/// `git_signals.repo_path` is stored RAW; compare via
/// [`dep_within_active_root`]. Empty on first run (no git analysis yet) or
/// when the table is absent; callers treat empty as "unscoped".
pub(crate) fn active_repo_roots(conn: &rusqlite::Connection) -> Vec<String> {
    conn.prepare(
        "SELECT DISTINCT repo_path FROM git_signals
         WHERE commit_hash IS NOT NULL AND commit_hash != ''
         AND timestamp > datetime('now', '-60 days')",
    )
    .and_then(|mut stmt| {
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in temporal: {e}");
                    None
                }
            })
            .collect())
    })
    .unwrap_or_default()
}

/// Alias -> family pairs for ecosystem congruence. Mirrors
/// [`crate::ecosystem::Ecosystem::parse`] (a test pins the two together);
/// kept as data so the same table can be rendered into SQL. Labels not
/// listed are their own family (lowercased).
const ECOSYSTEM_FAMILY_ALIASES: &[(&str, &str)] = &[
    ("npm", "javascript"),
    ("typescript", "javascript"),
    ("node", "javascript"),
    ("js", "javascript"),
    ("ts", "javascript"),
    ("cargo", "rust"),
    ("crates.io", "rust"),
    ("crates", "rust"),
    ("pypi", "python"),
    ("pip", "python"),
    ("py", "python"),
    ("golang", "go"),
    ("maven", "java"),
    ("kotlin", "java"),
    ("gradle", "java"),
    ("c#", "csharp"),
    ("dotnet", "csharp"),
    ("nuget", "csharp"),
    ("composer", "php"),
    ("packagist", "php"),
    ("rubygems", "ruby"),
    ("gem", "ruby"),
    ("flutter", "dart"),
    ("pub", "dart"),
];

/// The ecosystem family a `project_dependencies.language` or
/// `user_dependencies.ecosystem` label belongs to: rust <-> rust,
/// javascript/typescript/npm <-> javascript, python/pypi <-> python, ...
pub(crate) fn ecosystem_family(label: &str) -> String {
    let lower = label.trim().to_lowercase();
    ECOSYSTEM_FAMILY_ALIASES
        .iter()
        .find(|(alias, _)| *alias == lower)
        .map_or(lower, |(_, family)| (*family).to_string())
}

/// SQL rendering of [`ecosystem_family`] for a column expression, so the
/// read-time join applies exactly the Rust rule.
fn ecosystem_family_sql(column: &str) -> String {
    use std::fmt::Write as _;
    let arms = ECOSYSTEM_FAMILY_ALIASES
        .iter()
        .fold(String::new(), |mut acc, (alias, family)| {
            // Writing to a String cannot fail.
            let _ = write!(acc, " WHEN '{alias}' THEN '{family}'");
            acc
        });
    format!("CASE LOWER({column}){arms} ELSE LOWER({column}) END")
}

/// Tier (ii) of version resolution: a dep still version-less after the
/// same-project lookup may resolve from a lockfile elsewhere under the SAME
/// active repo root (a workspace member's crate resolved by the workspace
/// `Cargo.lock`), same ecosystem family. Never across roots or ecosystems;
/// with no active roots (first run) nothing widens.
fn resolve_versions_within_roots(
    conn: &rusqlite::Connection,
    deps: &mut [ProjectDependency],
    active_roots: &[String],
) {
    if active_roots.is_empty() || deps.iter().all(|d| d.version.is_some()) {
        return;
    }
    let Ok(mut stmt) = conn.prepare(
        "SELECT ud.project_path, ud.ecosystem, ud.version FROM user_dependencies ud
         WHERE LOWER(ud.package_name) = LOWER(?1)
           AND ud.version IS NOT NULL AND ud.version <> ''
         ORDER BY ud.is_direct DESC, ud.last_seen_at DESC",
    ) else {
        return;
    };
    for dep in deps.iter_mut().filter(|d| d.version.is_none()) {
        let family = ecosystem_family(&dep.language);
        let dep_path = dep.project_path.clone();
        let Ok(rows) = stmt.query_map(params![dep.package_name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        }) else {
            continue;
        };
        dep.version = rows
            .flatten()
            .find(|(path, ecosystem, _)| {
                ecosystem_family(ecosystem) == family
                    && shares_active_root(&dep_path, path, active_roots)
            })
            .map(|(_, _, version)| version);
    }
}

/// Do two project paths sit under one common active repo root?
fn shares_active_root(a: &str, b: &str, active_roots: &[String]) -> bool {
    active_roots.iter().any(|root| {
        let root = std::slice::from_ref(root);
        dep_within_active_root(a, root) && dep_within_active_root(b, root)
    })
}

/// Is this dependency's project inside one of the active repo roots?
///
/// Both sides go through [`crate::project_inclusion::comparison_form`] because
/// they are captured differently: `git_signals.repo_path` is stored RAW
/// (`D:\4DA`) while `project_dependencies.project_path` is canonicalized
/// (`d:/4da/src-tauri`). Comparing them unnormalized matched nothing on
/// Windows, for every row, on every run (2026-08-26 audit, R2).
///
/// Matching is path-BOUNDARY, not raw prefix, in both directions: a root
/// contains its subprojects (`d:/4da` covers `d:/4da/src-tauri`) and a dep
/// path at or above a root still counts (a repo root recorded deeper than the
/// manifest), but `d:/4da` must never match `d:/4da-experiments`.
pub(crate) fn dep_within_active_root(dep_path: &str, active_roots: &[String]) -> bool {
    let dep = crate::project_inclusion::comparison_form(dep_path);
    let dep = dep.trim_end_matches('/');
    if dep.is_empty() {
        return false;
    }
    active_roots.iter().any(|root| {
        let root = crate::project_inclusion::comparison_form(root);
        let root = root.trim_end_matches('/');
        if root.is_empty() {
            return false;
        }
        dep == root || dep.starts_with(&format!("{root}/")) || root.starts_with(&format!("{dep}/"))
    })
}

// ============================================================================
// Temporal Events
// ============================================================================

/// Record a new temporal event
pub fn record_event(
    conn: &rusqlite::Connection,
    event_type: &str,
    subject: &str,
    data: &serde_json::Value,
    source_item_id: Option<i64>,
    expires_at: Option<&str>,
) -> Result<i64> {
    let data_str = serde_json::to_string(data)?;
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, subject, data_str, source_item_id, expires_at],
    )
    .context("Failed to record temporal event")?;
    Ok(conn.last_insert_rowid())
}

/// Query temporal events by type and optional time range
pub fn query_events(
    conn: &rusqlite::Connection,
    event_type: &str,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<TemporalEvent>> {
    let query = if since.is_some() {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1 AND created_at >= ?2
         ORDER BY created_at DESC LIMIT ?3"
    } else {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1
         ORDER BY created_at DESC LIMIT ?2"
    };

    let mut stmt = conn.prepare(query)?;

    let results: Vec<TemporalEvent> = if let Some(since_val) = since {
        stmt.query_map(params![event_type, since_val, limit as i64], map_event)
    } else {
        stmt.query_map(params![event_type, limit as i64], map_event)
    }?
    .filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in temporal: {e}");
            None
        }
    })
    .collect();

    Ok(results)
}

fn map_event(row: &rusqlite::Row) -> rusqlite::Result<TemporalEvent> {
    let data_str: String = row.get(3)?;
    let data = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
    Ok(TemporalEvent {
        id: row.get(0)?,
        event_type: row.get(1)?,
        subject: row.get(2)?,
        data,
        source_item_id: row.get(4)?,
        created_at: row.get(5)?,
        expires_at: row.get(6)?,
    })
}

#[cfg(test)]
#[path = "temporal_tests.rs"]
mod tests;
