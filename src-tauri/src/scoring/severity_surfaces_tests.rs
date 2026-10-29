// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One grade per advisory on all five surfaces (review of #986, B1): the
//! Upgrade Plan, Preemption, the Brief, Knowledge Gaps and the scorer's
//! security lane (Signal) grade the same advisory group to the same tier.
//!
//! - The `nix` shape (CVE-2021-45707, live mirror 2026-10-18): GHSA-76w9 CVSS
//!   6.7 / label medium, GHSA-wgrg label high with no CVSS, RUSTSEC-2021-0119
//!   ungraded. One group: high. Before, the plan, alerts and Brief read medium
//!   (highest CVSS band only), Knowledge Gaps high, and the scorer's single
//!   first row medium.
//! - The GO-2024-3333 shape: an ungraded record whose GHSA alias is stored
//!   for ANOTHER package (OSV files the Go record under golang.org/x/net and
//!   the GHSA under golang.org/x/net/html), so the alias row never matches an
//!   install. The record inherits high from the alias on every surface; before,
//!   only the matcher's surfaces could read it.

use super::*;
use crate::db::Database;
use crate::osv::types::MatchedAdvisory;

#[allow(clippy::too_many_arguments)]
fn advisory(
    db: &Database,
    id: &str,
    package: &str,
    cvss: Option<f64>,
    label: Option<&str>,
    aliases: &str,
    summary: &str,
) {
    db.upsert_osv_advisory_with_meta(
        id,
        summary,
        None,
        package,
        "crates.io",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"9.0.0"}]}]"#),
        Some(r#"["9.0.0"]"#),
        cvss.map(|_| "CVSS_V3"),
        cvss,
        None,
        None,
        None,
        None,
        Some(aliases),
        label,
    )
    .expect("advisory");
}

fn seed(db: &Database) {
    // nix, as the live mirror holds it.
    advisory(
        db,
        "GHSA-76w9-p8mg-j927",
        "nix",
        Some(6.7),
        Some("medium"),
        r#"["CVE-2021-45707","GHSA-wgrg-5h56-jg27","RUSTSEC-2021-0119"]"#,
        "Out-of-bounds Write in nix",
    );
    advisory(
        db,
        "GHSA-wgrg-5h56-jg27",
        "nix",
        None,
        Some("high"),
        r#"["CVE-2021-45707","GHSA-76w9-p8mg-j927","RUSTSEC-2021-0119"]"#,
        "Out-of-bounds write in nix::unistd::getgrouplist",
    );
    advisory(
        db,
        "RUSTSEC-2021-0119",
        "nix",
        None,
        None,
        r#"["CVE-2021-45707","GHSA-76w9-p8mg-j927","GHSA-wgrg-5h56-jg27"]"#,
        "Out-of-bounds write in nix::unistd::getgrouplist",
    );
    // The GO-2024-3333 shape: the graded alias names another package.
    advisory(
        db,
        "GO-2024-3333",
        "xnet",
        None,
        None,
        r#"["CVE-2024-45338","GHSA-w32m-9786-jp63"]"#,
        "Non-linear parsing of case-insensitive content",
    );
    advisory(
        db,
        "GHSA-w32m-9786-jp63",
        "xnet-html",
        None,
        Some("high"),
        r#"["CVE-2024-45338","GO-2024-3333"]"#,
        "Non-linear parsing of case-insensitive content",
    );
    for package in ["nix", "xnet"] {
        db.store_dependency("/proj/app", package, Some("1.0.0"), "rust", false, None)
            .expect("dependency");
    }
}

