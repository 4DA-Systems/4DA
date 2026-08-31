// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for the Preemption LIST transport (AD-036): the single visibility
//! filter (counts == rendered cards), the per-item transport trim, and the
//! payload measurement against the 2026-08-31 live-audit shape (149 items,
//! 304,868 bytes in one IPC response).

use super::*;
use crate::evidence::types::{
    Action, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints, TierScope,
};

// ────────────────────────────────────────────────────────────────────────────
// Fixtures — deliberately mirror the REAL producers' shapes and string sizes
// (`upgrade_plan::into_evidence_item`, `osv_matches_to_alerts` +
// `to_evidence_item`, `llm_judged_to_alerts`), so the measurement test below
// is evidence about the live payload, not about toy data.
// ────────────────────────────────────────────────────────────────────────────

fn base_item(id: String, urgency: Urgency, confidence: Confidence) -> EvidenceItem {
    EvidenceItem {
        id,
        kind: EvidenceKind::Alert,
        title: String::new(),
        explanation: String::new(),
        confidence,
        urgency,
        reversibility: None,
        evidence: vec![],
        evidence_total: None,
        affected_projects: vec![],
        affected_deps: vec![],
        suggested_actions: vec![],
        precedents: vec![],
        refutation_condition: None,
        lens_hints: LensHints::preemption_only(),
        created_at: 1_756_600_000_000,
        expires_at: None,
    }
}

fn advisory_citation(pkg: &str, seq: usize) -> EvidenceCitation {
    EvidenceCitation {
        source: "osv-advisory".to_string(),
        title: format!(
            "{pkg} prototype pollution via deep merge (GHSA-{seq:04}-w7xf-qm{:02}p)",
            seq % 90
        ),
        url: Some(format!(
            "https://osv.dev/vulnerability/GHSA-{seq:04}-w7xf-qm{:02}p",
            seq % 90
        )),
        freshness_days: 41.708_332,
        relevance_note: "CVSS 8.8; affects installed 4.17.20".to_string(),
    }
}

/// A ranked Upgrade Plan step with `advisories` citations (the audit's
/// "Show 7 more" cards were exactly this shape: 8 advisory rows + the
/// project-scan row, 2 rendered).
fn plan_step(i: usize, advisories: usize, urgency: Urgency) -> EvidenceItem {
    let pkg = format!("package-{i:03}");
    let mut evidence: Vec<EvidenceCitation> = (0..advisories)
        .map(|j| advisory_citation(&pkg, i * 10 + j))
        .collect();
    evidence.push(EvidenceCitation {
        source: "project-scan".to_string(),
        title: "Installed in 4 projects".to_string(),
        url: None,
        freshness_days: 0.0,
        relevance_note: "Affected projects: D:\\dev\\alpha-service, D:\\dev\\beta-console, D:\\dev\\gamma-worker, D:\\dev\\delta-api".to_string(),
    });
    EvidenceItem {
        title: format!(
            "Upgrade {pkg} to >= 4.17.21 \u{2014} clears {advisories} advisories across 4 projects"
        ),
        explanation: format!(
            "{pkg} has {advisories} version-confirmed advisories matching installed versions \
             across 4 projects. Fixable now via a direct dependency bump. Advisories: \
             GHSA-{i:04}-w7xf-qm01p, GHSA-{i:04}-w7xf-qm02p, GHSA-{i:04}-w7xf-qm03p (+more)."
        ),
        evidence,
        affected_projects: vec![
            "D:\\dev\\alpha-service".to_string(),
            "D:\\dev\\beta-console".to_string(),
            "D:\\dev\\gamma-worker".to_string(),
            "D:\\dev\\delta-api".to_string(),
        ],
        affected_deps: vec![pkg.clone()],
        suggested_actions: vec![
            Action {
                action_id: "review_security".to_string(),
                label: "Review advisories".to_string(),
                description: format!("Review the {advisories} advisories affecting {pkg}"),
            },
            Action {
                action_id: "view_source".to_string(),
                label: "Open advisory".to_string(),
                description: "Open the advisory source".to_string(),
            },
            Action {
                action_id: "snooze_7d".to_string(),
                label: "Snooze 7 days".to_string(),
                description: "Hide this upgrade step for a week".to_string(),
            },
            Action {
                action_id: "dismiss".to_string(),
                label: "Dismiss".to_string(),
                description: "Dismiss this upgrade step".to_string(),
            },
        ],
        lens_hints: LensHints::upgrade_plan(),
        ..base_item(
            format!("upgrade-plan:npm:{pkg}"),
            urgency,
            Confidence::heuristic(0.9),
        )
    }
}

