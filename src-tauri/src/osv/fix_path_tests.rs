// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `osv::fix_path`'s host half: the tool detection and the manifest
//! reads that look at a real directory. The rule's own tests moved with it to
//! `fourda_depengine::fix_path` (single-engine C0.4).

use super::*;

// ---- reading the parent's own requirement from disk ----

#[test]
fn node_modules_parent_manifest_is_read_at_the_exact_version() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = tmp
        .path()
        .join("node_modules/.pnpm/micromatch@4.0.5/node_modules/micromatch");
    std::fs::create_dir_all(&store).expect("mkdir");
    std::fs::write(
        store.join("package.json"),
        r#"{"name":"micromatch","version":"4.0.5","dependencies":{"braces":"^3.0.2"}}"#,
    )
    .expect("write");
    let reqs =
        installed_parent_requirements(Manager::Pnpm, tmp.path(), "micromatch", "4.0.5", "braces");
    assert_eq!(reqs, vec![Requirement::npm("^3.0.2")]);
    // Another installed version of the parent is not this edge's parent.
    assert!(installed_parent_requirements(
        Manager::Pnpm,
        tmp.path(),
        "micromatch",
        "4.0.4",
        "braces"
    )
    .is_empty());
}

#[test]
fn managers_are_detected_per_ecosystem() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let member = tmp.path().join("crates/member");
    std::fs::create_dir_all(&member).expect("mkdir");
    std::fs::write(tmp.path().join("Cargo.lock"), "version = 3\n").expect("write");
    std::fs::write(tmp.path().join("pnpm-lock.yaml"), "lockfileVersion: 6.0\n").expect("write");
    assert_eq!(Manager::detect(&member, "crates.io"), Some(Manager::Cargo));
    assert_eq!(Manager::detect(tmp.path(), "npm"), Some(Manager::Pnpm));
    assert_eq!(Manager::detect(tmp.path(), "PyPI"), None);
    // A pnpm workspace root updates every importer only with `-r` (trpc).
    std::fs::write(tmp.path().join("pnpm-workspace.yaml"), "packages: []\n").expect("write");
    assert_eq!(
        Manager::detect(tmp.path(), "npm"),
        Some(Manager::PnpmWorkspace)
    );
    assert_eq!(
        Manager::PnpmWorkspace
            .refresh_command("picomatch", "2.3.1", "2.3.2")
            .as_deref(),
        Some("pnpm update -r picomatch --depth Infinity")
    );
    let berry = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        berry.path().join("yarn.lock"),
        "# This file is generated\n\n__metadata:\n  version: 6\n",
    )
    .expect("write");
    assert_eq!(
        Manager::detect(berry.path(), "npm"),
        Some(Manager::YarnBerry)
    );
    assert!(Manager::Npm.records_requirements() && !Manager::Pnpm.records_requirements());
}
