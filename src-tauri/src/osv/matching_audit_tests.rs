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
