// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `parent_hint` — real values from the live 2026-10-02 DB. Moved
//! from the app's `osv/parent_hint_tests.rs` (single-engine C0.4); the
//! registry-row standing tests stayed there with `standing_from`.

use super::*;
use crate::fix_path::Basis;
use crate::fix_target::UpgradeType;
use crate::lines::InstallSite;
use crate::range::check_version_affected;

const BRIDGE: &str = "d:/runyourempire/bridge/src-tauri";
const APP: &str = "d:/4da/src-tauri";
const LIB: &str = "d:/runyourempire/victauri";

fn line(installed: &str, target: &str, project: &str, direct: bool) -> LineTarget {
    LineTarget {
        installed_version: installed.to_string(),
        target_version: Some(target.to_string()),
        clears_all_known: true,
        introduces: Vec::new(),
        malicious: Vec::new(),
        held: None,
        upgrade_type: Some(UpgradeType::Major),
        sites: vec![InstallSite {
            project_path: project.to_string(),
            is_direct: direct,
            is_dev: false,
        }],
    }
}

fn edge(project: &str, parent: &str, pv: Option<&str>, spec: Option<&str>) -> EdgeObs {
    EdgeObs {
        project: project.to_string(),
        parent: parent.to_string(),
        parent_version: pv.map(str::to_string),
        child_spec: spec.map(str::to_string),
    }
}

fn inst(project: &str, package: &str, version: &str, direct: bool) -> InstalledObs {
    InstalledObs {
        project: project.to_string(),
        package: package.to_string(),
        version: version.to_string(),
        is_direct: direct,
    }
}

/// rmcp's three advisories as stored: fixed 2.0.0, 2.0.0 and 2.1.0, plus the
/// DNS-rebinding one fixed 1.4.0. 3.x is outside all of them.
fn rmcp_clear(v: &str) -> bool {
    let ranges = [
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#.to_string()),
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.1.0"}]}]"#.to_string()),
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.4.0"}]}]"#.to_string()),
    ];
    ranges
        .iter()
        .all(|r| !check_version_affected("", Some(v), r).0)
}

/// The live rmcp graph: Cargo.lock edges carry no child version; the app
/// project still has stale edges from victauri-plugin 0.8.5 / 0.8.8 that a
/// rescan never deleted, but only 0.9.0 is installed there.
fn rmcp_graph() -> (Vec<EdgeObs>, Vec<InstalledObs>) {
    let edges = vec![
        edge(APP, "victauri-plugin", Some("0.8.5"), None),
        edge(APP, "victauri-plugin", Some("0.8.8"), None),
        edge(APP, "victauri-plugin", Some("0.9.0"), None),
        edge(BRIDGE, "victauri-plugin", Some("0.8.4"), None),
        edge(LIB, "victauri-plugin", Some("0.8.8"), None),
    ];
    let installed = vec![
        inst(APP, "rmcp", "3.4.1", false),
        inst(APP, "victauri-plugin", "0.9.0", true),
        inst(LIB, "rmcp", "3.1.2", true),
        inst(LIB, "victauri-plugin", "0.8.8", false),
        inst(BRIDGE, "rmcp", "1.7.0", false),
        inst(BRIDGE, "victauri-plugin", "0.8.4", true),
    ];
    (edges, installed)
}

