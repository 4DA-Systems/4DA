// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The Upgrade Plan's work order (AD-049): every step belongs to an item and
//! every item has its step, a step's target is the one its item's title
//! shows, the mechanism follows the install sites, and the persisted envelope
//! is fail-closed on any other schema version.

use super::{Mechanism, UpgradeStep, VERIFICATION};
use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::{persist_upgrade_plan, read_upgrade_plan_snapshot, EvidenceItem};
use crate::osv::fix_target::UpgradeType;
use crate::test_utils::test_db;

const UNDICI_CXRH: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"5.29.0"}]},{"type":"SEMVER","events":[{"introduced":"6.0.0"},{"fixed":"6.21.2"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.0"}]}]"#;
const UNDICI_8XCM: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"6.28.0"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.29.0"}]},{"type":"SEMVER","events":[{"introduced":"8.0.0"},{"fixed":"8.9.0"}]}]"#;
const UNDICI_3WWX: &str = r#"[{"type":"SEMVER","events":[{"introduced":"6.25.0"},{"fixed":"6.28.1"}]},{"type":"SEMVER","events":[{"introduced":"7.28.0"},{"fixed":"7.29.1"}]},{"type":"SEMVER","events":[{"introduced":"8.1.0"},{"fixed":"8.10.2"}]}]"#;
const BRACE_Q2HR: &str = r#"[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"5.0.12"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.21"}]}]"#;

fn ranged(db: &Database, id: &str, package: &str, ecosystem: &str, ranges: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("{id} in {package}"),
        None,
        package,
        ecosystem,
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

/// `introduced: 0` and nothing else — every version affected, no fix.
fn unfixed(db: &Database, id: &str, package: &str, summary: &str, cvss: Option<f64>) {
    db.upsert_osv_advisory(
        id,
        summary,
        None,
        package,
        "crates.io",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#),
        None,
        cvss.map(|_| "CVSS_V3"),
        cvss,
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

/// One project's inventory for one ecosystem, as ONE lockfile walk records it
/// (`store_dependency_instances` replaces the project's instance set).
fn install(db: &Database, project: &str, eco: &str, packages: &[(&str, &str, bool)]) {
    let mut instances = Vec::new();
    for (package, version, direct) in packages {
        db.store_dependency(project, package, Some(version), eco, false, None)
            .unwrap();
        instances.push(DependencyInstanceInput {
            package_name: package.to_string(),
            version: version.to_string(),
            is_direct: *direct,
            is_dev: false,
            scope: "runtime".to_string(),
        });
    }
    db.store_dependency_instances(project, eco, &instances)
        .unwrap();
}

/// Every shape the plan emits: a direct bump, a transitive major jump, a
/// mixed cohort, a vulnerability with no fix, and the maintenance-notice item.
fn mixed_fixture() -> Database {
    let db = test_db();
    install(
        &db,
        "/navcal",
        "npm",
        &[
            ("undici", "5.28.4", false),
            ("brace-expansion", "1.1.12", false),
        ],
    );
    install(&db, "/vscode", "npm", &[("undici", "7.28.0", true)]);
    install(&db, "/web", "npm", &[("brace-expansion", "1.1.12", true)]);
    install(
        &db,
        "/relay",
        "crates.io",
        &[("rsa", "0.9.8", true), ("paste", "1.0.15", false)],
    );
    ranged(
        &db,
        "GHSA-cxrh-j4jr-qwg3",
        "undici",
        "npm",
        UNDICI_CXRH,
        7.5,
    );
    ranged(
        &db,
        "GHSA-8xcm-r25x-g524",
        "undici",
        "npm",
        UNDICI_8XCM,
        7.5,
    );
    ranged(
        &db,
        "GHSA-3wwx-pv8p-q78v",
        "undici",
        "npm",
        UNDICI_3WWX,
        7.5,
    );
    ranged(
        &db,
        "GHSA-q2hr-2g5m-vwhr",
        "brace-expansion",
        "npm",
        BRACE_Q2HR,
        6.5,
    );
    unfixed(&db, "RUSTSEC-2023-0071", "rsa", "Marvin attack", Some(5.9));
    unfixed(
        &db,
        "RUSTSEC-2024-0436",
        "paste",
        "paste - no longer maintained",
        None,
    );
    db
}

fn built(db: &Database) -> (Vec<EvidenceItem>, Vec<UpgradeStep>) {
    let plan = super::super::build_upgrade_plan(db);
    assert_eq!(plan.drops, 0, "no plan item may fail validation");
    (plan.items, plan.steps)
}

fn step_for<'a>(steps: &'a [UpgradeStep], package: &str, project: &str) -> &'a UpgradeStep {
    steps
        .iter()
        .find(|s| {
            s.package == package
                && s.lines
                    .iter()
                    .any(|l| l.sites.iter().any(|site| site.project == project))
        })
        .unwrap_or_else(|| panic!("a step for {package} in {project}: {steps:#?}"))
}

/// The parity contract: one computation, two renderings. Every step names an
/// item that exists and every item has its step; the step's targets are
/// exactly the ones the title shows; a "No fix published" item is `no_fix`.
#[test]
fn steps_and_items_are_one_plan() {
    let db = mixed_fixture();
    let (items, steps) = built(&db);
    assert!(items.len() >= 5, "{items:#?}");

    for step in &steps {
        assert!(
            items.iter().any(|i| i.id == step.item_id),
            "orphan step {}",
            step.item_id
        );
        assert_eq!(step.verification, VERIFICATION);
    }
    for item in &items {
        let own: Vec<&UpgradeStep> = steps.iter().filter(|s| s.item_id == item.id).collect();
        assert!(!own.is_empty(), "item without a step: {}", item.id);
        if item.id == "upgrade-plan:informational" {
            assert!(own.iter().all(|s| s.mechanism == Mechanism::NoFix));
            continue;
        }
        assert_eq!(own.len(), 1, "one step per package row: {}", item.id);
        let step = own[0];
        assert_eq!(item.affected_deps, vec![step.package.clone()]);
        let mut targets: Vec<&str> = step
            .lines
            .iter()
            .filter_map(|l| l.target.as_deref())
            .collect();
        targets.sort_by_key(|t| semver::Version::parse(t).expect("semver target"));
        targets.dedup();
        if targets.is_empty() {
            assert!(item.title.starts_with("No fix published"), "{}", item.title);
            assert_eq!(step.mechanism, Mechanism::NoFix);
        } else {
            let shown = format!("to >= {}", targets.join(" / "));
            assert!(
                item.title.contains(&shown),
                "step targets {shown:?} vs title {:?}",
                item.title
            );
            assert_ne!(step.mechanism, Mechanism::NoFix);
        }
        let mut projects: Vec<&str> = step
            .lines
            .iter()
            .flat_map(|l| l.sites.iter().map(|s| s.project.as_str()))
            .collect();
        projects.sort_unstable();
        projects.dedup();
        assert_eq!(projects, item.affected_projects, "{}", item.id);
    }
}

#[test]
fn mechanism_follows_the_install_sites() {
    let db = mixed_fixture();
    let (_, steps) = built(&db);

    let navcal_undici = step_for(&steps, "undici", "/navcal");
    assert_eq!(navcal_undici.mechanism, Mechanism::LockfileOrParentUpdate);
    assert_eq!(navcal_undici.ecosystem, "npm");
    assert_eq!(navcal_undici.lines.len(), 1);
    let line = &navcal_undici.lines[0];
    assert_eq!(line.installed, "5.28.4");
    assert_eq!(line.target.as_deref(), Some("6.28.1"));
    assert_eq!(line.upgrade_type, Some(UpgradeType::Major));
    assert!(line.clears_all_known);

    let vscode_undici = step_for(&steps, "undici", "/vscode");
    assert_eq!(vscode_undici.mechanism, Mechanism::ManifestBump);
    assert_eq!(vscode_undici.lines[0].target.as_deref(), Some("7.29.1"));
    assert_eq!(
        vscode_undici.lines[0].upgrade_type,
        Some(UpgradeType::Minor)
    );

    // One version, one advisory set, direct in /web and transitive in
    // /navcal: one row, both mechanisms.
    let brace = step_for(&steps, "brace-expansion", "/navcal");
    assert_eq!(brace.mechanism, Mechanism::Mixed, "{brace:#?}");
    assert_eq!(brace.lines[0].sites.len(), 2);
    assert_eq!(brace.lines[0].upgrade_type, Some(UpgradeType::Patch));

    let rsa = step_for(&steps, "rsa", "/relay");
    assert_eq!(rsa.mechanism, Mechanism::NoFix);
    assert_eq!(rsa.ecosystem, "crates.io");
    assert_eq!(rsa.lines[0].target, None);
    assert!(!rsa.lines[0].clears_all_known);
    assert_eq!(rsa.advisory_ids, vec!["RUSTSEC-2023-0071"]);

    let paste = step_for(&steps, "paste", "/relay");
    assert_eq!(paste.item_id, "upgrade-plan:informational");
    assert_eq!(paste.mechanism, Mechanism::NoFix);
}

/// The serialized names are the contract an agent reads.
#[test]
fn a_step_serializes_to_the_documented_shape() {
    let db = mixed_fixture();
    let (_, steps) = built(&db);
    let step = step_for(&steps, "undici", "/navcal");
    let v = serde_json::to_value(step).unwrap();
    assert_eq!(v["mechanism"], "lockfile_or_parent_update");
    assert_eq!(v["ecosystem"], "npm");
    assert_eq!(v["package"], "undici");
    assert_eq!(v["lines"][0]["installed"], "5.28.4");
    assert_eq!(v["lines"][0]["target"], "6.28.1");
    assert_eq!(v["lines"][0]["upgrade_type"], "major");
    assert_eq!(v["lines"][0]["clears_all_known"], true);
    assert_eq!(
        v["lines"][0]["sites"][0],
        serde_json::json!({"project": "/navcal", "direct": false, "dev": false})
    );
    assert!(v["advisory_ids"].as_array().unwrap().len() >= 2);
    assert!(v["item_id"]
        .as_str()
        .unwrap()
        .starts_with("upgrade-plan:npm:undici"));
    let rsa = serde_json::to_value(step_for(&steps, "rsa", "/relay")).unwrap();
    assert_eq!(rsa["mechanism"], "no_fix");
    assert_eq!(rsa["lines"][0]["target"], serde_json::Value::Null);
    assert_eq!(rsa["lines"][0]["upgrade_type"], serde_json::Value::Null);
}

#[test]
fn the_snapshot_carries_the_steps_and_never_an_orphan() {
    let db = mixed_fixture();
    let plan = super::super::build_upgrade_plan(&db);
    let mut steps = plan.steps.clone();
    let mut orphan = steps[0].clone();
    orphan.item_id = "upgrade-plan:npm:not-in-the-plan".to_string();
    steps.push(orphan);
    persist_upgrade_plan(&db, &plan.items, &steps, plan.drops, None);

    let snap = read_upgrade_plan_snapshot(&db).expect("current schema reads back");
    assert_eq!(snap.schema_version, 4);
    assert_eq!(snap.steps, plan.steps, "the orphan is never written");
    assert_eq!(snap.items, plan.items);
}

/// The reader is fail-closed on any other schema version: a v3 envelope (no
/// `steps`) and a future v5 both read as absent, so the caller recomputes;
/// after the recompute the current envelope reads back with its steps.
#[test]
fn another_schema_version_reads_as_absent_until_recomputed() {
    let db = mixed_fixture();
    let plan = super::super::build_upgrade_plan(&db);
    persist_upgrade_plan(&db, &plan.items, &plan.steps, plan.drops, None);
    let current: serde_json::Value =
        serde_json::from_str(&db.get_kv("upgrade_plan_snapshot").unwrap().unwrap()).unwrap();

    let mut v3 = current.clone();
    v3["schema_version"] = 3.into();
    v3.as_object_mut().unwrap().remove("steps");
    db.set_kv("upgrade_plan_snapshot", &v3.to_string()).unwrap();
    assert!(
        read_upgrade_plan_snapshot(&db).is_none(),
        "v3 is not trusted"
    );

    let mut v5 = current;
    v5["schema_version"] = 5.into();
    db.set_kv("upgrade_plan_snapshot", &v5.to_string()).unwrap();
    assert!(
        read_upgrade_plan_snapshot(&db).is_none(),
        "v5 is not trusted"
    );

    // The recompute every writer performs restores a readable plan.
    let again = super::super::build_upgrade_plan(&db);
    persist_upgrade_plan(&db, &again.items, &again.steps, again.drops, None);
    let snap = read_upgrade_plan_snapshot(&db).expect("recomputed");
    assert_eq!(snap.steps, again.steps);
}

/// Live verification on a COPY of the corpus (never the live file):
/// `FOURDA_VERIFY_DB=<copy> cargo test --lib live_snapshot_work_order -- --ignored --nocapture`
/// Prunes manifest-less projects on the copy, builds the plan, persists the
/// snapshot INTO THE COPY (so an out-of-process reader can be pointed at it)
/// and prints every step.
#[test]
#[ignore = "requires FOURDA_VERIFY_DB pointing at a real database snapshot"]
fn live_snapshot_work_order() {
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
        crate::db::prune_orphaned_project_dependencies(&conn, &crate::db::project_gone_from_disk)
            .expect("prune");
    }
    let plan = super::super::build_upgrade_plan(&db);
    persist_upgrade_plan(&db, &plan.items, &plan.steps, plan.drops, None);
    println!(
        "PLAN items={} steps={} drops={}",
        plan.items.len(),
        plan.steps.len(),
        plan.drops
    );
    for item in &plan.items {
        println!("ITEM {} | {}", item.id, item.title);
    }
    for step in &plan.steps {
        println!("STEP {}", serde_json::to_string(step).unwrap());
    }
    for step in &plan.steps {
        assert!(plan.items.iter().any(|i| i.id == step.item_id));
    }
    for item in &plan.items {
        assert!(
            plan.steps.iter().any(|s| s.item_id == item.id),
            "{}",
            item.id
        );
    }
    assert_eq!(plan.drops, 0);
}
