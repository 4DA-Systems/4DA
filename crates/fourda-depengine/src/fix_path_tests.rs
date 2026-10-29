// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `fix_path`, moved from the app's `osv/fix_path_tests.rs`
//! (single-engine C0.4; the tests that need a real directory stayed there).
//! The oracle cases are the 2026-10-10 fix-path oracle (apply the
//! recommendation in a copy, re-resolve, re-scan with osv-scanner) with the
//! advisory ranges pinned from the fixture mirror.

use super::*;
use crate::fix_target::clean_version;
use crate::lockfile::npm_bundle::npm_bundle_owner;
use crate::needs::{resolve, Answer, Need};
use crate::npm_range::requirement_admits;

fn ranges(json: &[&str]) -> Vec<Option<String>> {
    json.iter().map(|r| Some((*r).to_string())).collect()
}

fn walk(installed: &str, owned: &[Option<String>]) -> Option<String> {
    let refs: Vec<&Option<String>> = owned.iter().collect();
    clean_version("", installed, &refs)
}

const fn sem(intro: &'static str, fixed: &'static str) -> (&'static str, &'static str) {
    (intro, fixed)
}

fn semver_ranges(pairs: &[(&str, &str)]) -> Vec<Option<String>> {
    pairs
        .iter()
        .map(|(i, f)| {
            Some(format!(
                r#"[{{"type":"SEMVER","events":[{{"introduced":"{i}"}},{{"fixed":"{f}"}}]}}]"#
            ))
        })
        .collect()
}

// ---- (a) target selection: the smallest version no known advisory affects ----

/// openssl 0.10.38 (nushell): every advisory's own fix tops out at 0.10.79,
/// but GHSA-phqj-4mhp-q6mq affects 0.10.50..<0.10.80 — 0.10.79 is itself
/// vulnerable. The clean target is 0.10.80.
#[test]
fn openssl_target_steps_past_a_window_the_max_fix_lands_in() {
    let openssl = semver_ranges(&[
        sem("0.9.7", "0.10.48"),
        sem("0.10.39", "0.10.72"),
        sem("0.10.24", "0.10.78"),
        sem("0.10.0", "0.10.55"),
        sem("0.9.24", "0.10.78"),
        sem("0.10.50", "0.10.80"),
        sem("0", "0.10.66"),
        sem("0.10.0", "0.10.70"),
        sem("0.10.29", "0.10.60"),
        sem("0.9.7", "0.10.79"),
        sem("0.10.0", "0.10.79"),
        sem("0.0.0-0", "0.10.48"),
    ]);
    assert_eq!(walk("0.10.38", &openssl).as_deref(), Some("0.10.80"));
    // The per-advisory maximum alone is the 0.10.79 the TS engine printed.
    let without_phqj: Vec<Option<String>> = openssl
        .iter()
        .filter(|r| !r.as_deref().is_some_and(|s| s.contains("0.10.80")))
        .cloned()
        .collect();
    assert_eq!(walk("0.10.38", &without_phqj).as_deref(), Some("0.10.79"));
}

#[test]
fn single_window_targets_are_the_fix() {
    let jsonwebtoken = semver_ranges(&[sem("0", "9.0.0"), sem("0", "9.0.0"), sem("0", "4.2.2")]);
    assert_eq!(walk("8.5.1", &jsonwebtoken).as_deref(), Some("9.0.0"));
    let minimist = ranges(&[
        r#"[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"fixed":"1.2.6"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"0.2.4"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.2"}]}]"#,
    ]);
    assert_eq!(walk("1.2.5", &minimist).as_deref(), Some("1.2.6"));
    let mio = semver_ranges(&[sem("0.7.2", "0.8.11"), sem("0.7.0", "0.7.6")]);
    assert_eq!(walk("0.8.0", &mio).as_deref(), Some("0.8.11"));
    let lodash = semver_ranges(&[
        sem("0", "4.18.0"),
        sem("4.0.0", "4.18.0"),
        sem("4.0.0", "4.17.23"),
        sem("0", "4.17.21"),
    ]);
    assert_eq!(walk("4.17.21", &lodash).as_deref(), Some("4.18.0"));
}

