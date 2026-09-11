// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `ace::dep_scope` (extracted via #[path]).

use super::DevScope;
use crate::ace::scanner::ProjectScanner;

/// Shaped like the live `paddle-webhook` lockfile (pnpm v9, 2026-09-10): the
/// importer's only real work is `devDependencies`, and `sandbox` reaches it
/// only as `vercel` -> `sandbox` -> `@vercel/sandbox`. Around that: a runtime
/// root (`hono`), a package both trees reach (`ms`), an orphan nothing
/// reaches (`left-pad`), peer-suffixed keys and references, optional children
/// on both sides, an aliased child, and the `{}` / trailing-list shapes v9
/// snapshots use.
const V9_PADDLE_SHAPED: &str = "\
lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    dependencies:
      '@hono/node-server':
        specifier: ^2.1.0
        version: 2.1.0(hono@4.13.5)
      hono:
        specifier: ^4.13.5
        version: 4.13.5
    devDependencies:
      vercel:
        specifier: ^54.20.1
        version: 54.20.1(@emnapi/core@1.10.0)(@emnapi/runtime@1.10.0)

packages:

  '@hono/node-server@2.1.0':
    resolution: {integrity: sha512-a}
    engines: {node: '>=18.14.1'}
    peerDependencies:
      hono: ^4

  '@vercel/sandbox@2.1.1':
    resolution: {integrity: sha512-b}

  bufferutil@4.0.8:
    resolution: {integrity: sha512-c}

  debug@4.4.3:
    resolution: {integrity: sha512-d}

  fsevents@2.3.3:
    resolution: {integrity: sha512-e}
    os: [darwin]

  hono@4.13.5:
    resolution: {integrity: sha512-f}

  left-pad@1.3.0:
    resolution: {integrity: sha512-g}

  ms@2.1.3:
    resolution: {integrity: sha512-h}

  sandbox@3.1.2:
    resolution: {integrity: sha512-i}

  string-width@4.2.3:
    resolution: {integrity: sha512-j}

  vercel@54.20.1:
    resolution: {integrity: sha512-k}
    engines: {node: '>= 18'}
    hasBin: true

snapshots:

  '@hono/node-server@2.1.0(hono@4.13.5)':
    dependencies:
      hono: 4.13.5
      ms: 2.1.3
    optionalDependencies:
      bufferutil: 4.0.8

  '@vercel/sandbox@2.1.1':
    dependencies:
      ms: 2.1.3
      string-width-cjs: string-width@4.2.3

  bufferutil@4.0.8: {}

  debug@4.4.3:
    dependencies:
      ms: 2.1.3

  fsevents@2.3.3:
    optional: true

  hono@4.13.5: {}

  left-pad@1.3.0: {}

  ms@2.1.3: {}

  sandbox@3.1.2:
    dependencies:
      '@vercel/sandbox': 2.1.1
      debug: 4.4.3
    transitivePeerDependencies:
      - supports-color

  string-width@4.2.3: {}

  vercel@54.20.1(@emnapi/core@1.10.0)(@emnapi/runtime@1.10.0):
    dependencies:
      sandbox: 3.1.2
    optionalDependencies:
      fsevents: 2.3.3
";

#[test]
fn a_package_reached_only_through_dev_tooling_is_dev_only() {
    let scope = DevScope::from_pnpm_lock(V9_PADDLE_SHAPED);
    for (name, version) in [
        ("vercel", "54.20.1"),
        ("sandbox", "3.1.2"),
        ("@vercel/sandbox", "2.1.1"),
        ("debug", "4.4.3"),
    ] {
        assert!(
            scope.is_dev_only(name, version),
            "{name}@{version} ships only with dev tooling"
        );
        assert_eq!(scope.label(name, version), "dev");
    }
}

#[test]
fn runtime_shared_and_orphaned_packages_are_not_dev_only() {
    let scope = DevScope::from_pnpm_lock(V9_PADDLE_SHAPED);
    assert!(!scope.is_dev_only("hono", "4.13.5"), "a runtime root");
    assert_eq!(scope.label("hono", "4.13.5"), "runtime");
    assert!(
        !scope.is_dev_only("ms", "2.1.3"),
        "reached from BOTH roots: it ships"
    );
    assert_eq!(scope.label("ms", "2.1.3"), "runtime");
    assert!(
        !scope.is_dev_only("left-pad", "1.3.0"),
        "reached from neither root: unknown is never dev"
    );
    assert_eq!(scope.label("left-pad", "1.3.0"), "unknown");
}

