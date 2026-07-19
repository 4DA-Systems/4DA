// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the Upgrade Plan ranking brain (extracted via #[path]).

use crate::db::Database;
use crate::evidence::{validate_item, EvidenceItem, EvidenceKind, Urgency, ACTION_IDS};
use crate::test_utils::test_db;

/// Items-only build for the tests; production takes `build_upgrade_plan_with_drops`
/// (which also surfaces the validation-drop canary the snapshot persists).
fn build_upgrade_plan(db: &Database) -> Vec<EvidenceItem> {
    super::build_upgrade_plan_with_drops(db).0
}

/// Store an advisory affecting `package < fixed` (a real SEMVER range), so a
/// dep at a lower version becomes a version-CONFIRMED match.
#[allow(clippy::too_many_arguments)]
fn advisory(db: &Database, id: &str, package: &str, ecosystem: &str, fixed: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("Vulnerability in {package}"),
        None,
        package,
        ecosystem,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(cvss),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

#[test]
fn cold_start_empty_db_yields_empty_plan() {
    let db = test_db();
    assert!(
        build_upgrade_plan(&db).is_empty(),
        "no deps + no advisories must render nothing (cold-start silent)"
    );
}

#[test]
fn confirmed_vuln_produces_a_valid_plan_step() {
    let db = test_db();
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-lodash-1", "lodash", "npm", "4.17.21", 7.5);

    let plan = build_upgrade_plan(&db);
    assert_eq!(plan.len(), 1, "one confirmed package -> one step");
    let step = &plan[0];

    // Every emitted item is schema-valid (the boundary contract).
    validate_item(step).expect("plan item must pass validate_item");

    assert_eq!(step.kind, EvidenceKind::Alert);
    assert!(
        step.title.starts_with("Upgrade lodash"),
        "title: {}",
        step.title
    );
    assert!(
        step.title.contains(">= 4.17.21"),
        "target version in title: {}",
        step.title
    );
    assert_eq!(step.affected_deps, vec!["lodash"]);
    assert_eq!(step.affected_projects, vec!["/proj/a"]);
    assert!(
        step.lens_hints.upgrade_plan,
        "must carry the upgrade_plan lens hint"
    );
    assert!(
        step.lens_hints.preemption,
        "renders inside the Preemption lens"
    );
    assert!(!step.evidence.is_empty(), "Alert requires citations");
    assert_eq!(step.urgency, Urgency::High, "CVSS 7.5 -> High");
    // Heuristic provenance -> excluded from the OSV-verified free floor (Signal-tier).
    assert!(matches!(
        step.confidence.provenance,
        crate::evidence::ConfidenceProvenance::Heuristic
    ));
}

#[test]
fn every_emitted_item_is_schema_valid_across_a_mixed_plan() {
    let db = test_db();
    db.store_dependency("/proj/a", "next", Some("14.0.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-next-1", "next", "npm", "14.2.0", 9.8);
    db.store_dependency("/proj/a", "vite", Some("5.0.0"), "npm", true, None)
        .unwrap();
    advisory(&db, "GHSA-vite-1", "vite", "npm", "5.1.0", 5.3);
    db.store_dependency("/proj/b", "axios", Some("1.5.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-axios-1", "axios", "npm", "1.6.0", 7.1);

    let plan = build_upgrade_plan(&db);
    assert!(plan.len() >= 3);
    for item in &plan {
        validate_item(item).unwrap_or_else(|e| panic!("invalid item {}: {e:?}", item.id));
        // No fake-action affordance ever (doctrine rule 5): actions are all in
        // the informational ACTION_IDS allow-list; none executes an upgrade.
        for a in &item.suggested_actions {
            assert!(
                ACTION_IDS.contains(&a.action_id.as_str()),
                "action {} not in the canonical allow-list",
                a.action_id
            );
            assert!(
                !["execute", "update", "apply", "upgrade", "install"]
                    .contains(&a.action_id.as_str()),
                "the plan must never claim to perform the upgrade (action {})",
                a.action_id
            );
        }
    }
}

#[test]
fn ranking_puts_critical_before_lower_severity() {
    let db = test_db();
    db.store_dependency("/proj/a", "low-pkg", Some("1.0.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-low", "low-pkg", "npm", "2.0.0", 5.0); // Medium
    db.store_dependency("/proj/a", "crit-pkg", Some("1.0.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-crit", "crit-pkg", "npm", "2.0.0", 9.9); // Critical

    let plan = build_upgrade_plan(&db);
    let crit = plan
        .iter()
        .position(|i| i.affected_deps == vec!["crit-pkg"])
        .unwrap();
    let low = plan
        .iter()
        .position(|i| i.affected_deps == vec!["low-pkg"])
        .unwrap();
    assert!(crit < low, "critical must rank above medium");
    assert_eq!(plan[crit].urgency, Urgency::Critical);
}

#[test]
fn ranking_puts_fixable_now_before_waiting_on_upstream() {
    let db = test_db();
    // Same severity; one direct (fixable now), one transitive (waiting).
    db.store_dependency("/proj/a", "direct-pkg", Some("1.0.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-direct", "direct-pkg", "npm", "2.0.0", 7.5);
    db.store_transitive_dependency("/proj/a", "trans-pkg", Some("1.0.0"), "npm", false)
        .unwrap();
    advisory(&db, "GHSA-trans", "trans-pkg", "npm", "2.0.0", 7.5);

    let plan = build_upgrade_plan(&db);
    let direct = plan
        .iter()
        .position(|i| i.affected_deps == vec!["direct-pkg"])
        .unwrap();
    let trans = plan
        .iter()
        .position(|i| i.affected_deps == vec!["trans-pkg"])
        .unwrap();
    assert!(
        direct < trans,
        "fixable-now (direct) ranks above waiting-on-upstream (transitive)"
    );
    assert!(
        plan[trans].explanation.contains("upstream"),
        "transitive step explains it waits on upstream"
    );
}

#[test]
fn one_package_many_advisories_folds_into_one_step() {
    let db = test_db();
    db.store_dependency("/proj/a", "lodash", Some("4.17.10"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-lo-1", "lodash", "npm", "4.17.19", 7.0);
    advisory(&db, "GHSA-lo-2", "lodash", "npm", "4.17.21", 7.5);
    advisory(&db, "GHSA-lo-3", "lodash", "npm", "4.17.20", 6.0);

    let plan = build_upgrade_plan(&db);
    assert_eq!(
        plan.len(),
        1,
        "three advisories on one package -> one upgrade step"
    );
    let step = &plan[0];
    assert!(
        step.title.contains("clears 3 advisories"),
        "title: {}",
        step.title
    );
    // Target is the HIGHEST fix (clears the most).
    assert!(step.title.contains(">= 4.17.21"), "title: {}", step.title);
}

#[test]
fn unconfirmed_matches_are_excluded_from_the_plan() {
    let db = test_db();
    db.store_dependency("/proj/a", "mystery", Some("1.0.0"), "npm", false, None)
        .unwrap();
    // Advisory with NO affected range -> check_version_affected is conservative
    // (affected but UNCONFIRMED). Such a "maybe" is not an "upgrade this".
    db.upsert_osv_advisory(
        "GHSA-unconf",
        "Unconfirmable advisory",
        None,
        "mystery",
        "npm",
        None, // no ranges
        None,
        Some("CVSS_V3"),
        Some(9.0),
        None,
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();

    assert!(
        build_upgrade_plan(&db).is_empty(),
        "a name-only (unconfirmed) match must not become a plan step"
    );
}

#[test]
fn dev_only_package_is_downranked_but_still_present() {
    let db = test_db();
    // A HIGH (7.5) advisory, but the affected dep is dev-only -> discounted to Medium.
    db.store_dependency("/proj/a", "eslint", Some("8.0.0"), "npm", true, None)
        .unwrap();
    advisory(&db, "GHSA-eslint", "eslint", "npm", "8.1.0", 7.5);

    let plan = build_upgrade_plan(&db);
    assert_eq!(plan.len(), 1);
    assert_eq!(
        plan[0].urgency,
        Urgency::Medium,
        "dev-only HIGH is discounted one level to Medium (labelled, not suppressed)"
    );
    assert!(
        plan[0].explanation.contains("dev-only"),
        "the discount is labelled in the explanation"
    );
}

#[test]
fn cross_project_multiplicity_widens_blast_radius_and_ranks_up() {
    let db = test_db();
    // Two packages, same severity; one hits 3 projects, the other 1.
    for p in ["/proj/a", "/proj/b", "/proj/c"] {
        db.store_dependency(p, "wide", Some("1.0.0"), "npm", false, None)
            .unwrap();
    }
    advisory(&db, "GHSA-wide", "wide", "npm", "2.0.0", 7.5);
    db.store_dependency("/proj/a", "narrow", Some("1.0.0"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-narrow", "narrow", "npm", "2.0.0", 7.5);

    let plan = build_upgrade_plan(&db);
    let wide = plan
        .iter()
        .position(|i| i.affected_deps == vec!["wide"])
        .unwrap();
    let narrow = plan
        .iter()
        .position(|i| i.affected_deps == vec!["narrow"])
        .unwrap();
    assert!(
        wide < narrow,
        "the bump that clears more projects ranks higher"
    );
    assert_eq!(plan[wide].affected_projects.len(), 3);
    assert!(
        plan[wide].title.contains("across 3 projects"),
        "title: {}",
        plan[wide].title
    );
}

#[test]
fn long_project_list_never_produces_an_over_length_citation_note() {
    // Regression for the CitationNoteTooLong bug fixed in #316 (truncate budgeted
    // 1 byte for the 3-byte "…" ellipsis → 202-byte notes → validate_item reject
    // → the plan step silently dropped in release). Live-caught on the founder's
    // corpus; hermetic fixtures had short paths and missed it. This reproduces
    // the exact trigger: a package across many long project paths + a long
    // advisory summary, both of which drive the citation `truncate` path.
    let db = test_db();
    for i in 0..12 {
        let proj = format!("C:/Users/dev/workspace/monorepo-{i}/packages/service-{i}/frontend");
        db.store_dependency(&proj, "lodash", Some("4.17.20"), "npm", false, None)
            .unwrap();
    }
    db.upsert_osv_advisory(
        "GHSA-long-1",
        &"Prototype pollution in lodash allows an attacker to modify the prototype of a base object. ".repeat(4),
        None,
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        Some("CVSS_V3"),
        Some(9.8),
        Some("https://osv.dev/GHSA-long-1"),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();

    let plan = build_upgrade_plan(&db);
    assert_eq!(plan.len(), 1, "the plan step must survive (not be dropped)");
    validate_item(&plan[0]).expect("plan item must pass validate_item");
    for cite in &plan[0].evidence {
        assert!(
            cite.relevance_note.len() <= 200,
            "citation note {} bytes exceeds the 200-byte bound: {:?}",
            cite.relevance_note.len(),
            cite.relevance_note
        );
    }
}

#[test]
fn truncate_result_never_exceeds_the_byte_budget() {
    // Unit guard on the helper across ASCII + multibyte inputs, over the budget
    // range the plan actually uses (>= the 3-byte ellipsis width; call sites pass
    // 160 and 200). Would fail against the pre-#316 code, which overshot by 2.
    for &max in &[3usize, 4, 10, 50, 160, 200] {
        for s in [
            "",
            "short",
            &"x".repeat(500),
            &"é".repeat(300),
            &"世界".repeat(200),
        ] {
            let out = super::truncate(s, max);
            assert!(
                out.len() <= max || s.len() <= max,
                "truncate(<{} bytes>, {}) = {} bytes",
                s.len(),
                max,
                out.len()
            );
        }
    }
}

#[test]
fn platform_inactive_package_is_excluded_from_the_plan() {
    // The plan answers "which upgrade matters to YOUR build". A package that is
    // platform-inactive in every project (a cfg(not(windows))-only crate on
    // Windows) is genuinely irrelevant — excluded from the ranked plan. It is
    // NOT hidden: Preemption still surfaces its advisory in the collapsed "other
    // build targets" group. An active package with the same severity stays.
    let db = test_db();
    {
        let conn = db.conn.lock();
        conn.execute_batch(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_direct, language, platform_active) VALUES
                ('/proj/a', 'cargotoml', 'winapi', '0.3.0', 1, 'rust', 0),
                ('/proj/a', 'packagejson', 'axios', '1.5.0', 1, 'javascript', 1);",
        )
        .unwrap();
    }
    advisory(&db, "GHSA-winapi-1", "winapi", "crates.io", "0.4.0", 7.5);
    advisory(&db, "GHSA-axios-1", "axios", "npm", "1.6.0", 7.5);

    let plan = build_upgrade_plan(&db);
    let pkgs: Vec<String> = plan.iter().flat_map(|i| i.affected_deps.clone()).collect();
    assert!(
        pkgs.iter().any(|p| p == "axios"),
        "platform-active package must be in the plan; got {pkgs:?}"
    );
    assert!(
        !pkgs.iter().any(|p| p == "winapi"),
        "platform-inactive package must be excluded from the plan; got {pkgs:?}"
    );
}

#[test]
fn persist_and_read_upgrade_plan_snapshot_round_trips() {
    use super::{persist_upgrade_plan, read_upgrade_plan_snapshot};
    let db = test_db();
    // No snapshot yet -> None.
    assert!(read_upgrade_plan_snapshot(&db).is_none());

    // Build a real plan and persist it.
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-snap-1", "lodash", "npm", "4.17.21", 7.5);
    let plan = build_upgrade_plan(&db);
    assert_eq!(plan.len(), 1);
    persist_upgrade_plan(&db, &plan, 0, None);

    let snap = read_upgrade_plan_snapshot(&db).expect("snapshot present after persist");
    assert_eq!(snap.item_count, 1);
    assert_eq!(snap.items.len(), 1);
    assert_eq!(snap.items[0].affected_deps, vec!["lodash"]);
    assert!(!snap.generated_at.is_empty());
    assert!(!snap.generator_version.is_empty());
    // v2 envelope metadata.
    assert!(!snap.expires_at.is_empty(), "staleness horizon is stamped");
    assert_ne!(
        snap.expires_at, snap.generated_at,
        "expiry is offset from generation by the sync horizon"
    );
    assert!(
        snap.entitlement_scope_at_generation == "signal"
            || snap.entitlement_scope_at_generation == "free",
        "tier scope is one of the two known values, got {:?}",
        snap.entitlement_scope_at_generation
    );
    assert_eq!(snap.validation_drop_count, 0, "a valid plan drops nothing");
    assert!(
        !snap.dependency_inventory_hash.is_empty(),
        "inventory hash is stamped even when the inventory is empty"
    );
    // This test seeds project_dependencies but no dependency_instances.
    assert!(
        !snap.multi_version_coverage,
        "no multi-version inventory -> coverage gate is not green"
    );
    // This (GUI-style) persist attributes no engine run.
    assert!(
        snap.engine_run_id.is_none(),
        "GUI/test persist stamps no engine_run_id"
    );
    // Items survive the JSON round-trip byte-for-byte.
    assert_eq!(snap.items, plan);
}

#[test]
fn envelope_stamps_engine_run_id_and_source_freshness() {
    use super::{persist_upgrade_plan, read_upgrade_plan_snapshot};
    let db = test_db();

    // The headless path attributes the engine run id it is handed.
    persist_upgrade_plan(&db, &[], 0, Some(4242));
    let snap = read_upgrade_plan_snapshot(&db).unwrap();
    assert_eq!(
        snap.engine_run_id,
        Some(4242),
        "headless persist stamps the run id"
    );
    // No OSV sync recorded yet -> no freshness floor.
    assert!(
        snap.source_freshness.is_none(),
        "no ecosystem synced -> no freshness floor"
    );

    // Once an ecosystem has synced, the plan records the freshness floor; a GUI
    // persist (None) carries no run id.
    db.update_osv_sync_status("npm", 100, None).unwrap();
    persist_upgrade_plan(&db, &[], 0, None);
    let snap = read_upgrade_plan_snapshot(&db).unwrap();
    assert!(
        snap.source_freshness.is_some(),
        "a synced ecosystem sets the freshness floor"
    );
    assert!(snap.engine_run_id.is_none(), "GUI persist -> no run id");
}

#[test]
fn persist_upgrade_plan_records_an_empty_plan() {
    // "Evaluated, nothing to do" must be distinguishable from "never computed":
    // an empty plan still writes a snapshot with a fresh timestamp and 0 items.
    use super::{persist_upgrade_plan, read_upgrade_plan_snapshot};
    let db = test_db();
    persist_upgrade_plan(&db, &[], 0, None);
    let snap = read_upgrade_plan_snapshot(&db).expect("empty plan is still persisted");
    assert_eq!(snap.item_count, 0);
    assert!(snap.items.is_empty());
    assert!(!snap.generated_at.is_empty());
}

#[test]
fn persist_upgrade_plan_is_latest_wins() {
    use super::{persist_upgrade_plan, read_upgrade_plan_snapshot};
    let db = test_db();
    db.store_dependency("/proj/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();
    advisory(&db, "GHSA-lw-1", "lodash", "npm", "4.17.21", 7.5);
    persist_upgrade_plan(&db, &build_upgrade_plan(&db), 0, None);
    // A later empty compute overwrites the single row.
    persist_upgrade_plan(&db, &[], 0, None);
    let snap = read_upgrade_plan_snapshot(&db).unwrap();
    assert_eq!(
        snap.item_count, 0,
        "latest-wins: the empty snapshot replaced the earlier one"
    );
}

#[test]
fn envelope_metadata_reflects_inventory_coverage_and_hash() {
    use super::{build_upgrade_plan_with_drops, persist_upgrade_plan, read_upgrade_plan_snapshot};
    use crate::db::DependencyInstanceInput;
    let db = test_db();
    let mk = |name: &str, ver: &str| DependencyInstanceInput {
        package_name: name.to_string(),
        version: ver.to_string(),
        is_direct: true,
        is_dev: false,
        scope: "runtime".to_string(),
    };

    // No multi-version inventory -> gate not green; hash of the empty set is a
    // stable non-empty constant.
    persist_upgrade_plan(&db, &[], 0, None);
    let empty_hash = read_upgrade_plan_snapshot(&db)
        .unwrap()
        .dependency_inventory_hash;
    assert!(!empty_hash.is_empty());

    // Populate the inventory (two versions of one package = the multi-version case).
    db.store_dependency_instances(
        "/proj/a",
        "javascript",
        &[mk("lodash", "4.17.20"), mk("lodash", "4.17.21")],
    )
    .unwrap();
    persist_upgrade_plan(&db, &[], 0, None);
    let snap1 = read_upgrade_plan_snapshot(&db).unwrap();
    assert!(
        snap1.multi_version_coverage,
        "instances present -> coverage gate green"
    );
    assert_ne!(
        snap1.dependency_inventory_hash, empty_hash,
        "hash reflects the populated inventory"
    );

    // Unchanged inventory -> identical hash (deterministic).
    persist_upgrade_plan(&db, &[], 0, None);
    assert_eq!(
        read_upgrade_plan_snapshot(&db)
            .unwrap()
            .dependency_inventory_hash,
        snap1.dependency_inventory_hash,
        "hash is stable for an unchanged inventory"
    );

    // Changed inventory -> different hash (a reader can detect drift).
    db.store_dependency_instances("/proj/a", "javascript", &[mk("lodash", "4.17.20")])
        .unwrap();
    persist_upgrade_plan(&db, &[], 0, None);
    assert_ne!(
        read_upgrade_plan_snapshot(&db)
            .unwrap()
            .dependency_inventory_hash,
        snap1.dependency_inventory_hash,
        "hash changes when the inventory changes"
    );

    // build_upgrade_plan_with_drops reports zero drops for a clean (empty) build.
    let (items, drops) = build_upgrade_plan_with_drops(&db);
    assert!(items.is_empty());
    assert_eq!(drops, 0);
}

#[test]
fn read_upgrade_plan_snapshot_rejects_incompatible_schema_version() {
    // A snapshot written by a future/incompatible build is treated as absent
    // (fail closed) — a reader must never act on a snapshot it cannot trust.
    use super::read_upgrade_plan_snapshot;
    let db = test_db();
    // A well-formed envelope but with a schema_version we do not understand.
    let bogus = r#"{"schema_version":999999,"generated_at":"2026-01-01T00:00:00Z","generator_version":"x","item_count":0,"items":[]}"#;
    db.set_kv("upgrade_plan_snapshot", bogus).unwrap();
    assert!(read_upgrade_plan_snapshot(&db).is_none());

    // Garbage JSON is also treated as absent, not a panic.
    db.set_kv("upgrade_plan_snapshot", "not json").unwrap();
    assert!(read_upgrade_plan_snapshot(&db).is_none());
}
