// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The safe releases a version constraint admits (single-engine C0.5): the
//! `range_check` an agent gets back when it asks about `"7.x"`, `"^1.20"`,
//! `"~=3.1"` or `"v1"` instead of an exact version.
//!
//! Why a constraint: an agent told "pin ws 7.x" writes the 7.x release it
//! remembers. In the G7 agent evaluation (2026-10-11) that was ws 7.5.10 and
//! body-parser 1.20.3, both affected by an advisory their next patch fixes.
//! Asked with the constraint, the check answers with the smallest and newest
//! release in it that no known advisory affects.
//!
//! The rule is `@4da/mcp-server` 6.2.x `checkRange`
//! (`src/tools/dependency-check-versions.ts`), ported pure: the host supplies
//! the package's releases (and the ones its registry withdrew) and its
//! advisories; nothing here reads a registry or OSV. Pinned by the shared
//! rule vectors `range_check.jsonl` (rc-001..rc-028).
//!
//! - Candidates are the installable releases (published, not withdrawn) the
//!   constraint admits ([`crate::constraint::constraint_admits`]), ascending;
//!   pre-releases only when the constraint admits no stable release.
//! - A release is safe when no advisory affects it. Maintenance notices never
//!   make a release unsafe. An advisory whose window cannot be decided for a
//!   release counts as affecting it (never read as clean) and is named in
//!   `unconfirmed`.
//! - The host's held-dependency verdicts (npm, audit 6.2.5 D-4: a release
//!   whose exact-pinned or narrowly ranged dependency is affected, or could
//!   not be judged) come in as `held_unsafe` and rule those releases out too.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::constraint::constraint_admits;
use crate::ecosystem::normalize;
use crate::matching::package_key;
use crate::osv::AdvisoryRecord;
use crate::range::check_version_affected;
use crate::registry::RegistryReleases;
use crate::version_order::OrderedVersion;

/// How many of the newest safe releases [`RangeCheck::safe_versions`] holds.
pub const SAFE_VERSIONS_SHOWN: usize = 8;

/// How many advisories [`RangeCheck::affecting_all`] names at most.
pub const AFFECTING_ALL_SHOWN: usize = 10;

/// One advisory on the package, as the rule reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeAdvisory {
    pub id: String,
    /// The merged ranges as stored JSON (`osv::serialize_ranges`); `None`
    /// is "no range information", which affects every release.
    #[serde(default)]
    pub ranges: Option<String>,
    /// A maintenance notice: never makes a release unsafe.
    #[serde(default)]
    pub informational: bool,
}

impl From<&AdvisoryRecord> for RangeAdvisory {
    fn from(row: &AdvisoryRecord) -> Self {
        Self {
            id: row.advisory_id.clone(),
            ranges: row.affected_ranges.clone(),
            informational: row.informational,
        }
    }
}

/// The stored rows that name one package (its key as
/// [`crate::matching::package_key`] forms it, in its OSV ecosystem), as the
/// rule reads them.
pub fn package_advisories(
    records: &[AdvisoryRecord],
    ecosystem: &str,
    name: &str,
) -> Vec<RangeAdvisory> {
    let (eco, key) = (normalize(ecosystem), package_key(name, ecosystem));
    records
        .iter()
        .filter(|r| {
            normalize(&r.ecosystem) == eco && package_key(&r.package_name, &r.ecosystem) == key
        })
        .map(RangeAdvisory::from)
        .collect()
}

/// What [`range_check`] is asked.
#[derive(Debug, Clone, Copy)]
pub struct RangeQuery<'a> {
    pub ecosystem: &'a str,
    pub constraint: &'a str,
    /// The version installed now: with it, the answer names the smallest
    /// safe release above it.
    pub from: Option<&'a str>,
    /// Every release the registry lists, and the ones it withdrew.
    pub registry: &'a RegistryReleases,
    pub advisories: &'a [RangeAdvisory],
    /// Releases the host's held-dependency check ruled out.
    pub held_unsafe: &'a [String],
}

