// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE dependency storage: direct and transitive dependency discovery from lockfiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tracing::info;

use crate::ace::lockfile::{self, LockFormat, LockfileOutcome, LockfileRead, LockfileReport};
use crate::ace::repo_identity::{self, RepoScope, Step};
use crate::db::Database;
use crate::db::DependencyInstanceInput;
use crate::get_ace_engine;

#[path = "dependencies_manifests.rs"]
mod manifests;
#[path = "dependencies_store.rs"]
mod store;

use manifests::{direct_names, keep_names};

/// Build the multi-version instance list (`dependency_instances`, Phase 92)
/// from a parsed lockfile's `(name, version)` pairs, classifying `is_direct`
/// by manifest membership. A lockfile that resolves a package at multiple
/// versions yields multiple instances here — the data the collapsing
/// `store_dependency` upsert discards. `is_dev`/`scope` start as `false` /
/// `"unknown"`; the Cargo processor then fills them from the lockfile graph.
fn instances_from_packages(
    packages: &[(String, String)],
    direct_deps: &[String],
    case_insensitive: bool,
) -> Vec<DependencyInstanceInput> {
    packages
        .iter()
        .map(|(name, version)| {
            let is_direct = !direct_deps.is_empty()
                && direct_deps.iter().any(|d| {
                    if case_insensitive {
                        d.eq_ignore_ascii_case(name)
                    } else {
                        d == name
                    }
                });
            DependencyInstanceInput {
                package_name: name.clone(),
                version: version.clone(),
                is_direct,
                is_dev: false,
                scope: "unknown".to_string(),
            }
        })
        .collect()
}

/// Store discovered direct dependencies from ACE into user_dependencies table.
pub(super) fn store_direct_dependencies(db: &Database) {
    if let Ok(ace) = get_ace_engine() {
        if let Ok(tech) = ace.get_detected_tech() {
            if let Ok(conn) = crate::open_db_connection() {
                // The INVENTORY, not the stack-grounding set: a dormant or
                // scratch project's manifest rows must keep refreshing so its
                // advisories stay true in Preemption (AD-043).
                if let Ok(deps) = crate::temporal::get_inventory_dependencies(&conn) {
                    for dep in &deps {
                        let ecosystem = &dep.language;
                        // The manifest scan is authoritative for is_direct in
                        // BOTH directions (a go.mod `// indirect` module is
                        // downgraded even if an old row stored it direct) and
                        // carries provenance through to user_dependencies.
                        db.store_manifest_dependency(
                            &dep.project_path,
                            &dep.package_name,
                            dep.version.as_deref(),
                            ecosystem,
                            dep.is_dev,
                            dep.is_direct,
                            &dep.detected_from,
                        )
                        .ok();
                    }
                    if !deps.is_empty() {
                        info!(target: "4da::ace", count = deps.len(), "Stored dependencies in user_dependencies table");
                    }
                }
            }
            drop(tech);
        }
    }
}

/// One pending directory of the lockfile walk: where it is, how deep, and
/// which repository (if any) it belongs to.
type PendingDir = (PathBuf, u8, RepoScope);

/// Parse lockfiles for transitive dependency discovery and store in the database.
///
/// Two gates the manifest scan had and this walk lacked (2026-09-04 audit — a
/// nested third-party clone contributed 1,811 of 7,933 `user_dependencies`
/// rows and every `rkyv` advisory): a subdirectory that is a checkout of a
/// DIFFERENT repository is skipped as foreign code (`repo_identity`), and
/// scaffolding paths are skipped by relevance. Dormancy is NOT a skip reason
/// — see [`lockfile_dir_is_relevant`].
///
/// Returns the walk's report — every lockfile read, failed, unsupported or
/// skipped — which is also logged and saved to `kv_store` so the scan
/// summary can show it (no silent drops).
pub(super) fn store_lockfile_dependencies(db: &Database, scan_paths: &[PathBuf]) -> LockfileReport {
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let mut report = LockfileReport::default();
    let mut lockfile_count = 0u32;
    for dir in collect_lockfile_dirs_reporting(scan_paths, &mut report) {
        let project_path = dir.to_string_lossy().to_string();
        lockfile_count += process_lockfile_dir(db, &scanner, &dir, &project_path, &mut report);
    }
    if lockfile_count > 0 {
        info!(target: "4da::ace", count = lockfile_count, "Stored transitive dependencies from lockfiles");
    }
    report.log();
    if let Err(e) = db.set_kv(
        lockfile::report::REPORT_KV_KEY,
        &report.to_json().to_string(),
    ) {
        tracing::warn!(target: "4da::ace", error = %e, "Failed to save the lockfile walk report");
    }
    report
}

