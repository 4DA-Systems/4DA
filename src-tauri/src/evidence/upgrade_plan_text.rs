// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The per-line words of an Upgrade Plan step: its `fix-target` citations
//! and the explanation's target sentence. Split out of `upgrade_plan.rs`
//! (file-size gate) when the step gained its blocked-upstream title (Y5).

use crate::osv::fix_target::{self, LineTarget};

use super::types::EvidenceCitation;
use super::upgrade_plan::truncate;

/// One `fix-target` citation per installed version (Phase 1.2), carried in
/// the canonical item rather than a new type: title `"1.1.12 -> 1.1.21"`, and
/// a note with the upgrade type, whether it clears every known advisory, and
/// each install site as `project (direct|transitive[, dev])`.
pub(super) fn line_citations(lines: &[LineTarget]) -> Vec<EvidenceCitation> {
    lines
        .iter()
        .map(|l| {
            let none = if l.introduces.is_empty() {
                "no fix published"
            } else {
                "no clean fix"
            };
            let target = l.target_version.as_deref().unwrap_or(none);
            let kind = match l.upgrade_type {
                Some(fix_target::UpgradeType::Patch) => "patch",
                Some(fix_target::UpgradeType::Minor) => "minor",
                Some(fix_target::UpgradeType::Major) => "major",
                None => "unknown",
            };
            let clears = if l.target_version.is_none() {
                ""
            } else if !fix_target::held_clean(l) {
                // npm (D-4): what the target installs is affected, or was
                // not checked; never "clears every known advisory".
                match l.held {
                    Some(fourda_depengine::held::HeldOutcome::Blocked { .. }) => {
                        "; NOT a clean fix: a dependency it pins or narrowly ranges is affected"
                    }
                    _ => "; what it pins or narrowly ranges was NOT CHECKED",
                }
            } else if l.clears_all_known {
                "; clears every known advisory"
            } else {
                "; clears only the advisories with a fix"
            };
            let sites: Vec<String> = l
                .sites
                .iter()
                .map(|s| {
                    format!(
                        "{} ({}{})",
                        s.project_path,
                        if s.is_direct { "direct" } else { "transitive" },
                        if s.is_dev { ", dev" } else { "" }
                    )
                })
                .collect();
            EvidenceCitation {
                source: "fix-target".to_string(),
                title: truncate(&format!("{} -> {target}", l.installed_version), 160),
                url: None,
                freshness_days: 0.0,
                relevance_note: truncate(
                    &format!("{kind} upgrade{clears}. Installed in: {}", sites.join(", ")),
                    200,
                ),
            }
        })
        .collect()
}

/// The per-line sentence of a step's explanation: where each copy goes when
/// there is more than one line, that the move is a major one, and that the
/// target leaves an unfixed advisory open. Empty in the common case (one line,
/// in-major, clears everything) — the title already says it.
pub(super) fn target_detail(package: &str, lines: &[LineTarget], major: bool) -> String {
    let mut out = String::new();
    if lines.iter().filter(|l| l.target_version.is_some()).count() > 1 {
        out.push_str(&format!(
            " Each installed version has its own target: {}.",
            fix_target::describe_lines(lines)
        ));
    }
    if major {
        out.push_str(
            " The target crosses a major version — breaking changes are possible; \
             check the changelog before upgrading.",
        );
    }
    if lines
        .iter()
        .any(|l| l.target_version.is_some() && !l.clears_all_known)
    {
        out.push_str(&format!(
            " No published version of {package} clears every known advisory; \
             the target clears the ones that have a fix."
        ));
    }
    // npm (audit 6.2.5 D-4): what each target installs.
    out.push_str(&fix_target::held_sentences(package, lines));
    out
}