/// The answer, in the MCP's `range_check` field names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeCheck {
    /// The constraint evaluated, as given.
    pub range: String,
    /// Installable releases the constraint admits (stable ones, unless it
    /// admits only pre-releases).
    pub releases_in_range: usize,
    /// How many of them are not safe.
    pub vulnerable_in_range: usize,
    pub smallest_safe_in_range: Option<String>,
    pub newest_safe_in_range: Option<String>,
    /// Up to [`SAFE_VERSIONS_SHOWN`] of the newest safe releases, ascending.
    pub safe_versions: Vec<String>,
    /// When nothing in range is safe: the advisories that affect every
    /// release in it (at most [`AFFECTING_ALL_SHOWN`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affecting_all: Option<Vec<String>>,
    /// When nothing in range is safe: the newest safe stable release of the
    /// package outside it (`null` when there is none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_safe_outside_range: Option<Option<String>>,
    /// With `from`: the smallest safe release above it (`null` when none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smallest_safe_upgrade: Option<Option<String>>,
    /// Withdrawn releases in range that no advisory affects and that sit
    /// below the pick (the smallest safe upgrade with `from`, else the
    /// smallest safe release; every one when nothing is safe), ascending.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withheld: Vec<String>,
    /// Releases otherwise safe that `held_unsafe` ruled out.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub held_excluded: usize,
    /// Advisories whose window could not be decided for some release in
    /// play: counted as affecting it, so that release is NOT CHECKED.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unconfirmed: Vec<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Why [`range_check`] gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeCheckError {
    /// The constraint is not one this ecosystem's syntax reads.
    UnreadableConstraint {
        ecosystem: String,
        constraint: String,
    },
}

impl std::fmt::Display for RangeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnreadableConstraint {
                ecosystem,
                constraint,
            } => write!(f, "{ecosystem} cannot read the constraint {constraint:?}"),
        }
    }
}

impl std::error::Error for RangeCheckError {}

/// A release, parsed once.
struct Release {
    text: String,
    order: OrderedVersion,
    prerelease: bool,
}

/// The advisory verdicts for one ecosystem, with the undecided ones noted.
struct Judge<'a> {
    ecosystem: &'a str,
    advisories: &'a [RangeAdvisory],
    unconfirmed: std::cell::RefCell<BTreeSet<String>>,
}

impl Judge<'_> {
    /// The ids of the advisories (maintenance notices left out) that affect
    /// `version`, in input order, deduplicated.
    fn affecting(&self, version: &str) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        for a in self.advisories.iter().filter(|a| !a.informational) {
            let (affected, confirmed) =
                check_version_affected(self.ecosystem, Some(version), &a.ranges);
            if affected && !confirmed {
                self.unconfirmed.borrow_mut().insert(a.id.clone());
            }
            if affected && !ids.contains(&a.id.as_str()) {
                ids.push(a.id.as_str());
            }
        }
        ids
    }

    fn is_safe(&self, version: &str) -> bool {
        self.affecting(version).is_empty()
    }
}

/// The releases a constraint admits, judged against every advisory the host
/// gives for the package. An unreadable constraint is an error, never an
/// empty range.
pub fn range_check(query: &RangeQuery<'_>) -> Result<RangeCheck, RangeCheckError> {
    let eco = query.ecosystem;
    let admits = |v: &str| constraint_admits(eco, query.constraint, v) == Some(true);
    if constraint_admits(eco, query.constraint, probe(eco)).is_none() {
        return Err(RangeCheckError::UnreadableConstraint {
            ecosystem: eco.to_string(),
            constraint: query.constraint.to_string(),
        });
    }
    let installable = releases(eco, &query.registry.published, |v| {
        !query.registry.excludes(eco, v)
    });
    let admitted: Vec<&Release> = installable.iter().filter(|r| admits(&r.text)).collect();
    let stable: Vec<&Release> = admitted.iter().copied().filter(|r| !r.prerelease).collect();
    let any_stable = !stable.is_empty();
    let candidates = if any_stable { stable } else { admitted };
    let judge = Judge {
        ecosystem: eco,
        advisories: query.advisories,
        unconfirmed: std::cell::RefCell::default(),
    };
    let held = |v: &str| query.held_unsafe.iter().any(|h| same(eco, h, v));
    let own_safe: Vec<&Release> = candidates
        .iter()
        .copied()
        .filter(|r| judge.is_safe(&r.text))
        .collect();
    let safe: Vec<&Release> = own_safe
        .iter()
        .copied()
        .filter(|r| !held(&r.text))
        .collect();
    let text = |r: &&Release| r.text.clone();
    let mut out = RangeCheck {
        range: query.constraint.to_string(),
        releases_in_range: candidates.len(),
        vulnerable_in_range: candidates.len() - safe.len(),
        smallest_safe_in_range: safe.first().map(text),
        newest_safe_in_range: safe.last().map(text),
        safe_versions: safe[safe.len().saturating_sub(SAFE_VERSIONS_SHOWN)..]
            .iter()
            .map(text)
            .collect(),
        held_excluded: own_safe.len() - safe.len(),
        ..RangeCheck::default()
    };
    if !candidates.is_empty() && safe.is_empty() {
        out.affecting_all = Some(affecting_all(&judge, &candidates));
        let outside = installable
            .iter()
            .rfind(|r| !r.prerelease && !held(&r.text) && judge.is_safe(&r.text));
        out.newest_safe_outside_range = Some(outside.map(|r| r.text.clone()));
    }
    // `Some(None)`: a `from` that does not read as a version, above which
    // nothing can be named.
    let from = query.from.map(|f| OrderedVersion::parse_in(f, eco));
    let above_from = |r: &Release| match &from {
        None => true,
        Some(None) => false,
        Some(Some(f)) => r.order.compare(f) == Some(Ordering::Greater),
    };
    if from.is_some() {
        out.smallest_safe_upgrade = Some(safe.iter().find(|r| above_from(r)).map(text));
    }
    let pick = match &out.smallest_safe_upgrade {
        Some(upgrade) => upgrade.clone(),
        None => out.smallest_safe_in_range.clone(),
    };
    let pick = pick.and_then(|p| OrderedVersion::parse_in(&p, eco));
    let withdrawn = releases(eco, &query.registry.excluded, |_| true);
    out.withheld = withdrawn
        .iter()
        .filter(|r| admits(&r.text) && (!any_stable || !r.prerelease) && above_from(r))
        .filter(|r| {
            pick.as_ref()
                .is_none_or(|p| r.order.compare(p) == Some(Ordering::Less))
        })
        .filter(|r| judge.is_safe(&r.text))
        .map(|r| r.text.clone())
        .collect();
    out.unconfirmed = judge.unconfirmed.into_inner().into_iter().collect();
    Ok(out)
}