/// Walk `scan_paths` and return every directory whose lockfiles may feed
/// `user_dependencies`: it holds at least one lockfile/manifest, is not
/// excluded by the inclusion policy, passes the relevance gate, and is not a
/// nested checkout of somebody else's repository. Pure with respect to the
/// database, so the walk's decisions are testable on a temp tree.
#[cfg(test)]
fn collect_lockfile_dirs(scan_paths: &[PathBuf]) -> Vec<PathBuf> {
    collect_lockfile_dirs_reporting(scan_paths, &mut LockfileReport::default())
}

/// [`collect_lockfile_dirs`], recording every directory holding a dependency
/// file that a gate skipped.
fn collect_lockfile_dirs_reporting(
    scan_paths: &[PathBuf],
    report: &mut LockfileReport,
) -> Vec<PathBuf> {
    // "Your Stack" exclusions (tier 3), fetched once per walk: lockfiles under
    // a user-excluded project must not feed user_dependencies (the OSV / audit
    // surface). Tiers 1+2 are handled by is_scan_excluded_below plus the
    // DB write guards.
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    let mut selected = Vec::new();

    for root in scan_paths {
        if !root.exists() || !root.is_dir() {
            continue;
        }
        let mut dirs_to_visit: Vec<PendingDir> =
            vec![(root.clone(), 0u8, repo_identity::scope_at(root))];
        while let Some((dir, depth, scope)) = dirs_to_visit.pop() {
            if depth > lockfile::MAX_WALK_DEPTH {
                if lockfile::probe(&dir).is_some() {
                    report.skip_dir(&dir, "deeper than the walk's depth limit");
                }
                continue;
            }
            // Canonical inclusion policy, judged below the root the user
            // configured: covers a walk ROOTED inside an agent tree and
            // tier-2/3 dirs whose names aren't in the skip list.
            let project_path = dir.to_string_lossy().to_string();
            let excluded = if crate::project_inclusion::is_scan_excluded_below(root, &dir) {
                Some("agent infrastructure or test-fixture scaffolding")
            } else if crate::project_inclusion::is_user_excluded(&project_path, &user_excluded) {
                Some("excluded by the user (Your Stack)")
            } else {
                None
            };
            if let Some(reason) = excluded {
                if lockfile::probe(&dir).is_some() {
                    report.skip_dir(&dir, reason);
                }
                continue;
            }
            if let Some(probe) = lockfile::probe(&dir) {
                if lockfile_dir_is_relevant(root, &dir, &probe) {
                    selected.push(dir.clone());
                } else {
                    report.skip_dir(&dir, "example / fixture / scaffolding path");
                }
            }
            queue_subdirectories(&dir, depth, &scope, &mut dirs_to_visit, report);
        }
    }
    selected
}

/// The manifest scan's relevance gate (`ace/mod.rs`) applied to a lockfile's
/// directory — minus the dormancy half (AD-043).
///
/// The floor conflated two reasons for a low score (`scanner::path_relevance`),
/// and the dormancy half made a repository the user still owns VANISH:
/// `navcal`, dormant since ~2025-11, holds 31 packages with published
/// advisories and 4DA said nothing about it anywhere (2026-09-07).
///
/// Indexing a dormant project does not make it urgent: `user_dependencies`
/// carries no relevance, the gated reads (`project_dependencies`, still
/// floored) are unchanged, and Preemption collapses the findings into ONE
/// quiet notice (`evidence::collapse_dormant_alerts`). Scaffolding is still
/// skipped — it is scaffolding whatever its git log says — judged below the
/// walk root: where the user keeps a project is not evidence it is scaffolding.
fn lockfile_dir_is_relevant(root: &Path, dir: &Path, probe: &Path) -> bool {
    let relevance = crate::ace::scanner::compute_project_relevance_below(root, probe);
    if relevance >= crate::ace::scanner::PROJECT_RELEVANCE_FLOOR
        || crate::ace::scanner::forced_relevant_by_context_dir(probe)
    {
        return true;
    }
    if crate::ace::scanner::path_relevance_below(root, probe)
        >= crate::ace::scanner::PROJECT_RELEVANCE_FLOOR
    {
        info!(
            target: "4da::ace",
            dir = %dir.display(),
            relevance,
            "Lockfile walk: indexing a DORMANT project — it is still the user's, and its advisories are still true"
        );
        return true;
    }
    info!(
        target: "4da::ace",
        dir = %dir.display(),
        relevance,
        floor = crate::ace::scanner::PROJECT_RELEVANCE_FLOOR,
        "Lockfile walk: skipping example/fixture path"
    );
    false
}