/// next-auth 4.22.1: one advisory per release line (4.x and 5.0 betas);
/// the 4.x install's clean target stays on 4.x.
#[test]
fn next_auth_stays_on_its_release_line() {
    let next_auth = ranges(&[
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.24.12"}]},{"type":"SEMVER","events":[{"introduced":"5.0.0-beta.0"},{"fixed":"5.0.0-beta.30"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"4.10.3"},{"fixed":"4.24.15"}]},{"type":"SEMVER","events":[{"introduced":"5.0.0-beta.1"},{"fixed":"5.0.0-beta.32"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.24.5"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"5.0.0-beta.1"},{"fixed":"5.0.0-beta.32"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.24.15"}]}]"#,
    ]);
    assert_eq!(walk("4.22.1", &next_auth).as_deref(), Some("4.24.15"));
}

/// braces 3.0.2: GHSA-vfj7 is `last_affected: 3.0.3` with no published fix,
/// so no version clears everything; the walk declines rather than guessing
/// (the plan then shows 3.0.3 as "clears the advisories that have a fix").
#[test]
fn braces_has_no_clean_version_to_claim() {
    let braces = ranges(&[
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"3.0.3"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"last_affected":"3.0.3"}]}]"#,
    ]);
    assert_eq!(walk("3.0.2", &braces), None);
}

// ---- (b) refresh or parent: the three "waiting on upstream" cases ----

/// minimist 1.2.5 (proshop-mern, npm): package-lock records mkdirp's
/// requirement `^1.2.5`, which admits 1.2.6 — a refresh is enough.
#[test]
fn minimist_npm_requirement_admits_the_fix() {
    assert_eq!(
        refresh_verdict("1.2.5", "1.2.6", &[Requirement::npm("^1.2.5")]),
        Refresh::Enough(Basis::Requirement)
    );
    assert_eq!(
        Manager::Npm
            .refresh_command("minimist", "1.2.5", "1.2.6")
            .as_deref(),
        Some("npm update minimist")
    );
}

/// braces 3.0.2 (taxonomy, pnpm): pnpm-lock records only the resolution, so
/// the verdict is inferred — 3.0.2 -> 3.0.3 is a patch inside every caret or
/// tilde range that admits 3.0.2.
#[test]
fn braces_pnpm_patch_is_a_semver_refresh() {
    assert_eq!(
        refresh_verdict("3.0.2", "3.0.3", &[]),
        Refresh::Enough(Basis::Semver)
    );
    assert_eq!(
        Manager::Pnpm
            .refresh_command("braces", "3.0.2", "3.0.3")
            .as_deref(),
        Some("pnpm update braces --depth Infinity")
    );
    // Read from the installed parent (micromatch requires ^3.0.2): definitive.
    assert_eq!(
        refresh_verdict("3.0.2", "3.0.3", &[Requirement::npm("^3.0.2")]),
        Refresh::Enough(Basis::Requirement)
    );
}

/// mio 0.8.0 (nushell, Cargo): tokio's registry manifest requires
/// `mio = "0.8.1"` (a Cargo caret), which admits 0.8.11.
#[test]
fn mio_cargo_requirement_admits_the_fix() {
    assert_eq!(
        refresh_verdict("0.8.0", "0.8.11", &[Requirement::cargo("0.8.1")]),
        Refresh::Enough(Basis::Requirement)
    );
    assert_eq!(
        Manager::Cargo
            .refresh_command("mio", "0.8.0", "0.8.11")
            .as_deref(),
        Some("cargo update -p mio@0.8.0 --precise 0.8.11")
    );
    // Without the checkout, the same answer by semver inference.
    assert_eq!(
        refresh_verdict("0.8.0", "0.8.11", &[]),
        Refresh::Enough(Basis::Semver)
    );
}