/// The per-package OSV-verified alert the plan step regroups (same package).
fn osv_alert(i: usize, urgency: Urgency, deps: Vec<String>) -> EvidenceItem {
    let pkg = deps
        .first()
        .cloned()
        .unwrap_or_else(|| format!("package-{i:03}"));
    let mut evidence: Vec<EvidenceCitation> = (0..3)
        .map(|j| {
            let mut c = advisory_citation(&pkg, i * 10 + j);
            c.source = "osv".to_string();
            c.relevance_note = "relevance 1.00".to_string();
            c
        })
        .collect();
    evidence.push(EvidenceCitation {
        source: "version_context".to_string(),
        title: "Installed: 4.17.20 \u{2192} update to >= 4.17.21 (direct)".to_string(),
        url: None,
        freshness_days: 0.0,
        relevance_note: "Dependency version metadata from project scan".to_string(),
    });
    EvidenceItem {
        title: format!("{pkg}@4.17.20: 3 known vulnerabilities"),
        explanation: format!(
            "GHSA-{i:04}-w7xf-qm01p, GHSA-{i:04}-w7xf-qm02p and 1 more (3 vulnerabilities) \
             affect {pkg}@4.17.20 in dev/alpha-service, dev/beta-console. \
             Scope: direct dependency. Update to >= 4.17.21."
        ),
        evidence,
        affected_projects: vec![
            "D:\\dev\\alpha-service".to_string(),
            "D:\\dev\\beta-console".to_string(),
        ],
        affected_deps: deps,
        suggested_actions: vec![
            Action {
                action_id: "investigate".to_string(),
                label: format!("Review 3 advisories for {pkg}"),
                description: format!(
                    "Review 3 advisories for this direct dependency and update {pkg} if affected."
                ),
            },
            Action {
                action_id: "dismiss".to_string(),
                label: "Not affected".to_string(),
                description:
                    "Dismiss if you've confirmed your version is outside the affected range."
                        .to_string(),
            },
        ],
        ..base_item(
            format!("osv-pkg-{pkg}-npm"),
            urgency,
            Confidence::osv_verified(0.95),
        )
    }
}

/// A Tier-2 LLM-assessed alert (long free-text explanation).
fn llm_alert(i: usize, urgency: Urgency) -> EvidenceItem {
    EvidenceItem {
        title: format!("Breaking change ahead for the ecosystem tooling wave {i}"),
        explanation: format!(
            "The maintainers announced a staged rollout that deprecates the current plugin \
             resolution order and replaces it with a manifest-first scheme. Projects that rely \
             on implicit resolution will silently pick up the wrong plugin version once the \
             registry flips the default. Wave {i} lands behind a feature flag first, so there \
             is a window to pin the resolver before the default changes."
        ),
        evidence: vec![EvidenceCitation {
            source: "hackernews".to_string(),
            title: format!("Ecosystem tooling wave {i}: staged deprecation announced"),
            url: Some(format!("https://news.ycombinator.com/item?id=414{i:04}")),
            freshness_days: 3.541_666,
            relevance_note: "relevance 0.72".to_string(),
        }],
        affected_projects: vec!["D:\\dev\\alpha-service".to_string()],
        affected_deps: vec![],
        suggested_actions: vec![
            Action {
                action_id: "investigate".to_string(),
                label: "Investigate".to_string(),
                description: "Review the source and assess impact on your projects.".to_string(),
            },
            Action {
                action_id: "dismiss".to_string(),
                label: "Dismiss".to_string(),
                description: "Dismiss if this doesn't affect your projects.".to_string(),
            },
        ],
        ..base_item(
            format!("llm-{}", 41_000 + i),
            urgency,
            Confidence::llm_assessed(0.72),
        )
    }
}

fn feed_bytes(feed: &EvidenceFeed) -> usize {
    serde_json::to_string(feed).expect("feed serializes").len()
}

