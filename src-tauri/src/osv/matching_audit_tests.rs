use super::*;
use crate::test_utils::test_db;

#[test]
fn transitive_and_dev_dependencies_are_matched_but_worktrees_are_not() {
    let db = test_db();
    db.store_transitive_dependency(
        "/project/runtime",
        "vulnerable-pkg",
        Some("1.0.0"),
        "npm",
        false,
    )
    .unwrap();
    db.store_dependency(
        "/project/dev",
        "vulnerable-pkg",
        Some("1.0.0"),
        "npm",
        true,
        None,
    )
    .unwrap();
    db.store_dependency(
        "/project/.claude/worktrees/branch",
        "vulnerable-pkg",
        Some("1.0.0"),
        "npm",
        false,
        None,
    )
    .unwrap();
    db.upsert_osv_advisory(
        "GHSA-transitive-dev",
        "Vuln in vulnerable-pkg",
        None,
        "vulnerable-pkg",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#),
        Some(r#"["2.0.0"]"#),
        None,
        Some(8.0),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].dependency_instances.len(), 2);
    assert!(matches[0]
        .dependency_instances
        .iter()
        .any(|instance| !instance.is_direct && !instance.is_dev));
    assert!(matches[0]
        .dependency_instances
        .iter()
        .any(|instance| instance.is_direct && instance.is_dev));
    assert!(!matches[0]
        .project_paths
        .iter()
        .any(|path| path.contains("worktrees")));
}

#[test]
fn confirmed_instance_drives_version_and_project_scope() {
    let db = test_db();
    db.store_dependency("/project/unknown", "lodash", None, "npm", false, None)
        .unwrap();
    db.store_transitive_dependency(
        "/project/confirmed",
        "lodash",
        Some("4.17.20"),
        "npm",
        false,
    )
    .unwrap();
    db.upsert_osv_advisory(
        "GHSA-confirmed-scope",
        "Prototype pollution in lodash",
        None,
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        None,
        Some(7.5),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].installed_version.as_deref(), Some("4.17.20"));
    assert_eq!(matches[0].project_paths, vec!["/project/confirmed"]);
    assert_eq!(matches[0].dependency_instances.len(), 2);
}

/// Phase 1.4: a window whose bound cannot be parsed was never evaluated, so
/// "confirmed not affected" was a claim about nothing. It is unknown
/// (conservative, unconfirmed) — unless another window decides it.
#[test]
fn unparseable_bound_is_unknown_not_confirmed_safe() {
    let ranges = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"r1.0~weird"}]}]"#
            .to_string(),
    );
    assert_eq!(
        check_version_affected(Some("2.0.0"), &ranges),
        (true, false),
        "an undecided window must not read as confirmed-not-affected"
    );
    // `1.0.post1` is a PEP 440 bound now (`osv::version_order`): it decides.
    let pep440 = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.0.post1"}]}]"#.to_string(),
    );
    assert_eq!(
        check_version_affected(Some("2.0.0"), &pep440),
        (false, true)
    );
    let ranges = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"weird"},{"fixed":"3.0.0"}]}]"#.to_string(),
    );
    assert_eq!(
        check_version_affected(Some("2.0.0"), &ranges),
        (true, false)
    );

    // A parseable window that DOES contain the version still confirms it.
    let mixed = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"r1.0~weird"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.0"}]}]"#
            .to_string(),
    );
    assert_eq!(check_version_affected(Some("2.0.5"), &mixed), (true, true));
}

/// Phase 1.1 at the matcher: each copy carries its OWN line's fix and clean
/// version; only the advisory-level field stays machine-wide.
#[test]
fn each_copy_carries_its_own_line_targets() {
    let db = test_db();
    db.store_dependency(
        "/navcal",
        "brace-expansion",
        Some("1.1.12"),
        "npm",
        true,
        None,
    )
    .unwrap();
    db.store_dependency(
        "/webhook",
        "brace-expansion",
        Some("5.0.9"),
        "npm",
        true,
        None,
    )
    .unwrap();
    db.upsert_osv_advisory(
        "GHSA-q2hr-2g5m-vwhr",
        "ReDoS in brace-expansion",
        None,
        "brace-expansion",
        "npm",
        Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"5.0.12"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.21"}]}]"#,
        ),
        Some(r#"["5.0.12","1.1.21"]"#),
        None,
        Some(6.5),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    let copy = |project: &str| {
        matches[0]
            .dependency_instances
            .iter()
            .find(|d| d.project_path == project)
            .expect("a matched copy in this project")
    };
    assert_eq!(copy("/navcal").fixed_version.as_deref(), Some("1.1.21"));
    assert_eq!(copy("/navcal").clean_version.as_deref(), Some("1.1.21"));
    assert_eq!(copy("/webhook").fixed_version.as_deref(), Some("5.0.12"));
    assert_eq!(copy("/webhook").clean_version.as_deref(), Some("5.0.12"));
    assert_eq!(
        matches[0].fixed_version.as_deref(),
        Some("5.0.12"),
        "the advisory-level field is the machine-wide maximum, by design"
    );
}
