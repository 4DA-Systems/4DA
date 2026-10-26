// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One severity per advisory, on every surface and in both engines.
//!
//! - [`row_grade`]: one record's OWN grade (curated label, else CVSS band).
//! - [`cluster_grade`]: one alias group's grade: the most severe own grade,
//!   and only when no record has one, the highest grade among the records'
//!   GHSA aliases in the local mirror (the engine crate's
//!   `fourda_depengine::severity`, shared with `@4da/mcp-server`'s
//!   `gradeFromAliases`), naming the alias it came from. It is written once,
//!   over [`SeverityRecord`], so the matcher's rows (Upgrade Plan, Preemption,
//!   Brief) and the mirror's own rows (Knowledge Gaps, the scorer's security
//!   lane) are graded by the same code.
//! - [`AliasGrades`]: the mirror's GHSA grades, read in ONE query per pass
//!   (the matching pass derives it from the rows it already holds; the scorer
//!   reads a copy the database keeps until the mirror is written), never per
//!   advisory, and never from the network.
//! - [`cluster_urgency`]: the ONE base urgency the Upgrade Plan and the alert
//!   path both grade a cluster by, including the rule for a cluster that is
//!   still unknown. `vectors/severity.jsonl` pins it all.

use std::collections::HashMap;

use fourda_depengine::severity::{ghsa_aliases, grade_from_aliases};

use crate::evidence::Urgency;

use super::types::{cvss_band, MatchedAdvisory, StoredAdvisory};

pub use fourda_depengine::severity::tier_rank;

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

/// The summary half of the maintenance-notice predicate
/// (`fourda_depengine::severity::is_maintenance_notice`, the MCP's rule): the
/// summary says `unmaintained` or `no longer maintained` as words. The scorer
/// reads it over item titles; an advisory row reads [`is_notice`].
pub(crate) fn maintenance_text(summary: &str) -> bool {
    fourda_depengine::severity::is_maintenance_notice(summary, None)
}

/// THE maintenance-notice predicate for a mirror row or a match: the verdict
/// the sync stored (`stored`, the engine crate's `osv::is_notice_for`, where
/// a class OSV states decides on its own: `unmaintained` / `notice` is a
/// notice, `unsound` is not, whatever the summary says), else, for a row
/// synced before the verdict was stored, the summary ([`maintenance_text`]). A notice is never graded from an alias, the Upgrade Plan
/// folds it into one Watch item, [`cluster_urgency`] grades it Watch on the
/// alert path, and the fix-target walk never lets it block or be a target.
pub(crate) fn is_notice(stored: Option<bool>, summary: &str) -> bool {
    stored.unwrap_or_else(|| maintenance_text(summary))
}

/// A record whose notice verdict the alias-group rule can settle.
pub trait NoticeVerdict: SeverityRecord {
    fn settle_not_notice(&mut self);
}

impl NoticeVerdict for MatchedAdvisory {
    fn settle_not_notice(&mut self) {
        self.informational = Some(false);
    }
}

impl NoticeVerdict for StoredAdvisory {
    fn settle_not_notice(&mut self) {
        self.informational = Some(false);
    }
}

/// THE alias-group notice rule, both engines' (6.2.5 audit F-7, with the
/// MCP's `advisory-groups.ts`): records linked by id or alias
/// ([`alias_groups`]) are one bug, and the group is a maintenance notice ONLY
/// when every record in it is. One vulnerability record makes every record of
/// its group a vulnerability, so no surface reads a member as a notice:
/// serde_yml's GHSA-hhw4-xg65-fp2x states no class and its summary says
/// "unsound and unmaintained" (a notice by wording), while its alias
/// RUSTSEC-2025-0068 states `unsound`. Every mirror read that returns rows
/// applies this to the rows it returns.
pub fn settle_group_notices<R: NoticeVerdict>(rows: &mut [R]) {
    let mixed: Vec<usize> = {
        let refs: Vec<&R> = rows.iter().collect();
        alias_group_indices(&refs)
            .into_iter()
            .filter(|g| g.iter().any(|&i| !refs[i].is_informational()))
            .flatten()
            .filter(|&i| refs[i].is_informational())
            .collect()
    };
    for i in mixed {
        rows[i].settle_not_notice();
    }
}