#[test]
fn names_the_installed_parent_and_the_newer_releases_seen_here() {
    let (edges, installed) = rmcp_graph();
    let lines = [line("1.7.0", "2.1.0", BRIDGE, false)];
    let links = parent_links("rmcp", &lines, &edges, &installed, &rmcp_clear);
    assert_eq!(links.len(), 1, "{links:?}");
    let l = &links[0];
    assert_eq!(l.parent, "victauri-plugin");
    assert_eq!(l.parent_version, "0.8.4");
    assert!(l.parent_is_direct);
    assert_eq!(l.child_version, "1.7.0");
    // Cargo.lock records no requirement and none is on disk here: the
    // verdict is inferred — 1.7.0 -> 2.1.0 crosses a semver boundary.
    assert_eq!(l.requirement, None);
    assert_eq!(l.refresh, Refresh::ParentMustMove(Basis::Semver));
    assert_eq!(l.command, None);
    // Stale 0.8.5 edges are ignored (not installed); 0.8.8 and 0.9.0 are
    // real installs that resolved a clear rmcp.
    let seen: Vec<(&str, &str, &str)> = l
        .resolutions
        .iter()
        .map(|r| {
            (
                r.parent_version.as_str(),
                r.child_version.as_str(),
                r.project.as_str(),
            )
        })
        .collect();
    assert_eq!(seen, vec![("0.8.8", "3.1.2", LIB), ("0.9.0", "3.4.1", APP)]);
}

#[test]
fn a_newer_parent_that_still_resolves_a_vulnerable_child_is_not_cited() {
    let (mut edges, mut installed) = rmcp_graph();
    // A third project on victauri-plugin 0.8.6 that still resolves rmcp 1.9.0.
    edges.push(edge("d:/other", "victauri-plugin", Some("0.8.6"), None));
    installed.push(inst("d:/other", "victauri-plugin", "0.8.6", true));
    installed.push(inst("d:/other", "rmcp", "1.9.0", false));
    let lines = [line("1.7.0", "2.1.0", BRIDGE, false)];
    let links = parent_links("rmcp", &lines, &edges, &installed, &rmcp_clear);
    assert!(
        links[0]
            .resolutions
            .iter()
            .all(|r| r.parent_version != "0.8.6"),
        "{:?}",
        links[0].resolutions
    );
}

#[test]
fn an_ambiguous_child_copy_is_not_attributed() {
    let (edges, mut installed) = rmcp_graph();
    // The app project now holds two rmcp copies: which one 0.9.0 pulls is
    // unknown from a version-less Cargo.lock edge.
    installed.push(inst(APP, "rmcp", "1.6.0", false));
    let lines = [line("1.7.0", "2.1.0", BRIDGE, false)];
    let links = parent_links("rmcp", &lines, &edges, &installed, &rmcp_clear);
    assert_eq!(
        links[0]
            .resolutions
            .iter()
            .map(|r| r.parent_version.as_str())
            .collect::<Vec<_>>(),
        vec!["0.8.8"]
    );
}

#[test]
fn a_direct_site_needs_no_parent() {
    let (edges, installed) = rmcp_graph();
    let lines = [line("1.7.0", "2.1.0", BRIDGE, true)];
    assert!(parent_links("rmcp", &lines, &edges, &installed, &rmcp_clear).is_empty());
}

/// A web app's brace-expansion 1.1.12 under minimatch 3.1.2 (package-lock range
/// `^1.1.7`): the target 1.1.21 is inside the range, so a lockfile refresh is
/// enough — no parent bump. browserslist 4.26.3 under autoprefixer `^4.24.4`
/// with target 4.28.7: same.
#[test]
fn an_npm_range_that_admits_the_target_says_refresh() {
    let webapp = "d:/work/webapp";
    let edges = vec![
        edge(webapp, "minimatch", Some("3.1.2"), Some("^1.1.7")),
        edge(webapp, "__root__", Some("0.1.0"), Some("^9.0.0")),
    ];
    let installed = vec![
        inst(webapp, "brace-expansion", "1.1.12", false),
        inst(webapp, "minimatch", "3.1.2", false),
    ];
    let lines = [line("1.1.12", "1.1.21", webapp, false)];
    let links = parent_links("brace-expansion", &lines, &edges, &installed, &|_| true);
    assert_eq!(links.len(), 1, "the root edge is not a parent: {links:?}");
    assert_eq!(links[0].requirement.as_deref(), Some("^1.1.7"));
    assert_eq!(links[0].refresh, Refresh::Enough(Basis::Requirement));
    assert!(!links[0].parent_is_direct);
}

