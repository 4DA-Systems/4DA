// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scope classification through the lockfile walk's store: which installed
//! copies are DIRECT and which are dev (6.2.5 parity audit A-3 / A-4).
//!
//! The shared rule vectors (`tests/conformance/vectors/`) have no
//! scope-classification file and the corpus harness leaves direct/dev labels
//! out, so these end-to-end tests are where the rule is pinned:
//! - a copy is direct only where a manifest declaration resolves (the hoisted
//!   copy, a workspace member's own copy, a pnpm importer's pin, the yarn
//!   entry whose descriptor is the declared range, the crate the project's
//!   own crates list); every other copy of the same name is transitive;
//! - a workspace member's declarations stay direct;
//! - a Python dependency declared only in a PEP 735 group is direct and dev.

use std::path::Path;

use super::*;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).unwrap();
    std::fs::write(path, text).unwrap();
}

/// `name@version -> (is_direct, is_dev)` for every stored instance.
fn walk(dir: &Path) -> std::collections::BTreeMap<String, (bool, bool)> {
    let db = crate::test_utils::test_db();
    let project = "/scope/project";
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let mut report = LockfileReport::default();
    process_lockfile_dir(&db, &scanner, &dir.to_path_buf(), project, &mut report);
    db.get_dependency_instances(project)
        .unwrap()
        .into_iter()
        .map(|i| {
            (
                format!("{}@{}", i.package_name, i.version),
                (i.is_direct, i.is_dev),
            )
        })
        .collect()
}

fn direct(map: &std::collections::BTreeMap<String, (bool, bool)>, id: &str) -> bool {
    map.get(id)
        .unwrap_or_else(|| panic!("{id} not stored: {map:?}"))
        .0
}

#[test]
fn npm_nested_copy_of_a_declared_name_is_transitive_member_copy_is_direct() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"],
            "dependencies":{"debug":"^4.3.0","express":"^4.18.0"}}"#,
    );
    write(
        &dir.join("packages/ui/package.json"),
        r#"{"name":"@me/ui","dependencies":{"lodash":"4.17.20"}}"#,
    );
    write(
        &dir.join("package-lock.json"),
        r#"{"name":"root","lockfileVersion":3,"packages":{
            "":{"name":"root","workspaces":["packages/*"]},
            "node_modules/@me/ui":{"resolved":"packages/ui","link":true},
            "packages/ui":{"name":"@me/ui","dependencies":{"lodash":"4.17.20"}},
            "node_modules/debug":{"version":"4.3.4"},
            "node_modules/express":{"version":"4.18.2"},
            "node_modules/express/node_modules/debug":{"version":"2.6.9"},
            "node_modules/lodash":{"version":"4.17.21"},
            "packages/ui/node_modules/lodash":{"version":"4.17.20"}}}"#,
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4"), "the hoisted, declared copy");
    assert!(!direct(&map, "debug@2.6.9"), "express's own debug");
    assert!(direct(&map, "express@4.18.2"));
    assert!(
        direct(&map, "lodash@4.17.20"),
        "a workspace member's own copy of what it declares stays direct"
    );
    // The hoisted lodash is not what `@me/ui` resolves (it has its own), and
    // nothing else declares lodash: some dependency's copy.
    assert!(!direct(&map, "lodash@4.17.21"));
}

/// npm hoists a member's dependency to the root when nothing conflicts: the
/// hoisted copy is then the member's declared one (MCP F-4 is the opposite
/// bug — it graded these transitive).
#[test]
fn npm_member_declaration_hoisted_to_the_root_stays_direct() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    );
    write(
        &dir.join("packages/ui/package.json"),
        r#"{"name":"@me/ui","dependencies":{"minimist":"^1.2.5"}}"#,
    );
    write(
        &dir.join("package-lock.json"),
        r#"{"name":"root","lockfileVersion":3,"packages":{
            "":{"name":"root","workspaces":["packages/*"]},
            "node_modules/@me/ui":{"resolved":"packages/ui","link":true},
            "packages/ui":{"name":"@me/ui","dependencies":{"minimist":"^1.2.5"}},
            "node_modules/minimist":{"version":"1.2.5"},
            "node_modules/mkdirp/node_modules/minimist":{"version":"0.0.8"},
            "node_modules/mkdirp":{"version":"0.5.1"}}}"#,
    );
    let map = walk(dir);
    assert!(direct(&map, "minimist@1.2.5"));
    assert!(!direct(&map, "minimist@0.0.8"));
}

