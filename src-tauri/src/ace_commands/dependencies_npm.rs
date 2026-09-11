// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! npm lockfile processors (`package-lock.json`, `pnpm-lock.yaml`) — split
//! out of `dependencies.rs` (file-size gate) when they learned where each
//! package ships (AD-046, `ace::dep_scope`).
//!
//! Before this both wrote `is_dev = false` for every instance AND every
//! collapsed row. Measured on the founder instance 2026-09-10: all 8,346
//! `dependency_instances` rows read `is_dev = 0, scope = 'unknown'`, and the
//! walk's `store_dependency(.., false, ..)` also overwrote the manifest scan's
//! dev flag on direct devDependencies — paddle-webhook's `vercel`,
//! `typescript` and `@types/node` all read `is_dev = 0`. So no dev-only
//! discount could ever fire on a lockfile-walked project.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::ace::dep_scope::DevScope;
use crate::ace::scanner::ProjectScanner;
use crate::db::{Database, DependencyInstanceInput};

use super::{instances_from_packages, prune_stale_rows, read_package_json_deps};

/// Process a package-lock.json, storing transitive deps and updating direct dep versions.
/// Returns the number of transitive dependencies stored.
pub(super) fn process_package_lock(
    db: &Database,
    scanner: &ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let pkg_lock = dir.join("package-lock.json");
    if !pkg_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&pkg_lock) else {
        return 0;
    };

    let direct_deps = read_package_json_deps(scanner, dir);

    // Capture the parent->child graph for reachability (Step 1, silent).
    let edges = ProjectScanner::parse_package_lock_edges(&content);
    db.store_dependency_edges(project_path, "javascript", &edges)
        .ok();

    let packages = ProjectScanner::parse_package_lock_json(&content);
    let scope = DevScope::from_package_lock(&content);
    store_npm_packages(db, project_path, &packages, &direct_deps, &scope)
}

/// Process a pnpm-lock.yaml, storing transitive deps and updating direct dep versions.
/// Returns the number of transitive dependencies stored.
pub(super) fn process_pnpm_lock(
    db: &Database,
    scanner: &ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let pnpm_lock = dir.join("pnpm-lock.yaml");
    if !pnpm_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&pnpm_lock) else {
        return 0;
    };

    let direct_deps = read_package_json_deps(scanner, dir);

    // Capture the parent->child graph for reachability (Step 1, silent).
    let edges = ProjectScanner::parse_pnpm_lock_edges(&content);
    db.store_dependency_edges(project_path, "javascript", &edges)
        .ok();

    let packages = ProjectScanner::parse_pnpm_lock_yaml(&content);
    let scope = DevScope::from_pnpm_lock(&content);
    store_npm_packages(db, project_path, &packages, &direct_deps, &scope)
}

/// The shared tail: the instance inventory with each package's lockfile
/// scope, then the collapsed rows (whose `is_dev` now agrees with it), then
/// the prune. Returns the number of transitive packages stored.
fn store_npm_packages(
    db: &Database,
    project_path: &str,
    packages: &[(String, String)],
    direct_deps: &[String],
    scope: &DevScope,
) -> u32 {
    db.store_dependency_instances(
        project_path,
        "javascript",
        &scoped_instances(packages, direct_deps, scope),
    )
    .ok();

    let dev_only = dev_only_names(packages, scope);
    let mut count = 0u32;
    for (name, version) in packages {
        let is_dev = dev_only.contains(&name.to_ascii_lowercase());
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                is_dev,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                is_dev,
                None,
            )
            .ok();
        }
    }
    prune_stale_rows(db, project_path, "javascript", packages, direct_deps);
    count
}

/// Every instance with its lockfile scope: `is_dev` only when a dev root and
/// no runtime root reaches that exact version; `scope` is `runtime` | `dev` |
/// `unknown`.
fn scoped_instances(
    packages: &[(String, String)],
    direct_deps: &[String],
    scope: &DevScope,
) -> Vec<DependencyInstanceInput> {
    let mut instances = instances_from_packages(packages, direct_deps, false);
    for instance in &mut instances {
        instance.is_dev = scope.is_dev_only(&instance.package_name, &instance.version);
        instance.scope = scope
            .label(&instance.package_name, &instance.version)
            .to_string();
    }
    instances
}

