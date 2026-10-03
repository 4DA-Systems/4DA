// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `briefing_prompt` (extracted via #[path]).

use super::*;
use crate::brief_facts::{FixPath, SecuritySite, UpgradeSite};

fn cand(id: i64, title: &str) -> WorthKnowingCandidate {
    WorthKnowingCandidate {
        id,
        title: title.to_string(),
        url: None,
        source_type: "hackernews".to_string(),
        published: "2026-10-01".to_string(),
        excerpt: format!("body of {id}"),
    }
}

/// The structured verdict channel (AD-035) lives or dies by this contract:
/// the prompt must demand the fenced `rejects` trailer that
/// `brief_rejections::extract_rejects_trailer` parses, keyed by the
/// `<source_item>` index attribute.
#[test]
fn prompt_carries_the_rejects_trailer_contract() {
    let prompt = briefing_system_prompt();
    assert!(prompt.contains("MACHINE TRAILER"));
    assert!(prompt.contains("```rejects"));
    assert!(
        prompt.contains(r#"[{"idx": 3, "reason": "self-promotional"}"#),
        "the example row teaches the exact shape the parser accepts"
    );
    assert!(prompt.contains("`index` attribute"));
    assert!(prompt.contains("Output `[]` inside the block if you filtered nothing"));
}

#[test]
fn prompt_scopes_verdict_indices_to_the_numbered_slate() {
    let prompt = briefing_system_prompt();
    assert!(prompt.contains("ONLY the numbered items under \"Today's N items\""));
    assert!(prompt.contains("never emit an idx for one"));
}

/// Decision 2: the sections are the brief's contract with the tab parser,
/// and the facts-first rules are what stop the 2026-10-01 defects.
#[test]
fn prompt_is_facts_first_with_the_new_sections() {
    let prompt = briefing_system_prompt();
    for heading in [
        "## Act now",
        "## Upgrades to plan",
        "## Worth knowing",
        "## Still open",
    ] {
        assert!(prompt.contains(heading), "missing {heading}");
    }
    assert!(prompt.contains("give the fix exactly as the fact's fix clause says"));
    assert!(prompt.contains("A fact marked UNCHANGED does not go here"));
    assert!(prompt.contains("you have not read its changelog"));
    assert!(
        prompt.contains("never bumping it in a manifest")
            || prompt.contains("never fixed by bumping it in a manifest")
    );
    assert!(prompt.contains("Never compute, round or guess a version or a fix"));
    assert!(
        prompt.starts_with(UNTRUSTED_CONTENT_DEFENSE_CLAUSE),
        "untrusted-content defense clause must lead the prompt"
    );
}

fn rendered_pairs(text: &str) -> Vec<(usize, i64)> {
    text.split("<source_item ")
        .skip(1)
        .filter_map(|block| {
            let idx = block
                .strip_prefix("index=\"")?
                .split_once('"')?
                .0
                .parse::<usize>()
                .ok()?;
            let id = block
                .split_once(" id=\"")?
                .1
                .split_once('"')?
                .0
                .parse::<i64>()
                .ok()?;
            Some((idx, id))
        })
        .collect()
}

/// THE regression guard for the #560/#580 defect class: the ids a verdict
/// maps through are the ids the prompt rendered, filter included.
#[test]
fn verdict_indices_address_the_candidate_the_prompt_actually_showed() {
    let items = vec![
        cand(11, "first real item"),
        cand(22, "   "),
        cand(33, "second real item"),
        cand(44, ""),
        cand(55, "third real item"),
    ];
    let slate = build_candidate_slate(&items, true);
    let pairs = rendered_pairs(&slate.text);
    assert_eq!(pairs, vec![(1, 11), (2, 33), (3, 55)]);
    assert_eq!(
        slate.ids,
        pairs.iter().map(|(_, id)| *id).collect::<Vec<_>>()
    );

    let rejects = [crate::brief_rejections::TrailerReject {
        idx: 2,
        reason: "self-promotional".to_string(),
    }];
    let mapped = crate::brief_rejections::map_rejects_to_item_ids(&rejects, &slate.ids);
    assert_eq!(mapped, vec![(33, "self-promotional".to_string())]);
}

#[test]
fn slate_is_capped_and_ids_stop_with_the_prompt() {
    let items: Vec<_> = (1..=40).map(|n| cand(n, "titled")).collect();
    let slate = build_candidate_slate(&items, true);
    assert_eq!(slate.ids.len(), PROMPT_SLATE_TAKE);
    assert_eq!(rendered_pairs(&slate.text).len(), PROMPT_SLATE_TAKE);
    let rejects = [crate::brief_rejections::TrailerReject {
        idx: PROMPT_SLATE_TAKE + 1,
        reason: "off-stack".to_string(),
    }];
    assert!(crate::brief_rejections::map_rejects_to_item_ids(&rejects, &slate.ids).is_empty());
}

/// The excerpt is the point of the redesign (the brief used to be written
/// from titles), and `titles_only` must still withhold it.
#[test]
fn excerpts_reach_the_prompt_only_when_bodies_may_leave() {
    let items = vec![cand(7, "Deser: Rethinking Rust Serialization")];
    let with = build_candidate_slate(&items, true);
    assert!(
        with.text.contains("<excerpt>body of 7</excerpt>"),
        "{}",
        with.text
    );
    assert!(with.text.contains(r#"published="2026-10-01""#));
    let without = build_candidate_slate(&items, false);
    assert!(!without.text.contains("<excerpt>"), "{}", without.text);
    assert!(!without.text.contains("body of 7"));
}

fn rmcp_fact(status: FactStatus) -> SecurityFact {
    SecurityFact {
        key: "crates.io:rmcp:atlas/bridge/src-tauri".into(),
        package: "rmcp".into(),
        ecosystem: "crates.io".into(),
        urgency: AlertUrgency::High,
        worst_tier: Some("high".into()),
        advisory_count: 3,
        advisory_ids: vec!["GHSA-9pj6-vhgr-3mwh".into()],
        title: "rmcp: session leak".into(),
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
                proven: None,
            },
        }],
        not_compiled: vec![],
        first_seen: Some("2026-09-16".into()),
        status,
    }
}