/// What a grader reads from one advisory record, whichever table it came from.
pub trait SeverityRecord {
    fn record_id(&self) -> &str;
    fn record_aliases(&self) -> &[String];
    /// The record's own grade ([`row_grade`]); `None` when it has none.
    fn own_grade(&self) -> Option<&'static str>;
    /// A maintenance notice ([`is_notice`]).
    fn is_informational(&self) -> bool;
    /// A grade already inherited for this record (the matcher stores it on
    /// the match so every surface can name it); `None` otherwise.
    fn inherited(&self) -> Option<Graded> {
        None
    }
}

impl SeverityRecord for MatchedAdvisory {
    fn record_id(&self) -> &str {
        &self.advisory_id
    }
    fn record_aliases(&self) -> &[String] {
        &self.aliases
    }
    fn own_grade(&self) -> Option<&'static str> {
        // An inherited label is not the record's own.
        self.severity_from
            .is_none()
            .then(|| row_grade(self.cvss_score, self.severity_label.as_deref()))
            .flatten()
    }
    fn is_informational(&self) -> bool {
        is_notice(self.informational, &self.summary)
    }
    fn inherited(&self) -> Option<Graded> {
        Some(Graded {
            tier: canonical_tier(self.severity_label.as_deref()?)?,
            from: Some(self.severity_from.clone()?),
        })
    }
}

impl SeverityRecord for StoredAdvisory {
    fn record_id(&self) -> &str {
        &self.advisory_id
    }
    fn record_aliases(&self) -> &[String] {
        &self.aliases
    }
    fn own_grade(&self) -> Option<&'static str> {
        row_grade(self.cvss_score, self.severity_label.as_deref())
    }
    fn is_informational(&self) -> bool {
        is_notice(self.informational, &self.summary)
    }
}

/// The record may take an alias's grade: it has none of its own and is not a
/// maintenance notice. The ONE predicate (S2 of the #986 review).
pub fn takes_alias_grade(r: &impl SeverityRecord) -> bool {
    r.own_grade().is_none() && !r.is_informational()
}

/// The mirror's grade of every GHSA id it holds graded: the highest of its
/// rows (one advisory is stored once per package it names), keyed upper-case.
#[derive(Debug, Clone, Default)]
pub struct AliasGrades(HashMap<String, &'static str>);

impl AliasGrades {
    pub fn get(&self, id: &str) -> Option<&'static str> {
        self.0.get(&id.trim().to_ascii_uppercase()).copied()
    }

    fn insert(&mut self, id: &str, grade: &'static str) {
        let slot = self
            .0
            .entry(id.trim().to_ascii_uppercase())
            .or_insert(grade);
        if tier_rank(grade) > tier_rank(slot) {
            *slot = grade;
        }
    }

    /// From mirror rows already read (the matching pass): no query.
    pub(crate) fn from_rows(rows: &[StoredAdvisory]) -> Self {
        let mut grades = Self::default();
        for row in rows.iter().filter(|r| is_ghsa(&r.advisory_id)) {
            if let Some(grade) = row.own_grade() {
                grades.insert(&row.advisory_id, grade);
            }
        }
        grades
    }

    /// ONE query: every active, graded GHSA row of the mirror.
    pub(crate) fn load(conn: &rusqlite::Connection) -> Self {
        let mut grades = Self::default();
        let Ok(mut stmt) = conn.prepare(
            "SELECT advisory_id, cvss_score, severity_label FROM osv_advisories
             WHERE withdrawn_at IS NULL AND advisory_id LIKE 'GHSA-%'
               AND (severity_label IS NOT NULL OR cvss_score > 0)",
        ) else {
            return grades;
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        });
        if let Ok(rows) = rows {
            for (id, cvss, label) in rows.flatten() {
                if let Some(grade) = row_grade(cvss, label.as_deref()) {
                    grades.insert(&id, grade);
                }
            }
        }
        grades
    }
}