#[test]
fn an_npm_range_that_excludes_the_target_says_the_parent_must_move() {
    let p = "d:/p";
    let edges = vec![edge(p, "tailwind-thing", Some("1.0.0"), Some("~7.4.0"))];
    let installed = vec![
        inst(p, "tar", "7.4.3", false),
        inst(p, "tailwind-thing", "1.0.0", true),
    ];
    let lines = [line("7.4.3", "7.5.21", p, false)];
    let links = parent_links("tar", &lines, &edges, &installed, &|_| true);
    assert_eq!(
        links[0].refresh,
        Refresh::ParentMustMove(Basis::Requirement)
    );
}

/// navcal (audit A7): brace-expansion 2.0.2 sits under a NESTED minimatch
/// 9.0.5 (`^2.0.1`), while the hoisted minimatch 3.1.2 (`^1.1.7`) holds 1.1.12.
/// With the nested edge stored, 2.0.2 is attributed to 9.0.5 and its target
/// 2.1.7 is inside `^2.0.1`: a refresh, read from the requirement.
#[test]
fn a_nested_parent_is_named_for_its_own_copy() {
    let navcal = "d:/runyourempire/navcal";
    let edges = vec![
        edge(navcal, "minimatch", Some("3.1.2"), Some("^1.1.7")),
        edge(navcal, "minimatch", Some("9.0.5"), Some("^2.0.1")),
    ];
    let installed = vec![
        inst(navcal, "brace-expansion", "1.1.12", false),
        inst(navcal, "brace-expansion", "2.0.2", false),
        inst(navcal, "minimatch", "3.1.2", false),
        inst(navcal, "minimatch", "9.0.5", false),
    ];
    let lines = [line("2.0.2", "2.1.7", navcal, false)];
    let links = parent_links("brace-expansion", &lines, &edges, &installed, &|_| true);
    assert_eq!(links.len(), 1, "{links:?}");
    assert_eq!(links[0].parent_version, "9.0.5");
    assert_eq!(links[0].requirement.as_deref(), Some("^2.0.1"));
    assert_eq!(links[0].refresh, Refresh::Enough(Basis::Requirement));
    let unlinked = unlinked_sites("brace-expansion", &lines, &links, &|_| None);
    assert!(
        unlinked.is_empty(),
        "no longer 'no parent 4DA can read': {unlinked:?}"
    );
}

/// NodeGoat (audit E8): debug 2.2.0 bundled inside nyc. `npm update debug`
/// cannot move it, so it is never "a refresh is enough" and gets no command,
/// even where the project's tool is not known.
#[test]
fn a_bundled_copy_waits_on_its_owner() {
    let p = "d:/nodegoat";
    let edges = vec![edge(p, "nyc", Some("14.1.1"), Some("2.2.0 (bundled)"))];
    let installed = vec![
        inst(p, "debug", "2.2.0", false),
        inst(p, "nyc", "14.1.1", true),
    ];
    let lines = [line("2.2.0", "2.6.9", p, false)];
    for manager in [None, Some(Manager::Npm)] {
        let readers = Readers {
            manager: &|_| manager,
            requirements: &|_, _, _, _, _| Vec::new(),
            resolved_children: &|_, _, _, _| None,
        };
        let links = parent_links_with("debug", &lines, &edges, &installed, &|_| true, &readers);
        assert_eq!(links.len(), 1, "{manager:?}: {links:?}");
        assert_eq!(links[0].parent, "nyc");
        assert_eq!(links[0].requirement.as_deref(), Some("2.2.0 (bundled)"));
        assert_eq!(
            links[0].refresh,
            Refresh::ParentMustMove(Basis::Requirement)
        );
        assert_eq!(links[0].command, None);
    }
    // npm `overrides` cannot reach a bundled copy: no override is offered.
    let links = parent_links("debug", &lines, &edges, &installed, &|_| true);
    assert!(links[0].is_bundled());
    use crate::fix_path::override_hint;
    assert_eq!(override_hint(Some(Manager::Npm), "npm", true), None);
    assert_eq!(override_hint(None, "npm", true), None);
    assert!(override_hint(Some(Manager::Npm), "npm", false).is_some());
}

