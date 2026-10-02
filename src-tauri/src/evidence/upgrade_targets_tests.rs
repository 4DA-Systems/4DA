// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Upgrade Plan correctness (dependency-handoff Phase 1): targets per project
//! and release line, major jumps flagged, citations scoped to the row, unique
//! ids, and the per-line facts carried as `fix-target` citations. Every
//! fixture is a shape taken from the live plan of 2026-10-01.

use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::EvidenceItem;
use crate::test_utils::test_db;

const BRACE_Q2HR: &str = r#"[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"5.0.12"}]},{"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"3.0.9"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.7"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.21"}]}]"#;
const BRACE_3JXR: &str = r#"[{"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"5.0.7"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.16"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.2"}]}]"#;
const BRACE_F886: &str = r#"[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"5.0.5"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.13"}]}]"#;

const UNDICI_CXRH: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"5.29.0"}]},{"type":"SEMVER","events":[{"introduced":"6.0.0"},{"fixed":"6.21.2"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.0"}]}]"#;
const UNDICI_8XCM: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"6.28.0"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.29.0"}]},{"type":"SEMVER","events":[{"introduced":"8.0.0"},{"fixed":"8.9.0"}]}]"#;
const UNDICI_3WWX: &str = r#"[{"type":"SEMVER","events":[{"introduced":"6.25.0"},{"fixed":"6.28.1"}]},{"type":"SEMVER","events":[{"introduced":"7.28.0"},{"fixed":"7.29.1"}]},{"type":"SEMVER","events":[{"introduced":"8.1.0"},{"fixed":"8.10.2"}]}]"#;

fn build(db: &Database) -> Vec<EvidenceItem> {
    let (items, drops) = super::build_upgrade_plan_with_drops(db);
    assert_eq!(drops, 0, "no plan item may fail validation");
    items
}

fn ranged_advisory(db: &Database, id: &str, package: &str, ranges: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("{id} in {package}"),
        None,
        package,
        "npm",
        Some(ranges),
        None,
        Some("CVSS_V3"),
        Some(cvss),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

/// A transitive dev copy, exactly as the lockfile walk records it.
fn install(db: &Database, project: &str, package: &str, versions: &[&str]) {
    db.store_dependency(project, package, Some(versions[0]), "npm", true, None)
        .unwrap();
    let instances: Vec<DependencyInstanceInput> = versions
        .iter()
        .map(|v| DependencyInstanceInput {
            package_name: package.to_string(),
            version: v.to_string(),
            is_direct: false,
            is_dev: true,
            scope: "dev".to_string(),
        })
        .collect();
    db.store_dependency_instances(project, "npm", &instances)
        .unwrap();
}

fn brace_fixture() -> Database {
    let db = test_db();
    install(&db, "/navcal", "brace-expansion", &["1.1.12"]);
    install(&db, "/vscode", "brace-expansion", &["1.1.15"]);
    install(&db, "/webhook", "brace-expansion", &["1.1.18", "5.0.9"]);
    ranged_advisory(
        &db,
        "GHSA-q2hr-2g5m-vwhr",
        "brace-expansion",
        BRACE_Q2HR,
        6.5,
    );
    ranged_advisory(
        &db,
        "GHSA-3jxr-9vmj-r5cp",
        "brace-expansion",
        BRACE_3JXR,
        6.5,
    );
    ranged_advisory(
        &db,
        "GHSA-f886-m6hf-6m8v",
        "brace-expansion",
        BRACE_F886,
        6.5,
    );
    db
}

fn row_for<'a>(items: &'a [EvidenceItem], project: &str) -> &'a EvidenceItem {
    items
        .iter()
        .find(|i| i.affected_projects == vec![project.to_string()])
        .expect("a plan row names exactly this project")
}

/// The `fix-target` citations of a step (one per installed version).
fn line_cites(item: &EvidenceItem) -> Vec<(&str, &str)> {
    item.evidence
        .iter()
        .filter(|c| c.source == "fix-target")
        .map(|c| (c.title.as_str(), c.relevance_note.as_str()))
        .collect()
}

/// Live 2026-10-01: navcal (1.1.12) and the vscode extension (1.1.15) were
/// told ">= 5.0.12" because another project held 5.0.9. Each copy stays on
/// its own line; the project with two lines gets both targets.
#[test]
fn targets_are_per_project_and_per_release_line() {
    let db = brace_fixture();
    let items = build(&db);

    for project in ["/navcal", "/vscode"] {
        let row = row_for(&items, project);
        assert!(
            row.title.contains(">= 1.1.21") && !row.title.contains("5.0.12"),
            "{project}: {}",
            row.title
        );
        assert!(!row.title.contains("(major)"), "{}", row.title);
    }

    let webhook = row_for(&items, "/webhook");
    assert!(
        webhook.title.contains(">= 1.1.21 / 5.0.12"),
        "two lines, two targets: {}",
        webhook.title
    );
    assert!(
        webhook
            .explanation
            .contains("1.1.18 -> 1.1.21, 5.0.9 -> 5.0.12"),
        "{}",
        webhook.explanation
    );
    assert_eq!(
        line_cites(webhook)
            .iter()
            .map(|(title, _)| *title)
            .collect::<Vec<_>>(),
        vec!["1.1.18 -> 1.1.21", "5.0.9 -> 5.0.12"]
    );

    // Phase 1.2, carried in the canonical item: target, upgrade type,
    // clears-all, and each install site with its scope.
    let navcal = line_cites(row_for(&items, "/navcal"));
    assert_eq!(navcal.len(), 1);
    assert_eq!(navcal[0].0, "1.1.12 -> 1.1.21");
    assert_eq!(
        navcal[0].1,
        "patch upgrade; clears every known advisory. Installed in: /navcal (transitive, dev)"
    );
}

/// The two brace-expansion exposures that led with GHSA-3jxr shared one id;
/// triage on one silenced the other.
#[test]
fn rows_of_one_package_never_share_an_id() {
    let db = brace_fixture();
    let items = build(&db);
    assert_eq!(items.len(), 3, "three exposures, three rows: {items:#?}");
    let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3, "ids must be unique: {ids:?}");
}

/// A citation names the version installed in THIS row's projects — not the
/// advisory's representative from some other project.
#[test]
fn citations_cite_only_the_rows_own_installs() {
    let db = brace_fixture();
    let items = build(&db);
    let navcal = row_for(&items, "/navcal");
    let notes: Vec<&str> = navcal
        .evidence
        .iter()
        .filter(|c| c.source == "osv-advisory")
        .map(|c| c.relevance_note.as_str())
        .collect();
    assert!(!notes.is_empty());
    for note in notes {
        assert!(note.contains("1.1.12"), "{note}");
        assert!(
            !note.contains("1.1.15") && !note.contains("5.0.9"),
            "{note}"
        );
    }
}

/// navcal's undici 5.28.4: the clean path crosses into 6.x and past the
/// 6.25.0 window to 6.28.1 — a major jump, flagged. A 7.x copy elsewhere
/// stays on 7.x and does not set navcal's target.
#[test]
fn a_major_jump_is_flagged_and_other_projects_do_not_raise_it() {
    let db = test_db();
    install(&db, "/navcal", "undici", &["5.28.4"]);
    install(&db, "/vscode", "undici", &["7.28.0"]);
    ranged_advisory(&db, "GHSA-cxrh-j4jr-qwg3", "undici", UNDICI_CXRH, 7.5);
    ranged_advisory(&db, "GHSA-8xcm-r25x-g524", "undici", UNDICI_8XCM, 7.5);
    ranged_advisory(&db, "GHSA-3wwx-pv8p-q78v", "undici", UNDICI_3WWX, 7.5);

    let items = build(&db);
    let navcal = row_for(&items, "/navcal");
    assert!(
        navcal.title.contains(">= 6.28.1 (major)"),
        "{}",
        navcal.title
    );
    assert!(navcal.explanation.contains("crosses a major version"));
    assert!(line_cites(navcal)[0].1.starts_with("major upgrade"));

    let vscode = row_for(&items, "/vscode");
    assert!(vscode.title.contains(">= 7.29.1"), "{}", vscode.title);
    assert!(!vscode.title.contains("(major)"), "{}", vscode.title);
    assert!(line_cites(vscode)[0].1.starts_with("minor upgrade"));
}

/// Live verification on a real snapshot of the corpus (never the live file):
/// `FOURDA_VERIFY_DB=<copy> cargo test --lib live_snapshot_upgrade_targets -- --ignored --nocapture`
/// Prunes manifest-less projects on the COPY, rebuilds the plan, and prints
/// every step with its per-line targets.
#[test]
#[ignore = "requires FOURDA_VERIFY_DB pointing at a real database snapshot"]
fn live_snapshot_upgrade_targets() {
    let Ok(path) = std::env::var("FOURDA_VERIFY_DB") else {
        return;
    };
    assert!(
        !path.replace('\\', "/").ends_with("4DA/data/4da.db"),
        "point FOURDA_VERIFY_DB at a COPY, never the live database"
    );
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    {
        let conn = db.conn.lock();
        let pruned = crate::db::prune_orphaned_project_dependencies(
            &conn,
            &crate::db::project_gone_from_disk,
        )
        .expect("prune");
        println!("PRUNE {pruned:?}");
    }
    let (items, drops) = super::build_upgrade_plan_with_drops(&db);
    println!("PLAN items={} drops={drops}", items.len());
    for item in &items {
        println!("- {} | {}", item.id, item.title);
        println!("    projects: {:?}", item.affected_projects);
        for (title, note) in line_cites(item) {
            println!("    line {title} | {note}");
        }
        for c in item
            .evidence
            .iter()
            .filter(|c| c.source == "dependency-path")
        {
            println!("    path {} | {}", c.title, c.relevance_note);
        }
        if item.explanation.contains("Fixed only upstream") {
            println!("    why  {}", item.explanation);
        }
    }
    let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(before, ids.len(), "duplicate plan ids");
    assert_eq!(drops, 0, "no plan item may fail validation");
}
