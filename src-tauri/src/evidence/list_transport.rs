// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Preemption LIST transport — "the list ships what the list shows" (AD-036).
//!
//! Two 2026-08-31 live-audit findings live and die here:
//!
//! 1. **305 KB payloads.** `get_preemption_alerts` shipped ~149 items at ~2 KB
//!    each — full citation arrays (9 rows behind a "Show 7 more" the user may
//!    never click), 200-char relevance notes the Preemption card never renders,
//!    and `null` after `null` of optional fields. The list response now embeds
//!    only what the collapsed card actually paints; the full item stays intact
//!    in the feed cache and is served by `get_preemption_item_detail` when a
//!    card expands.
//!
//! 2. **Counts that contradicted the screen.** The command reported
//!    total=149 / critical=15 / high=67 while the header rendered
//!    120 / 12 / 41, because the VIEW filtered client-side (locally dismissed
//!    ids, per-package OSV alerts regrouped under their Upgrade Plan step,
//!    other-build-target rows excluded from the urgency tallies) and the
//!    command counted the unfiltered vec. That filter now has exactly ONE
//!    definition — [`preemption_visible_feed`] — applied in the command's
//!    response mapping; the view renders every item it receives and echoes the
//!    feed's counts verbatim.
//!
//! Trimming is a TRANSPORT concern: functions here consume an owned
//! [`EvidenceFeed`] clone at the command boundary and never mutate the cached
//! feed, the persisted upgrade-plan snapshot, or anything a non-list consumer
//! reads (doctrine rule 1 — `EvidenceItem` stays canonical; no parallel DTO).

use std::collections::HashSet;

use super::types::{ConfidenceProvenance, EvidenceFeed, EvidenceItem, Urgency};

/// Citations embedded per LIST item (plus the `version_context` strip, which
/// the collapsed card always renders). Matches the card's collapsed rendering
/// exactly (2 rows before "Show N more"); everything beyond comes from the
/// detail path on expand.
pub const LIST_EVIDENCE_CAP: usize = 2;

/// Byte cap for LIST explanations. The card clamps its collapsed rendering at
/// 280 *chars*; 360 bytes leaves normal explanations untouched and cuts only
/// the long LLM tail — which the detail fetch restores on "more".
pub const LIST_EXPLANATION_CAP_BYTES: usize = 360;

/// Upgrade Plan steps embedded in the LIST response. The view renders the
/// top-ranked steps and collapses the rest behind "show more"
/// (`UPGRADE_PLAN_VISIBLE_CAP` in `PreemptionView.tsx` — keep the two in
/// sync); shipping a 100+-step ranked tail nobody has expanded was the single
/// largest slice of the 305 KB payload. Plan items arrive rank-ordered from
/// the builder, so "first N in feed order" IS "top N as rendered". The
/// summary counts are computed BEFORE this cap (the urgency bar counts
/// collapsed steps too, exactly as the view always did); expanding the
/// section refetches with `full_plan = true`. Non-plan items are never
/// capped.
pub const LIST_PLAN_STEP_CAP: usize = 25;

/// Citation source rendered as the card's version strip — always embedded.
const VERSION_CONTEXT_SOURCE: &str = "version_context";

/// THE Preemption visibility filter — the one definition of "what the user
/// sees", shared by the returned items AND the returned counts. Mirrors (and
/// replaces) the filter `PreemptionView` used to run client-side:
///
/// 1. Locally dismissed/snoozed ids are removed (the view passes its persisted
///    dismissal set; 7-day TTL is enforced client-side before the call).
/// 2. A per-package OSV-verified alert whose every affected dep is covered by
///    an Upgrade Plan step is REGROUPED away — same facts, richer framing in
///    the plan section; never two cards. Plan coverage is computed from the
///    post-dismissal set, so dismissing a plan step resurfaces its underlying
///    advisory alert. Other-build-target rows are never regrouped (they live
///    in their own collapsed section).
/// 3. Summary counts are recomputed from the survivors, with other-build-target
///    rows excluded from the critical/high tallies (their section is collapsed
///    and they are Watch-capped upstream — the tally must match the bar).
pub fn preemption_visible_feed(feed: EvidenceFeed, dismissed_ids: &[String]) -> EvidenceFeed {
    let dismissed: HashSet<&str> = dismissed_ids.iter().map(String::as_str).collect();
    let mut items = feed.items;
    if !dismissed.is_empty() {
        items.retain(|i| !dismissed.contains(i.id.as_str()));
    }

    let plan_packages: HashSet<String> = items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .flat_map(|i| i.affected_deps.iter().map(|d| d.to_lowercase()))
        .collect();
    if !plan_packages.is_empty() {
        items.retain(|i| !is_plan_covered(i, &plan_packages));
    }

    let critical_count = items
        .iter()
        .filter(|i| counts_toward_urgency_bar(i) && i.urgency == Urgency::Critical)
        .count();
    let high_count = items
        .iter()
        .filter(|i| counts_toward_urgency_bar(i) && i.urgency == Urgency::High)
        .count();

    EvidenceFeed {
        total: items.len(),
        critical_count,
        high_count,
        items,
        score: feed.score,
        total_tracked: feed.total_tracked,
        weak_match_count: feed.weak_match_count,
        data_freshness: feed.data_freshness,
        tier_scope: feed.tier_scope,
    }
}