/// NodeGoat (audit E8): needle 2.2.4's `^2.1.2` admits debug 2.2.0, but the
/// lockfile tree resolves needle to the hoisted 2.6.9. Read from the tree,
/// needle is not a parent of 2.2.0, so it adds no "npm update debug" for a
/// copy it never held; connect, which does hold 2.2.0, stays.
#[test]
fn the_lockfile_tree_decides_which_copy_a_parent_holds() {
    let p = "d:/nodegoat";
    let edges = vec![
        edge(p, "needle", Some("2.2.4"), Some("^2.1.2")),
        edge(p, "connect", Some("3.4.1"), Some("~2.2.0")),
    ];
    let installed = vec![
        inst(p, "debug", "2.2.0", false),
        inst(p, "debug", "2.6.9", false),
        inst(p, "needle", "2.2.4", true),
        inst(p, "connect", "3.4.1", true),
    ];
    let lines = [line("2.2.0", "2.6.9", p, false)];
    let tree = |_: &str, parent: &str, _: &str, _: &str| -> Option<Vec<String>> {
        Some(vec![match parent {
            "needle" => "2.6.9".to_string(),
            _ => "2.2.0".to_string(),
        }])
    };
    let readers = Readers {
        manager: &|_| Some(Manager::Npm),
        requirements: &|_, _, _, _, _| Vec::new(),
        resolved_children: &tree,
    };
    let links = parent_links_with("debug", &lines, &edges, &installed, &|_| true, &readers);
    let parents: Vec<&str> = links.iter().map(|l| l.parent.as_str()).collect();
    assert_eq!(parents, vec!["connect"], "{links:?}");
    // Without a tree (Readers::NONE) the range guess still names both.
    assert_eq!(
        parent_links("debug", &lines, &edges, &installed, &|_| true).len(),
        2
    );
}

#[test]
fn an_edge_whose_range_excludes_the_installed_copy_is_not_its_parent() {
    let p = "d:/p";
    // Two copies of tar; this parent requires the 6.x line, not 7.4.3.
    let edges = vec![edge(p, "old-thing", Some("2.0.0"), Some("^6.1.0"))];
    let installed = vec![
        inst(p, "tar", "7.4.3", false),
        inst(p, "tar", "6.2.1", false),
        inst(p, "old-thing", "2.0.0", true),
    ];
    let lines = [line("7.4.3", "7.5.21", p, false)];
    assert!(parent_links("tar", &lines, &edges, &installed, &|_| true).is_empty());
}

#[test]
fn requirement_admits_reads_npm_and_cargo_specs() {
    let cases: &[(&str, &str, Option<bool>)] = &[
        ("^1.1.7", "1.1.21", Some(true)),
        ("^1.1.7", "2.0.0", Some(false)),
        ("~7.4.0", "7.5.21", Some(false)),
        (">= 4.21.0", "4.28.7", Some(true)),
        (">=1.0.0 <2.0.0", "1.9.9", Some(true)),
        (">=1.0.0 <2.0.0", "2.0.0", Some(false)),
        ("1.2.3", "1.2.3", Some(true)),
        ("1.2.3", "1.2.4", Some(false)), // bare = exact in npm
        ("1.x", "1.9.0", Some(true)),
        ("1.2", "1.2.9", Some(true)),
        ("1.2", "1.3.0", Some(false)),
        ("^1.0.0 || ^2.0.0", "2.1.0", Some(true)),
        ("1.0.0 - 1.5.0", "1.5.0", Some(true)),
        ("*", "9.9.9", Some(true)),
        ("4.28.1(postcss@8.4.0)", "4.28.1", Some(true)), // pnpm peer suffix
        ("npm:string-width@^4.2.0", "4.2.3", Some(true)),
        ("latest", "1.0.0", None),
        ("file:../sdk", "1.0.0", None),
        ("git+https://example.com/x.git", "1.0.0", None),
        ("latest || ^1.0.0", "2.0.0", None), // unreadable alternative: unknown
    ];
    for (spec, version, expected) in cases {
        assert_eq!(
            requirement_admits(spec, version),
            *expected,
            "{spec} admits {version}"
        );
    }
}

