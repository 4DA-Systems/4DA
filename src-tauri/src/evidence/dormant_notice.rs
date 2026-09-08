// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! A dormant project is NAMED once, not hidden and not shouted about.
//!
//! Two failure modes sit either side of this module, and it exists because
//! both are wrong:
//!
//! - **Silence.** `PROJECT_RELEVANCE_FLOOR` (0.15) versus a git-recency score
//!   of 0.1 for anything idle over 90 days means a dormant repo's lockfile is
//!   never walked and its rows never pass the read gates. Measured
//!   2026-09-07: `D:\runyourempire\navcal`, dormant since ~2025-11, holds 31
//!   packages with known OSV advisories, and 4DA showed **nothing** about it
//!   anywhere. A user who reads Preemption as "my security posture" was told
//!   their posture was clean about a repo they still own and may redeploy.
//! - **Noise.** The other extreme — 31 Watch rows about a repo nobody is
//!   deploying — buries the findings that are today's work. That is what
//!   `cap_dormant_items` alone produces once the rows do get in.
//!
//! So: one quiet summary row per dormant project, carrying the count and the
//! dormancy, with the advisory citations attached as evidence. The individual
//! alerts are replaced, not dropped — nothing about the project is invented
//! and nothing is concealed. A dormant project with no vulnerable packages
//! emits nothing at all (doctrine rule 6: no "nothing to report" rows).

use std::collections::{BTreeMap, HashSet};

use super::liveness::ProjectLiveness;
use super::types::{
    Action, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints, Urgency,
};

/// Citations carried onto the summary. Enough to prove the claim; the whole
/// point is that this row is small.
const MAX_SUMMARY_CITATIONS: usize = 5;

/// `validate::validate_item` caps `title.len()` — BYTES, not chars — at 120,
/// and rejects a trailing period. Both are load-bearing here: a project name
/// is arbitrary user text, and in debug builds a title that fails validation
/// hard-panics (doctrine rule 9).
const MAX_TITLE_BYTES: usize = 120;

/// Replace every alert whose affected projects are ALL dormant with one
/// summary row per dormant project. Returns how many alerts were collapsed.
///
/// Untouched: any item with an active — or merely unknown — affected project
/// (`ProjectLiveness::all_dormant` is false for both), any item that is not
/// an [`EvidenceKind::Alert`], and any item with no affected project at all.
/// An ecosystem-wide advisory that names no project of the user's is not a
/// statement about a dormant repo and must not be filed under one.
pub fn collapse_dormant_alerts(items: &mut Vec<EvidenceItem>, liveness: &ProjectLiveness) -> usize {
    let collapsible: Vec<EvidenceItem> = {
        let mut kept = Vec::with_capacity(items.len());
        let mut taken = Vec::new();
        for item in items.drain(..) {
            if is_collapsible(&item, liveness) {
                taken.push(item);
            } else {
                kept.push(item);
            }
        }
        *items = kept;
        taken
    };
    if collapsible.is_empty() {
        return 0;
    }

    // BTreeMap so the summaries come out in a stable path order rather than
    // whatever the hasher decided this run.
    let mut by_project: BTreeMap<String, Vec<EvidenceItem>> = BTreeMap::new();
    for item in &collapsible {
        for project in &item.affected_projects {
            by_project
                .entry(project.clone())
                .or_default()
                .push(item.clone());
        }
    }

    let collapsed = collapsible.len();
    for (project, alerts) in by_project {
        if let Some(summary) = summarise(&project, &alerts, liveness) {
            items.push(summary);
        }
    }
    collapsed
}

fn is_collapsible(item: &EvidenceItem, liveness: &ProjectLiveness) -> bool {
    matches!(item.kind, EvidenceKind::Alert)
        && !item.lens_hints.dormant_notice
        // The Upgrade Plan is its own ranked lane with its own dormancy cap
        // (`cap_dormant_items`, applied before the plan is persisted for the
        // MCP to read). Folding a ranked "upgrade X" step into a security
        // summary would misreport what it is, and would make the in-app feed
        // disagree with the persisted snapshot.
        && !item.lens_hints.upgrade_plan
        && liveness.all_dormant(&item.affected_projects)
}