/// A version every ecosystem reads, to tell an unreadable constraint from
/// one that admits nothing.
fn probe(ecosystem: &str) -> &'static str {
    if crate::ecosystem::osv_name(ecosystem) == Some("Go") {
        "v1.0.0"
    } else {
        "1.0.0"
    }
}

/// `versions` (deduplicated as versions, unreadable ones dropped) that pass
/// `keep`, ascending.
fn releases(ecosystem: &str, versions: &[String], keep: impl Fn(&str) -> bool) -> Vec<Release> {
    let mut out: Vec<Release> = versions
        .iter()
        .filter(|v| keep(v))
        .filter_map(|v| {
            let order = OrderedVersion::parse_in(v, ecosystem)?;
            Some(Release {
                text: v.clone(),
                prerelease: is_prerelease(ecosystem, v, &order),
                order,
            })
        })
        .collect();
    out.sort_by(|a, b| a.order.compare(&b.order).unwrap_or(Ordering::Equal));
    out.dedup_by(|a, b| a.order.compare(&b.order) == Some(Ordering::Equal));
    out
}

/// A pre-release, a PEP 440 dev release, or a Go pseudo-version.
fn is_prerelease(ecosystem: &str, text: &str, order: &OrderedVersion) -> bool {
    order.is_prerelease()
        || (crate::ecosystem::osv_name(ecosystem) == Some("Go") && is_pseudo_version(text))
}

/// Go's `vX.Y.Z-yyyymmddhhmmss-abcdefabcdef` (and its `.0.` variants): 14
/// digits, a dash, 12 hex digits.
fn is_pseudo_version(text: &str) -> bool {
    let b = text.as_bytes();
    b.windows(27).any(|w| {
        w[..14].iter().all(u8::is_ascii_digit)
            && w[14] == b'-'
            && w[15..]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
    })
}

/// Equal as versions in `ecosystem`, or the same text.
fn same(ecosystem: &str, a: &str, b: &str) -> bool {
    a == b
        || OrderedVersion::parse_in(a, ecosystem)
            .zip(OrderedVersion::parse_in(b, ecosystem))
            .and_then(|(x, y)| x.compare(&y))
            == Some(Ordering::Equal)
}

/// The advisories that affect every candidate, in first-seen order.
fn affecting_all(judge: &Judge<'_>, candidates: &[&Release]) -> Vec<String> {
    let per: Vec<Vec<&str>> = candidates
        .iter()
        .map(|r| judge.affecting(&r.text))
        .collect();
    let mut seen: Vec<&str> = Vec::new();
    for id in per.iter().flatten() {
        if !seen.contains(id) && per.iter().all(|ids| ids.contains(id)) {
            seen.push(id);
        }
    }
    seen.into_iter()
        .take(AFFECTING_ALL_SHOWN)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "range_check_tests.rs"]
mod tests;