/// Read every dependency file in one project directory and store it: Cargo
/// through its own processor (host reachability, workspace members), every
/// other ecosystem merged across its lockfiles and written once
/// (`dependencies_store`). Every outcome lands in `report`.
fn process_lockfile_dir(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
    report: &mut LockfileReport,
) -> u32 {
    let outcomes = lockfile::read_dir(dir);
    let mut count = 0u32;
    let mut by_ecosystem: BTreeMap<&'static str, Vec<&LockfileRead>> = BTreeMap::new();
    for outcome in &outcomes {
        report.record(outcome);
        if let LockfileOutcome::Read(read) = outcome {
            if read.format == LockFormat::Cargo {
                count += process_cargo_lock(db, scanner, dir, project_path);
            } else {
                by_ecosystem
                    .entry(read.format.ecosystem())
                    .or_default()
                    .push(read);
            }
        }
    }
    for (ecosystem, reads) in by_ecosystem {
        let direct = direct_names(scanner, dir, ecosystem, &reads);
        let keep = keep_names(dir, ecosystem);
        count += store::store_ecosystem(db, project_path, ecosystem, &reads, &direct, &keep);
    }
    store_go_directive_dependencies(db, dir, project_path);
    count
}

/// Go stdlib/toolchain synthetic deps carry their VERSION from the go.mod
/// directives (`go 1.22.3` / `toolchain go1.22.5`), which the manifest
/// persistence path drops (it stores version: None). OSV publishes
/// standard-library and toolchain advisories against the package names
/// "stdlib"/"toolchain" (ecosystem Go) with SEMVER ranges, so without the
/// version the matcher can only produce unconfirmed matches that Preemption
/// filters out. Done per directory, not per lockfile: a stdlib-only project
/// has no go.sum.
fn store_go_directive_dependencies(db: &Database, dir: &Path, project_path: &str) {
    let Ok(content) = std::fs::read_to_string(dir.join("go.mod")) else {
        return;
    };
    for (name, version) in crate::ace::scanner::ProjectScanner::parse_go_directives(&content) {
        if let Err(e) = db.store_manifest_dependency(
            project_path,
            &name,
            Some(&version),
            "go",
            false,
            true,
            "manifest",
        ) {
            tracing::warn!(
                target: "4da::ace",
                error = %e,
                package = %name,
                "Failed to store Go directive synthetic dependency"
            );
        }
    }
}

/// Queue `dir`'s subdirectories, skipping build/cache/agent dirs and any
/// nested checkout of a DIFFERENT repository (a vendored third-party clone
/// is that project's stack, not the user's). Skips log both remotes and are
/// recorded in the walk report.
fn queue_subdirectories(
    dir: &Path,
    depth: u8,
    scope: &RepoScope,
    stack: &mut Vec<PendingDir>,
    report: &mut LockfileReport,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if lockfile::SKIPPED_DIR_NAMES.contains(&name) {
            continue;
        }
        match repo_identity::step_into(&entry_path, scope) {
            Step::Continue(next) => stack.push((entry_path, depth + 1, next)),
            Step::ForeignRepo { nested, enclosing } => {
                info!(
                    target: "4da::ace",
                    dir = %entry_path.display(),
                    nested_remote = nested.as_deref().unwrap_or("(none)"),
                    enclosing_remote = enclosing.as_deref().unwrap_or("(none)"),
                    "Lockfile walk: skipping nested checkout of a different repository"
                );
                report.skip_dir(
                    &entry_path,
                    format!(
                        "nested checkout of a different repository ({})",
                        nested.as_deref().unwrap_or("no origin")
                    ),
                );
            }
        }
    }
}

/// Item 7 of the ACE input-hygiene fix: rows this project's lockfile no
/// longer resolves are stale. Mirrors `temporal::prune_removed_dependencies`
/// (which prunes only `project_dependencies`); the keep-list is the lockfile's
/// packages plus whatever the manifest scan still declares for this project.
fn prune_stale_rows(
    db: &Database,
    project_path: &str,
    ecosystem: &str,
    packages: &[(String, String)],
    extra_names: &[String],
) {
    let names: Vec<String> = packages
        .iter()
        .map(|(name, _)| name.clone())
        .chain(extra_names.iter().cloned())
        .collect();
    match db.prune_stale_user_dependencies(project_path, ecosystem, &names) {
        Ok(0) => {}
        Ok(removed) => info!(
            target: "4da::ace",
            project = %project_path,
            ecosystem,
            removed,
            "Pruned user_dependencies rows no longer present in the lockfile/manifest"
        ),
        Err(e) => tracing::warn!(
            target: "4da::ace",
            error = %e,
            project = %project_path,
            ecosystem,
            "Failed to prune stale user_dependencies rows"
        ),
    }
}