/// braces 3.0.2 in a pnpm project (taxonomy, 2026-10-10 oracle): pnpm-lock
/// records the resolution `3.0.2`, never a requirement. With micromatch's
/// installed manifest on disk (`^3.0.2`) the refresh is a read fact and the
/// step names pnpm's command; it used to say "update micromatch, or refresh
/// the lockfile if its requirement admits braces 3.0.3".
#[test]
fn a_pnpm_resolution_reads_the_installed_parent_and_names_the_command() {
    let p = "d:/work/taxonomy";
    let edges = vec![edge(p, "micromatch", Some("4.0.5"), Some("3.0.2"))];
    let installed = vec![
        inst(p, "braces", "3.0.2", false),
        inst(p, "micromatch", "4.0.5", false),
    ];
    let lines = [line("3.0.2", "3.0.3", p, false)];
    let manager = |_: &str| Some(Manager::Pnpm);
    let requirements = |_: &str, _: Manager, parent: &str, version: &str, child: &str| {
        assert_eq!((parent, version, child), ("micromatch", "4.0.5", "braces"));
        vec![Requirement::npm("^3.0.2")]
    };
    let readers = Readers {
        manager: &manager,
        requirements: &requirements,
        resolved_children: &|_, _, _, _| None,
    };
    let links = parent_links_with("braces", &lines, &edges, &installed, &|_| true, &readers);
    assert_eq!(links.len(), 1, "{links:?}");
    assert_eq!(links[0].refresh, Refresh::Enough(Basis::Requirement));
    assert_eq!(links[0].requirement.as_deref(), Some("^3.0.2"));
    assert_eq!(
        links[0].command.as_deref(),
        Some("pnpm update braces --depth Infinity")
    );
    // Without the manifest on disk: the same answer, inferred, still named.
    let none = |_: &str, _: Manager, _: &str, _: &str, _: &str| Vec::new();
    let readers = Readers {
        manager: &manager,
        requirements: &none,
        resolved_children: &|_, _, _, _| None,
    };
    let links = parent_links_with("braces", &lines, &edges, &installed, &|_| true, &readers);
    assert_eq!(links[0].refresh, Refresh::Enough(Basis::Semver));
    assert!(links[0].command.is_some());
}

/// mio 0.8.0 in nushell (Cargo): tokio 1.17.0's registry manifest requires
/// `0.8.1`, a Cargo caret that admits 0.8.11 — the command is cargo's precise
/// update; the stale "waiting on upstream" is gone.
#[test]
fn a_cargo_parent_requirement_is_read_with_cargo_semantics() {
    let p = "d:/work/nushell";
    let edges = vec![edge(p, "tokio", Some("1.17.0"), Some("0.8.0"))];
    let installed = vec![
        inst(p, "mio", "0.8.0", false),
        inst(p, "tokio", "1.17.0", false),
    ];
    let lines = [line("0.8.0", "0.8.11", p, false)];
    let manager = |_: &str| Some(Manager::Cargo);
    let requirements =
        |_: &str, _: Manager, _: &str, _: &str, _: &str| vec![Requirement::cargo("0.8.1")];
    let readers = Readers {
        manager: &manager,
        requirements: &requirements,
        resolved_children: &|_, _, _, _| None,
    };
    let links = parent_links_with("mio", &lines, &edges, &installed, &|_| true, &readers);
    assert_eq!(links[0].refresh, Refresh::Enough(Basis::Requirement));
    assert_eq!(links[0].requirement.as_deref(), Some("^0.8.1"));
    assert_eq!(
        links[0].command.as_deref(),
        Some("cargo update -p mio@0.8.0 --precise 0.8.11")
    );
    // An `=` pin from the parent sends the fix to the parent, with no command.
    let pinned =
        |_: &str, _: Manager, _: &str, _: &str, _: &str| vec![Requirement::cargo("=0.8.0")];
    let readers = Readers {
        manager: &manager,
        requirements: &pinned,
        resolved_children: &|_, _, _, _| None,
    };
    let links = parent_links_with("mio", &lines, &edges, &installed, &|_| true, &readers);
    assert_eq!(
        links[0].refresh,
        Refresh::ParentMustMove(Basis::Requirement)
    );
    assert_eq!(links[0].command, None);
}