#[test]
fn peer_suffixes_aliases_and_optional_children_resolve_to_real_packages() {
    let scope = DevScope::from_pnpm_lock(V9_PADDLE_SHAPED);
    // `@hono/node-server@2.1.0(hono@4.13.5)` is `@hono/node-server` 2.1.0.
    assert_eq!(scope.label("@hono/node-server", "2.1.0"), "runtime");
    // A runtime package's optional dependency ships with it …
    assert_eq!(scope.label("bufferutil", "4.0.8"), "runtime");
    // … and a dev package's optional dependency is dev tooling too.
    assert!(scope.is_dev_only("fsevents", "2.3.3"));
    // `string-width-cjs: string-width@4.2.3` is an alias of `string-width`.
    assert!(scope.is_dev_only("string-width", "4.2.3"));
}

/// The identities `parse_pnpm_lock_yaml` hands `dependency_instances` must be
/// the ones the graph is keyed by, or the verdict never reaches a row.
#[test]
fn every_instance_the_walk_stores_gets_its_verdict() {
    let scope = DevScope::from_pnpm_lock(V9_PADDLE_SHAPED);
    let mut dev_only: Vec<String> = ProjectScanner::parse_pnpm_lock_yaml(V9_PADDLE_SHAPED)
        .iter()
        .filter(|(name, version)| scope.is_dev_only(name, version))
        .map(|(name, _)| name.clone())
        .collect();
    dev_only.sort();
    assert_eq!(
        dev_only,
        vec![
            "@vercel/sandbox",
            "debug",
            "fsevents",
            "sandbox",
            "string-width",
            "vercel"
        ]
    );
}

/// npm computes the verdict itself; the entry's `"dev": true` is it.
#[test]
fn package_lock_v3_uses_npms_own_dev_flag() {
    let lock = r#"{
        "name": "app", "lockfileVersion": 3, "requires": true,
        "packages": {
            "": { "name": "app",
                  "dependencies": { "express": "^4.18.2" },
                  "devDependencies": { "jest": "^29.7.0" } },
            "node_modules/express": { "version": "4.18.2" },
            "node_modules/ms": { "version": "2.1.3" },
            "node_modules/jest": { "version": "29.7.0", "dev": true },
            "node_modules/@jest/core": { "version": "29.7.0", "dev": true },
            "node_modules/jest/node_modules/ms": { "version": "2.0.0", "dev": true },
            "node_modules/fsevents": { "version": "2.3.3", "devOptional": true, "optional": true },
            "packages/app-lib": { "name": "app-lib", "version": "0.1.0" }
        }
    }"#;
    let scope = DevScope::from_package_lock(lock);
    assert!(scope.is_dev_only("jest", "29.7.0"));
    assert!(scope.is_dev_only("@jest/core", "29.7.0"));
    assert!(
        scope.is_dev_only("ms", "2.0.0"),
        "a nested copy keeps its own verdict"
    );
    assert!(!scope.is_dev_only("ms", "2.1.3"));
    assert!(!scope.is_dev_only("express", "4.18.2"));
    assert!(
        !scope.is_dev_only("fsevents", "2.3.3"),
        "devOptional is also an optional RUNTIME dependency"
    );
    assert_eq!(
        scope.label("app-lib", "0.1.0"),
        "unknown",
        "a workspace member directory is not an installed package"
    );
}

#[test]
fn package_lock_v1_tree_is_walked_down_to_nested_copies() {
    let lock = r#"{ "lockfileVersion": 1, "dependencies": {
        "express": { "version": "4.18.2", "requires": { "debug": "2.6.9" } },
        "debug": { "version": "2.6.9" },
        "mocha": { "version": "10.0.0", "dev": true,
                   "dependencies": { "debug": { "version": "4.3.4", "dev": true } } }
    } }"#;
    let scope = DevScope::from_package_lock(lock);
    assert!(scope.is_dev_only("mocha", "10.0.0"));
    assert!(scope.is_dev_only("debug", "4.3.4"));
    assert!(!scope.is_dev_only("debug", "2.6.9"));
    assert!(!scope.is_dev_only("express", "4.18.2"));
}

