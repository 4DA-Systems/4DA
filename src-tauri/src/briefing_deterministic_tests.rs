// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the deterministic brief floor (pure rendering — no feed/DB needed).

use super::{build_deterministic_brief, FloorReason};
use crate::brief_facts::{
    BriefFacts, FactStatus, FixPath, SecurityFact, SecuritySite, UpgradeFact, UpgradeSite,
    WorthKnowingCandidate,
};
use crate::preemption::AlertUrgency;

fn rmcp(status: FactStatus) -> SecurityFact {
    SecurityFact {
        key: "crates.io:rmcp:atlas/bridge/src-tauri".into(),
        package: "rmcp".into(),
        ecosystem: "crates.io".into(),
        urgency: AlertUrgency::High,
        worst_tier: Some("high".into()),
        advisory_count: 3,
        advisory_ids: vec![],
        title: "Unauthenticated session leak".into(),
        sites: vec![SecuritySite {
            label: "atlas/bridge/src-tauri".into(),
            installed: Some("1.7.0".into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Parent {
                parent: "victauri-plugin".into(),
                parent_version: "0.8.4".into(),
                to: "2.1.0".into(),
                by_requirement: false,
            },
        }],
        first_seen: Some("2026-09-16".into()),
        status,
    }
}

fn fastembed(status: FactStatus) -> UpgradeFact {
    UpgradeFact {
        key: "crates.io:fastembed".into(),
        package: "fastembed".into(),
        ecosystem: "crates.io".into(),
        announced: "7.1.0".into(),
        published: Some("2026-09-22".into()),
        yanked: false,
        dev_only: false,
        majors_behind: 2,
        pre_one: false,
        sites: vec![UpgradeSite {
            label: "4da/src-tauri".into(),
            installed: "5.17.4".into(),
        }],
        item_id: 1,
        url: Some("https://crates.io/crates/fastembed".into()),
        status,
    }
}

#[test]
fn new_facts_render_in_the_narrated_sections() {
    let facts = BriefFacts {
        security: vec![rmcp(FactStatus::New)],
        upgrades: vec![fastembed(FactStatus::New)],
        worth_knowing: vec![WorthKnowingCandidate {
            id: 9,
            title: "Deser: Rethinking Rust Serialization".into(),
            url: Some("https://lucumr.pocoo.org/2026/9/29/deser/".into()),
            source_type: "hackernews".into(),
            published: "2026-09-29".into(),
            excerpt: String::new(),
        }],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(out.contains("## Act now"), "{out}");
    assert!(
        out.contains("**rmcp** (crates.io, High — 3 advisories)"),
        "{out}"
    );
    assert!(out.contains("atlas/bridge/src-tauri on 1.7.0"), "{out}");
    assert!(out.contains("upgrade victauri-plugin"), "{out}");
    assert!(out.contains("## Upgrades to plan"), "{out}");
    assert!(out.contains("fastembed 7.1.0"), "{out}");
    assert!(out.contains("4da/src-tauri on 5.17.4"), "{out}");
    assert!(out.contains("2 major versions behind"), "{out}");
    assert!(out.contains(
        "[Deser: Rethinking Rust Serialization](https://lucumr.pocoo.org/2026/9/29/deser/)"
    ));
    assert!(!out.contains("Nothing new touches your code today"));
}

/// The 2026-10-01 nag: rmcp led ten consecutive briefs. Unchanged facts
/// fold into one Still-open line.
#[test]
fn unchanged_facts_fold_into_still_open() {
    let facts = BriefFacts {
        security: vec![rmcp(FactStatus::Unchanged {
            since: "2026-09-16".into(),
        })],
        upgrades: vec![fastembed(FactStatus::Unchanged {
            since: "2026-10-02".into(),
        })],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(!out.contains("## Act now"), "{out}");
    assert!(!out.contains("## Upgrades to plan"), "{out}");
    assert!(
        out.contains("Nothing new touches your code today."),
        "{out}"
    );
    assert!(out.contains("## Still open"), "{out}");
    assert!(
        out.contains("rmcp (atlas/bridge/src-tauri, since 2026-09-16)"),
        "{out}"
    );
    assert!(
        out.contains("fastembed (4da/src-tauri on 5.17.4, since 2026-10-02)"),
        "{out}"
    );
}

#[test]
fn an_empty_day_is_one_honest_line() {
    let out = build_deterministic_brief(&BriefFacts::default(), FloorReason::NoCapableModel);
    assert!(out.contains("Nothing new touches your code today."));
    assert!(!out.contains("## Still open"));
    assert!(
        !out.contains("C:\\") && !out.contains("D:\\"),
        "no absolute paths"
    );
}