/// The 2026-08-31 audit-shaped corpus: 149 items whose untrimmed IPC payload
/// reproduces the measured 304,868 characters (calibration asserted below).
/// Composition: 98 ranked plan steps (a third carrying the audited 8-advisory
/// "Show 7 more" tail), the 30 per-package OSV alerts those steps regroup,
/// 19 LLM-assessed items, one multi-dep OSV alert only partially covered by
/// the plan, and one platform-inactive (other-build-target) OSV alert.
fn audit_shaped_feed() -> EvidenceFeed {
    let mut items: Vec<EvidenceItem> = Vec::new();
    for i in 0..98 {
        let advisories = match i % 4 {
            0 => 8,
            1 => 4,
            2 => 3,
            _ => 2,
        };
        let urgency = match i % 4 {
            0 => Urgency::Critical,
            1 | 2 => Urgency::High,
            _ => Urgency::Medium,
        };
        items.push(plan_step(i, advisories, urgency));
    }
    // The 30 covered per-package alerts (same packages as the first 30 steps).
    for i in 0..30 {
        let urgency = if i % 10 == 0 {
            Urgency::Critical
        } else {
            Urgency::High
        };
        items.push(osv_alert(i, urgency, vec![format!("package-{i:03}")]));
    }
    for i in 0..19 {
        let urgency = if i % 2 == 0 {
            Urgency::Medium
        } else {
            Urgency::Watch
        };
        items.push(llm_alert(i, urgency));
    }
    // Partially covered: names a planned package AND one with no plan step.
    items.push(osv_alert(
        900,
        Urgency::High,
        vec!["package-000".to_string(), "unplanned-package".to_string()],
    ));
    // Platform-inactive: regroup-exempt, urgency-bar-exempt.
    let mut other = osv_alert(901, Urgency::Watch, vec!["package-001".to_string()]);
    other.lens_hints.other_build_target = true;
    items.push(other);
    let mut feed = EvidenceFeed::from_items(items);
    feed.tier_scope = Some(TierScope::Full);
    feed
}

fn simple_item(id: &str, urgency: Urgency, confidence: Confidence) -> EvidenceItem {
    EvidenceItem {
        title: format!("alert {id}"),
        evidence: vec![advisory_citation("pkg", 1)],
        ..base_item(id.to_string(), urgency, confidence)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// The single visibility filter (count coherence)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn dismissed_ids_are_removed_and_counts_recomputed() {
    let feed = EvidenceFeed::from_items(vec![
        simple_item("a", Urgency::Critical, Confidence::osv_verified(0.9)),
        simple_item("b", Urgency::High, Confidence::llm_assessed(0.7)),
        simple_item("c", Urgency::High, Confidence::llm_assessed(0.7)),
    ]);
    let out = preemption_visible_feed(feed, &["b".to_string()]);
    assert_eq!(out.total, 2);
    assert_eq!(out.critical_count, 1);
    assert_eq!(out.high_count, 1, "the dismissed High must leave the tally");
    assert!(out.items.iter().all(|i| i.id != "b"));
}

#[test]
fn plan_covered_osv_alert_is_regrouped_out_of_items_and_counts() {
    let plan = EvidenceItem {
        affected_deps: vec!["Lodash".to_string()],
        lens_hints: LensHints::upgrade_plan(),
        ..simple_item("plan-lodash", Urgency::High, Confidence::heuristic(0.9))
    };
    let covered = EvidenceItem {
        affected_deps: vec!["lodash".to_string()],
        ..simple_item("osv-lodash", Urgency::High, Confidence::osv_verified(0.95))
    };
    let uncovered = EvidenceItem {
        affected_deps: vec!["axios".to_string()],
        ..simple_item(
            "osv-axios",
            Urgency::Critical,
            Confidence::osv_verified(0.95),
        )
    };
    let out = preemption_visible_feed(
        EvidenceFeed::from_items(vec![plan, covered, uncovered]),
        &[],
    );
    assert_eq!(
        out.total, 2,
        "the covered per-package alert is regrouped away"
    );
    assert!(out.items.iter().all(|i| i.id != "osv-lodash"));
    // Counts describe the survivors — the audit's 15-vs-12 / 67-vs-41 gap was
    // exactly these covered duplicates being counted but never rendered.
    assert_eq!(out.critical_count, 1);
    assert_eq!(out.high_count, 1);
}

#[test]
fn partially_covered_and_non_osv_items_are_never_regrouped() {
    let plan = EvidenceItem {
        affected_deps: vec!["lodash".to_string()],
        lens_hints: LensHints::upgrade_plan(),
        ..simple_item("plan-lodash", Urgency::High, Confidence::heuristic(0.9))
    };
    // Covers lodash AND axios — axios has no plan step, so this alert stays.
    let multi = EvidenceItem {
        affected_deps: vec!["lodash".to_string(), "axios".to_string()],
        ..simple_item("osv-multi", Urgency::High, Confidence::osv_verified(0.95))
    };
    // Heuristic provenance mentioning lodash — never regrouped.
    let chain = EvidenceItem {
        affected_deps: vec!["lodash".to_string()],
        ..simple_item("chain-lodash", Urgency::Medium, Confidence::heuristic(0.5))
    };
    let out = preemption_visible_feed(EvidenceFeed::from_items(vec![plan, multi, chain]), &[]);
    let ids: Vec<&str> = out.items.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"osv-multi"));
    assert!(ids.contains(&"chain-lodash"));
    assert_eq!(out.total, 3);
}

