// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Review of #981 (B1): the Brief's fact for a declared parent names the
//! release the route sentences name (express 4.22.3 clears the path-to-regexp
//! and qs copies it pins; vitest 4.1.11 clears @vitest/mocker), not its own
//! advisories' fix.

use super::*;

/// The fix paths of the Brief's fact for `package` `installed`, declared in
/// `project`, whose own advisories are fixed in `own_fix`.
fn fact_paths(
    db: &Database,
    project: &str,
    package: &str,
    installed: &str,
    own_fix: &str,
) -> String {
    let alert: PreemptionAlert = serde_json::from_value(serde_json::json!({
        "id": format!("osv:{package}"),
        "alert_type": "SecurityAdvisory",
        "title": format!("{package}@{installed}: 1 known vulnerability"),
        "explanation": "",
        "evidence": [],
        "affected_projects": [project],
        "affected_dependencies": [package],
        "urgency": "high",
        "confidence": 0.9,
        "predicted_window": null,
        "suggested_actions": [],
        "created_at": "2026-10-18T00:00:00Z",
        "osv_verified": true,
        "installed_version": installed,
        "fixed_version": own_fix,
        "is_direct": true
    }))
    .expect("alert shape");
    let (matched, not_compiled) =
        crate::osv::matching::get_matched_advisories_with_not_compiled(db).unwrap_or_default();
    let novelty = Novelty::default();
    let liveness = crate::evidence::ProjectLiveness::default();
    let ctx = FactCtx {
        db,
        matched: &matched,
        not_compiled: &not_compiled,
        conn: None,
        novelty: &novelty,
        liveness: &liveness,
    };
    let fact = security_fact(&alert, &ctx, &mut LabelCache::default()).expect("fact");
    format!(
        "{:?}",
        fact.sites.iter().map(|s| &s.fix_path).collect::<Vec<_>>()
    )
}

#[test]
fn the_briefs_express_fact_names_4_22_3() {
    let (db, project) = crate::osv::parent_release_fixture::nodegoat();
    let paths = fact_paths(&db, &project, "express", "4.16.4", "4.20.0");
    assert!(paths.contains("4.22.3"), "{paths}");
    assert!(!paths.contains("4.20.0"), "{paths}");
}

#[test]
fn the_briefs_vitest_fact_names_4_1_11() {
    let (db, project) = crate::osv::parent_release_fixture::vitest();
    let paths = fact_paths(&db, &project, "vitest", "4.0.0", "4.1.0");
    assert!(paths.contains("4.1.11"), "{paths}");
    assert!(!paths.contains("\"4.1.0\""), "{paths}");
}
