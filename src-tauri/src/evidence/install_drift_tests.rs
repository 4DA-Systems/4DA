// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `evidence::install_drift` (extracted via #[path]).

use std::path::{Path, PathBuf};

use super::*;
use crate::db::Database;
use crate::evidence::{validate_item, ConfidenceProvenance};
use crate::test_utils::test_db;

/// A project directory with its own `.git` (so the Node-style lookup stops
/// here, and no ancestor repository on the test machine leaks in), a
/// package.json, and the named lockfile.
fn project(root: &Path, name: &str, lockfile: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{"name":"fixture"}"#).unwrap();
    std::fs::write(dir.join(lockfile), "lockfileVersion: '9.0'\n").unwrap();
    dir
}

/// `node_modules/<package>/package.json` reporting `version`.
fn install(dir: &Path, package: &str, version: &str) {
    let package_dir = package
        .split('/')
        .fold(dir.join("node_modules"), |d, segment| d.join(segment));
    std::fs::create_dir_all(&package_dir).unwrap();
    std::fs::write(
        package_dir.join("package.json"),
        format!(r#"{{"name":"{package}","version":"{version}"}}"#),
    )
    .unwrap();
}

fn path_of(dir: &Path) -> String {
    dir.to_string_lossy().into_owned()
}

/// Pin a DIRECT npm dependency the way the lockfile walk records it. Raw SQL
/// because the production writer refuses temp-dir paths by design, and these
/// fixtures live in one.
fn pin(db: &Database, dir: &Path, package: &str, version: &str, dev: bool) {
    db.conn
        .lock()
        .execute(
            "INSERT INTO dependency_instances
                 (project_path, ecosystem, package_name, version, is_direct, is_dev, scope, detected_at)
             VALUES (?1, 'npm', ?2, ?3, 1, ?4, ?5, datetime('now'))",
            rusqlite::params![
                path_of(dir),
                package,
                version,
                dev as i32,
                if dev { "dev" } else { "runtime" }
            ],
        )
        .unwrap();
}

/// An advisory affecting `package < fixed` in `ecosystem`.
fn advisory(db: &Database, id: &str, package: &str, ecosystem: &str, fixed: &str) {
    db.upsert_osv_advisory(
        id,
        &format!("{package}: request smuggling"),
        None,
        package,
        ecosystem,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(7.5),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn drift(db: &Database) -> Vec<ProjectDrift> {
    let conn = db.conn.lock();
    projects_with(&conn, &ProjectLiveness::from_entries(&[]), &|_| false)
}

/// THE live defect: the lockfile moved to the fix, node_modules did not.
#[test]
fn a_stale_install_behind_a_merged_fix_is_one_high_row_naming_pnpm_install() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "mcp-4da-server", "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.1");
    install(&dir, "typescript", "5.9.3");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    pin(&db, &dir, "typescript", "5.9.3", true);
    advisory(&db, "GHSA-hono-smuggle", "hono", "npm", "4.13.5");

    let rows = drift(&db);
    assert_eq!(rows.len(), 1, "one row per project: {rows:?}");
    let row = &rows[0];
    assert_eq!(row.urgency(), Urgency::High);
    assert_eq!(row.lockfile, Lockfile::Pnpm);
    assert_eq!(row.packages.len(), 1, "typescript is in sync: {row:?}");
    assert_eq!(row.lead_fix(), Some(("hono", "4.13.1", "4.13.5")));

    let item = row.to_evidence_item();
    validate_item(&item).expect("the row must validate");
    assert_eq!(item.id, format!("install-drift:{}", path_of(&dir)));
    assert_eq!(item.urgency, Urgency::High);
    assert!(
        item.title
            .starts_with("mcp-4da-server — node_modules is out of sync with pnpm-lock.yaml"),
        "{}",
        item.title
    );
    assert!(
        item.title.contains("hono: 4.13.1 installed, 4.13.5 pinned"),
        "{}",
        item.title
    );
    assert!(
        item.explanation.contains("`pnpm install`"),
        "{}",
        item.explanation
    );
    assert!(
        item.explanation
            .contains("the fix is merged but not running"),
        "{}",
        item.explanation
    );
    assert_eq!(item.affected_deps, vec!["hono".to_string()]);
    assert_eq!(item.affected_projects, vec![path_of(&dir)]);
    assert!(item.lens_hints.preemption && is_install_drift_id(&item.id));
    assert_eq!(
        item.confidence.provenance,
        ConfidenceProvenance::OsvVerified
    );
    assert_eq!(
        item.evidence[0].url.as_deref(),
        Some("https://osv.dev/GHSA-hono-smuggle"),
        "the advisory leads: a card's 'Open advisory' opens the first citation"
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|a| a.label == "Run pnpm install in mcp-4da-server"),
        "{:?}",
        item.suggested_actions
    );
}

