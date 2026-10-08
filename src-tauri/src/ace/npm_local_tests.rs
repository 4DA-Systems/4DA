// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `ace::npm_local` — private npm names never reach the registry.

use super::*;
use crate::ace::scanner::ProjectScanner;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, content).expect("write");
}

fn deps_of<'a>(signals: &'a [ProjectSignal], dir: &Path) -> &'a ProjectSignal {
    signals
        .iter()
        .find(|s| s.manifest_path == dir.join("package.json"))
        .expect("signal for dir")
}

/// The audit 2026-10-07 shape: a workspace member referenced by a plain
/// range (`"*"`, `"^0.13.0"`), not a `workspace:` protocol spec.
#[test]
fn workspace_member_with_plain_range_is_not_a_registry_dependency() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(
        &root.join("package.json"),
        r#"{"name":"mono","private":true,"workspaces":["packages/*"],
            "dependencies":{"@4da/agent-framework":"*","react":"^18.2.0"}}"#,
    );
    write(
        &root.join("packages/agent-framework/package.json"),
        r#"{"name":"@4da/agent-framework","version":"0.13.0","dependencies":{"zod":"^3.0.0"}}"#,
    );
    write(
        &root.join("packages/app/package.json"),
        r#"{"name":"@4da/app","dependencies":{"@4da/agent-framework":"^0.13.0","lodash":"^4.17.0"},
            "devDependencies":{"mono":"*"}}"#,
    );

    let signals = ProjectScanner::new().scan_directory(root).expect("scan");
    assert_eq!(deps_of(&signals, root).dependencies, vec!["react"]);
    let app = deps_of(&signals, &root.join("packages/app"));
    assert_eq!(app.dependencies, vec!["lodash"]);
    assert!(
        app.dev_dependencies.is_empty(),
        "{:?}",
        app.dev_dependencies
    );
    assert_eq!(
        deps_of(&signals, &root.join("packages/agent-framework")).dependencies,
        vec!["zod"]
    );
}

#[test]
fn pnpm_workspace_yaml_members_are_local() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(
        &root.join("pnpm-workspace.yaml"),
        "packages:\n  - 'libs/*'\n  - \"!libs/ignored\"\n",
    );
    write(
        &root.join("package.json"),
        r#"{"name":"root","dependencies":{"internal-lib":"^1.0.0","vite":"^5.0.0"}}"#,
    );
    write(
        &root.join("libs/internal/package.json"),
        r#"{"name":"internal-lib"}"#,
    );

    let signals = ProjectScanner::new().scan_directory(root).expect("scan");
    assert_eq!(deps_of(&signals, root).dependencies, vec!["vite"]);
}

#[test]
fn private_package_names_are_dropped_everywhere() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(
        &root.join("tools/package.json"),
        r#"{"name":"acme-internal-tools","private":true}"#,
    );
    write(
        &root.join("site/package.json"),
        r#"{"name":"site","dependencies":{"acme-internal-tools":"^2.0.0","astro":"^4.0.0"}}"#,
    );

    let signals = ProjectScanner::new().scan_directory(root).expect("scan");
    assert_eq!(
        deps_of(&signals, &root.join("site")).dependencies,
        vec!["astro"]
    );
}

#[test]
fn npmrc_routed_scope_is_dropped_public_scope_kept() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(
        &root.join(".npmrc"),
        "@corp:registry=https://npm.pkg.github.com/\n@types:registry=https://registry.npmjs.org/\n",
    );
    write(
        &root.join("package.json"),
        r#"{"name":"svc","dependencies":{"@corp/secret-sdk":"^1.0.0","@types/node":"^20.0.0","express":"^4.0.0"}}"#,
    );

    let mut signals = ProjectScanner::new().scan_directory(root).expect("scan");
    assert_eq!(
        deps_of(&signals, root).dependencies,
        vec!["@types/node", "express"]
    );

    // A user-level ~/.npmrc scope applies to every project.
    let user: HashSet<String> = ["@types".to_string()].into_iter().collect();
    drop_local_npm_deps_with(&mut signals, &user);
    assert_eq!(deps_of(&signals, root).dependencies, vec!["express"]);
}

#[test]
fn unrelated_projects_keep_a_public_name_they_share_with_a_scanned_package() {
    // A clone of a public library (non-private, no workspace) must not strip
    // that library from a DIFFERENT project that depends on it.
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(&root.join("zod-fork/package.json"), r#"{"name":"zod"}"#);
    write(
        &root.join("app/package.json"),
        r#"{"name":"app","dependencies":{"zod":"^3.0.0"}}"#,
    );
    let signals = ProjectScanner::new().scan_directory(root).expect("scan");
    assert_eq!(
        deps_of(&signals, &root.join("app")).dependencies,
        vec!["zod"]
    );
}

#[test]
fn npmrc_scope_parsing() {
    let scopes = private_registry_scopes(
        "registry=https://registry.npmjs.org/\n\
         @Acme:registry = \"https://npm.acme.internal/\"\n\
         @yarnpub:registry=https://registry.yarnpkg.com\n\
         //npm.acme.internal/:_authToken=${TOKEN}\n",
    );
    assert_eq!(scopes, ["@acme".to_string()].into_iter().collect());
}

#[test]
fn pnpm_globs_parse_only_the_packages_list() {
    let globs = pnpm_workspace_globs("packages:\n  - apps/*\n  - 'libs/**'\ncatalog:\n  - nope\n");
    assert_eq!(globs, vec!["apps/*", "libs/**"]);
}
