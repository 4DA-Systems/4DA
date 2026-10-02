// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the lockfile walk (`ace_commands::dependencies`) — extracted so
//! the production module stays under the Rust file-size ceiling (loaded via
//! #[path], the dependency_health_tests.rs precedent).

use super::*;

/// A repository: `.git/` with a config naming `origin` and a fresh `HEAD`
/// (fresh = recent git activity, so relevance is 1.0).
fn repo(dir: &Path, origin: &str) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(
        dir.join(".git").join("config"),
        format!("[remote \"origin\"]\n\turl = {origin}\n"),
    )
    .unwrap();
    std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
}

fn lockfile(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, "{}").unwrap();
    path
}

fn age_days(path: &Path, days: u64) {
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

/// THE live case (2026-09-04 audit): `navcal/vercel-workflow` — a clone
/// of `vercel/workflow` nested in the user's own repo — fed 1,811
/// `user_dependencies` rows. Its lockfile must not be walked; a nested
/// checkout of the SAME repository and a plain subproject still are.
#[test]
fn walk_skips_nested_foreign_repo_and_keeps_own_checkouts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("navcal");
    repo(&root, "https://github.com/runyourempire/NAVCAL.git");
    lockfile(&root, "package-lock.json");

    let foreign = root.join("vercel-workflow");
    repo(&foreign, "https://github.com/vercel/workflow.git");
    lockfile(&foreign, "package-lock.json");
    lockfile(&foreign.join("packages").join("core"), "Cargo.lock");

    let mirror = root.join("tools").join("mirror");
    repo(&mirror, "git@github.com:runyourempire/NAVCAL.git");
    lockfile(&mirror, "Cargo.lock");

    let member = root.join("crates").join("member");
    lockfile(&member, "Cargo.lock");

    let selected = collect_lockfile_dirs(&[root.clone()]);
    assert!(selected.contains(&root), "own repo root: {selected:?}");
    assert!(
        selected.contains(&mirror),
        "same-remote checkout: {selected:?}"
    );
    assert!(selected.contains(&member), "plain subproject: {selected:?}");
    assert!(
        !selected.iter().any(|d| d.starts_with(&foreign)),
        "nested foreign clone (and everything under it) must be skipped: {selected:?}"
    );
}

/// CHANGED 2026-09-08: this asserted that a project idle 120 days is gated
/// out — the defect, not the rule. `navcal` (dormant since ~2025-11, 31
/// known-vulnerable packages) was invisible on every surface because of it.
/// A dormant project is now INDEXED and named once by
/// `evidence::collapse_dormant_alerts`; only scaffolding paths are skipped.
#[test]
fn walk_indexes_a_dormant_project_and_a_fresh_one_alike() {
    let tmp = tempfile::tempdir().unwrap();
    let dormant = tmp.path().join("kairos-mvp");
    age_days(&lockfile(&dormant, "package-lock.json"), 120);
    let fresh = tmp.path().join("active-app");
    lockfile(&fresh, "Cargo.lock");

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert!(selected.contains(&fresh), "{selected:?}");
    assert!(
        selected.contains(&dormant),
        "a dormant project is still the user's — its advisories are still true: {selected:?}"
    );
}

/// The negative half of the gate above: dormancy is forgiven, scaffolding is
/// not. An `examples/` tree is scaffolding whatever its git log says, and
/// admitting it was never the point of the dormancy change.
#[test]
fn walk_still_skips_example_and_fixture_paths_however_fresh() {
    let tmp = tempfile::tempdir().unwrap();
    let example = tmp.path().join("examples").join("hello-world");
    lockfile(&example, "Cargo.lock"); // touched now -> recency 1.0
    let real = tmp.path().join("real-app");
    lockfile(&real, "Cargo.lock");

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert!(selected.contains(&real), "{selected:?}");
    assert!(
        !selected.contains(&example),
        "an example path stays out on its PATH score, not its recency: {selected:?}"
    );
}

/// A dormant EXAMPLE path fails both halves — the dormancy waiver must not
/// become a back door for scaffolding.
#[test]
fn walk_skips_a_path_that_is_both_scaffolding_and_dormant() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("fixtures").join("sample-app");
    age_days(&lockfile(&fixture, "Cargo.lock"), 200);

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert!(
        !selected.contains(&fixture),
        "scaffolding is skipped on its path score regardless of age: {selected:?}"
    );
}