#[test]
fn an_excluding_requirement_sends_the_fix_to_the_parent() {
    // navcal undici 5.28.4 pinned exactly by @vercel/node.
    assert_eq!(
        refresh_verdict("5.28.4", "5.28.5", &[Requirement::npm("5.28.4")]),
        Refresh::ParentMustMove(Basis::Requirement)
    );
    // A Cargo `=` pin, and one of two declarations excluding it.
    assert_eq!(
        refresh_verdict("0.8.0", "0.8.11", &[Requirement::cargo("=0.8.0")]),
        Refresh::ParentMustMove(Basis::Requirement)
    );
    assert_eq!(
        refresh_verdict(
            "0.8.0",
            "0.8.11",
            &[
                Requirement::cargo("0.8"),
                Requirement::cargo("~0.8.0, <0.8.5")
            ]
        ),
        Refresh::ParentMustMove(Basis::Requirement)
    );
    // rmcp 1.7.0 -> 2.1.0 with no requirement read: a semver-incompatible jump.
    assert_eq!(
        refresh_verdict("1.7.0", "2.1.0", &[]),
        Refresh::ParentMustMove(Basis::Semver)
    );
    // A wide requirement admits even a major jump.
    assert_eq!(
        refresh_verdict("1.1.11", "5.0.12", &[Requirement::npm(">=1.1.7")]),
        Refresh::Enough(Basis::Requirement)
    );
}

#[test]
fn unreadable_inputs_are_unknown_never_a_guess() {
    // An unreadable requirement falls back to the versions; unreadable
    // versions are unknown.
    assert_eq!(
        refresh_verdict("2.0.0", "2.0.1", &[Requirement::npm("1 - 3")]),
        Refresh::Enough(Basis::Semver)
    );
    assert_eq!(refresh_verdict("garbage", "1.0.0", &[]), Refresh::Unknown);
    // Yarn 1 has no command that moves one transitive copy in place.
    assert_eq!(
        Manager::YarnClassic.refresh_command("braces", "3.0.2", "3.0.3"),
        None
    );
    assert_eq!(
        Manager::YarnBerry
            .refresh_command("braces", "3.0.2", "3.0.3")
            .as_deref(),
        Some("yarn up -R braces")
    );
}

#[test]
fn npm_requirements_are_read_with_npm_semantics() {
    assert_eq!(requirement_admits("5.28.4", "5.28.5"), Some(false));
    assert_eq!(requirement_admits("^1.1.7", "1.1.12"), Some(true));
    assert_eq!(requirement_admits("~7.4.0", "7.4.9"), Some(true));
    assert_eq!(requirement_admits(">= 1.0.0 < 2.0.0", "2.1.0"), Some(false));
    assert_eq!(requirement_admits("^2.0.0 || ^3.0.0", "3.4.1"), Some(true));
    assert_eq!(requirement_admits("1.x", "1.4.0"), Some(true));
    assert_eq!(
        requirement_admits("npm:string-width@^4.2.0", "4.2.3"),
        Some(true)
    );
    assert_eq!(requirement_admits("workspace:*", "1.0.0"), None);
    assert_eq!(requirement_admits("latest", "1.0.0"), None, "a dist-tag");
    assert_eq!(requirement_admits(">= 4.21.0", "5.0.0"), Some(true));
    assert_eq!(requirement_admits(">=1.0.0 <2.0.0", "2.0.0"), Some(false));
    assert_eq!(requirement_admits("1.2.3 - 2.3.4", "2.3.4"), Some(true));
    assert_eq!(requirement_admits("1.2.3 - 2.3.4", "2.3.5"), Some(false));
    assert_eq!(requirement_admits("1 - 3", "2.0.0"), None);
    assert_eq!(requirement_admits(">=", "1.0.0"), None, "dangling operator");
    assert_eq!(requirement_admits("*", "9.9.9"), Some(true));
    // Cargo dialect: a bare version is a caret.
    assert_eq!(Requirement::cargo("0.8.1").admits("0.8.11"), Some(true));
    assert_eq!(Requirement::cargo("0.8.1").admits("0.9.0"), Some(false));
}

