// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the per-ecosystem lockfile store (`dependencies_store.rs`).

use std::path::Path;

use super::*;
use crate::ace::lockfile::{read_text, LockFormat, LockfileOutcome};
use crate::test_utils::test_db;

fn read(name: &str, format: LockFormat, content: &str) -> LockfileRead {
    match read_text(Path::new(name), format, content) {
        LockfileOutcome::Read(read) => read,
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// paddle-webhook's shape, cut down: `vercel` is a devDependency that
/// brings `sandbox` -> `@vercel/sandbox`; `hono` is the runtime root; and
/// `debug` is installed twice — once at runtime, once only for dev.
const PNPM_LOCK: &str = "\
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

/// The AD-046 live defect, end to end through the store: the manifest scan
/// wrote `vercel` dev, the walk overwrote it, and `sandbox` was recorded
/// runtime with no way to heal.
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

    let lock = read("pnpm-lock.yaml", LockFormat::Pnpm, PNPM_LOCK);
    let direct = vec![
        "debug".to_string(),
        "hono".to_string(),
        "vercel".to_string(),
    ];
    store_ecosystem(&db, project, "javascript", &[&lock], &direct, &[]);

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
        "the walk keeps the manifest's dev flag"
    );
    assert!(row_is_dev("sandbox"), "a transitive row heals");
    assert!(!row_is_dev("hono"));
    assert!(
        !row_is_dev("debug"),
        "one runtime copy makes the row runtime"
    );
}

/// Two lockfiles of one ecosystem in one directory: both survive. Each used
/// to replace the other's instances and prune the other's rows.
#[test]
fn two_lockfiles_of_one_ecosystem_are_merged_not_replaced() {
    let db = test_db();
    let project = "/proj/py";
    let poetry = read(
        "poetry.lock",
        LockFormat::Poetry,
        "[[package]]\nname = \"requests\"\nversion = \"2.31.0\"\n\n[[package]]\nname = \"urllib3\"\nversion = \"2.1.0\"\n",
    );
    let reqs = read(
        "requirements.txt",
        LockFormat::Requirements,
        "flask==3.0.0\nrequests==2.31.0\n",
    );
    let direct = vec!["flask".to_string(), "requests".to_string()];
    store_ecosystem(&db, project, "python", &[&poetry, &reqs], &direct, &[]);

    let mut names: Vec<String> = db
        .get_dependency_instances(project)
        .unwrap()
        .into_iter()
        .map(|i| format!("{}@{}", i.package_name, i.version))
        .collect();
    names.sort();
    assert_eq!(names, ["flask@3.0.0", "requests@2.31.0", "urllib3@2.1.0"]);
    let rows = db.get_project_dependencies(project).unwrap();
    assert_eq!(rows.len(), 3, "{rows:?}");
}

/// The collapsed row keeps the copy the project itself resolves to (the
/// hoisted npm copy), not whichever nested copy sorted last.
#[test]
fn the_collapsed_row_keeps_the_hoisted_copy() {
    let lock = read(
        "package-lock.json",
        LockFormat::NpmLock,
        r#"{"lockfileVersion":3,"packages":{
            "":{"name":"app"},
            "node_modules/a/node_modules/semver":{"version":"5.7.1"},
            "node_modules/semver":{"version":"7.6.0"},
            "node_modules/z/node_modules/semver":{"version":"6.3.1","dev":true}
        }}"#,
    );
    let rows = collapse(&merge_reads(&[&lock]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].version, "7.6.0");
    assert!(!rows[0].all_dev);
}