/// One summary row for `project`. `None` when there is nothing to say.
fn summarise(
    project: &str,
    alerts: &[EvidenceItem],
    liveness: &ProjectLiveness,
) -> Option<EvidenceItem> {
    if alerts.is_empty() {
        return None;
    }
    let packages: HashSet<String> = alerts
        .iter()
        .flat_map(|a| a.affected_deps.iter().map(|d| d.to_lowercase()))
        .collect();
    // Every collapsed alert names at least one package in practice; if none
    // do, there is no honest count to report and therefore no row.
    if packages.is_empty() {
        return None;
    }
    let days = liveness.dormant_days(project)?;

    let mut evidence: Vec<EvidenceCitation> = alerts
        .iter()
        .flat_map(|a| a.evidence.iter().cloned())
        .collect();
    evidence.dedup_by(|a, b| a.source == b.source && a.title == b.title);
    evidence.truncate(MAX_SUMMARY_CITATIONS);
    // `validate::requires_evidence` demands at least one citation for an
    // Alert. An alert with none would produce an unvalidatable summary, so
    // the summary is not emitted rather than emitted broken.
    if evidence.is_empty() {
        return None;
    }

    let name = project_leaf(project);
    let count = packages.len();
    let noun = if count == 1 { "package" } else { "packages" };
    let title = truncate_title(format!(
        "{name} — dormant {days} days, {count} known vulnerable {noun} not audited"
    ));

    Some(EvidenceItem {
        id: format!("dormant-notice:{project}"),
        kind: EvidenceKind::Alert,
        title,
        explanation: format!(
            "{name} has had no git or manifest activity for {days} days. \
             It still holds {count} {noun} with published advisories, and \
             nothing here has been audited since it went quiet. Not today's \
             emergency — but it is yours, and it is not clean."
        ),
        // The underlying matches are version-confirmed OSV data; the summary
        // asserts nothing beyond their count, so it inherits their
        // provenance rather than claiming a stronger one.
        confidence: Confidence::osv_verified(0.9),
        urgency: Urgency::Watch,
        reversibility: None,
        evidence,
        evidence_total: Some(alerts.iter().map(|a| a.evidence.len()).sum()),
        affected_projects: vec![project.to_string()],
        affected_deps: {
            let mut deps: Vec<String> = packages.into_iter().collect();
            deps.sort();
            deps
        },
        suggested_actions: vec![Action {
            action_id: "review_security".to_string(),
            label: "Audit before the next deploy".to_string(),
            description:
                "Reactivate and run an audit before the next deploy, or archive the project."
                    .to_string(),
        }],
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints {
            dormant_notice: true,
            ..LensHints::preemption_only()
        },
        created_at: chrono::Utc::now().timestamp_millis(),
        expires_at: None,
    })
}

/// The last path segment — "navcal", not the whole absolute path.
fn project_leaf(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
}

