// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One severity per advisory, on every surface and in both engines.
//!
//! - [`grade_unrated`]: a record its source never graded takes the highest
//!   grade among its GHSA aliases in the local mirror (the rule is the engine
//!   crate's `fourda_depengine::severity`, shared with `@4da/mcp-server`'s
//!   `gradeFromAliases`). It runs once per matching pass over the mirror rows
//!   the pass already read: no network, and no query per advisory.
//! - [`cluster_severity`]: one vulnerability cluster's tier, and the alias it
//!   was inherited from when it was, so no surface presents an inherited grade
//!   as the record's own.
//! - [`cluster_urgency`]: the ONE base urgency the Upgrade Plan and the alert
//!   path both grade a cluster by, including the rule for a cluster that is
//!   still unknown. `vectors/severity.jsonl` pins all three.

use std::collections::{HashMap, HashSet};

use fourda_depengine::severity::{
    ghsa_aliases, grade_from_aliases, is_maintenance_notice, tier_rank,
};

use crate::evidence::Urgency;

use super::types::{cvss_band, MatchedAdvisory, StoredAdvisory};

/// The four-tier vocabulary (`moderate` reads `medium`).
pub(crate) fn canonical_tier(label: &str) -> Option<&'static str> {
    match label.trim().to_ascii_lowercase().as_str() {
        "critical" => Some("critical"),
        "high" => Some("high"),
        "medium" | "moderate" => Some("medium"),
        "low" => Some("low"),
        _ => None,
    }
}

/// One record's own grade, the ONE per-record rule every grader reads: the
/// source's curated label first (`database_specific.severity`, which the
/// mirror stores as `severity_label`), else the band of a scorable CVSS
/// score. `None` when it has neither.
///
/// Label first because GitHub's label follows the CVSS v4 score that neither
/// engine computes: 123 of the 3,923 pinned conformance records carry a label
/// and a CVSS v3 band that disagree, all GHSA, 113 of them beside a v4
/// vector (GHSA-23hp-3jrh-7fpw: v3 7.5, label CRITICAL, v4 VA:H/SA:H).
pub(crate) fn row_grade(cvss_score: Option<f64>, label: Option<&str>) -> Option<&'static str> {
    label
        .and_then(canonical_tier)
        .or_else(|| cvss_score.filter(|s| *s > 0.0).map(cvss_band))
}

/// The record carries no grade of its own (no curated label, no CVSS score)
/// and is not a maintenance notice (those are never graded from an alias).
fn is_unrated(m: &MatchedAdvisory) -> bool {
    m.severity_from.is_none()
        && row_grade(m.cvss_score, m.severity_label.as_deref()).is_none()
        && !is_maintenance_notice(&m.summary)
}

/// Grade every unrated match from its GHSA aliases, read from `mirror` (the
/// active rows the matching pass loaded, any package and any ecosystem). Sets
/// `severity_label` to the inherited grade and `severity_from` to the alias
/// that gave it; a match no alias grades is left unknown.
pub(crate) fn grade_unrated(matches: &mut [MatchedAdvisory], mirror: &[StoredAdvisory]) {
    let wanted: HashSet<String> = matches
        .iter()
        .filter(|m| is_unrated(m))
        .flat_map(|m| ghsa_aliases(&m.advisory_id, &m.aliases))
        .map(|id| id.to_ascii_uppercase())
        .collect();
    if wanted.is_empty() {
        return;
    }
    let grades = mirror_grades(mirror, &wanted);
    for m in matches.iter_mut().filter(|m| is_unrated(m)) {
        let read = |id: &str| grades.get(&id.to_ascii_uppercase()).copied();
        if let Some(graded) = grade_from_aliases(&m.advisory_id, &m.aliases, read) {
            m.severity_label = Some(graded.severity);
            m.severity_from = Some(graded.from);
        }
    }
}

/// The grade of each wanted id (upper-cased) in the mirror: the highest of its
/// rows, since one advisory is stored once per package it names.
fn mirror_grades(
    mirror: &[StoredAdvisory],
    wanted: &HashSet<String>,
) -> HashMap<String, &'static str> {
    let mut grades: HashMap<String, &'static str> = HashMap::new();
    for row in mirror {
        let key = row.advisory_id.trim().to_ascii_uppercase();
        if !wanted.contains(&key) {
            continue;
        }
        let Some(grade) = row_grade(row.cvss_score, row.severity_label.as_deref()) else {
            continue;
        };
        let slot = grades.entry(key).or_insert(grade);
        if tier_rank(grade) > tier_rank(slot) {
            *slot = grade;
        }
    }
    grades
}

/// A tier, and the GHSA alias it was inherited from (`None`: the record's own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graded {
    pub tier: &'static str,
    pub from: Option<String>,
}

impl Graded {
    /// `high`, or `high (from GHSA-w32m-9786-jp63)` when inherited.
    pub fn label(&self) -> String {
        labelled(self.tier, self.from.as_deref())
    }
}

/// `tier`, followed by the alias it came from when it was inherited.
pub fn labelled(tier: &str, from: Option<&str>) -> String {
    match from {
        Some(id) => format!("{tier} (from {id})"),
        None => tier.to_string(),
    }
}