#[test]
fn a_matching_install_is_not_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.5");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    advisory(&db, "GHSA-hono-smuggle", "hono", "npm", "4.13.5");
    assert!(drift(&db).is_empty());
}

#[test]
fn a_project_that_was_never_installed_is_not_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "fresh-clone", "pnpm-lock.yaml");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    assert!(
        drift(&db).is_empty(),
        "no node_modules at all: nothing runs to disagree with"
    );
}

/// A workspace hoists to the repository root; Node finds it there, and so
/// must this.
#[test]
fn a_hoisted_install_is_found_where_node_would_find_it() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("monorepo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    install(&root, "hono", "4.13.1");
    let member = root.join("packages").join("api");
    std::fs::create_dir_all(member.join("node_modules")).unwrap();
    std::fs::write(member.join("package.json"), "{}").unwrap();
    std::fs::write(member.join("package-lock.json"), "{}").unwrap();
    let db = test_db();
    pin(&db, &member, "hono", "4.13.5", false);

    let rows = drift(&db);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let copy = rows[0].packages[0]
        .installed
        .as_ref()
        .expect("resolved from the parent node_modules");
    assert_eq!(copy.version, "4.13.1");
    assert!(
        copy.manifest.starts_with(root.join("node_modules")),
        "{:?}",
        copy.manifest
    );
    assert_eq!(rows[0].lockfile.install_command(), "npm ci");
}

/// The repository root bounds the search: a stray copy ABOVE it is not the
/// project's install, and the row says precisely where it looked.
#[test]
fn the_search_stops_at_the_repository_root() {
    let tmp = tempfile::tempdir().unwrap();
    install(tmp.path(), "zod", "3.0.0");
    let dir = project(tmp.path(), "app", "yarn.lock");
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    let db = test_db();
    pin(&db, &dir, "zod", "3.24.4", false);

    let rows = drift(&db);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].packages[0].installed.is_none());
    assert_eq!(rows[0].urgency(), Urgency::Watch);
    assert_eq!(rows[0].lockfile.install_command(), "yarn install");
    let item = rows[0].to_evidence_item();
    validate_item(&item).expect("the row must validate");
    assert!(
        item.title.contains("zod: not installed, 3.24.4 pinned"),
        "{}",
        item.title
    );
    assert_eq!(
        item.confidence.provenance,
        ConfidenceProvenance::Heuristic,
        "no advisory involved: a filesystem fact alone"
    );
    assert!(
        !item
            .suggested_actions
            .iter()
            .any(|a| a.action_id == "view_source"),
        "nothing to open"
    );
}

#[test]
fn an_unparseable_manifest_is_skipped_without_panicking() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "pnpm-lock.yaml");
    let broken = dir.join("node_modules").join("hono");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("package.json"), "{ not json").unwrap();
    install(&dir, "zod", "3.0.0");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    pin(&db, &dir, "zod", "3.24.4", false);

    let rows = drift(&db);
    assert_eq!(rows.len(), 1, "the readable drift still reports");
    let names: Vec<&str> = rows[0]
        .packages
        .iter()
        .map(|p| p.package.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["zod"],
        "an unreadable manifest is skipped, never guessed"
    );
}

/// The ecosystem guard: a crates.io advisory for a crate that happens to be
/// called `hono` says nothing about the npm package.
#[test]
fn a_same_named_crates_io_advisory_never_exposes_an_npm_install() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.1");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    advisory(&db, "RUSTSEC-hono", "hono", "crates.io", "4.13.5");

    let rows = drift(&db);
    assert_eq!(rows.len(), 1, "still drift: {rows:?}");
    assert_eq!(
        rows[0].urgency(),
        Urgency::Watch,
        "an npm install is judged against npm advisories only"
    );
    assert!(rows[0].packages[0].cleared_by_install.is_empty());
    assert!(rows[0].lead_fix().is_none());
}

#[test]
fn an_install_exposed_either_way_is_medium_and_says_the_pin_needs_an_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.1");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    advisory(&db, "GHSA-hono-later", "hono", "npm", "4.14.0");

    let rows = drift(&db);
    assert_eq!(rows[0].urgency(), Urgency::Medium);
    assert!(
        rows[0].lead_fix().is_none(),
        "the pin is not a fix to state"
    );
    let item = rows[0].to_evidence_item();
    validate_item(&item).expect("the row must validate");
    assert!(
        item.explanation.contains("the pin needs an upgrade too"),
        "{}",
        item.explanation
    );
    assert_eq!(
        item.confidence.provenance,
        ConfidenceProvenance::OsvVerified
    );
}