/// A yarn 1 project stores no edges: the copy is unlinked, its route is the
/// semver inference, and yarn 1 has no command to name.
#[test]
fn a_copy_with_no_readable_parent_gets_a_semver_route() {
    let p = "d:/work/express";
    let lines = [line("1.2.5", "1.2.6", p, false)];
    let yarn = |_: &str| Some(Manager::YarnClassic);
    let unlinked = unlinked_sites("minimist", &lines, &[], &yarn);
    assert_eq!(unlinked.len(), 1);
    assert_eq!(unlinked[0].refresh, Refresh::Enough(Basis::Semver));
    assert_eq!(unlinked[0].command, None);
    let npm = |_: &str| Some(Manager::Npm);
    let unlinked = unlinked_sites("minimist", &lines, &[], &npm);
    assert_eq!(unlinked[0].command.as_deref(), Some("npm update minimist"));
}

#[test]
fn the_override_hint_names_the_projects_own_tool() {
    use crate::fix_path::override_hint;
    assert_eq!(
        override_hint(Some(Manager::Npm), "npm", false).as_deref(),
        Some("an `overrides` entry in package.json (npm)")
    );
    assert_eq!(
        override_hint(Some(Manager::PnpmWorkspace), "npm", false).as_deref(),
        Some("a `pnpm.overrides` entry in package.json")
    );
    assert_eq!(
        override_hint(Some(Manager::YarnClassic), "npm", false).as_deref(),
        Some("a `resolutions` entry in package.json (yarn)")
    );
    assert!(override_hint(None, "npm", false)
        .is_some_and(|h| h.contains("`overrides`") && h.contains("`resolutions`")));
    // Cargo has no key that forces a crate outside its parent's line.
    assert_eq!(
        override_hint(Some(Manager::Cargo), "crates.io", false),
        None
    );
    assert_eq!(override_hint(None, "crates.io", false), None);
}

// ---- the same links, with the files read through the host (C0.4) ----

/// braces 3.0.2 in two projects: taxonomy (pnpm: the requirement is in
/// micromatch's store manifest) and an npm app whose `package-lock.json`
/// tree says anymatch holds the hoisted 3.0.3, not this copy (audit E8).
fn two_tools() -> (Vec<LineTarget>, Vec<EdgeObs>, Vec<InstalledObs>) {
    let (tax, app) = ("/w/taxonomy", "/w/app");
    let mut l = line("3.0.2", "3.0.3", tax, false);
    l.sites.push(InstallSite {
        project_path: app.to_string(),
        is_direct: false,
        is_dev: false,
    });
    let edges = vec![
        edge(tax, "micromatch", Some("4.0.5"), Some("3.0.2")),
        edge(app, "micromatch", Some("4.0.5"), Some("^3.0.2")),
        edge(app, "anymatch", Some("3.1.3"), Some("^3.0.2")),
    ];
    let installed = vec![
        inst(tax, "braces", "3.0.2", false),
        inst(tax, "micromatch", "4.0.5", true),
        inst(app, "braces", "3.0.2", false),
        inst(app, "braces", "3.0.3", false),
        inst(app, "micromatch", "4.0.5", true),
        inst(app, "anymatch", "3.1.3", true),
    ];
    (vec![l], edges, installed)
}