/// Process a Cargo.lock file, storing transitive deps and updating direct dep versions.
/// Returns the number of transitive dependencies stored.
fn process_cargo_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let cargo_lock = dir.join("Cargo.lock");
    if !cargo_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&cargo_lock) else {
        return 0;
    };
    let (count, packages) = store_cargo_lock(db, scanner, dir, project_path, &content);
    crate::ace::cargo_resolve::record_unreachable_crates(db, dir, project_path, &packages);
    count
}

/// Everything [`process_cargo_lock`] stores from one lockfile, minus the
/// `cargo tree` host-reachability probe. Returns the transitive count and the
/// packages stored (the project's own crates excluded).
fn store_cargo_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &Path,
    project_path: &str,
    content: &str,
) -> (u32, Vec<(String, String)>) {
    use crate::ace::cargo_lock_facts::{self, CargoDevScope};

    let roots = cargo_manifest_roots(scanner, dir, content);
    let direct_deps: Vec<String> = roots
        .runtime
        .iter()
        .chain(roots.dev.iter())
        .cloned()
        .collect();

    // Capture the parent->child graph for reachability (Step 1, silent). The
    // project's own crates stay in the GRAPH — reachability runs through them.
    let edges = crate::ace::scanner::ProjectScanner::parse_cargo_lock_edges(content);
    db.store_dependency_edges(project_path, "rust", &edges).ok();

    // ...but never in the INVENTORY: a workspace member or path crate (no
    // `source` in the lockfile) is the project itself, not an installed copy
    // of a package.
    let local = cargo_lock_facts::local_packages(content);
    let packages: Vec<(String, String)> =
        crate::ace::scanner::ProjectScanner::parse_cargo_lock(content)
            .into_iter()
            .filter(|key| !local.contains(key))
            .collect();
    let scope = CargoDevScope::new(content, &roots.runtime, &roots.dev);

    let mut instances = instances_from_packages(&packages, &direct_deps, false);
    for instance in &mut instances {
        instance.is_dev = scope.is_dev_only(&instance.package_name);
        instance.scope = scope.label(&instance.package_name).to_string();
    }
    db.store_dependency_instances(project_path, "rust", &instances)
        .ok();

    let mut count = 0u32;
    for (name, version) in &packages {
        let is_dev = scope.is_dev_only(name);
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "rust",
                is_dev,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "rust",
                is_dev,
                None,
            )
            .ok();
        }
    }
    prune_stale_rows(db, project_path, "rust", &packages, &direct_deps);
    (count, packages)
}

/// The roots a `Cargo.toml` declares, minus the project's own crates.
struct CargoRoots {
    /// `[dependencies]`, `[workspace.dependencies]`, `[target.*.dependencies]`.
    runtime: Vec<String>,
    /// `[dev-dependencies]` not also declared as a runtime root.
    dev: Vec<String>,
}

/// Read the directory's `Cargo.toml` (empty roots when there is none) and
/// drop every name the lockfile resolves only from the project's own tree.
fn cargo_manifest_roots(
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &Path,
    lock_content: &str,
) -> CargoRoots {
    let manifest_path = dir.join("Cargo.toml");
    let Ok(toml_content) = std::fs::read_to_string(&manifest_path) else {
        return CargoRoots {
            runtime: Vec::new(),
            dev: Vec::new(),
        };
    };
    let mut signal = crate::ace::scanner::ProjectSignal {
        manifest_type: crate::ace::scanner::ManifestType::CargoToml,
        manifest_path: manifest_path.clone(),
        project_name: None,
        languages: vec!["rust".to_string()],
        frameworks: Vec::new(),
        dependencies: Vec::new(),
        dev_dependencies: Vec::new(),
        indirect_dependencies: Vec::new(),
        target_dependencies: Vec::new(),
        detected_at: String::new(),
        project_license: None,
        project_relevance: 1.0, // lockfile processing uses default; relevance applied at manifest scan
    };
    scanner.parse_cargo_toml(&toml_content, &mut signal);
    let mut local = crate::ace::cargo_lock_facts::workspace_local_crate_names(&manifest_path);
    local.extend(crate::ace::cargo_lock_facts::local_only_names(lock_content));
    crate::ace::cargo_lock_facts::drop_local_crates(&local, &mut signal);

    // `[target.'cfg(...)'.dependencies]` entries are DIRECT runtime deps —
    // the manifest names them. Dropping them classified every cfg-gated crate
    // as transitive, costing it the direct-dependency urgency rank.
    let mut runtime = signal.dependencies;
    runtime.extend(signal.target_dependencies.into_iter().map(|(name, _)| name));
    let dev = signal
        .dev_dependencies
        .into_iter()
        .filter(|d| !runtime.contains(d))
        .collect();
    CargoRoots { runtime, dev }
}

#[cfg(test)]
#[path = "dependencies_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dependencies_fixture_audit.rs"]
mod fixture_audit;