/// v6 keeps its edges in `packages:` and its roots at the top level — and
/// keeps working. Its scoped keys (`/@types/node@20.11.0`) used to parse as
/// the package `@types`.
#[test]
fn pnpm_v6_still_works() {
    let lock = "\
lockfileVersion: '6.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

dependencies:
  express:
    specifier: ^4.18.2
    version: 4.18.2

devDependencies:
  '@types/express':
    specifier: ^4.17.21
    version: 4.17.21
  jest:
    specifier: ^29.7.0
    version: 29.7.0

packages:

  /@types/express@4.17.21:
    resolution: {integrity: sha512-a}
    dependencies:
      '@types/node': 20.11.0
    dev: true

  /@types/node@20.11.0:
    resolution: {integrity: sha512-b}
    dev: true

  /body-parser@1.20.1:
    resolution: {integrity: sha512-c}
    dev: false

  /express@4.18.2:
    resolution: {integrity: sha512-d}
    dependencies:
      body-parser: 1.20.1
    dev: false

  /jest@29.7.0:
    resolution: {integrity: sha512-e}
    dependencies:
      body-parser: 1.20.1
    dev: true
";
    let scope = DevScope::from_pnpm_lock(lock);
    assert!(scope.is_dev_only("@types/express", "4.17.21"));
    assert!(scope.is_dev_only("@types/node", "20.11.0"));
    assert!(scope.is_dev_only("jest", "29.7.0"));
    assert!(!scope.is_dev_only("express", "4.18.2"));
    assert!(
        !scope.is_dev_only("body-parser", "1.20.1"),
        "reached from both roots"
    );
}

#[test]
fn pnpm_v5_inline_roots_and_underscore_peer_suffixes() {
    let lock = "\
lockfileVersion: 5.4

specifiers:
  express: ^4.18.2
  mocha: ^10.0.0

dependencies:
  express: 4.18.2

devDependencies:
  mocha: 10.0.0

packages:

  /debug/2.6.9:
    resolution: {integrity: sha512-a}
    dev: false

  /debug/4.3.4_supports-color@8.1.1:
    resolution: {integrity: sha512-b}
    dependencies:
      supports-color: 8.1.1
    dev: true

  /express/4.18.2:
    resolution: {integrity: sha512-c}
    dependencies:
      debug: 2.6.9
    dev: false

  /mocha/10.0.0:
    resolution: {integrity: sha512-d}
    dependencies:
      debug: 4.3.4_supports-color@8.1.1
    dev: true

  /supports-color/8.1.1:
    resolution: {integrity: sha512-e}
    dev: true
";
    let scope = DevScope::from_pnpm_lock(lock);
    assert!(scope.is_dev_only("mocha", "10.0.0"));
    assert!(
        scope.is_dev_only("debug", "4.3.4"),
        "the v5 `_peer` suffix is not part of the version"
    );
    assert!(scope.is_dev_only("supports-color", "8.1.1"));
    assert!(!scope.is_dev_only("debug", "2.6.9"));
    assert!(!scope.is_dev_only("express", "4.18.2"));
}

/// The walk attributes a workspace's whole lockfile to the directory holding
/// it, so every importer's roots count — and a `link:` is not a package.
#[test]
fn every_importer_of_a_workspace_contributes_roots() {
    let lock = "\
lockfileVersion: '9.0'

importers:

  .:
    devDependencies:
      turbo:
        specifier: ^2.0.0
        version: 2.0.0

  packages/api:
    dependencies:
      '@repo/shared':
        specifier: workspace:*
        version: link:../shared
      zod:
        specifier: ^3.24.4
        version: 3.24.4
    devDependencies:
      vitest:
        specifier: ^4.1.11
        version: 4.1.11

packages:

  turbo@2.0.0:
    resolution: {integrity: sha512-a}

  vitest@4.1.11:
    resolution: {integrity: sha512-b}

  zod@3.24.4:
    resolution: {integrity: sha512-c}

snapshots:

  turbo@2.0.0: {}

  vitest@4.1.11: {}

  zod@3.24.4: {}
";
    let scope = DevScope::from_pnpm_lock(lock);
    assert_eq!(
        scope.label("zod", "3.24.4"),
        "runtime",
        "a workspace member's runtime dependency ships"
    );
    assert!(scope.is_dev_only("vitest", "4.1.11"));
    assert!(scope.is_dev_only("turbo", "2.0.0"));
    assert_eq!(scope.label("@repo/shared", "link:../shared"), "unknown");
}

