// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the delivered-morning audit: shown versions are found in each
//! part of the window, checked against the replay's truth, and a version the
//! database has no record of is a fault.

use super::*;
use crate::brief_facts::{FactStatus, SecurityFact, SecuritySite, UpgradeFact, UpgradeSite};
use crate::briefing_deterministic::FloorReason;
use crate::preemption::AlertUrgency;

/// The three truth tables, with only the columns the check reads.
fn truth() -> Connection {
    let conn = Connection::open_in_memory().expect("memory db");
    conn.execute_batch(
        "CREATE TABLE dependency_instances (package_name TEXT, version TEXT);
         CREATE TABLE osv_advisories (package_name TEXT, fixed_versions TEXT);
         CREATE TABLE source_items (title TEXT);
         INSERT INTO dependency_instances VALUES ('hono', '4.13.3'), ('ai', '5.0.86');
         INSERT INTO osv_advisories VALUES ('hono', '[\"4.13.5\"]');
         INSERT INTO source_items VALUES ('ai v7.0.130');",
    )
    .expect("fixture");
    conn
}

fn security(pkg: &str, installed: &str, to: &str) -> SecurityFact {
    SecurityFact {
        key: format!("npm:{pkg}:navcal"),
        package: pkg.into(),
        ecosystem: "npm".into(),
        urgency: AlertUrgency::High,
        worst_tier: Some("high".into()),
        worst_tier_from: None,
        advisory_count: 1,
        advisory_ids: vec!["GHSA-aaaa-bbbb-cccc".into()],
        title: format!("{pkg} advisory"),
        sites: vec![SecuritySite {
            label: "navcal".into(),
            installed: Some(installed.into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Bump {
                to: to.into(),
                fix: None,
            },
        }],
        not_compiled: vec![],
        first_seen: None,
        status: FactStatus::New,
    }
}

fn upgrade(pkg: &str, installed: &str, announced: &str) -> UpgradeFact {
    UpgradeFact {
        key: format!("npm:{pkg}"),
        package: pkg.into(),
        ecosystem: "npm".into(),
        announced: announced.into(),
        published: None,
        yanked: false,
        dev_only: false,
        majors_behind: 2,
        pre_one: false,
        sites: vec![UpgradeSite {
            label: "navcal".into(),
            installed: installed.into(),
        }],
        item_id: 1,
        url: None,
        status: FactStatus::New,
    }
}

fn briefing(json: serde_json::Value) -> BriefingNotification {
    let mut base = serde_json::json!({
        "title": "4DA Intelligence Briefing",
        "items": [],
        "total_relevant": 0,
    });
    if let (Some(b), Some(extra)) = (base.as_object_mut(), json.as_object()) {
        for (k, v) in extra {
            b.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(base).expect("briefing literal")
}

fn facts_morning(facts: &BriefFacts) -> BriefingNotification {
    let md = crate::briefing_deterministic::build_deterministic_brief(
        facts,
        FloorReason::Morning(crate::briefing_deterministic::MorningWhy::Abstained),
    );
    briefing(serde_json::json!({ "facts_brief": md }))
}

#[test]
fn the_replay_truth_accepts_installed_fix_and_registry_versions() {
    let conn = truth();
    assert!(version_is_true(&conn, "hono", "4.13.3"), "installed");
    assert!(
        version_is_true(&conn, "Hono", "4.13.5"),
        "OSV fix, any case"
    );
    assert!(version_is_true(&conn, "ai", "7.0.130"), "registry title");
    assert!(
        !version_is_true(&conn, "hono", "4.7.5"),
        "brief 170's invented target"
    );
}

#[test]
fn a_clean_facts_view_records_its_versions_and_no_fault() {
    let conn = truth();
    let facts = BriefFacts {
        security: vec![security("hono", "4.13.3", "4.13.5")],
        upgrades: vec![upgrade("ai", "5.0.86", "7.0.130")],
        ..BriefFacts::default()
    };
    let b = facts_morning(&facts);
    let a = audit_morning(&conn, &b, Some(&facts), 6);
    assert!(a.version_faults.is_empty(), "{a:?}");
    for v in ["hono 4.13.3", "hono 4.13.5", "ai 5.0.86", "ai 7.0.130"] {
        assert!(a.versions_shown.contains(&v.to_string()), "{v}: {a:?}");
    }
    assert_eq!((a.items_shown, a.cap), (1, 6));
}

#[test]
fn a_facts_view_with_an_unrecorded_version_is_a_fault() {
    let conn = truth();
    let facts = BriefFacts {
        security: vec![security("hono", "4.13.3", "4.7.5")],
        ..BriefFacts::default()
    };
    let b = facts_morning(&facts);
    let a = audit_morning(&conn, &b, Some(&facts), 6);
    assert_eq!(a.version_faults, vec!["fix: hono 4.7.5".to_string()]);
}

#[test]
fn a_fact_version_the_text_does_not_show_is_not_audited() {
    let conn = truth();
    let facts = BriefFacts {
        security: vec![security("hono", "4.13.3", "4.7.5")],
        ..BriefFacts::default()
    };
    // The window showed only a summary: the facts were never rendered.
    let b = briefing(serde_json::json!({ "facts_brief": "## Act now\n- hono on 4.13.3\n" }));
    let a = audit_morning(&conn, &b, Some(&facts), 6);
    assert!(a.version_faults.is_empty(), "{a:?}");
    assert_eq!(a.versions_shown, vec!["hono 4.13.3".to_string()]);
}

#[test]
fn a_summary_naming_an_invented_target_is_a_fault() {
    let conn = truth();
    // Brief 170, 2026-09-09: hono came in through an item's matched deps.
    let b = briefing(serde_json::json!({
        "synthesis": "Bump hono to >= 4.7.5 in d:/4da/mcp-4da-server today.",
        "items": [{"title": "CVE in hono", "source_type": "cve", "score": 0.9,
                   "signal_type": null, "matched_deps": ["hono"]}],
    }));
    let a = audit_morning(&conn, &b, None, 6);
    assert_eq!(a.version_faults, vec!["summary: hono 4.7.5".to_string()]);
    assert_eq!(a.items_shown, 0);
}

#[test]
fn a_summary_quoting_its_input_or_the_truth_is_clean() {
    let conn = truth();
    let b = briefing(serde_json::json!({
        "synthesis": "hono 4.13.5 fixes the cookie bug. Bun 1.3.2 ships a new bundler.",
        "items": [{"title": "Bun 1.3.2 released", "source_type": "hackernews", "score": 0.7,
                   "signal_type": null, "matched_deps": ["hono", "bun"]}],
    }));
    let a = audit_morning(&conn, &b, None, 6);
    assert!(a.version_faults.is_empty(), "{a:?}");
    assert_eq!(
        a.versions_shown,
        vec!["hono 4.13.5".to_string(), "bun 1.3.2".to_string()]
    );
}

#[test]
fn an_abstained_summary_shows_no_versions() {
    let conn = truth();
    let b = briefing(serde_json::json!({
        "synthesis": "Low signal -- no noteworthy intelligence overnight. hono 9.9.9",
    }));
    assert_eq!(
        audit_morning(&conn, &b, None, 6),
        MorningAudit {
            cap: 6,
            ..Default::default()
        }
    );
}

#[test]
fn fresh_alert_cards_are_audited_and_the_unchanged_strip_is_not() {
    let conn = truth();
    let card = |pkg: &str, fixed: &str, persistent: bool| {
        serde_json::json!({
            "title": format!("{pkg} advisory"),
            "urgency": "high",
            "explanation": format!("{pkg} 4.13.3 is affected; fixed in {fixed}."),
            "package_name": pkg,
            "installed_version": "4.13.3",
            "fixed_version": format!(">= {fixed}"),
            "persistent_unchanged": persistent,
        })
    };
    let b = briefing(serde_json::json!({
        "preemption_alerts": [card("hono", "4.13.5", false), card("hono", "4.0.1", true)],
    }));
    let a = audit_morning(&conn, &b, None, 6);
    assert!(a.version_faults.is_empty(), "{a:?}");
    assert_eq!(
        a.versions_shown,
        vec!["hono 4.13.3".to_string(), "hono 4.13.5".to_string()]
    );

    let b = briefing(serde_json::json!({ "preemption_alerts": [card("hono", "4.0.1", false)] }));
    let a = audit_morning(&conn, &b, None, 6);
    assert_eq!(a.version_faults, vec!["card fix: hono 4.0.1".to_string()]);
}

#[test]
fn versions_match_whole_tokens_only() {
    assert!(shows_version("bump to 4.13.5.", "4.13.5"));
    assert!(shows_version("(4.13.5)", "4.13.5"));
    assert!(!shows_version("14.13.5", "4.13.5"));
    assert!(!shows_version("4.13.50", "4.13.5"));
    assert!(!shows_version("4.13.5.1", "4.13.5"));
    assert_eq!(version_tokens(">= 4.13.5, v5.0.1"), vec!["4.13.5", "5.0.1"]);
}

#[test]
fn prose_claims_stop_at_the_sentence_and_the_next_package() {
    let pk = vec!["hono".to_string(), "@clerk/shared".to_string()];
    let claims = prose_version_claims(
        "Upgrade hono to 4.13.5. React 19.2 shipped. shared 2.1.0 and hono 4.13.3 too.",
        &pk,
    );
    assert_eq!(
        claims,
        vec![
            ("hono".to_string(), "4.13.5".to_string()),
            ("@clerk/shared".to_string(), "2.1.0".to_string()),
            ("hono".to_string(), "4.13.3".to_string()),
        ]
    );
}
