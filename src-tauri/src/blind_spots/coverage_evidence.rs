// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Evidence-bound risk for coverage gaps (audit 2026-10-07, wave 4c).
//!
//! Two rules live here, both about what a coverage gap may CLAIM:
//!
//! 1. **Risk follows evidence, not inattention.** `classify_dep_risk` used to
//!    turn "days since you last opened an item about this dependency" into
//!    risk, with never-opened read as 999 days — so schemars with one routine
//!    release in three projects was "critical". Never engaged is now its own
//!    state (no day-based escalation), and a dependency whose only evidence is
//!    routine releases / analyses / discussion is capped at `medium`: only a
//!    security advisory or breaking change can make a gap high or critical.
//! 2. **An exact registry link is coverage, at any age.** A crate whose last
//!    registry release is 33 days old is covered and quiet, not
//!    "unmonitored" — the 14-day signal window measures activity, not whether
//!    a source watches the package.

use std::collections::HashMap;

use super::{gap_row_breakdown, normalize_dep_name, risk_ord, UncoveredDep};
use crate::scoring_config;

/// Classify a gap's risk from unseen-signal volume, spread and — only when
/// the user HAS engaged with this dependency before — how long ago that was.
/// `days_since_engaged = None` means never engaged: that is not "999 days of
/// neglect", it is no history, and it never escalates on its own.
pub(super) fn classify_dep_risk(
    days_since_engaged: Option<u32>,
    unseen_signals: u32,
    project_count: usize,
) -> String {
    let older_than = |limit: u32| days_since_engaged.is_some_and(|d| d > limit);
    if older_than(scoring_config::BLIND_SPOT_RISK_CRITICAL_DAYS as u32)
        && project_count > scoring_config::BLIND_SPOT_RISK_CRITICAL_PROJECTS as usize
    {
        "critical".to_string()
    } else if older_than(scoring_config::BLIND_SPOT_RISK_HIGH_DAYS as u32)
        || (unseen_signals > scoring_config::BLIND_SPOT_RISK_HIGH_UNSEEN_SIGNALS as u32
            && project_count > scoring_config::BLIND_SPOT_RISK_HIGH_PROJECTS as usize)
    {
        "high".to_string()
    } else if older_than(scoring_config::BLIND_SPOT_RISK_MEDIUM_DAYS as u32)
        || unseen_signals > scoring_config::BLIND_SPOT_RISK_MEDIUM_UNSEEN_SIGNALS as u32
    {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

/// The highest risk a gap may carry given what its evidence actually is.
/// Security advisories / breaking changes allow anything; every other kind
/// of evidence (releases, analyses, discussion) is at most `medium`.
pub(super) fn evidence_capped_risk(risk: &str, has_security_evidence: bool) -> String {
    // risk_ord: lower = more severe, so ">= medium" means medium or milder.
    if has_security_evidence || risk_ord(risk) >= risk_ord("medium") {
        risk.to_string()
    } else {
        "medium".to_string()
    }
}

/// Apply [`evidence_capped_risk`] to every gap that HAS signals (zero-signal
/// gaps carry no evidence to judge and keep their coverage-based risk), then
/// restore the most-severe-first order the cap may have disturbed.
pub(super) fn cap_risk_to_evidence(mut uncovered: Vec<UncoveredDep>) -> Vec<UncoveredDep> {
    for d in uncovered
        .iter_mut()
        .filter(|d| d.available_signal_count > 0)
    {
        let has_security = gap_row_breakdown(d).1.is_some_and(|b| b.security > 0);
        d.risk_level = evidence_capped_risk(&d.risk_level, has_security);
    }
    uncovered.sort_by(|a, b| {
        risk_ord(&a.risk_level)
            .cmp(&risk_ord(&b.risk_level))
            .then(b.days_since_last_signal.cmp(&a.days_since_last_signal))
    });
    uncovered
}

/// Registry ecosystems each eligible dependency has an EXACT registry link
/// in, at any age — keyed by normalised name. Reads the `_blind_spot_deps`
/// temp table `find_uncovered_deps` fills; when it is absent the query fails
/// and the map is empty (the pre-fix behaviour, never a hard error).
pub(super) fn registry_linked_deps(
    conn: &rusqlite::Connection,
) -> HashMap<String, Vec<&'static str>> {
    let sql = "SELECT DISTINCT bd.name, si.source_type
               FROM _blind_spot_deps bd
               JOIN source_item_dependencies sid
                 ON LOWER(REPLACE(sid.package_name, '-', '_')) = bd.name
               JOIN source_items si ON si.id = sid.source_item_id
               WHERE sid.match_type IN ('exact_registry', 'registry')";
    let mut out: HashMap<String, Vec<&'static str>> = HashMap::new();
    let Ok(mut stmt) = conn.prepare(sql) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return out;
    };
    for (name, source_type) in rows.flatten() {
        if let Some(eco) = crate::osv::exposure::registry_source_ecosystem(&source_type) {
            out.entry(name).or_default().push(eco);
        }
    }
    out
}

/// Is `package_name` (in `ecosystem`) covered by an exact registry link? An
/// ecosystem the registry map cannot name accepts a link from any registry.
pub(super) fn is_registry_covered(
    linked: &HashMap<String, Vec<&'static str>>,
    package_name: &str,
    ecosystem: &str,
) -> bool {
    let wanted = crate::osv::exposure::canonical(ecosystem);
    linked
        .get(&normalize_dep_name(package_name))
        .is_some_and(|ecos| ecos.iter().any(|e| wanted.is_none_or(|w| w == *e)))
}

#[cfg(test)]
#[path = "coverage_evidence_tests.rs"]
mod tests;