#[test]
fn other_build_target_rows_stay_but_never_enter_the_urgency_tallies() {
    let mut other = EvidenceItem {
        affected_deps: vec!["winapi".to_string()],
        ..simple_item(
            "osv-winapi",
            Urgency::Critical,
            Confidence::osv_verified(0.95),
        )
    };
    other.lens_hints.other_build_target = true;
    let plan = EvidenceItem {
        affected_deps: vec!["winapi".to_string()],
        lens_hints: LensHints::upgrade_plan(),
        ..simple_item("plan-winapi", Urgency::High, Confidence::heuristic(0.9))
    };
    let out = preemption_visible_feed(EvidenceFeed::from_items(vec![plan, other]), &[]);
    // Kept (its collapsed section renders it) even though its dep is planned…
    assert_eq!(
        out.total, 2,
        "other-target rows are grouped, never swallowed"
    );
    // …but the bar tallies exclude it (the section is collapsed).
    assert_eq!(out.critical_count, 0);
    assert_eq!(out.high_count, 1);
}

#[test]
fn dismissing_the_plan_step_resurfaces_the_underlying_alert() {
    let plan = EvidenceItem {
        affected_deps: vec!["lodash".to_string()],
        lens_hints: LensHints::upgrade_plan(),
        ..simple_item("plan-lodash", Urgency::High, Confidence::heuristic(0.9))
    };
    let covered = EvidenceItem {
        affected_deps: vec!["lodash".to_string()],
        ..simple_item("osv-lodash", Urgency::High, Confidence::osv_verified(0.95))
    };
    let out = preemption_visible_feed(
        EvidenceFeed::from_items(vec![plan, covered]),
        &["plan-lodash".to_string()],
    );
    // Plan coverage is computed AFTER dismissals: with the step gone, the raw
    // advisory alert must come back (the facts must stay on screen somewhere).
    assert_eq!(out.total, 1);
    assert_eq!(out.items[0].id, "osv-lodash");
}

#[test]
fn feed_metadata_survives_the_mapping() {
    let mut feed = EvidenceFeed::from_items(vec![simple_item(
        "a",
        Urgency::High,
        Confidence::osv_verified(0.9),
    )]);
    feed.tier_scope = Some(TierScope::FreeFloor);
    feed.score = Some(42.0);
    feed.total_tracked = Some(47);
    feed.weak_match_count = Some(3);
    let out = present_preemption_list(feed, &[], false);
    assert_eq!(out.tier_scope, Some(TierScope::FreeFloor));
    assert_eq!(out.score, Some(42.0));
    assert_eq!(out.total_tracked, Some(47));
    assert_eq!(out.weak_match_count, Some(3));
}

// ────────────────────────────────────────────────────────────────────────────
// Per-item transport trim
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn trim_caps_citations_keeps_version_context_and_records_the_real_total() {
    let item = osv_alert(7, Urgency::High, vec!["package-007".to_string()]);
    assert_eq!(
        item.evidence.len(),
        4,
        "fixture: 3 advisories + version_context"
    );
    let trimmed = trim_item_for_list(item);
    assert_eq!(
        trimmed.evidence_total,
        Some(4),
        "the card's 'Show N more' math"
    );
    assert_eq!(
        trimmed.evidence.len(),
        LIST_EVIDENCE_CAP + 1,
        "cap + the version strip"
    );
    assert!(
        trimmed
            .evidence
            .iter()
            .any(|c| c.source == "version_context"),
        "the version strip is always embedded — the collapsed card renders it"
    );
    // Order preserved: the card's 'investigate' action opens evidence[0].url.
    assert_eq!(trimmed.evidence[0].source, "osv");
}