/// The five tiers for one package: plan, Preemption, Brief, Knowledge Gaps,
/// and the scorer (the matcher verdict and the mirror lookup, which must agree).
fn five_tiers(
    db: &Database,
    package: &str,
    item_id: &str,
) -> Vec<(&'static str, Option<&'static str>)> {
    let tier_of = |u: crate::evidence::Urgency| match u {
        crate::evidence::Urgency::Critical => "critical",
        crate::evidence::Urgency::High => "high",
        crate::evidence::Urgency::Medium => "medium",
        crate::evidence::Urgency::Watch => "low",
    };
    let plan = crate::evidence::build_upgrade_plan(db)
        .items
        .iter()
        .find(|i| i.affected_deps.iter().any(|d| d == package))
        .map(|i| tier_of(i.urgency));
    let matches = crate::osv::matching::get_matched_advisories(db).expect("matches");
    let group: Vec<&MatchedAdvisory> = matches
        .iter()
        .filter(|m| m.package_name == package && m.is_version_confirmed)
        .collect();
    let projects = vec!["/proj/app".to_string()];
    let alert = tier_of(crate::preemption::alert_urgency_to_canonical(
        &crate::preemption::osv_alert_urgency(&group, &projects),
    ));
    let brief = crate::osv::severity::group_severity(&group).map(|g| g.tier);
    let grades = crate::osv::severity::AliasGrades::load(&db.conn.lock());
    let gaps = crate::knowledge_decay::advisory_tier_for(
        &db.conn.lock(),
        package,
        &[crate::osv::exposure::Install::new(Some("rust"), "1.0.0")],
        &grades,
    );
    let verdict = crate::scoring::security_verdict::matcher_verdict(db, &[item_id.to_string()])
        .and_then(|v| v.tier);
    let edge = DepMatch {
        package_name: package.to_string(),
        confidence: 0.9,
        version_delta: dependencies::VersionDelta::Unknown,
        is_dev: false,
        is_direct: true,
        version: Some("1.0.0".to_string()),
        ecosystem: "rust".to_string(),
        corroborated: true,
        raw_name: Some(package.to_string()),
        project_paths: projects.clone(),
    };
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let tags: Vec<String> = Vec::new();
    let title = format!("[{item_id}] {package}: advisory");
    let input = ScoringInput {
        id: 1,
        title: &title,
        url: None,
        content: "",
        source_type: "osv",
        embedding: &zero,
        created_at: None,
        detected_lang: "",
        source_tags: &tags,
        tags_json: None,
        feed_origin: None,
        source_id: None,
    };
    let mirror = mirror_severity_tier(db, &input, &edge, Some(item_id));
    vec![
        ("plan", plan),
        ("preemption", Some(alert)),
        ("brief", brief),
        ("knowledge_gaps", gaps),
        ("signal_verdict", verdict),
        ("signal_mirror", mirror),
    ]
}

#[test]
fn all_five_surfaces_grade_the_nix_and_go_shapes_alike() {
    let db = crate::test_utils::test_db();
    seed(&db);
    for (package, item_id) in [("nix", "GHSA-76w9-p8mg-j927"), ("xnet", "GO-2024-3333")] {
        let tiers = five_tiers(&db, package, item_id);
        for (surface, tier) in &tiers {
            assert_eq!(*tier, Some("high"), "{package} on {surface}: {tiers:?}");
        }
    }
    // The inherited grade names its alias where the matcher carries it.
    let matches = crate::osv::matching::get_matched_advisories(&db).expect("matches");
    let go = matches
        .iter()
        .find(|m| m.advisory_id == "GO-2024-3333")
        .expect("GO-2024-3333 matched");
    assert_eq!(go.severity_from.as_deref(), Some("GHSA-w32m-9786-jp63"));
    let rustsec = matches
        .iter()
        .find(|m| m.advisory_id == "RUSTSEC-2021-0119")
        .expect("RUSTSEC-2021-0119 matched");
    let group = [rustsec];
    // Alone, the RUSTSEC record inherits; in its group, the GHSA's own high wins.
    assert_eq!(
        crate::osv::severity::cluster_severity(&group).map(|g| g.label()),
        Some("high (from GHSA-wgrg-5h56-jg27)".to_string())
    );
}