/// Names whose EVERY version in this lockfile is dev-only. `user_dependencies`
/// keeps one row per package, so one runtime copy makes the row runtime.
fn dev_only_names(packages: &[(String, String)], scope: &DevScope) -> HashSet<String> {
    let mut all_dev: HashMap<String, bool> = HashMap::new();
    for (name, version) in packages {
        let dev = scope.is_dev_only(name, version);
        all_dev
            .entry(name.to_ascii_lowercase())
            .and_modify(|all| *all &= dev)
            .or_insert(dev);
    }
    all_dev
        .into_iter()
        .filter_map(|(name, dev)| dev.then_some(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;

    /// paddle-webhook's shape, cut down: `vercel` is a devDependency that
    /// brings `sandbox` -> `@vercel/sandbox`; `hono` is the runtime root; and
    /// `debug` is installed twice — once at runtime, once only for dev.
    const LOCK: &str = "\
lockfileVersion: '9.0'

importers:

  .:
    dependencies:
      debug:
        specifier: ^2.6.9
        version: 2.6.9
      hono:
        specifier: ^4.13.5
        version: 4.13.5
    devDependencies:
      vercel:
        specifier: ^54.20.1
        version: 54.20.1

packages:

  '@vercel/sandbox@2.1.1':
    resolution: {integrity: sha512-a}

  debug@2.6.9:
    resolution: {integrity: sha512-b}

  debug@4.4.3:
    resolution: {integrity: sha512-c}

  hono@4.13.5:
    resolution: {integrity: sha512-d}

  sandbox@3.1.2:
    resolution: {integrity: sha512-e}

  vercel@54.20.1:
    resolution: {integrity: sha512-f}

snapshots:

  '@vercel/sandbox@2.1.1': {}

  debug@2.6.9: {}

  debug@4.4.3: {}

  hono@4.13.5: {}

  sandbox@3.1.2:
    dependencies:
      '@vercel/sandbox': 2.1.1
      debug: 4.4.3

  vercel@54.20.1:
    dependencies:
      sandbox: 3.1.2
";

    /// The whole live defect, end to end through the store: before AD-046 the
    /// manifest scan wrote `vercel` dev, the walk overwrote it, and `sandbox`
    /// was recorded runtime with no way to heal.
    #[test]
    fn the_walk_records_lockfile_scope_on_instances_and_collapsed_rows() {
        let db = test_db();
        let project = "/proj/paddle-webhook";
        db.store_manifest_dependency(
            project,
            "vercel",
            None,
            "javascript",
            true,
            true,
            "manifest",
        )
        .unwrap();
        db.store_transitive_dependency(project, "sandbox", Some("3.1.2"), "javascript", false)
            .unwrap();

        let packages = ProjectScanner::parse_pnpm_lock_yaml(LOCK);
        let direct = vec![
            "debug".to_string(),
            "hono".to_string(),
            "vercel".to_string(),
        ];
        store_npm_packages(
            &db,
            project,
            &packages,
            &direct,
            &DevScope::from_pnpm_lock(LOCK),
        );

        let instances = db.get_dependency_instances(project).unwrap();
        let instance = |name: &str, version: &str| {
            instances
                .iter()
                .find(|i| i.package_name == name && i.version == version)
                .unwrap_or_else(|| panic!("{name}@{version} instance"))
                .clone()
        };
        let sandbox = instance("sandbox", "3.1.2");
        assert!(sandbox.is_dev && !sandbox.is_direct, "{sandbox:?}");
        assert_eq!(sandbox.scope, "dev");
        assert!(instance("@vercel/sandbox", "2.1.1").is_dev);
        assert!(instance("vercel", "54.20.1").is_dev);
        assert!(!instance("hono", "4.13.5").is_dev);
        assert_eq!(instance("hono", "4.13.5").scope, "runtime");
        assert!(instance("debug", "4.4.3").is_dev, "the dev-only copy");
        assert!(!instance("debug", "2.6.9").is_dev, "the runtime copy");

        let rows = db.get_project_dependencies(project).unwrap();
        let row_is_dev = |name: &str| {
            rows.iter()
                .find(|r| r.package_name == name)
                .unwrap_or_else(|| panic!("{name} row"))
                .is_dev
        };
        assert!(
            row_is_dev("vercel"),
            "the walk no longer overwrites the manifest's dev flag"
        );
        assert!(row_is_dev("sandbox"), "a transitive row heals");
        assert!(!row_is_dev("hono"));
        assert!(
            !row_is_dev("debug"),
            "one runtime copy makes the collapsed row runtime"
        );
    }

    #[test]
    fn a_lockfile_without_roots_changes_nothing() {
        let scope = DevScope::from_pnpm_lock("lockfileVersion: '9.0'\n");
        let instances =
            scoped_instances(&[("sandbox".to_string(), "3.1.2".to_string())], &[], &scope);
        assert!(!instances[0].is_dev, "unknown is never dev");
        assert_eq!(instances[0].scope, "unknown");
    }

    /// Production-shape verification on a SNAPSHOT of the founder database
    /// (`recipe-live-verify-rust-on-db-snapshot`; an online backup, never a
    /// file copy — the live DB is WAL). Re-walks the REAL paddle-webhook
    /// lockfile into the snapshot and checks both measured outcomes end to end,
    /// then prints what the install-drift detector reads off the real disk:
    ///   FOURDA_VERIFY_DB=<snapshot> FOURDA_VERIFY_REPO=<repo root> \
    ///     cargo test --lib live_snapshot -- --ignored --nocapture
    #[test]
    #[ignore = "requires FOURDA_VERIFY_DB (a founder-DB snapshot) and FOURDA_VERIFY_REPO"]
    fn live_snapshot_sandbox_goes_dev_only_and_the_fixable_bump_leads() {
        let (Ok(snapshot), Ok(repo)) = (
            std::env::var("FOURDA_VERIFY_DB"),
            std::env::var("FOURDA_VERIFY_REPO"),
        ) else {
            panic!("set FOURDA_VERIFY_DB and FOURDA_VERIFY_REPO");
        };
        crate::register_sqlite_vec_extension();
        let db = Database::new(std::path::Path::new(&snapshot)).expect("open snapshot");
        let dir = PathBuf::from(&repo).join("paddle-webhook");
        let project = dir.to_string_lossy().to_string();
        process_pnpm_lock(&db, &ProjectScanner::new(), &dir, &project);

        let instances = db.get_dependency_instances(&project).unwrap();
        for name in ["vercel", "sandbox", "@vercel/sandbox"] {
            let found: Vec<_> = instances
                .iter()
                .filter(|i| i.package_name == name)
                .collect();
            println!("{name}: {found:?}");
            assert!(
                !found.is_empty() && found.iter().all(|i| i.is_dev && i.scope == "dev"),
                "{name} ships only with dev tooling"
            );
        }

        let plan = crate::evidence::build_upgrade_plan_with_drops(&db).0;
        for item in plan.iter().take(8) {
            println!("  plan {:?} {}", item.urgency, item.title);
        }
        let position = |pkg: &str| {
            plan.iter()
                .position(|i| i.affected_deps.iter().any(|d| d == pkg))
                .unwrap_or_else(|| panic!("{pkg} is not in the plan"))
        };
        assert_eq!(
            plan[position("sandbox")].urgency,
            crate::evidence::Urgency::Medium
        );
        assert!(
            position("jsonwebtoken") < position("sandbox"),
            "the fixable-now jsonwebtoken bump leads sandbox"
        );

        // The alert path grades the same installs the same way (AD-046).
        let matches = crate::osv::matching::get_matched_advisories(&db).unwrap();
        let sandbox: Vec<&crate::osv::types::MatchedAdvisory> = matches
            .iter()
            .filter(|m| m.package_name == "sandbox" && m.is_version_confirmed)
            .collect();
        let key = project.replace('\\', "/").to_lowercase();
        let (projects, group) = crate::osv::identity::split_by_exposure(&sandbox)
            .into_iter()
            .find(|(projects, _)| projects.contains(&key))
            .expect("sandbox exposure in paddle-webhook");
        let alert = crate::preemption::osv_alert_urgency(&group, &projects);
        println!("alert path: sandbox {alert:?} for {projects:?}");
        assert!(matches!(alert, crate::preemption::AlertUrgency::Medium));

        // What the install-drift detector reads off the real disk right now.
        let conn = db.conn.lock();
        for drift in crate::evidence::install_drift::projects(&conn) {
            let item = drift.to_evidence_item();
            crate::evidence::validate_item(&item).expect("a drift row validates");
            println!("  drift {:?}: {}", item.urgency, item.title);
        }
    }
}