#[test]
fn trim_blanks_unrendered_text_and_rounds_freshness() {
    let trimmed = trim_item_for_list(plan_step(3, 8, Urgency::High));
    assert!(
        trimmed.evidence.iter().all(|c| c.relevance_note.is_empty()),
        "the Preemption card never renders relevance notes"
    );
    assert!(
        trimmed
            .suggested_actions
            .iter()
            .all(|a| a.description.is_empty()),
        "action tooltips come back with the detail fetch"
    );
    assert!(
        !trimmed.suggested_actions.is_empty(),
        "labels stay — buttons render"
    );
    assert!(trimmed.precedents.is_empty());
    for c in &trimmed.evidence {
        assert_eq!(
            (c.freshness_days * 10.0).round() / 10.0,
            c.freshness_days,
            "freshness ships at the 0.1-day precision the card renders"
        );
    }
}

#[test]
fn trim_caps_explanation_on_a_char_boundary() {
    let mut item = llm_alert(1, Urgency::Medium);
    item.explanation =
        "解析器の既定値が変わると暗黙のプラグイン解決に依存しているプロジェクトは静かに壊れます。"
            .repeat(20);
    assert!(item.explanation.len() > LIST_EXPLANATION_CAP_BYTES);
    let trimmed = trim_item_for_list(item);
    assert!(trimmed.explanation.len() <= LIST_EXPLANATION_CAP_BYTES);
    assert!(trimmed.explanation.ends_with('\u{2026}'));
    // Short explanations pass through untouched.
    let short = trim_item_for_list(simple_item("s", Urgency::Watch, Confidence::heuristic(0.5)));
    assert!(!short.explanation.ends_with('\u{2026}'));
}

#[test]
fn plan_cap_holds_back_the_collapsed_tail_but_counts_it() {
    let feed = audit_shaped_feed();
    let capped = present_preemption_list(feed.clone(), &[], false);
    let full = present_preemption_list(feed, &[], true);
    // Counts describe the visible set in BOTH shapes (the header bar counts
    // collapsed plan steps, exactly as the view always did)…
    assert_eq!(capped.total, full.total);
    assert_eq!(capped.critical_count, full.critical_count);
    assert_eq!(capped.high_count, full.high_count);
    // …but the capped response ships only the rendered plan steps.
    let capped_plan = capped
        .items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .count();
    let full_plan = full
        .items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .count();
    assert_eq!(capped_plan, LIST_PLAN_STEP_CAP);
    assert_eq!(full_plan, 98);
    // The view derives the show-more count from exactly this identity:
    assert_eq!(
        capped.total - capped.items.len(),
        full_plan - LIST_PLAN_STEP_CAP,
        "total - items.len() must equal the held-back plan steps"
    );
    // Rank order preserved: the shipped 25 are the FIRST 25 in feed order.
    let shipped: Vec<&str> = capped
        .items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .map(|i| i.id.as_str())
        .collect();
    let top_of_full: Vec<&str> = full
        .items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .take(LIST_PLAN_STEP_CAP)
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(shipped, top_of_full);
}

// ────────────────────────────────────────────────────────────────────────────
// The payload measurement — before/after bytes on the audit shape
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn audit_shaped_payload_shrinks_at_least_5x_and_lands_under_60kb() {
    let feed = audit_shaped_feed();
    assert_eq!(feed.total, 149, "the audit measured 149 items");

    let before = feed_bytes(&feed);
    // Calibration gate: the fixture must reproduce the audited payload
    // (304,868 chars) within ±20%, or the reduction claim below is about
    // toy data, not about the live response.
    assert!(
        (245_000..=365_000).contains(&before),
        "fixture drifted off the audited 304,868-byte shape: {before} bytes"
    );

    let list = present_preemption_list(feed, &[], false);
    let after = feed_bytes(&list);
    println!(
        "preemption list payload: {before} -> {after} bytes ({:.1}x lighter), \
         {} items shipped of {} visible",
        before as f64 / after as f64,
        list.items.len(),
        list.total,
    );

    // Count coherence on the same corpus: the 30 covered per-package alerts
    // leave both the items AND the counts (the audit's 149-vs-120 gap).
    assert_eq!(list.total, 119, "149 minus the 30 plan-covered duplicates");

    assert!(
        after < 60_000,
        "list payload must land under ~60KB, got {after} bytes (before: {before})"
    );
    assert!(
        before >= after * 5,
        "list payload must be at least 5x lighter: {before} -> {after} \
         ({:.1}x)",
        before as f64 / after as f64
    );
}