/// A mirror write drops the kept alias grades, so the scorer never reads a
/// grade the mirror no longer holds.
#[test]
fn a_mirror_write_drops_the_kept_alias_grades() {
    let db = crate::test_utils::test_db();
    seed(&db);
    assert_eq!(db.alias_grades().get("GHSA-w32m-9786-jp63"), Some("high"));
    advisory(
        &db,
        "GHSA-w32m-9786-jp63",
        "xnet-html",
        None,
        Some("low"),
        r#"["GO-2024-3333"]"#,
        "x",
    );
    assert_eq!(db.alias_grades().get("GHSA-w32m-9786-jp63"), Some("low"));
}

/// #986: maintenance notices are never counted or worded as vulnerabilities.
/// The four unmaintained notices installed on the operator machine
/// (live mirror copy, 2026-10-18): no fix, no CVSS, maintenance wording.
#[test]
fn maintenance_notices_are_never_worded_as_vulnerabilities() {
    let db = crate::test_utils::test_db();
    let notices = [
        (
            "RUSTSEC-2026-0370",
            "zvariant",
            "`zvariant` is unmaintained",
        ),
        (
            "RUSTSEC-2026-0371",
            "zbus_names",
            "`zbus_names` is unmaintained",
        ),
        (
            "RUSTSEC-2026-0372",
            "zvariant_derive",
            "`zvariant_derive` is unmaintained",
        ),
        (
            "RUSTSEC-2024-0370",
            "proc-macro-error",
            "proc-macro-error is unmaintained",
        ),
    ];
    for (id, package, summary) in notices {
        db.upsert_osv_advisory_with_meta(
            id,
            summary,
            None,
            package,
            "crates.io",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}]"#),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("notice");
        db.store_dependency("/proj/app", package, Some("1.0.0"), "rust", false, None)
            .expect("dependency");
    }
    let matches = crate::osv::matching::get_matched_advisories(&db).expect("matches");
    assert_eq!(matches.len(), 4);
    let projects = vec!["/proj/app".to_string()];
    for (id, package, _) in notices {
        let group: Vec<&MatchedAdvisory> = matches
            .iter()
            .filter(|m| m.package_name == package)
            .collect();
        // Preemption: a notice group, Watch, worded as unmaintained.
        assert!(crate::osv::severity::is_notice_group(&group), "{id}");
        assert_eq!(
            crate::preemption::alert_urgency_to_canonical(&crate::preemption::osv_alert_urgency(
                &group, &projects
            )),
            crate::evidence::Urgency::Watch
        );
        assert_eq!(
            crate::osv::severity::notice_wording(package, &[id]),
            format!("unmaintained: {package} ({id}), no exploitable vulnerability")
        );
        // Knowledge Gaps: unmaintained, not vulnerable, and not graded.
        let installs = [crate::osv::exposure::Install::new(Some("rust"), "1.0.0")];
        assert!(crate::knowledge_decay::only_maintenance_notices(
            &db.conn.lock(),
            package,
            &installs
        ));
        let grades = crate::osv::severity::AliasGrades::default();
        assert_eq!(
            crate::knowledge_decay::advisory_tier_for(&db.conn.lock(), package, &installs, &grades),
            None
        );
        // Signal: the matcher's verdict says it is a notice.
        let verdict = crate::scoring::security_verdict::matcher_verdict(&db, &[id.to_string()])
            .expect("verdict");
        assert!(verdict.maintenance_only, "{id}");
        assert_eq!(verdict.tier, None);
    }
    // The plan: one Watch fold naming each notice as what it is.
    let plan = crate::evidence::build_upgrade_plan(&db);
    assert_eq!(
        plan.items.len(),
        1,
        "{:?}",
        plan.items.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
    let fold = &plan.items[0];
    assert_eq!(fold.urgency, crate::evidence::Urgency::Watch);
    for (id, package, _) in notices {
        assert!(
            fold.explanation.contains(&format!(
                "unmaintained: {package} ({id}), no exploitable vulnerability"
            )),
            "{}",
            fold.explanation
        );
    }
    let worded = fold.explanation.replace("no exploitable vulnerability", "");
    assert!(
        !worded.to_lowercase().contains("vulnerab"),
        "{}",
        fold.explanation
    );
    assert!(
        !worded.to_lowercase().contains("affects"),
        "{}",
        fold.explanation
    );
}