fn is_ghsa(id: &str) -> bool {
    id.trim()
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("GHSA-"))
}

/// The grade a record takes from its GHSA aliases, when it may take one.
fn alias_grade(r: &impl SeverityRecord, grades: &AliasGrades) -> Option<Graded> {
    if !takes_alias_grade(r) {
        return None;
    }
    r.inherited().or_else(|| {
        let g = grade_from_aliases(r.record_id(), r.record_aliases(), |id| grades.get(id))?;
        Some(Graded {
            tier: canonical_tier(&g.severity)?,
            from: Some(g.from),
        })
    })
}

/// Grade every unrated match from its GHSA aliases in `mirror` (the active
/// rows the matching pass loaded, any package and any ecosystem). Sets
/// `severity_label` to the inherited grade and `severity_from` to the alias
/// that gave it; a match no alias grades is left unknown.
pub(crate) fn grade_unrated(matches: &mut [MatchedAdvisory], mirror: &[StoredAdvisory]) {
    if !matches
        .iter()
        .any(|m| takes_alias_grade(m) && !ghsa_aliases(&m.advisory_id, &m.aliases).is_empty())
    {
        return;
    }
    let grades = AliasGrades::from_rows(mirror);
    for m in matches.iter_mut() {
        if let Some(g) = alias_grade(&*m, &grades) {
            m.severity_label = Some(g.tier.to_string());
            m.severity_from = g.from;
        }
    }
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

/// THE grade of one alias group, for every surface: the MOST SEVERE own
/// grade among its records ([`row_grade`]). GHSA-pwjp-ccjc-ghwg (label LOW)
/// and PYSEC-2026-52 (CVSS 9.8) are one bug, CVE-2026-4277: critical. Only
/// when no record has a grade of its own does the group take an inherited
/// one, naming the alias it came from (the smallest id on a tie). `None`
/// when nothing grades it.
pub fn cluster_grade<R: SeverityRecord>(cluster: &[&R], grades: &AliasGrades) -> Option<Graded> {
    let own = worst(
        cluster
            .iter()
            .filter_map(|r| r.own_grade())
            .map(|tier| Graded { tier, from: None }),
    );
    own.or_else(|| worst(cluster.iter().filter_map(|r| alias_grade(*r, grades))))
}

/// [`cluster_grade`] for a matcher cluster, whose inherited grades the
/// matching pass already stored on the matches.
pub fn cluster_severity(cluster: &[&MatchedAdvisory]) -> Option<Graded> {
    cluster_grade(cluster, &AliasGrades::default())
}

/// Mirror rows grouped by alias (union of ids and aliases), first-seen order.
pub fn alias_groups<'a, R: SeverityRecord>(rows: &[&'a R]) -> Vec<Vec<&'a R>> {
    alias_group_indices(rows)
        .into_iter()
        .map(|g| g.into_iter().map(|i| rows[i]).collect())
        .collect()
}