/// AD-051: advisories the project's build does not compile are listed as
/// "Not counted" with their ids, and the rules forbid presenting them.
#[test]
fn not_compiled_advisories_are_shown_as_not_counted() {
    let mut fact = rmcp_fact(FactStatus::New);
    fact.not_compiled = vec![crate::brief_facts::NotCompiledNote {
        advisory_id: "GHSA-33f5-2c5q-wgwj".into(),
        summary:
            "RMCP: Missing Resource Field Validation in OAuth Protected Resource Metadata Discovery"
                .into(),
    }];
    let text = render_facts_for_prompt(&BriefFacts {
        security: vec![fact],
        ..BriefFacts::default()
    });
    assert!(
        text.contains("Not counted — the code these advisories name is feature-gated out")
            && text.contains("GHSA-33f5-2c5q-wgwj (RMCP: Missing Resource Field Validation"),
        "{text}"
    );
    let system = briefing_system_prompt();
    assert!(system.contains("listed as \"Not counted\" is not a finding"));
}

#[test]
fn facts_block_states_the_fix_path_and_the_status() {
    let facts = BriefFacts {
        security: vec![rmcp_fact(FactStatus::Unchanged {
            since: "2026-09-16".into(),
        })],
        upgrades: vec![UpgradeFact {
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
            item_id: 113_713,
            url: None,
            status: FactStatus::New,
        }],
        ..BriefFacts::default()
    };
    let text = render_facts_for_prompt(&facts);
    assert!(
        text.contains("[HIGH] rmcp (crates.io) — 3 advisories"),
        "{text}"
    );
    assert!(
        text.contains("atlas/bridge/src-tauri: rmcp 1.7.0"),
        "{text}"
    );
    assert!(text.contains("upgrade victauri-plugin"), "{text}");
    assert!(text.contains("UNCHANGED since 2026-09-16"), "{text}");
    assert!(text.contains("First seen by 4DA: 2026-09-16"), "{text}");
    assert!(
        text.contains("fastembed 7.1.0 (crates.io, released 2026-09-22) — 4da/src-tauri on 5.17.4 (2 major versions behind). Status: NEW."),
        "{text}"
    );
}

#[test]
fn an_empty_lane_says_none_instead_of_inviting_invention() {
    let text = render_facts_for_prompt(&BriefFacts::default());
    assert!(text.contains("none — no confirmed advisory at HIGH or CRITICAL"));
    assert!(text.contains("UPGRADES") && text.contains("none."));
}