#[test]
fn an_optional_dependency_that_did_not_install_is_not_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "package-lock.json");
    std::fs::write(
        dir.join("package.json"),
        r#"{"optionalDependencies":{"fsevents":"^2.3.3"}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    let db = test_db();
    pin(&db, &dir, "fsevents", "2.3.3", false);
    assert!(drift(&db).is_empty());
}

#[test]
fn dormant_and_excluded_projects_are_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), "app", "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.1");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    let path = path_of(&dir);
    let conn = db.conn.lock();

    let dormant = ProjectLiveness::from_entries(&[(path.as_str(), 400)]);
    assert!(
        projects_with(&conn, &dormant, &|_| false).is_empty(),
        "a dormant project is named once by collapse_dormant_alerts, not here"
    );
    let live = ProjectLiveness::from_entries(&[]);
    assert!(
        projects_with(&conn, &live, &|p| p == path).is_empty(),
        "an excluded project is not the user's"
    );
    assert_eq!(projects_with(&conn, &live, &|_| false).len(), 1);
}

#[test]
fn the_install_command_follows_the_lockfile_that_wrote_the_pins() {
    let cases: [(&[&str], &str); 4] = [
        (&["package-lock.json"], "npm ci"),
        (&["pnpm-lock.yaml"], "pnpm install"),
        (&["yarn.lock"], "yarn install"),
        // The walk processes yarn last, so yarn's pins are the ones stored.
        (&["pnpm-lock.yaml", "yarn.lock"], "yarn install"),
    ];
    for (files, command) in cases {
        let tmp = tempfile::tempdir().unwrap();
        for file in files {
            std::fs::write(tmp.path().join(file), "").unwrap();
        }
        assert_eq!(
            Lockfile::detect(tmp.path()).map(Lockfile::install_command),
            Some(command),
            "{files:?}"
        );
    }
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(Lockfile::detect(empty.path()), None);
}

#[test]
fn a_hostile_package_name_never_leaves_node_modules() {
    for name in [
        "../../etc",
        "..",
        "@scope/../x",
        "a/b/c",
        r"C:\x",
        "@/x",
        "",
    ] {
        assert!(package_path(name).is_none(), "{name:?}");
    }
    assert_eq!(
        package_path("@vercel/sandbox"),
        Some(PathBuf::from("@vercel").join("sandbox"))
    );
    assert_eq!(package_path("hono"), Some(PathBuf::from("hono")));
}

#[test]
fn a_long_project_name_still_yields_a_valid_title() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = project(tmp.path(), &"n".repeat(150), "pnpm-lock.yaml");
    install(&dir, "hono", "4.13.1");
    let db = test_db();
    pin(&db, &dir, "hono", "4.13.5", false);
    let item = drift(&db)[0].to_evidence_item();
    assert!(item.title.len() <= 120, "byte budget: {}", item.title.len());
    validate_item(&item).expect("a truncated title still validates");
}

/// A row too long for the title budget names the count — never a word cut
/// in half. Live snapshot 2026-09-10: paddle-webhook's row ended "... pinn".
#[test]
fn an_overlong_title_names_the_count_instead_of_cutting_a_word() {
    let package = |name: &str, installed: &str, pinned: &[&str]| PackageDrift {
        package: name.to_string(),
        pinned: pinned.iter().map(|v| v.to_string()).collect(),
        installed: Some(InstalledCopy {
            version: installed.to_string(),
            manifest: PathBuf::from(format!(
                "d:/4da/paddle-webhook/node_modules/{name}/package.json"
            )),
        }),
        cleared_by_install: Vec::new(),
        still_exposed: Vec::new(),
        pin_exposed: false,
        dev_only: true,
    };
    let drift = ProjectDrift {
        project: "d:/4da/paddle-webhook".to_string(),
        lockfile: Lockfile::Pnpm,
        packages: vec![
            package("@types/node", "22.19.17", &["20.11.0", "26.4.0"]),
            package("typescript", "5.8.2", &["5.9.3"]),
            package("vercel", "54.1.0", &["54.20.1"]),
            package("undici", "6.21.0", &["6.28.0"]),
        ],
    };
    let item = drift.to_evidence_item();
    validate_item(&item).expect("the row must validate");
    assert_eq!(
        item.title,
        "paddle-webhook — node_modules is out of sync with pnpm-lock.yaml (4 packages)"
    );
    assert!(
        item.explanation
            .contains("@types/node 22.19.17 installed, 20.11.0/26.4.0 pinned"),
        "the detail moves to the explanation: {}",
        item.explanation
    );
}