#[test]
fn a_lockfile_it_cannot_read_marks_nothing_dev() {
    for content in [
        "",
        "not: a lockfile",
        "{ broken",
        // Packages but no importers: no roots, so no verdict.
        "lockfileVersion: '9.0'\n\npackages:\n\n  lodash@4.17.21:\n    resolution: {integrity: x}\n",
    ] {
        let scope = DevScope::from_pnpm_lock(content);
        assert!(
            !scope.is_dev_only("lodash", "4.17.21"),
            "no roots, no verdict: {content:?}"
        );
        assert_eq!(scope.label("lodash", "4.17.21"), "unknown");
    }
    assert!(!DevScope::from_package_lock("{ broken json").is_dev_only("lodash", "4.17.21"));
}

/// Production-shape verification against the repository's OWN pnpm v9
/// lockfiles — the files that carried the live defect. Fixtures cannot prove
/// the parser misses no real-world shape; a real lockfile can, because pnpm
/// keeps only packages some importer reaches, so any edge the parser drops
/// strands a package as `unknown`. Ignored because the files change with
/// every dependency bump; run it by hand after touching either parser:
///   `cargo test --lib real_repo_lockfiles -- --ignored --nocapture`
#[test]
#[ignore = "reads the repository's tracked lockfiles, which change with every bump"]
fn real_repo_lockfiles_parse_with_the_v9_graph() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |relative: &str| {
        std::fs::read_to_string(repo.join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
    };
    let tally = |content: &str| {
        let scope = DevScope::from_pnpm_lock(content);
        let packages = ProjectScanner::parse_pnpm_lock_yaml(content);
        let unknown: Vec<String> = packages
            .iter()
            .filter(|(n, v)| scope.label(n, v) == "unknown")
            .map(|(n, v)| format!("{n}@{v}"))
            .collect();
        let dev = packages
            .iter()
            .filter(|(n, v)| scope.is_dev_only(n, v))
            .count();
        (scope, packages, dev, unknown)
    };

    // paddle-webhook declares no runtime dependencies: everything it holds
    // ships only with dev tooling — sandbox included.
    let paddle = read("paddle-webhook/pnpm-lock.yaml");
    let edges = ProjectScanner::parse_pnpm_lock_edges(&paddle).len();
    let (_, packages, dev, unknown) = tally(&paddle);
    println!(
        "paddle-webhook: {} packages, {edges} edges, {dev} dev-only",
        packages.len()
    );
    assert!(edges > 0, "the v9 graph is read at all");
    assert!(unknown.is_empty(), "stranded: {unknown:?}");
    assert_eq!(dev, packages.len(), "a devDependencies-only project");

    // mcp-4da-server and the app itself: runtime AND dev roots.
    for (lockfile, runtime_pkg, dev_pkg) in [
        ("mcp-4da-server/pnpm-lock.yaml", "hono", "vitest"),
        ("pnpm-lock.yaml", "react", "vitest"),
    ] {
        let content = read(lockfile);
        let (scope, packages, dev, unknown) = tally(&content);
        println!(
            "{lockfile}: {} packages, {dev} dev-only, {} stranded",
            packages.len(),
            unknown.len()
        );
        assert!(unknown.is_empty(), "{lockfile} stranded: {unknown:?}");
        for (name, version) in &packages {
            if name == runtime_pkg {
                assert_eq!(scope.label(name, version), "runtime", "{name}@{version}");
            }
            if name == dev_pkg {
                assert!(scope.is_dev_only(name, version), "{name}@{version}");
            }
        }
    }
}