// ---- reading the parent's own requirement ----

#[test]
fn cargo_manifest_requirements_in_every_shape() {
    // crates.io's normalized form, as in tokio 1.x's published Cargo.toml.
    let normalized = "[package]\nname = \"tokio\"\nversion = \"1.17.0\"\n\n\
        [dependencies.mio]\nversion = \"0.8.1\"\noptional = true\n\n\
        [dev-dependencies.mio]\nversion = \"0.9\"\n\n\
        [target.\"cfg(unix)\".dependencies.mio]\nversion = \"0.8.4\"\nfeatures = [\"os-ext\"]\n";
    assert_eq!(
        cargo_requirements(normalized, "mio"),
        vec!["0.8.1", "0.8.4"]
    );
    // The inline forms, a rename and the `_`/`-` equivalence.
    let inline = "[dependencies]\nmio = \"0.8\"\nother = { version = \"1\" }\n\
        renamed = { package = \"tokio_util\", version = \"0.7.2\", default-features = false }\n\
        [build-dependencies]\ncc = \"1.0\"\n[dev-dependencies]\nmio = \"0.6\"\n";
    assert_eq!(cargo_requirements(inline, "mio"), vec!["0.8"]);
    assert_eq!(cargo_requirements(inline, "tokio-util"), vec!["0.7.2"]);
    assert_eq!(cargo_requirements(inline, "cc"), vec!["1.0"]);
    assert!(cargo_requirements(inline, "serde").is_empty());
}

/// NodeGoat's tree (audit E8): needle 2.2.4 is hoisted and resolves the
/// hoisted debug 2.6.9 although its `^2.1.2` also admits 2.2.0; connect
/// 3.4.1 resolves its own nested 2.2.0; finalhandler, nested inside connect,
/// climbs to connect's copy. npm's lookup, not the range, decides. A scoped
/// parent resolves the same way; a workspace member is not a dependency copy
/// (no answer: the range fallback); a copy of the parent BUNDLED inside nyc
/// resolves inside the frozen bundle and must not credit the hoisted copy.
#[test]
fn npm_lock_resolutions_follow_npms_lookup() {
    let lock: serde_json::Value = serde_json::from_str(
        r#"{"lockfileVersion": 2, "packages": {
            "": { "dependencies": { "needle": "^2.2.4", "connect": "3.4.1", "@scope/kit": "1.0.0", "babel-traverse": "6.11.4", "nyc": "7.1.0" } },
            "node_modules/debug": { "version": "2.6.9" },
            "node_modules/needle": { "version": "2.2.4", "dependencies": { "debug": "^2.1.2" } },
            "node_modules/connect": { "version": "3.4.1", "dependencies": { "debug": "~2.2.0", "finalhandler": "0.4.1" } },
            "node_modules/connect/node_modules/debug": { "version": "2.2.0" },
            "node_modules/connect/node_modules/finalhandler": { "version": "0.4.1", "dependencies": { "debug": "~2.2.0" } },
            "node_modules/@scope/kit": { "version": "1.0.0", "dependencies": { "debug": "^2.0.0" } },
            "node_modules/@scope/kit/node_modules/debug": { "version": "2.3.0" },
            "packages/web": { "name": "web", "dependencies": { "debug": "^2.6.0" } },
            "node_modules/babel-traverse": { "version": "6.11.4", "dependencies": { "debug": "^2.2.0" } },
            "node_modules/nyc": { "version": "7.1.0", "dependencies": { "babel-traverse": "6.11.4", "debug": "^2.2.0" } },
            "node_modules/nyc/node_modules/babel-traverse": { "version": "6.11.4", "inBundle": true, "dependencies": { "debug": "^2.2.0" } },
            "node_modules/nyc/node_modules/debug": { "version": "2.2.0", "inBundle": true }
        }}"#,
    )
    .unwrap();
    let r = |p: &str, v: &str| npm_lock_resolutions(&lock, p, v, "debug");
    let one = |v: &str| Some(vec![v.to_string()]);
    assert_eq!(r("needle", "2.2.4"), one("2.6.9"));
    assert_eq!(r("connect", "3.4.1"), one("2.2.0"));
    assert_eq!(r("finalhandler", "0.4.1"), one("2.2.0"));
    assert_eq!(r("@scope/kit", "1.0.0"), one("2.3.0"));
    // Only the hoisted babel-traverse counts; its bundled twin inside nyc
    // resolves nyc's frozen 2.2.0, which is not what a refresh can move.
    assert_eq!(r("babel-traverse", "6.11.4"), one("2.6.9"));
    // A workspace member is not a dependency copy, and nothing matches a
    // version the lockfile does not hold: no answer, so the caller keeps its
    // range-based attribution (a database older than the lockfile on disk).
    assert_eq!(r("packages/web", "1.0.0"), None);
    assert_eq!(r("needle", "9.9.9"), None);
    // A v1 lockfile has no tree to read.
    let v1: serde_json::Value =
        serde_json::from_str(r#"{"lockfileVersion": 1, "dependencies": {}}"#).unwrap();
    assert_eq!(npm_lock_resolutions(&v1, "needle", "2.2.4", "debug"), None);
}