fn app_lock() -> serde_json::Value {
    serde_json::json!({"lockfileVersion": 3, "packages": {
        "node_modules/braces": {"version": "3.0.3"},
        "node_modules/anymatch": {"version": "3.1.3", "dependencies": {"braces": "^3.0.2"}},
        "node_modules/micromatch": {"version": "4.0.5", "dependencies": {"braces": "^3.0.2"}},
        "node_modules/micromatch/node_modules/braces": {"version": "3.0.2"}
    }})
}

fn manager_of(project: &str) -> Option<Manager> {
    match project {
        "/w/taxonomy" => Some(Manager::Pnpm),
        _ => Some(Manager::Npm),
    }
}

#[test]
fn the_host_round_trip_gives_the_links_the_readers_give() {
    use crate::needs::{resolve, Answer, Need};
    use std::sync::Arc;
    let (lines, edges, installed) = two_tools();
    let store_manifest =
        "/w/taxonomy/node_modules/.pnpm/micromatch@4.0.5/node_modules/micromatch/package.json";
    let answer = |need: &Need| match need {
        Need::NpmLock { dir } if dir == "/w/app" => Answer::Tree(Some(Arc::new(app_lock()))),
        Need::Listing { dir } if dir == "/w/taxonomy/node_modules/.pnpm" => {
            Answer::Names(Some(vec!["micromatch@4.0.5".into()]))
        }
        Need::Manifest { path } if path == store_manifest => Answer::Text(Some(
            r#"{"version":"4.0.5","dependencies":{"braces":"^3.0.2"}}"#.into(),
        )),
        Need::Listing { .. } => Answer::Names(None),
        Need::NpmLock { .. } => Answer::Tree(None),
        _ => Answer::Text(None),
    };
    let rounds = std::cell::Cell::new(0);
    let projects = Projects {
        manager: &manager_of,
        dir: &str::to_string,
        join: &|d: &str, n: &str| format!("{d}/{n}"),
    };
    let via_host = resolve(
        |provided| {
            rounds.set(rounds.get() + 1);
            parent_links_lookup(
                "braces",
                &lines,
                &edges,
                &installed,
                &|_| true,
                &projects,
                provided,
            )
        },
        answer,
    );
    assert_eq!(rounds.get(), 3, "two rounds of needs, then the answer");

    let requirements = |project: &str, _: Manager, _: &str, _: &str, _: &str| {
        if project == "/w/taxonomy" {
            vec![Requirement::npm("^3.0.2")]
        } else {
            Vec::new()
        }
    };
    let lock = app_lock();
    let resolved = |project: &str, parent: &str, version: &str, child: &str| {
        (project == "/w/app")
            .then(|| npm_lock_resolutions(&lock, parent, version, child))
            .flatten()
    };
    let readers = Readers {
        manager: &manager_of,
        requirements: &requirements,
        resolved_children: &resolved,
    };
    let direct = parent_links_with("braces", &lines, &edges, &installed, &|_| true, &readers);
    assert_eq!(via_host, direct);
    let named: Vec<(&str, &str, Option<&str>)> = via_host
        .iter()
        .map(|l| (l.project.as_str(), l.parent.as_str(), l.command.as_deref()))
        .collect();
    assert_eq!(
        named,
        vec![
            (
                "/w/taxonomy",
                "micromatch",
                Some("pnpm update braces --depth Infinity")
            ),
            ("/w/app", "micromatch", Some("npm update braces")),
        ]
    );
    assert!(via_host
        .iter()
        .all(|l| l.refresh == Refresh::Enough(Basis::Requirement)));
}