/// Fit a title inside [`MAX_TITLE_BYTES`] without ending in a period.
///
/// Cuts on a char boundary — a slice through a multi-byte project name would
/// panic — and appends no ellipsis: "..." ends in a period, which validation
/// rejects, and "…" is three bytes that would push the result back over the
/// byte budget it was cut to fit.
fn truncate_title(title: String) -> String {
    if title.len() <= MAX_TITLE_BYTES {
        return title;
    }
    let cut = (0..=MAX_TITLE_BYTES)
        .rev()
        .find(|i| title.is_char_boundary(*i))
        .unwrap_or(0);
    title[..cut].trim_end().trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alert(id: &str, project: &str, dep: &str) -> EvidenceItem {
        EvidenceItem {
            id: id.to_string(),
            kind: EvidenceKind::Alert,
            title: format!("{dep} advisory"),
            explanation: "An advisory affects this package".to_string(),
            confidence: Confidence::osv_verified(0.95),
            urgency: Urgency::High,
            reversibility: None,
            evidence: vec![EvidenceCitation {
                source: "osv".to_string(),
                title: format!("GHSA-{dep}"),
                url: None,
                freshness_days: 5.0,
                relevance_note: String::new(),
            }],
            evidence_total: None,
            affected_projects: vec![project.to_string()],
            affected_deps: vec![dep.to_string()],
            suggested_actions: vec![Action {
                action_id: "review_security".to_string(),
                label: "Review".to_string(),
                description: "Review".to_string(),
            }],
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at: 0,
            expires_at: None,
        }
    }

    fn dormant(paths: &[&str]) -> ProjectLiveness {
        ProjectLiveness::from_entries(&paths.iter().map(|p| (*p, 400_i64)).collect::<Vec<_>>())
    }

    #[test]
    fn many_alerts_for_one_dormant_project_become_one_summary() {
        let mut items = vec![
            alert("a", "/dev/navcal", "lodash"),
            alert("b", "/dev/navcal", "axios"),
            alert("c", "/dev/navcal", "minimist"),
        ];
        let collapsed = collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"]));

        assert_eq!(collapsed, 3, "all three were collapsed");
        assert_eq!(items.len(), 1, "and replaced by exactly one row");
        let summary = &items[0];
        assert!(summary.lens_hints.dormant_notice);
        assert_eq!(summary.urgency, Urgency::Watch, "quiet, never alarming");
        assert!(summary.title.contains("navcal"));
        assert!(summary.title.contains("3 known vulnerable packages"));
        assert_eq!(summary.affected_deps.len(), 3);
        assert_eq!(summary.affected_projects, vec!["/dev/navcal".to_string()]);
    }

    #[test]
    fn one_summary_per_dormant_project() {
        let mut items = vec![
            alert("a", "/dev/navcal", "lodash"),
            alert("b", "/dev/oldsite", "axios"),
        ];
        collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal", "/dev/oldsite"]));
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.lens_hints.dormant_notice));
    }

    #[test]
    fn the_summary_passes_schema_validation() {
        // Doctrine rule 9: every EvidenceItem is validated at materializer
        // output in debug builds. A summary that cannot validate would
        // hard-panic the app.
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"]));
        crate::evidence::validate_item(&items[0]).expect("summary must validate");
    }

    // ---- negative tests: the collapse must never swallow live work --------

    #[test]
    fn an_active_projects_alerts_are_never_collapsed() {
        let mut items = vec![
            alert("a", "/dev/live-app", "lodash"),
            alert("b", "/dev/live-app", "axios"),
        ];
        let collapsed = collapse_dormant_alerts(&mut items, &ProjectLiveness::default());
        assert_eq!(collapsed, 0);
        assert_eq!(items.len(), 2, "live findings are untouched");
        assert!(items.iter().all(|i| !i.lens_hints.dormant_notice));
    }

    #[test]
    fn one_active_project_on_an_alert_defeats_the_collapse() {
        // `all_dormant` requires EVERY path dormant. A cross-project advisory
        // that also hits a live repo is live work.
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        items[0].affected_projects.push("/dev/live-app".to_string());
        let collapsed = collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"]));
        assert_eq!(collapsed, 0);
        assert_eq!(items.len(), 1);
        assert!(!items[0].lens_hints.dormant_notice);
    }

    #[test]
    fn a_dormant_project_with_no_vulnerable_packages_emits_nothing() {
        // Cold-start rule: no "nothing to report" rows.
        let mut items: Vec<EvidenceItem> = Vec::new();
        let collapsed = collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"]));
        assert_eq!(collapsed, 0);
        assert!(items.is_empty(), "silence when there is nothing to say");
    }

    #[test]
    fn an_alert_with_no_affected_project_is_not_filed_under_a_dormant_one() {
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        items[0].affected_projects.clear();
        let collapsed = collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"]));
        assert_eq!(collapsed, 0, "ecosystem-wide advisories stay as they are");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn an_upgrade_plan_step_is_left_in_its_own_lane() {
        // The plan has its own dormancy cap and its own persisted snapshot;
        // folding a ranked step into a security summary would misreport it
        // and make the feed disagree with what the MCP reads.
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        items[0].lens_hints.upgrade_plan = true;
        assert_eq!(
            collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"])),
            0
        );
        assert_eq!(items.len(), 1);
        assert!(items[0].lens_hints.upgrade_plan);
    }

    #[test]
    fn a_non_alert_item_is_left_alone() {
        // Coverage gaps and upgrade steps have their own dormancy handling.
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        items[0].kind = EvidenceKind::Gap;
        assert_eq!(
            collapse_dormant_alerts(&mut items, &dormant(&["/dev/navcal"])),
            0
        );
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn collapsing_twice_does_not_collapse_the_summary_itself() {
        let mut items = vec![alert("a", "/dev/navcal", "lodash")];
        let liveness = dormant(&["/dev/navcal"]);
        collapse_dormant_alerts(&mut items, &liveness);
        let second = collapse_dormant_alerts(&mut items, &liveness);
        assert_eq!(second, 0, "idempotent — the summary is not re-summarised");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn a_long_project_name_still_yields_a_valid_title() {
        let long = format!("/dev/{}", "n".repeat(300));
        let mut items = vec![alert("a", &long, "lodash")];
        collapse_dormant_alerts(&mut items, &dormant(&[long.as_str()]));
        let title = &items[0].title;
        assert!(
            title.len() <= MAX_TITLE_BYTES,
            "byte budget, not char count"
        );
        assert!(!title.ends_with('.'), "titles carry no trailing period");
        crate::evidence::validate_item(&items[0]).expect("truncated title validates");
    }

    #[test]
    fn a_multibyte_project_name_is_cut_on_a_char_boundary() {
        // Slicing a UTF-8 string at an arbitrary byte index panics. A project
        // directory is arbitrary user text, so this is a real crash path.
        let long = format!("/dev/{}", "é".repeat(200));
        let mut items = vec![alert("a", &long, "lodash")];
        collapse_dormant_alerts(&mut items, &dormant(&[long.as_str()]));
        let item = &items[0];
        assert!(item.title.len() <= MAX_TITLE_BYTES);
        crate::evidence::validate_item(item).expect("multibyte title validates");
    }
}