/// Bundled means bundled inside a DEPENDENCY: a project's own or a workspace
/// member's `bundleDependencies` also set `inBundle`, but in the user's tree.
#[test]
fn npm_lock_entry_bundled_needs_a_dependency_as_bundler() {
    let lock: serde_json::Value = serde_json::from_str(
        r#"{"lockfileVersion": 3, "packages": {
            "": { "bundleDependencies": ["left-pad"] },
            "node_modules/left-pad": { "version": "1.3.0", "inBundle": true },
            "packages/web/node_modules/kleur": { "version": "4.1.5", "inBundle": true },
            "packages/web": { "name": "web" },
            "node_modules/nyc": { "version": "7.1.0" },
            "node_modules/nyc/node_modules/debug": { "version": "2.2.0", "inBundle": true },
            "node_modules/npm": { "version": "3.10.10" },
            "node_modules/npm/node_modules/node-gyp/node_modules/array-index": { "version": "1.0.0", "inBundle": true }
        }}"#,
    )
    .unwrap();
    let pkgs = lock["packages"].as_object().unwrap();
    assert!(!npm_lock_entry_bundled(pkgs, "node_modules/left-pad"));
    assert!(!npm_lock_entry_bundled(
        pkgs,
        "packages/web/node_modules/kleur"
    ));
    assert!(npm_lock_entry_bundled(
        pkgs,
        "node_modules/nyc/node_modules/debug"
    ));
    // An unlisted level (node-gyp) is passed over on the way to the owner.
    assert!(npm_lock_entry_bundled(
        pkgs,
        "node_modules/npm/node_modules/node-gyp/node_modules/array-index"
    ));
    assert_eq!(
        npm_bundle_owner(
            pkgs,
            "node_modules/npm/node_modules/node-gyp/node_modules/array-index"
        ),
        Some("node_modules/npm")
    );
    assert!(!npm_lock_entry_bundled(pkgs, "node_modules/nyc"));
}

// ---- the installed parent's manifest, through the host (C0.4) ----

fn slash(dir: &str, name: &str) -> String {
    format!("{dir}/{name}")
}

fn pnpm_lookup(provided: &Provided) -> Lookup<Vec<Requirement>> {
    installed_parent_requirements_lookup(
        Manager::Pnpm,
        "/w/taxonomy",
        "micromatch",
        "4.0.5",
        "braces",
        provided,
        &slash,
    )
}