/// The shared tier of one vulnerability cluster (the records of one alias
/// group): the MOST SEVERE own grade among its records, each record read as
/// [`row_grade`] reads it (its curated label, else a scorable CVSS band).
/// GHSA-pwjp-ccjc-ghwg (label LOW) and PYSEC-2026-52 (CVSS 9.8) are one bug,
/// CVE-2026-4277: critical. Only when no record has a grade of its own does
/// the cluster take an inherited one, naming the alias it came from (the
/// smallest id on a tie). `None` when nothing grades the cluster.
pub fn cluster_severity(cluster: &[&MatchedAdvisory]) -> Option<Graded> {
    let own = worst(
        cluster
            .iter()
            .filter(|m| m.severity_from.is_none())
            .filter_map(|m| row_grade(m.cvss_score, m.severity_label.as_deref()))
            .map(|tier| Graded { tier, from: None }),
    );
    own.or_else(|| {
        worst(cluster.iter().filter_map(|m| {
            Some(Graded {
                tier: canonical_tier(m.severity_label.as_deref()?)?,
                from: Some(m.severity_from.clone()?),
            })
        }))
    })
}

/// The most severe of several grades; inherited only when every grade at
/// that tier is inherited (the smallest alias id names it).
pub fn worst(grades: impl IntoIterator<Item = Graded>) -> Option<Graded> {
    let mut best: Option<Graded> = None;
    for g in grades {
        best = match best {
            None => Some(g),
            Some(b) if tier_rank(g.tier) > tier_rank(b.tier) => Some(g),
            Some(b) if tier_rank(g.tier) < tier_rank(b.tier) => Some(b),
            Some(b) => {
                let from = match (b.from, g.from) {
                    (Some(x), Some(y)) => Some(x.min(y)),
                    _ => None,
                };
                Some(Graded { tier: b.tier, from })
            }
        };
    }
    best
}

/// The worst grade across a package group's vulnerability clusters.
pub fn group_severity(group: &[&MatchedAdvisory]) -> Option<Graded> {
    worst(
        super::identity::cluster_by_vulnerability(group)
            .iter()
            .filter_map(|c| cluster_severity(c)),
    )
}

/// The base urgency (before the scope rule, AD-046) of one cluster, for the
/// Upgrade Plan AND the alert path. A graded cluster maps its tier (`low`
/// reads Watch). A cluster nothing grades, even through its aliases, is
/// Medium: a version-confirmed advisory, with no evidence to call it High or
/// to dismiss it to Watch. Two exceptions, each the source's own statement:
/// a cluster of maintenance notices only ([`is_informational`]: no fix, no
/// CVSS, "unmaintained" / "deprecated" text) is Watch, as the plan folds it;
/// a malicious-package record (`MAL-`, OpenSSF) is High.
pub fn cluster_urgency(cluster: &[&MatchedAdvisory]) -> Urgency {
    match cluster_severity(cluster).map(|g| g.tier) {
        Some("critical") => Urgency::Critical,
        Some("high") => Urgency::High,
        Some("medium") => Urgency::Medium,
        Some(_) => Urgency::Watch,
        None if cluster.iter().all(|m| is_informational(m)) => Urgency::Watch,
        None => unknown_urgency(cluster.iter().map(|m| m.advisory_id.as_str())),
    }
}

/// The still-unknown arm of [`cluster_urgency`], by the cluster's ids.
pub fn unknown_urgency<'a>(mut ids: impl Iterator<Item = &'a str>) -> Urgency {
    if ids.any(|id| {
        id.trim()
            .get(..4)
            .is_some_and(|p| p.eq_ignore_ascii_case("MAL-"))
    }) {
        Urgency::High
    } else {
        Urgency::Medium
    }
}

/// Maintenance-notice vocabulary (RustSec INFO advisories, npm deprecations).
const UNMAINTAINED_MARKERS: &[&str] = &[
    "unmaintained",
    "no longer maintained",
    "not maintained",
    "is deprecated",
    "has been deprecated",
    "deprecated crate",
    "abandoned",
    "archived",
];

/// A maintenance notice, not a vulnerability: no fixed version, no CVSS, and
/// the advisory text says so. An unsound-code or no-fix VULNERABILITY (rsa's
/// Marvin attack: CVSS 5.9, no fix) is NOT informational — it stays a step
/// that reads "No fix published". The Upgrade Plan folds these into one
/// Watch item, and [`cluster_urgency`] grades them Watch on the alert path.
pub(crate) fn is_informational(a: &MatchedAdvisory) -> bool {
    let no_fix = a
        .fixed_version
        .as_deref()
        .is_none_or(|v| v.trim().is_empty());
    if !no_fix || a.cvss_score.is_some() {
        return false;
    }
    let text = format!("{} {}", a.summary, a.details.as_deref().unwrap_or("")).to_lowercase();
    UNMAINTAINED_MARKERS.iter().any(|m| text.contains(m))
}

/// One sentence for an item whose grade is inherited, `None` otherwise:
/// the grade, where it came from, and that the record has none of its own.
pub fn inherited_note(graded: Option<&Graded>, group: &[&MatchedAdvisory]) -> Option<String> {
    let graded = graded?;
    let from = graded.from.as_deref()?;
    let records: Vec<&str> = group
        .iter()
        .filter(|m| m.severity_from.as_deref() == Some(from))
        .map(|m| m.advisory_id.as_str())
        .collect();
    let (subject, verb) = match records.as_slice() {
        [] => ("The advisory".to_string(), "carries no grade of its own"),
        [one] => ((*one).to_string(), "carries no grade of its own"),
        many => (many.join(", "), "carry no grade of their own"),
    };
    Some(format!(
        "Severity {}: {subject} {verb}, so the grade is that GHSA alias's.",
        graded.label()
    ))
}

#[cfg(test)]
#[path = "severity_tests.rs"]
mod tests;
