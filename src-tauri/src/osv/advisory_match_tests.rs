use super::*;
use crate::db::DependencyInstanceInput;
use crate::test_utils::test_db;

/// `Some(true)` a copy is confirmed affected; `Some(false)` every evaluated
/// copy is confirmed outside the range; `None` nothing decidable.
trait Verdict {
    fn verdict(&self) -> Option<bool>;
}
impl Verdict for AdvisoryExposure {
    fn verdict(&self) -> Option<bool> {
        if !self.confirmed.is_empty() {
            Some(true)
        } else if self.copies_checked > 0 && self.unconfirmed == 0 {
            Some(false)
        } else {
            None
        }
    }
}

const RMCP_RANGES: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#;
const HONO_RANGES: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.13.7"}]}]"#;

fn advisory(db: &Database, id: &str, pkg: &str, eco: &str, ranges: &str) -> StoredAdvisory {
    db.upsert_osv_advisory(
        id,
        "summary",
        None,
        pkg,
        eco,
        Some(ranges),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    db.get_osv_advisory_by_id(id).unwrap().unwrap()
}

/// Live 2026-10-02 (GHSA-9pj6-vhgr-3mwh): rmcp is DIRECT at 3.1.2 in
/// victauri (fixed) and TRANSITIVE at 1.7.0 in a sibling bridge app (exposed, only
/// in the lockfile inventory). The verdict must find the transitive copy.
#[test]
fn transitive_copy_outside_the_declaring_projects_is_exposed() {
    let db = test_db();
    db.store_dependency(
        "d:/runyourempire/victauri",
        "rmcp",
        Some("3.1.2"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_transitive_dependency(
        "d:/work/agent-bridge/src-tauri",
        "rmcp",
        Some("1.7.0"),
        "rust",
        false,
    )
    .unwrap();
    db.store_transitive_dependency("d:/4da/src-tauri", "rmcp", Some("3.4.1"), "rust", false)
        .unwrap();
    db.store_dependency_instances(
        "d:/work/agent-bridge/src-tauri",
        "crates.io",
        &[DependencyInstanceInput {
            package_name: "rmcp".into(),
            version: "1.7.0".into(),
            is_direct: false,
            is_dev: false,
            scope: "unknown".into(),
        }],
    )
    .unwrap();
    let adv = advisory(&db, "GHSA-9pj6-vhgr-3mwh", "rmcp", "crates.io", RMCP_RANGES);

    let exposure = exposure_for_advisory(&db, &adv);
    assert_eq!(exposure.verdict(), Some(true));
    assert_eq!(exposure.copies_checked, 3);
    assert_eq!(exposure.confirmed.len(), 1);
    let copy = &exposure.confirmed[0];
    assert_eq!(copy.project_path, "d:/work/agent-bridge/src-tauri");
    assert_eq!(copy.installed_version, "1.7.0");
    assert!(!copy.is_direct && !copy.is_dev);
}

/// Live 2026-10-02 (GHSA-hxh3-vqpv-xpqv): hono 4.13.9 in mcp-4da-server is
/// past the 4.13.7 fix — confirmed not affected.
#[test]
fn every_copy_past_the_fix_is_not_affected() {
    let db = test_db();
    db.store_dependency(
        "d:/4da/mcp-4da-server",
        "hono",
        Some("4.13.9"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    let adv = advisory(&db, "GHSA-hxh3-vqpv-xpqv", "hono", "npm", HONO_RANGES);

    let exposure = exposure_for_advisory(&db, &adv);
    assert_eq!(exposure.verdict(), Some(false));
    assert!(exposure.confirmed.is_empty());
    assert_eq!(exposure.copies_checked, 1);
}

#[test]
fn unknown_package_gives_no_verdict() {
    let db = test_db();
    let adv = advisory(&db, "GHSA-none", "hono", "npm", HONO_RANGES);
    assert_eq!(exposure_for_advisory(&db, &adv).verdict(), None);
}

#[test]
fn another_ecosystems_copy_is_not_evaluated() {
    let db = test_db();
    // An npm `rmcp` says nothing about the crates.io advisory.
    db.store_dependency("/p/web", "rmcp", Some("1.0.0"), "npm", false, None)
        .unwrap();
    let adv = advisory(&db, "GHSA-eco", "rmcp", "crates.io", RMCP_RANGES);
    let exposure = exposure_for_advisory(&db, &adv);
    assert_eq!(exposure.copies_checked, 0);
    assert_eq!(exposure.verdict(), None);
}

#[test]
fn a_versionless_copy_alone_is_undecided_not_safe() {
    let db = test_db();
    db.store_dependency("/p/a", "hono", None, "npm", false, None)
        .unwrap();
    let adv = advisory(&db, "GHSA-nover", "hono", "npm", HONO_RANGES);
    let exposure = exposure_for_advisory(&db, &adv);
    assert_eq!(exposure.unconfirmed, 1);
    assert_eq!(exposure.verdict(), None);
}

/// The verdict and Preemption's whole-mirror pass agree on the same data.
#[test]
fn agrees_with_get_matched_advisories() {
    let db = test_db();
    db.store_dependency("/p/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();
    db.store_dependency("/p/b", "lodash", Some("4.17.21"), "npm", true, None)
        .unwrap();
    let adv = advisory(
        &db,
        "GHSA-agree",
        "lodash",
        "npm",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#,
    );
    let exposure = exposure_for_advisory(&db, &adv);
    let matches = super::super::matching::get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    let exposed: Vec<&str> = exposure
        .confirmed
        .iter()
        .map(|c| c.project_path.as_str())
        .collect();
    assert_eq!(
        exposed,
        matches[0]
            .project_paths
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
}