/// The hoisted copy and pnpm's store listing are asked first; a store entry
/// is asked only once the listing names it, and only the entries of exactly
/// this version (`name@4.0.5`, `name@4.0.5_peer`, `name@4.0.5(peer)`).
#[test]
fn the_parent_manifest_is_asked_for_round_by_round() {
    let hoisted = "/w/taxonomy/node_modules/micromatch/package.json";
    let store = "/w/taxonomy/node_modules/.pnpm";
    let mut provided = Provided::default();
    assert_eq!(
        pnpm_lookup(&provided),
        Lookup::Needs(vec![
            Need::Manifest {
                path: hoisted.into()
            },
            Need::Listing { dir: store.into() },
        ])
    );
    // A different hoisted version is not this edge's parent.
    provided.manifests.insert(
        hoisted.into(),
        Some(r#"{"version":"4.0.4","dependencies":{"braces":"^2.0.0"}}"#.into()),
    );
    provided.listings.insert(
        store.into(),
        Some(vec![
            "micromatch@4.0.50".into(),
            "micromatch@4.0.5_peer".into(),
            "braces@3.0.2".into(),
        ]),
    );
    let entry =
        "/w/taxonomy/node_modules/.pnpm/micromatch@4.0.5_peer/node_modules/micromatch/package.json";
    assert_eq!(
        pnpm_lookup(&provided),
        Lookup::Needs(vec![Need::Manifest { path: entry.into() }])
    );
    provided.manifests.insert(
        entry.into(),
        Some(r#"{"version":"v4.0.5","dependencies":{"braces":"^3.0.2"}}"#.into()),
    );
    assert_eq!(
        pnpm_lookup(&provided),
        Lookup::Done(vec![Requirement::npm("^3.0.2")])
    );
}

/// A hoisted copy at the exact version ends the read: the store is listed
/// (asked in the same round) but no entry of it is read.
#[test]
fn a_matching_hoisted_copy_needs_no_store_entry() {
    let answers = |need: &Need| match need {
        Need::Manifest { path } if path.ends_with("node_modules/micromatch/package.json") => {
            Answer::Text(Some(
                r#"{"version":"4.0.5","optionalDependencies":{"Braces":"~3.0.2"}}"#.into(),
            ))
        }
        Need::Listing { .. } => Answer::Names(Some(vec!["micromatch@4.0.5".into()])),
        _ => Answer::Text(None),
    };
    let asked = std::cell::RefCell::new(Vec::new());
    let got = resolve(pnpm_lookup, |need| {
        asked.borrow_mut().push(need.clone());
        answers(need)
    });
    assert_eq!(got, vec![Requirement::npm("~3.0.2")]);
    assert_eq!(asked.borrow().len(), 2, "{:?}", asked.borrow());
}

/// Cargo asks for the release's own `Cargo.toml`; where cargo extracted it
/// is the host's to know. Nothing found: no requirement, so the verdict falls
/// back to semver.
#[test]
fn a_cargo_parent_is_read_from_the_crate_manifest_the_host_finds() {
    let lookup = |provided: &Provided| {
        installed_parent_requirements_lookup(
            Manager::Cargo,
            "/w/nushell",
            "tokio",
            "1.17.0",
            "mio",
            provided,
            &slash,
        )
    };
    assert_eq!(
        lookup(&Provided::default()),
        Lookup::Needs(vec![Need::CrateManifest {
            name: "tokio".into(),
            version: "1.17.0".into()
        }])
    );
    let manifest = "[dependencies.mio]\nversion = \"0.8.1\"\n";
    let got = resolve(lookup, |_| Answer::Text(Some(manifest.into())));
    assert_eq!(got, vec![Requirement::cargo("0.8.1")]);
    assert!(resolve(lookup, |_| Answer::Text(None)).is_empty());
}
