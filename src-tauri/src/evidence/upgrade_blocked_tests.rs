// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Y5 blocked upstream, on fixtures modelled on 4DA's own repo (2026-11-10):
//! zip 8.6.0 held by docx-rs 0.4.22 (its newest release) is blocked; rmcp
//! 3.4.1 under victauri-plugin's ^3.1 admits 3.9.0 (a lockfile refresh); a
//! bridge's rmcp 1.7.0 under victauri-plugin 0.8.4, with a newer
//! victauri-plugin resolved clean on this machine, is a parent update.
//! Project paths are generic.

use super::*;
use crate::ace::scanner::{DependencyEdge, EdgeScope};
use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::upgrade_steps::{Mechanism, UpgradeStep};
use crate::evidence::{validate_item, EvidenceItem};
use crate::osv::parent_hint::{ParentLink, ParentRelease, ParentStanding, Through};
use crate::test_utils::test_db;

const APP: &str = "d:/code/app/src-tauri";
const BRIDGE: &str = "d:/code/bridge/src-tauri";

fn advisory(db: &Database, id: &str, package: &str, fixed: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("{package} advisory {id}"),
        None,
        package,
        "crates.io",
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(cvss),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn instance(name: &str, version: &str, direct: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct: direct,
        is_dev: false,
        scope: "runtime".to_string(),
    }
}

fn edge(parent: &str, version: &str, child: &str) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: Some(version.to_string()),
        child: child.to_string(),
        child_version: None,
        scope: EdgeScope::Runtime,
    }
}

fn crates_row(db: &Database, title: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
         VALUES ('crates_io', ?1, ?1, '', ?1, X'00')",
        [title],
    )
    .unwrap();
}

fn built(db: &Database) -> (Vec<EvidenceItem>, Vec<UpgradeStep>) {
    let plan = crate::evidence::build_upgrade_plan(db).expect("upgrade plan");
    assert_eq!(plan.drops, 0, "no plan item may fail validation");
    (plan.items, plan.steps)
}

fn item<'a>(items: &'a [EvidenceItem], package: &str) -> &'a EvidenceItem {
    items
        .iter()
        .find(|i| i.affected_deps == vec![package.to_string()])
        .unwrap_or_else(|| {
            panic!(
                "{package}: {:?}",
                items.iter().map(|i| &i.title).collect::<Vec<_>>()
            )
        })
}

fn step<'a>(steps: &'a [UpgradeStep], package: &str) -> &'a UpgradeStep {
    steps
        .iter()
        .find(|s| s.package == package)
        .unwrap_or_else(|| panic!("{package}: {steps:#?}"))
}