#[test]
fn npm_v1_nested_copy_is_transitive() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"app","dependencies":{"debug":"^4","express":"^4"}}"#,
    );
    write(
        &dir.join("package-lock.json"),
        r#"{"lockfileVersion":1,"dependencies":{
            "express":{"version":"4.18.2","dependencies":{"debug":{"version":"2.6.9"}}},
            "debug":{"version":"4.3.4"}}}"#,
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4"));
    assert!(!direct(&map, "debug@2.6.9"));
}

#[test]
fn pnpm_only_the_importer_pin_is_direct() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"app","dependencies":{"debug":"^4.3.0","express":"^4.18.0"}}"#,
    );
    write(
        &dir.join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'

importers:

  .:
    dependencies:
      debug:
        specifier: ^4.3.0
        version: 4.3.4
      express:
        specifier: ^4.18.0
        version: 4.18.2

packages:

  debug@2.6.9:
    resolution: {integrity: sha512-a}

  debug@4.3.4:
    resolution: {integrity: sha512-b}

  express@4.18.2:
    resolution: {integrity: sha512-c}

snapshots:

  debug@2.6.9: {}

  debug@4.3.4: {}

  express@4.18.2:
    dependencies:
      debug: 2.6.9
",
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4"));
    assert!(!direct(&map, "debug@2.6.9"), "express's debug");
    assert!(direct(&map, "express@4.18.2"));
}

#[test]
fn yarn_classic_the_entry_matching_the_declared_range_is_direct() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"app","dependencies":{"debug":"^4.3.0","express":"^4.18.0"}}"#,
    );
    write(
        &dir.join("yarn.lock"),
        "# yarn lockfile v1


debug@2.6.9:
  version \"2.6.9\"

debug@^4.1.0, debug@^4.3.0:
  version \"4.3.4\"

express@^4.18.0:
  version \"4.18.2\"
  dependencies:
    debug \"2.6.9\"
",
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4"));
    assert!(!direct(&map, "debug@2.6.9"));
    assert!(direct(&map, "express@4.18.2"));
}

#[test]
fn yarn_berry_npm_protocol_descriptors_match_the_declared_range() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"app","devDependencies":{"nanoid":"^3.3.4"}}"#,
    );
    write(
        &dir.join("yarn.lock"),
        "__metadata:
  version: 6

\"nanoid@npm:^3.3.4\":
  version: 3.3.4
  resolution: \"nanoid@npm:3.3.4\"

\"nanoid@npm:^5.0.0\":
  version: 5.0.7
  resolution: \"nanoid@npm:5.0.7\"

\"app@workspace:.\":
  version: 0.0.0-use.local
  resolution: \"app@workspace:.\"
",
    );
    let map = walk(dir);
    assert!(direct(&map, "nanoid@3.3.4"));
    assert!(!direct(&map, "nanoid@5.0.7"), "another package's nanoid");
}

#[test]
fn a_declared_name_the_lockfile_cannot_place_keeps_every_copy_direct() {
    // A range written differently from the lock's descriptor (here `4.x`
    // against `^4.3.0`): the walk cannot tell which copy it resolves to,
    // and understating directness is not the safe failure.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"app","dependencies":{"debug":"4.x"}}"#,
    );
    write(
        &dir.join("yarn.lock"),
        "# yarn lockfile v1\n\ndebug@2.6.9:\n  version \"2.6.9\"\n\ndebug@^4.3.0:\n  version \"4.3.4\"\n",
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4") && direct(&map, "debug@2.6.9"));
}

#[test]
fn bun_hoisted_and_member_keys_are_direct_nested_keys_are_not() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"],"dependencies":{"debug":"^4","express":"^4"}}"#,
    );
    write(
        &dir.join("packages/a/package.json"),
        r#"{"name":"ws-a","dependencies":{"ms":"^2.0.0"}}"#,
    );
    write(
        &dir.join("bun.lock"),
        r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": { "name": "root", "dependencies": { "debug": "^4", "express": "^4" } },
    "packages/a": { "name": "ws-a", "dependencies": { "ms": "^2.0.0" } },
  },
  "packages": {
    "debug": ["debug@4.3.4", "", { "dependencies": { "ms": "2.1.2" } }, "sha512-a"],
    "express": ["express@4.18.2", "", { "dependencies": { "debug": "2.6.9" } }, "sha512-b"],
    "express/debug": ["debug@2.6.9", "", { "dependencies": { "ms": "2.0.0" } }, "sha512-c"],
    "ms": ["ms@2.1.2", "", {}, "sha512-d"],
    "ws-a/ms": ["ms@2.0.0", "", {}, "sha512-e"],
  }
}"#,
    );
    let map = walk(dir);
    assert!(direct(&map, "debug@4.3.4"));
    assert!(!direct(&map, "debug@2.6.9"), "express/debug is nested");
    assert!(
        direct(&map, "ms@2.0.0"),
        "ws-a's own copy of what it declares"
    );
    assert!(
        !direct(&map, "ms@2.1.2"),
        "ws-a resolves its own copy; the hoisted ms is debug's"
    );
}