/// The full LIST response mapping: visibility filter, collapsed-plan cap,
/// per-item transport trim. Counts are computed by the visibility filter
/// BEFORE the cap and trim — `total`/`critical_count`/`high_count` describe
/// the visible set (collapsed plan steps included, exactly as the header bar
/// always counted them), so `total - items.len()` is the number of plan steps
/// held back for the `full_plan` refetch.
pub fn present_preemption_list(
    feed: EvidenceFeed,
    dismissed_ids: &[String],
    full_plan: bool,
) -> EvidenceFeed {
    let mut feed = preemption_visible_feed(feed, dismissed_ids);
    if !full_plan {
        let mut plan_kept = 0usize;
        feed.items.retain(|i| {
            if !i.lens_hints.upgrade_plan {
                return true;
            }
            plan_kept += 1;
            plan_kept <= LIST_PLAN_STEP_CAP
        });
    }
    feed.items = feed.items.into_iter().map(trim_item_for_list).collect();
    feed
}

fn is_plan_covered(item: &EvidenceItem, plan_packages: &HashSet<String>) -> bool {
    if item.lens_hints.upgrade_plan || item.lens_hints.other_build_target {
        return false;
    }
    item.confidence.provenance == ConfidenceProvenance::OsvVerified
        && !item.affected_deps.is_empty()
        && item
            .affected_deps
            .iter()
            .all(|dep| plan_packages.contains(&dep.to_lowercase()))
}

/// Other-build-target rows sit in a collapsed section; the urgency bar (and
/// therefore the summary counts) never includes them.
fn counts_toward_urgency_bar(item: &EvidenceItem) -> bool {
    !item.lens_hints.other_build_target
}

/// Trim ONE item to what the collapsed list card renders (AD-036):
///
/// - `evidence`: the `version_context` strip plus the first
///   [`LIST_EVIDENCE_CAP`] citations; `evidence_total` records the real count
///   so the card can say "Show N more" and lazy-fetch the rest.
/// - `relevance_note`: blanked — the Preemption card has no surface that
///   renders it (the detail path keeps it).
/// - `freshness_days`: rounded to 0.1 — the card renders whole days/weeks.
/// - `explanation`: byte-capped past the card's collapsed clamp; "more"
///   hydrates the full text.
/// - action `description`s: blanked (hover tooltips; restored by detail).
/// - `precedents`: cleared — no list card renders them (and Preemption never
///   populates them today).
///
/// The input is an owned clone from the command boundary — stored/cached items
/// are never mutated.
pub fn trim_item_for_list(mut item: EvidenceItem) -> EvidenceItem {
    let full_citation_count = item.evidence.len();
    let mut kept = Vec::with_capacity(LIST_EVIDENCE_CAP + 1);
    let mut non_version_kept = 0usize;
    for mut citation in item.evidence.drain(..) {
        let keep = if citation.source == VERSION_CONTEXT_SOURCE {
            true
        } else if non_version_kept < LIST_EVIDENCE_CAP {
            non_version_kept += 1;
            true
        } else {
            false
        };
        if keep {
            citation.relevance_note = String::new();
            citation.freshness_days = (citation.freshness_days * 10.0).round() / 10.0;
            kept.push(citation);
        }
    }
    item.evidence = kept;
    item.evidence_total = Some(full_citation_count);
    item.explanation = cap_bytes_on_char_boundary(item.explanation, LIST_EXPLANATION_CAP_BYTES);
    for action in item.suggested_actions.iter_mut() {
        action.description = String::new();
    }
    item.precedents = Vec::new();
    item
}

/// Byte-cap with an ellipsis, snapped to a char boundary (the same rule the
/// upgrade-plan citation truncation uses — a raw byte slice panics on CJK).
fn cap_bytes_on_char_boundary(s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let budget = max.saturating_sub('\u{2026}'.len_utf8());
    let mut end = budget.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    // SAFE: `end` was snapped down to a char boundary above.
    #[allow(clippy::string_slice)]
    let head = &s[..end];
    format!("{head}\u{2026}")
}

#[cfg(test)]
#[path = "list_transport_tests.rs"]
mod tests;