/// zip 8.6.0 reaches the app only through docx-rs 0.4.22, the newest docx-rs
/// release on record; the fix is zip 9.0.5.
fn seed_zip(db: &Database, newer_docx: bool) {
    db.store_transitive_dependency(APP, "zip", Some("8.6.0"), "rust", false)
        .unwrap();
    db.store_dependency(APP, "docx-rs", Some("0.4.22"), "rust", false, None)
        .unwrap();
    db.store_dependency_instances(
        APP,
        "rust",
        &[
            instance("zip", "8.6.0", false),
            instance("docx-rs", "0.4.22", true),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        APP,
        "rust",
        &[
            edge("app", "0.1.0", "docx-rs"),
            edge("docx-rs", "0.4.22", "zip"),
        ],
    )
    .unwrap();
    advisory(db, "RUSTSEC-2026-9101", "zip", "9.0.5", 9.8);
    crates_row(db, "crates.io: docx-rs v0.4.21");
    crates_row(db, "crates.io: docx-rs v0.4.22");
    if newer_docx {
        crates_row(db, "crates.io: docx-rs v0.5.0");
    }
}

#[test]
fn a_parent_at_its_newest_release_is_blocked_upstream() {
    let db = test_db();
    seed_zip(&db, false);
    let (items, steps) = built(&db);
    let zip = item(&items, "zip");
    validate_item(zip).expect("schema-valid");
    assert_eq!(
        zip.title,
        "zip 8.6.0: blocked by docx-rs 0.4.22 (newest), no fix yet"
    );
    // Nothing to upgrade to: no changelog advice (live 2026-11-10).
    assert!(
        !zip.explanation.contains("changelog") && !zip.explanation.contains("before upgrading"),
        "{}",
        zip.explanation
    );
    assert!(
        zip.explanation.contains("Blocked upstream: "),
        "{}",
        zip.explanation
    );
    assert!(
        zip.explanation
            .contains("Blocked upstream: zip 8.6.0 comes in through docx-rs 0.4.22, the newest release, which does not allow a fixed zip (>= 9.0.5) yet. Options: fork docx-rs so it accepts zip >= 9.0.5 and point `[patch.crates-io]` at the fork"),
        "{}",
        zip.explanation
    );
    assert!(
        zip.explanation
            .contains("accept the risk with a written review date"),
        "{}",
        zip.explanation
    );
    let s = step(&steps, "zip");
    assert_eq!(s.mechanism, Mechanism::BlockedUpstream);
    assert_eq!(s.blocked_upstream.len(), 1);
    let hold = &s.blocked_upstream[0];
    assert_eq!(hold.installed, "8.6.0");
    assert_eq!(hold.cleared_version, "9.0.5");
    assert_eq!(
        (hold.parent.as_str(), hold.parent_version.as_str()),
        ("docx-rs", "0.4.22")
    );
    assert_eq!(
        (hold.held_by.as_str(), hold.held_by_newest.as_str()),
        ("docx-rs", "0.4.22")
    );
    assert_eq!(hold.options.len(), 3);
    // The work-order JSON (schema v5).
    let v = serde_json::to_value(s).unwrap();
    assert_eq!(v["mechanism"], "blocked_upstream");
    assert_eq!(v["blocked_upstream"][0]["held_by"], "docx-rs");
    assert_eq!(v["blocked_upstream"][0]["cleared_version"], "9.0.5");
    // The dependency-path citation carries the marker the hand-off reads.
    assert!(
        zip.evidence.iter().any(|c| c.source == "dependency-path"
            && c.relevance_note
                .ends_with("; blocked upstream: docx-rs 0.4.22 is the newest release")),
        "{:#?}",
        zip.evidence
    );
}

/// A newer docx-rs on record: not blocked; the undecided route stays.
#[test]
fn a_parent_with_a_newer_release_is_not_blocked() {
    let db = test_db();
    seed_zip(&db, true);
    let (items, steps) = built(&db);
    let zip = item(&items, "zip");
    assert!(
        zip.title.starts_with("Upgrade zip to >= 9.0.5"),
        "{}",
        zip.title
    );
    assert!(
        !zip.explanation.contains("blocked upstream"),
        "{}",
        zip.explanation
    );
    let s = step(&steps, "zip");
    assert_eq!(s.mechanism, Mechanism::LockfileOrParentUpdate);
    assert!(s.blocked_upstream.is_empty());
    assert!(serde_json::to_value(s)
        .unwrap()
        .get("blocked_upstream")
        .is_none());
}

/// rmcp 3.4.1 under victauri-plugin: three advisories (fixed 3.6.0, 3.7.0,
/// 3.9.0); 3.9.0 is semver-compatible, so the lockfile refresh reaches it.
#[test]
fn a_requirement_that_admits_the_fix_is_a_lockfile_refresh() {
    let db = test_db();
    db.store_transitive_dependency(APP, "rmcp", Some("3.4.1"), "rust", false)
        .unwrap();
    db.store_dependency_instances(
        APP,
        "rust",
        &[
            instance("rmcp", "3.4.1", false),
            instance("victauri-plugin", "0.10.3", true),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        APP,
        "rust",
        &[
            edge("app", "0.1.0", "victauri-plugin"),
            edge("victauri-plugin", "0.10.3", "rmcp"),
        ],
    )
    .unwrap();
    advisory(&db, "RUSTSEC-2026-0461", "rmcp", "3.6.0", 7.5);
    advisory(&db, "RUSTSEC-2026-0457", "rmcp", "3.7.0", 7.5);
    advisory(&db, "RUSTSEC-2026-0462", "rmcp", "3.9.0", 7.5);
    let (items, steps) = built(&db);
    let rmcp = item(&items, "rmcp");
    assert!(rmcp.title.contains("to >= 3.9.0"), "{}", rmcp.title);
    let s = step(&steps, "rmcp");
    assert_eq!(s.mechanism, Mechanism::LockfileRefresh);
    assert_eq!(s.lines[0].target.as_deref(), Some("3.9.0"));
    assert!(s.blocked_upstream.is_empty());
    assert_eq!(
        serde_json::to_value(s).unwrap()["mechanism"],
        "lockfile_refresh"
    );
}

/// The bridge's rmcp 1.7.0 needs a major jump its parent excludes; the app
/// runs victauri-plugin 0.9.0 resolved to a clean rmcp 3.4.1: a parent
/// release that admits the fix is proven, so the step is a parent update.
#[test]
fn a_proven_newer_parent_is_a_parent_update() {
    let db = test_db();
    db.store_transitive_dependency(BRIDGE, "rmcp", Some("1.7.0"), "rust", false)
        .unwrap();
    db.store_dependency_instances(
        BRIDGE,
        "rust",
        &[
            instance("rmcp", "1.7.0", false),
            instance("victauri-plugin", "0.8.4", true),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        BRIDGE,
        "rust",
        &[
            edge("bridge", "0.1.0", "victauri-plugin"),
            edge("victauri-plugin", "0.8.4", "rmcp"),
        ],
    )
    .unwrap();
    db.store_dependency(APP, "victauri-plugin", Some("0.9.0"), "rust", false, None)
        .unwrap();
    db.store_dependency_instances(
        APP,
        "rust",
        &[
            instance("rmcp", "3.4.1", false),
            instance("victauri-plugin", "0.9.0", true),
        ],
    )
    .unwrap();
    db.store_dependency_edges(APP, "rust", &[edge("victauri-plugin", "0.9.0", "rmcp")])
        .unwrap();
    advisory(&db, "GHSA-9pj6-vhgr-3mwh", "rmcp", "2.0.0", 7.5);
    let (_, steps) = built(&db);
    let s = step(&steps, "rmcp");
    assert_eq!(s.mechanism, Mechanism::ParentUpdate);
    assert_eq!(
        serde_json::to_value(s).unwrap()["mechanism"],
        "parent_update"
    );
}

#[test]
fn standing_decides_blocked() {
    let newest = ParentStanding::Newest { deprecated: false };
    let newer = ParentStanding::Newer {
        latest: "0.5.0".to_string(),
    };
    assert!(standing_blocked(&newest, None));
    assert!(standing_blocked(
        &ParentStanding::NoneAdmits { latest: None },
        None
    ));
    assert!(!standing_blocked(&newer, None));
    assert!(!standing_blocked(&ParentStanding::Unknown, None));
    assert!(!standing_blocked(
        &ParentStanding::Admits(Box::new(ParentRelease::default())),
        None
    ));
    // The parent has newer releases, but the declared dependency it arrives
    // through (jsonwebtoken, at its newest) holds it.
    let lever = Through {
        standing: ParentStanding::Newest { deprecated: false },
        ..Through::declared("jsonwebtoken", "11.1.0")
    };
    assert!(standing_blocked(&newer, Some(&lever)));
    let movable = Through {
        standing: newer.clone(),
        ..Through::declared("jsonwebtoken", "11.1.0")
    };
    assert!(!standing_blocked(&newer, Some(&movable)));
}

fn hold(installed: &str, parent: &str, held_by: &str, newest: &str, project: &str) -> BlockedHold {
    BlockedHold {
        project: project.to_string(),
        installed: installed.to_string(),
        cleared_version: "0.7.0".to_string(),
        parent: parent.to_string(),
        parent_version: "0.13.8".to_string(),
        requirement: Some("^0.5.4".to_string()),
        held_by: held_by.to_string(),
        held_by_newest: newest.to_string(),
        held_by_version: newest.to_string(),
        options: Vec::new(),
    }
}

/// Live 2026-11-10: "...which jsonwebtoken 11.1.0 (newest) brings in: no
/// release f" (cut at the 120-byte cap). The short form fits; one copy held
/// in two projects is one hold, not "(+1 more)".
#[test]
fn blocked_titles_fit_by_construction() {
    let a = hold("0.5.5", "elliptic-curve", "jsonwebtoken", "11.1.0", APP);
    let b = hold("0.5.5", "elliptic-curve", "jsonwebtoken", "11.1.0", BRIDGE);
    assert_eq!(
        blocked_title("crypto-bigint", &[a.clone(), b]).unwrap(),
        "crypto-bigint 0.5.5: blocked by jsonwebtoken 11.1.0 (newest), no fix yet"
    );
    let other = hold("0.4.9", "elliptic-curve", "p256", "0.13.2", APP);
    assert_eq!(
        blocked_title("crypto-bigint", &[a, other]).unwrap(),
        "crypto-bigint 0.5.5: blocked by jsonwebtoken 11.1.0 (newest), no fix yet (+1 more)"
    );
    assert_eq!(blocked_title("x", &[]), None);
    // The longest real names: a scoped npm package held by a long one.
    let scoped = "@typescript-eslint/typescript-estree-internal-utilities";
    let long = hold(
        "8.15.0-alpha.12",
        "@angular-devkit/build-angular",
        "@angular-devkit/build-angular-schematics-tooling",
        "19.2.11-next.3",
        APP,
    );
    let t = blocked_title(scoped, std::slice::from_ref(&long)).unwrap();
    assert!(t.len() <= 120, "{} bytes: {t}", t.len());
    assert!(t.ends_with("no fix yet"), "{t}");
    let absurd = "a".repeat(200);
    let t = blocked_title(&absurd, &[long]).unwrap();
    assert!(t.ends_with("no fix yet"), "{t}");
}

/// Live 2026-11-10: the crypto-bigint card printed the blocked sentence and
/// its options once per project. One sentence, the projects listed, the
/// options once.
#[test]
fn one_blocked_copy_in_two_projects_is_one_sentence() {
    let link = |project: &str| ParentLink {
        project: project.to_string(),
        child_version: "0.5.5".to_string(),
        target: Some("0.7.0".to_string()),
        parent: "elliptic-curve".to_string(),
        parent_version: "0.13.8".to_string(),
        parent_is_direct: false,
        requirement: Some("^0.5.4".to_string()),
        refresh: crate::osv::fix_path::Refresh::ParentMustMove(
            crate::osv::fix_path::Basis::Requirement,
        ),
        command: None,
        resolutions: Vec::new(),
        through: Some(Through {
            standing: ParentStanding::Newest { deprecated: false },
            ..Through::declared("jsonwebtoken", "11.1.0")
        }),
        standing: ParentStanding::Newer {
            latest: "0.14.0".to_string(),
        },
        override_hint: None,
        lever_unknown: None,
    };
    let routes = crate::osv::parent_hint::TransitiveRoutes {
        links: vec![link(APP), link(BRIDGE)],
        unlinked: Vec::new(),
    };
    let note = crate::evidence::upgrade_parent::upstream_note("crypto-bigint", &routes, true);
    assert_eq!(
        note,
        format!(
            "Blocked upstream: crypto-bigint 0.5.5 (in {APP}, {BRIDGE}) comes in through \
             elliptic-curve 0.13.8, which arrives through jsonwebtoken 11.1.0, the newest \
             jsonwebtoken release, and no jsonwebtoken release allows a fixed crypto-bigint \
             (>= 0.7.0) yet. Options: fork jsonwebtoken so it accepts crypto-bigint >= 0.7.0 \
             and point `[patch.crates-io]` at the fork; or backport the fix onto the \
             crypto-bigint line elliptic-curve allows and point `[patch.crates-io]` at that \
             fork; or accept the risk with a written review date"
        )
    );
    assert_eq!(note.matches("Options:").count(), 1);
}

/// Live 2026-11-10, 4da-ledger: request 2.88.2's lockfile edge requires qs
/// `~6.5.2`, but package.json overrides qs to `^6.16.0` (npm installed
/// 6.16.0). The override is the requirement: a lockfile refresh reaches the
/// fix 6.16.1, and the text says the override replaces request's own. (The
/// project directory is a temp dir, which the plan's inventory excludes, so
/// the route rule is exercised directly, as `load_routes` runs it.)
#[test]
fn a_package_json_override_replaces_the_parents_requirement() {
    use crate::osv::fix_path::{Basis, Manager, Refresh};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"ledger","overrides":{"qs":"^6.16.0"}}"#,
    )
    .unwrap();
    let mut links = vec![ParentLink {
        project: "d:/code/ledger".to_string(),
        child_version: "6.16.0".to_string(),
        target: Some("6.16.1".to_string()),
        parent: "request".to_string(),
        parent_version: "2.88.2".to_string(),
        parent_is_direct: false,
        requirement: Some("~6.5.2".to_string()),
        refresh: Refresh::ParentMustMove(Basis::Requirement),
        command: None,
        resolutions: Vec::new(),
        through: None,
        standing: ParentStanding::Newest { deprecated: true },
        override_hint: None,
        lever_unknown: None,
    }];
    let path = dir.path().to_string_lossy().to_string();
    crate::osv::parent_override::apply("qs", &mut links, &|_| path.clone(), &|_| {
        Some(Manager::Npm)
    });
    let l = &links[0];
    assert!(l.refresh.is_enough(), "{:?}", l.refresh);
    assert_eq!(l.command.as_deref(), Some("npm update qs"));
    assert!(!link_blocked(l));
    let routes = crate::osv::parent_hint::TransitiveRoutes {
        links,
        unlinked: Vec::new(),
    };
    let note = crate::evidence::upgrade_parent::upstream_note("qs", &routes, false);
    assert!(
        note.starts_with(
            "No manifest change needed: qs 6.16.0 comes in through request 2.88.2, whose \
             requirement ^6.16.0 (a package.json override, replacing request's own ~6.5.2) \
             already admits 6.16.1"
        ),
        "{note}"
    );
    assert!(note.contains("Run: `npm update qs`"), "{note}");
}

#[test]
fn options_follow_the_ecosystem() {
    let npm = blocked_options(Some("npm"), "uuid", "11.1.1", "request", "request", None);
    assert!(npm[0].starts_with("force uuid >= 11.1.1 with an override in package.json"));
    let pypi = blocked_options(
        Some("PyPI"),
        "urllib3",
        "2.5.0",
        "botocore",
        "botocore",
        None,
    );
    assert!(pypi[0].contains("a pip constraint cannot exceed botocore's requirement"));
    let cargo = blocked_options(
        Some("crates.io"),
        "rsa",
        "0.10.0",
        "jsonwebtoken",
        "jsonwebtoken",
        None,
    );
    assert!(cargo[0].contains("[patch.crates-io]"));
    assert_eq!(
        cargo.last().unwrap(),
        "accept the risk with a written review date"
    );
}