/// A virtual workspace: the root declares nothing, the member declares
/// `rand 0.8`, a build-dependency and a dev-dependency; `tempfile` pulls an
/// older `rand`. Before A-3 the member's crates were all transitive (only
/// the root manifest was read) — and had the root declared `rand`, BOTH
/// copies would have been direct.
#[test]
fn cargo_the_crates_own_packages_list_decide_direct_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(&dir.join("Cargo.toml"), "[workspace]\nmembers = [\"a\"]\n");
    write(
        &dir.join("a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies]\nrand = \"0.8\"\n\n\
         [build-dependencies]\nmemoffset = \"0.5\"\n\n[dev-dependencies]\ntempfile = \"3\"\n",
    );
    let lock = r#"version = 3

[[package]]
name = "a"
version = "0.1.0"
dependencies = [
 "memoffset",
 "rand 0.8.5",
 "tempfile",
]

[[package]]
name = "memoffset"
version = "0.5.6"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "rand"
version = "0.7.3"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "rand"
version = "0.8.5"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "tempfile"
version = "3.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "rand 0.7.3",
]
"#;
    let db = crate::test_utils::test_db();
    let project = "/scope/cargo";
    let scanner = crate::ace::scanner::ProjectScanner::new();
    store_cargo_lock(&db, &scanner, dir, project, lock);
    let instances = db.get_dependency_instances(project).unwrap();
    let is_direct = |name: &str, version: &str| {
        instances
            .iter()
            .find(|i| i.package_name == name && i.version == version)
            .unwrap_or_else(|| panic!("{name}@{version} in {instances:?}"))
            .is_direct
    };
    assert!(is_direct("rand", "0.8.5"), "the member's declared rand");
    assert!(!is_direct("rand", "0.7.3"), "tempfile's rand");
    assert!(
        is_direct("memoffset", "0.5.6"),
        "a build-dependency is declared"
    );
    assert!(
        is_direct("tempfile", "3.1.0"),
        "a dev-dependency is declared"
    );

    let rows = db.get_project_dependencies(project).unwrap();
    let rand = rows
        .iter()
        .find(|r| r.package_name == "rand")
        .expect("rand row");
    assert!(rand.is_direct);
    assert_eq!(
        rand.version.as_deref(),
        Some("0.8.5"),
        "the collapsed row keeps the declared copy's version"
    );
}

#[test]
fn cargo_lock_without_local_packages_falls_back_to_the_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
    );
    let lock = "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\n\
                source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
    let db = crate::test_utils::test_db();
    let project = "/scope/cargo-bare";
    let scanner = crate::ace::scanner::ProjectScanner::new();
    store_cargo_lock(&db, &scanner, dir, project, lock);
    let instances = db.get_dependency_instances(project).unwrap();
    assert!(instances.iter().all(|i| i.is_direct), "{instances:?}");
}

/// A-4, the audit's `x-f6c-py-uvlock` case: `urllib3` is declared only in
/// `[dependency-groups] dev`, and uv.lock records no scope.
#[test]
fn pep735_dev_group_dependency_is_direct_and_dev() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(
        &dir.join("pyproject.toml"),
        "[project]\nname = \"f6c\"\nversion = \"0.1.0\"\ndependencies = [\n  \"django==3.2.0\",\n]\n\n\
         [dependency-groups]\ntest = [\"pytest==8.0.0\"]\ndev = [\"urllib3==1.26.4\", {include-group = \"test\"}]\n",
    );
    write(
        &dir.join("uv.lock"),
        r#"version = 1
requires-python = ">=3.8"

[[package]]
name = "django"
version = "3.2"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "f6c"
version = "0.1.0"
source = { virtual = "." }

[[package]]
name = "pytest"
version = "8.0.0"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "urllib3"
version = "1.26.4"
source = { registry = "https://pypi.org/simple" }
"#,
    );
    let map = walk(dir);
    assert_eq!(map["urllib3@1.26.4"], (true, true), "direct and dev");
    assert_eq!(map["pytest@8.0.0"], (true, true), "via include-group");
    assert_eq!(map["django@3.2"], (true, false));
}
