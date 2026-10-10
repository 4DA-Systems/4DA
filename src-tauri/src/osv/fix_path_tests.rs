// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `osv::fix_path`. The oracle cases are the 2026-10-10 fix-path
//! oracle (apply the recommendation in a copy, re-resolve, re-scan with
//! osv-scanner) with the advisory ranges pinned from the fixture mirror.

use super::*;
use crate::osv::fix_target::clean_version;

fn ranges(json: &[&str]) -> Vec<Option<String>> {
    json.iter().map(|r| Some((*r).to_string())).collect()
}

fn walk(installed: &str, owned: &[Option<String>]) -> Option<String> {
    let refs: Vec<&Option<String>> = owned.iter().collect();
    clean_version(installed, &refs)
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