/// `[target.'cfg(...)'.dependencies]` entries are DIRECT deps — the manifest
/// names them. `process_cargo_lock` built its direct set from
/// `dependencies + dev_dependencies` only, so every cfg-gated crate was
/// classified transitive and lost the direct-dependency urgency rank.
#[test]
fn cfg_gated_manifest_deps_are_direct_not_transitive() {
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let manifest = r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = "1"

[dev-dependencies]
tempfile = "3"

[target.'cfg(windows)'.dependencies]
windows-sys = "0.60"
"#;
    let mut signal = crate::ace::scanner::ProjectSignal {
        manifest_type: crate::ace::scanner::ManifestType::CargoToml,
        manifest_path: PathBuf::from("Cargo.toml"),
        project_name: None,
        languages: vec!["rust".to_string()],
        frameworks: Vec::new(),
        dependencies: Vec::new(),
        dev_dependencies: Vec::new(),
        indirect_dependencies: Vec::new(),
        target_dependencies: Vec::new(),
        detected_at: String::new(),
        project_license: None,
        project_relevance: 1.0,
    };
    scanner.parse_cargo_toml(manifest, &mut signal);
    assert_eq!(
        signal.target_dependencies,
        vec![("windows-sys".to_string(), "cfg(windows)".to_string())],
        "the parser files cfg-gated deps separately — which is what the walk dropped"
    );

    // The direct set the walk now builds.
    let mut direct = signal.dependencies.clone();
    direct.extend(signal.dev_dependencies.clone());
    direct.extend(signal.target_dependencies.iter().map(|(n, _)| n.clone()));
    assert!(direct.contains(&"windows-sys".to_string()));
    assert!(direct.contains(&"serde".to_string()));
    assert!(direct.contains(&"tempfile".to_string()));

    // Negative half: a crate the manifest never names is still transitive.
    assert!(!direct.contains(&"windows-targets".to_string()));
}

/// Directories with nothing to read are never selected, and the build /
/// cache / agent-infra skip list still prunes descent.
#[test]
fn walk_selects_only_dirs_holding_a_lockfile() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    lockfile(&app, "Cargo.lock");
    lockfile(
        &app.join("node_modules").join("left-pad"),
        "package-lock.json",
    );
    lockfile(
        &app.join(".claude").join("worktrees").join("agent-x"),
        "Cargo.lock",
    );
    std::fs::create_dir_all(app.join("src")).unwrap();

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert_eq!(selected, vec![app]);
}

/// A workspace root's lockfile, cut down from the live 2026-10-02 shape: the
/// members `acme-cli` / `acme-core` have no `source`; `tempfile` is a
/// dev-dependency and `fastrand` reaches the project only through it.
const WORKSPACE_LOCK: &str = r#"version = 4

[[package]]
name = "acme-cli"
version = "0.8.8"
dependencies = [
 "acme-core",
 "serde",
 "tempfile",
]

[[package]]
name = "acme-core"
version = "0.8.8"
dependencies = [
 "serde",
]

[[package]]
name = "fastrand"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "tempfile"
version = "3.10.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "fastrand",
]
"#;

/// Workspace members are not installs of themselves, and dev-only crates keep
/// their dev flag through the walk (it used to write `is_dev = 0` for every
/// crate, overwriting the manifest's `[dev-dependencies]`).
#[test]
fn cargo_walk_skips_workspace_members_and_keeps_dev_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\n\
         acme-core = { path = \"crates/acme-core\", version = \"0.8.8\" }\n\
         serde = \"1\"\n\n[dev-dependencies]\ntempfile = \"3\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("Cargo.lock"), WORKSPACE_LOCK).unwrap();

    let db = crate::test_utils::test_db();
    let project = "/ws/acme";
    // A stale row from before the fix: the member stored as an install.
    db.store_transitive_dependency(project, "acme-core", Some("0.8.8"), "rust", false)
        .unwrap();
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let (_, stored) = store_cargo_lock(&db, &scanner, dir, project, WORKSPACE_LOCK);
    assert!(
        !stored.iter().any(|(n, _)| n.starts_with("acme-")),
        "members are not packages: {stored:?}"
    );

    let rows = db.get_project_dependencies(project).unwrap();
    let row = |name: &str| rows.iter().find(|r| r.package_name == name);
    assert!(row("acme-core").is_none(), "the stale member row is pruned");
    assert!(row("acme-cli").is_none());
    assert!(!row("serde").expect("serde").is_dev);
    assert!(
        row("tempfile").expect("tempfile").is_dev,
        "declared dev-only"
    );
    assert!(
        row("fastrand").expect("fastrand").is_dev,
        "reached only via tempfile"
    );

    let instances = db.get_dependency_instances(project).unwrap();
    assert!(!instances
        .iter()
        .any(|i| i.package_name.starts_with("acme-")));
    let inst = |name: &str| {
        instances
            .iter()
            .find(|i| i.package_name == name)
            .unwrap_or_else(|| panic!("{name} instance"))
    };
    assert_eq!(inst("tempfile").scope, "dev");
    assert!(inst("tempfile").is_dev && inst("tempfile").is_direct);
    assert_eq!(inst("serde").scope, "runtime");
    assert!(!inst("serde").is_dev);
}