/// [`alias_groups`] as indices into `rows`.
fn alias_group_indices<R: SeverityRecord>(rows: &[&R]) -> Vec<Vec<usize>> {
    let mut group_of: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let keys: Vec<String> = std::iter::once(row.record_id())
            .chain(row.record_aliases().iter().map(String::as_str))
            .map(|k| k.trim().to_ascii_uppercase())
            .filter(|k| !k.is_empty())
            .collect();
        let mut hits: Vec<usize> = keys
            .iter()
            .filter_map(|k| group_of.get(k).copied())
            .collect();
        hits.sort_unstable();
        hits.dedup();
        let target = match hits.first() {
            Some(&first) => {
                // Merge every other group this row joins into the first.
                for &other in hits.iter().skip(1).rev() {
                    let moved = std::mem::take(&mut groups[other]);
                    groups[first].extend(moved);
                }
                first
            }
            None => {
                groups.push(Vec::new());
                groups.len() - 1
            }
        };
        groups[target].push(index);
        for g in groups[target].iter().map(|&i| rows[i]) {
            for k in
                std::iter::once(g.record_id()).chain(g.record_aliases().iter().map(String::as_str))
            {
                group_of.insert(k.trim().to_ascii_uppercase(), target);
            }
        }
    }
    groups.into_iter().filter(|g| !g.is_empty()).collect()
}

/// The worst grade across mirror rows of possibly several advisories: each
/// alias group graded by [`cluster_grade`]. Knowledge Gaps and the scorer's
/// security lane read this.
pub fn rows_grade<R: SeverityRecord>(rows: &[&R], grades: &AliasGrades) -> Option<Graded> {
    worst(
        alias_groups(rows)
            .iter()
            .filter_map(|g| cluster_grade(g, grades)),
    )
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
/// a cluster of maintenance notices only ([`SeverityRecord::is_informational`])
/// is Watch, as the plan folds it; a malicious-package record (`MAL-`,
/// OpenSSF) on the installed version is Critical whatever else grades it:
/// malware is installed (recovery-lead, 2026-10-18). npm's
/// `0.0.<n>-security` holding placeholders never reach here; they are not
/// affected (`is_npm_security_holding`, audit L4).
pub fn cluster_urgency(cluster: &[&MatchedAdvisory]) -> Urgency {
    if contains_malware(cluster) {
        return Urgency::Critical;
    }
    match cluster_severity(cluster).map(|g| g.tier) {
        Some("critical") => Urgency::Critical,
        Some("high") => Urgency::High,
        Some("medium") => Urgency::Medium,
        Some(_) => Urgency::Watch,
        None if cluster.iter().all(|m| m.is_informational()) => Urgency::Watch,
        // Installed malware returned Critical above.
        None => Urgency::Medium,
    }
}

/// Installed malware in this group: it is Critical on every surface and the
/// AD-046 scope rule does not lower it. A dev-only or transitive malicious
/// package still runs its code (install scripts, the build, the test run) on
/// the developer's machine, so it is not the milder exposure the scope rule
/// discounts for vulnerabilities. Malware is the ONE shared predicate,
/// `fourda_depengine::is_malware` (via `MatchedAdvisory::is_malware`): a
/// `MAL-` id or alias, CWE-506 in `database_specific.cwe_ids` (crossenv
/// GHSA-c2m4-w5hm-vqjw has no MAL alias), or GitHub's malware title
/// ("Malicious Package in …", GHSA-28f8-hqmc-7ph8).
pub fn contains_malware(group: &[&MatchedAdvisory]) -> bool {
    group.iter().any(|m| m.is_malware())
}

/// A maintenance notice (the Upgrade Plan's informational fold).
pub(crate) fn is_informational(a: &MatchedAdvisory) -> bool {
    a.is_informational()
}

/// How every surface words a maintenance notice: never as a vulnerability,
/// never with "affects" or a severity (#986). `unmaintained: zvariant
/// (RUSTSEC-2026-0370), no exploitable vulnerability`.
pub fn notice_wording(package: &str, ids: &[&str]) -> String {
    if ids.is_empty() {
        format!("unmaintained: {package}, no exploitable vulnerability")
    } else {
        format!(
            "unmaintained: {package} ({}), no exploitable vulnerability",
            ids.join(", ")
        )
    }
}

/// A group of matches that are all maintenance notices.
pub fn is_notice_group(group: &[&MatchedAdvisory]) -> bool {
    !group.is_empty() && group.iter().all(|m| m.is_informational())
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
