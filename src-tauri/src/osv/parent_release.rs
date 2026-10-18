// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The parent release that fixes a nested advisory (readiness audit H1).
//!
//! A vulnerable TRANSITIVE copy whose parent pins it can only move when the
//! parent does. The Upgrade Plan said "upgrade express (check that it admits
//! path-to-regexp >= 0.1.13)" and stopped; the registry already knew the
//! answer: express 4.22.0 requires `~0.1.12`, which resolves to the clean
//! 0.1.13. This module names that release, from the registry's own per-release
//! requirements (`osv::release_facts`), by the rules `@4da/mcp-server`
//! applies (`upgrade-planner-fixpaths.ts`, fix/planner-advice-625 1840dab):
//!
//! - **(a)** the SMALLEST parent release above the installed one whose own
//!   requirement on the package resolves (its newest admitted release, what a
//!   fresh resolve picks) to a version at or above the target that no
//!   advisory with a published fix affects;
//! - **(b)** that release is itself clear of the parent's own advisories, and
//!   when the parent has its own vulnerable row the two agree: the row's
//!   target is raised to the highest release any transitive row needs from it
//!   (unless that crosses a major its own target does not, or an advisory of
//!   the parent affects it), and a row whose smallest release is lower is
//!   lifted to that target when it still admits a clean copy there — one
//!   release, one command;
//! - **(c)** when the new requirement still admits the installed vulnerable
//!   copy, an install keeps it (npm 10.9.2, 2026-10-17: `npm install
//!   express@4.22.0` kept path-to-regexp 0.1.12; `npm update path-to-regexp`
//!   then moved it to 0.1.13), so the advice names the refresh after it;
//! - **(f)** a move that crosses a major version is flagged.
//!
//! Rules (d) (no command for a parent the project's declarations cannot be
//! tied to) and (e) (commands through `osv::direct_upgrade`, the declared
//! operator kept) are the caller's: this module is pure and names no command.
//!
//! The shared rule vectors (`tests/conformance/vectors/parent_release.jsonl`)
//! pin it against live registry and OSV data, NodeGoat's express 4.16.4 among
//! them.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use fourda_depengine::range::check_version_affected;
use fourda_depengine::version_order::OrderedVersion;
use serde::{Deserialize, Serialize};

use super::fix_path::{requirement_admits, Dialect, Requirement};

/// What one release of a parent requires of one package: its requirement(s)
/// as written (empty when that release does not depend on it), and whether
/// it declares the package only as a peer (npm 7+ installs a missing peer, so
/// moving a package into `peerDependencies` is not dropping it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReq {
    #[serde(default, rename = "r", skip_serializing_if = "Vec::is_empty")]
    pub reqs: Vec<String>,
    #[serde(default, rename = "p", skip_serializing_if = "std::ops::Not::not")]
    pub peer: bool,
}

/// Per published release of a parent: what it requires of one package.
pub type RequirementHistory = BTreeMap<String, ReleaseReq>;

/// What the registry says about one package's releases, as the walk reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Releases {
    /// Installable releases (not withdrawn), any order; pre-releases are
    /// skipped by every reader.
    pub versions: Vec<String>,
    /// The registry's `latest` release, when it names one.
    pub latest: Option<String>,
    /// The `latest` release is deprecated: the package is retired whole.
    pub deprecated: bool,
}

/// One advisory as the walk reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    pub id: String,
    /// A malicious-package record (`fourda_depengine::is_malware`): a release it
    /// affects is never clean, never a target and never "the newest".
    pub malware: bool,
    /// Stored ranges JSON; `None` or unparseable makes every decision on the
    /// package unknown.
    pub ranges: Option<String>,
}

impl Advisory {
    /// From a stored id, its aliases, its CWE ids, its summary and its
    /// ranges; malware per `fourda_depengine::is_malware`.
    pub fn new(
        id: &str,
        aliases: &[String],
        cwe_ids: &[String],
        summary: &str,
        ranges: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            malware: fourda_depengine::is_malware(id, aliases, cwe_ids, summary),
            ranges,
        }
    }

    /// The ranges cannot be read: no data, unparseable, or empty.
    fn unreadable(&self) -> bool {
        self.ranges
            .as_deref()
            .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
            .is_none_or(|r| r.is_empty())
    }
}

pub use super::parent_release_hop::{through_release, Hop, HopParent};

/// One package's known advisories.
pub struct Advisories<'a> {
    pub ecosystem: &'a str,
    pub list: Vec<Advisory>,
}

/// THE predicate every reader shares (review of #981, B2; parity decision
/// 2): whether a release can be named. A malicious-package record that
/// affects it rules it out outright; so does ANY advisory the installed
/// version does not have (a move never introduces one, fixed or not); an
/// advisory the installed version already has rules it out when it has a
/// published fix (it does not also affect the package's newest
/// non-malicious release); one no release fixes does not (it cannot block a
/// move that fixes the others), and the release named says it remains
/// affected by it.
impl<'a> Advisories<'a> {
    /// From the advisories 4DA stores for one package.
    pub fn stored(ecosystem: &'a str, stored: Vec<super::types::StoredAdvisory>) -> Self {
        Self {
            ecosystem,
            list: stored
                .into_iter()
                .map(|a| {
                    Advisory::new(
                        &a.advisory_id,
                        &a.aliases,
                        &a.cwe_ids,
                        &a.summary,
                        a.affected_ranges,
                    )
                })
                .collect(),
        }
    }

    fn hits(&self, a: &Advisory, version: &str) -> bool {
        check_version_affected(self.ecosystem, Some(version), &a.ranges).0
    }

    /// Every advisory's stored ranges, for the fix-target walk.
    pub fn ranges(&self) -> Vec<&Option<String>> {
        self.list.iter().map(|a| &a.ranges).collect()
    }

    /// Some advisory affects `version`.
    pub fn affect(&self, version: &str) -> bool {
        self.list.iter().any(|a| self.hits(a, version))
    }

    /// The malicious-package record that affects `version`, if any.
    pub fn malware_at(&self, version: &str) -> Option<&str> {
        self.list
            .iter()
            .find(|a| a.malware && self.hits(a, version))
            .map(|a| a.id.as_str())
    }

    /// An advisory whose ranges cannot be read: then nothing can be claimed
    /// clean, and no release is named.
    pub fn unreadable(&self) -> Option<&str> {
        self.list
            .iter()
            .find(|a| a.unreadable())
            .map(|a| a.id.as_str())
    }

    /// The highest stable installable release no malicious-package record
    /// affects: the release an advisory must also affect to count as having
    /// no fix.
    pub fn newest(&self, releases: &Releases) -> Option<String> {
        ascending(&releases.versions, self.ecosystem)
            .into_iter()
            .rev()
            .find(|v| self.malware_at(v).is_none())
            .cloned()
    }

    /// Whether `version` can be named (see the predicate above).
    pub fn usable(&self, version: &str, newest: Option<&str>, installed: Option<&str>) -> bool {
        self.list.iter().all(|a| {
            if !self.hits(a, version) {
                return true;
            }
            // Never introduce an advisory (parity decision 2): one the
            // installed version does not have rules the release out, fixed
            // or not.
            if installed.is_some_and(|i| !self.hits(a, i)) {
                return false;
            }
            !a.malware && newest.is_some_and(|n| self.hits(a, n))
        })
    }

    /// The non-malicious advisories that affect `version` but not
    /// `installed`: what a move to it would bring in (the MCP's
    /// `introducedAt`).
    pub fn introduced_at(&self, version: &str, installed: &str) -> Vec<String> {
        self.list
            .iter()
            .filter(|a| !a.malware && self.hits(a, version) && !self.hits(a, installed))
            .map(|a| a.id.clone())
            .collect()
    }

    /// [`Self::usable`], except that advisories `installed` does not have are
    /// ignored, fixed or not (the MCP's `cleanButIntroduced`): whether only
    /// those block `version`. A malicious record affecting it still does.
    pub fn clean_but_introduced(
        &self,
        version: &str,
        newest: Option<&str>,
        installed: &str,
    ) -> bool {
        self.malware_at(version).is_none()
            && self.list.iter().all(|a| {
                !self.hits(a, version)
                    || !self.hits(a, installed)
                    || newest.is_some_and(|n| self.hits(a, n))
            })
    }

    /// Of `candidates`, the ones only advisories `installed` lacks block:
    /// the union of what they bring in, sorted and deduped (review of #994,
    /// S4). Empty when none.
    pub fn introduced_union<'v>(
        &self,
        candidates: impl Iterator<Item = &'v str>,
        newest: Option<&str>,
        installed: &str,
    ) -> Vec<String> {
        let mut out: Vec<String> = candidates
            .filter(|v| !self.usable(v, newest, Some(installed)))
            .filter(|v| self.clean_but_introduced(v, newest, installed))
            .flat_map(|v| self.introduced_at(v, installed))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The advisories no release fixes that still affect `version`.
    pub fn unfixed_at(&self, version: &str, newest: Option<&str>) -> Vec<String> {
        self.list
            .iter()
            .filter(|a| !a.malware && self.hits(a, version))
            .filter(|a| newest.is_some_and(|n| self.hits(a, n)))
            .map(|a| a.id.clone())
            .collect()
    }

    /// Some advisory has a way out (does not affect `newest`).
    pub fn any_fixable(&self, newest: Option<&str>) -> bool {
        self.list
            .iter()
            .any(|a| !newest.is_some_and(|n| self.hits(a, n)))
    }
}

/// The vulnerable copy, from the side of the package that must reach a clean
/// version.
pub struct Child<'a> {
    pub ecosystem: &'a str,
    pub installed: &'a str,
    pub target: &'a str,
    /// The package's installable releases; empty when unknown (then a
    /// requirement must admit the target itself).
    pub releases: &'a [String],
    /// No advisory with a published fix affects the version.
    pub clean: &'a dyn Fn(&str) -> bool,
}

/// A parent release that admits a clean copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub version: String,
    /// Its requirement on the package; `None` when it no longer depends on it.
    pub requirement: Option<String>,
    pub peer: bool,
    /// What a fresh resolve of that requirement picks, when the package's
    /// releases are known.
    pub resolves_to: Option<String>,
    /// The requirement still admits the installed vulnerable copy: an
    /// install keeps it, so a refresh must follow.
    pub keeps_installed: bool,
}

/// What can be said about one blocking parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// `found` is the smallest admitting release (before any agreement with
    /// the parent's own row).
    Release(Found),
    /// The parent's latest release is deprecated: it is retired whole.
    Deprecated,
    /// The installed parent is already its latest release.
    Latest,
    /// Every release was read and none above the installed one admits a
    /// clean copy while being clear of the parent's own advisories.
    NoneAdmits { latest: Option<String> },
    /// The parent's per-release requirements were not read.
    Unread,
    /// Nothing can be named: an advisory's ranges cannot be read (the why).
    Undecided(String),
    /// The installed parent is a release a malicious-package record affects
    /// (its id): it is removed or replaced, never moved.
    Malicious(String),
    /// A release admits a clean copy, but every such release brings in an
    /// advisory the installed parent does not have (these ids): none is named.
    Introduces(Vec<String>),
}

fn parse(v: &str, eco: &str) -> Option<OrderedVersion> {
    OrderedVersion::parse_in(v, eco)
}

/// `a` vs `b` as versions in `eco`; `None` when either cannot be ordered.
pub fn compare(a: &str, b: &str, eco: &str) -> Option<Ordering> {
    parse(a, eco)?.compare(&parse(b, eco)?)
}

fn stable(v: &str, eco: &str) -> bool {
    parse(v, eco).is_some_and(|p| !p.is_prerelease())
}

/// Sorted ascending, stable releases only.
pub(super) fn ascending<'a>(versions: &'a [String], eco: &str) -> Vec<&'a String> {
    let mut out: Vec<&String> = versions.iter().filter(|v| stable(v, eco)).collect();
    out.sort_by(|a, b| compare(a, b, eco).unwrap_or(Ordering::Equal));
    out
}

/// The highest stable installable release (the MCP's `newestOf`): the release
/// an advisory must also affect to count as having no fix. The `latest` tag
/// is not used here; a registry can tag an older line `latest`.
pub fn highest(releases: &Releases, eco: &str) -> Option<String> {
    ascending(&releases.versions, eco)
        .last()
        .map(|v| (*v).clone())
}

/// The release "already its latest" is judged against: the `latest` tag,
/// else the highest stable installable release.
pub(super) fn latest_of(releases: &Releases, eco: &str) -> Option<String> {
    releases.latest.clone().or_else(|| highest(releases, eco))
}

/// A move from `from` to `to` crosses a major version (a `0.x` minor counts).
pub fn is_major(from: &str, to: &str) -> bool {
    super::fix_target::upgrade_type(from, to) == Some(super::fix_target::UpgradeType::Major)
}

pub(super) fn admits(req: &str, version: &str, dialect: Dialect) -> Option<bool> {
    match dialect {
        Dialect::Npm => requirement_admits(req, version),
        Dialect::Cargo => Requirement::cargo(req).admits(version),
    }
}

/// Whether a parent release's requirements admit a clean copy: every
/// requirement's newest match is at or above the target and clean (without
/// the package's release list, the requirement must admit the target).
/// `None` when they do not; a release with no requirement admits trivially.
pub fn admits_clean(
    child: &Child<'_>,
    reqs: &[String],
    dialect: Dialect,
) -> Option<(Option<String>, bool)> {
    let eco = child.ecosystem;
    let mut resolves_to: Option<String> = None;
    let mut keeps = false;
    for req in reqs {
        if child.releases.is_empty() {
            if admits(req, child.target, dialect) != Some(true) {
                return None;
            }
        } else {
            let pick = ascending(child.releases, eco)
                .into_iter()
                .rev()
                .find(|v| admits(req, v, dialect) == Some(true))?;
            if compare(pick, child.target, eco)? == Ordering::Less || !(child.clean)(pick) {
                return None;
            }
            let lower = resolves_to
                .as_deref()
                .is_none_or(|r| compare(pick, r, eco) == Some(Ordering::Less));
            if lower {
                resolves_to = Some(pick.clone());
            }
        }
        keeps |= admits(req, child.installed, dialect) == Some(true);
    }
    Some((resolves_to, keeps))
}

/// `version` of the parent admits a clean copy and is clear of the parent's
/// own fixable advisories.
pub fn admits_at(
    child: &Child<'_>,
    history: &RequirementHistory,
    own_clean: &dyn Fn(&str) -> bool,
    version: &str,
    dialect: Dialect,
) -> Option<Found> {
    let entry = history.get(version)?;
    if !own_clean(version) {
        return None;
    }
    let (resolves_to, keeps_installed) = admits_clean(child, &entry.reqs, dialect)?;
    Some(Found {
        version: version.to_string(),
        requirement: entry.reqs.first().cloned(),
        peer: entry.peer,
        resolves_to,
        keeps_installed,
    })
}

/// Rule (a): the smallest installable parent release above `from`, clear of
/// its own advisories, whose requirement admits a clean copy.
pub fn smallest_admitting(
    child: &Child<'_>,
    parent: &Releases,
    from: &str,
    history: &RequirementHistory,
    own_clean: &dyn Fn(&str) -> bool,
    dialect: Dialect,
) -> Option<Found> {
    let eco = child.ecosystem;
    ascending(&parent.versions, eco)
        .into_iter()
        .filter(|v| compare(v, from, eco) == Some(Ordering::Greater))
        .find_map(|v| admits_at(child, history, own_clean, v, dialect))
}

/// What one blocking parent can do: rule (a), or why nothing.
pub fn standing(
    child: &Child<'_>,
    parent: &Releases,
    from: &str,
    history: Option<&RequirementHistory>,
    own_clean: &dyn Fn(&str) -> bool,
    dialect: Dialect,
) -> Standing {
    let eco = child.ecosystem;
    if parent.deprecated {
        return Standing::Deprecated;
    }
    let latest = latest_of(parent, eco);
    if latest
        .as_deref()
        .is_some_and(|l| compare(from, l, eco).is_some_and(|o| o != Ordering::Less))
    {
        return Standing::Latest;
    }
    let Some(history) = history else {
        return Standing::Unread;
    };
    match smallest_admitting(child, parent, from, history, own_clean, dialect) {
        Some(found) => Standing::Release(found),
        None => Standing::NoneAdmits { latest },
    }
}

/// Rule (b), the parent's own row: its target raised to the highest release
/// the transitive rows need from it (across a major too: parity decision 3),
/// unless one of its advisories affects that release. `None` when the parent
/// has no row of its own.
pub fn row_target(
    own_target: Option<&str>,
    needs: &[&str],
    parent_affected: &dyn Fn(&str) -> bool,
    eco: &str,
) -> Option<String> {
    let own = own_target?;
    let highest = needs
        .iter()
        .copied()
        .chain(std::iter::once(own))
        .max_by(|a, b| compare(a, b, eco).unwrap_or(Ordering::Equal))
        .unwrap_or(own);
    // Across a major too (parity decision 3): the plan flags it breaking and
    // says why; refusing left the parent's row on a release that does not
    // fix the copy it pins.
    let raise = compare(highest, own, eco) == Some(Ordering::Greater) && !parent_affected(highest);
    Some(if raise { highest } else { own }.to_string())
}

/// Rule (b), a transitive row: a smallest release below the parent's own
/// row target moves up to it when that target also admits a clean copy (a
/// major step included: the parent's row moves there anyway).
pub fn lift(
    found: &Found,
    row_target: Option<&str>,
    child: &Child<'_>,
    history: &RequirementHistory,
    own_clean: &dyn Fn(&str) -> bool,
    dialect: Dialect,
) -> Option<Found> {
    let target = row_target?;
    let below = compare(&found.version, target, child.ecosystem) == Some(Ordering::Less);
    if !below {
        return None;
    }
    admits_at(child, history, own_clean, target, dialect)
}

/// One transitive row a parent blocks: the copy, and the parent's
/// per-release requirement on it (`None` when not read).
pub struct RowIn<'a> {
    pub child: Child<'a>,
    pub history: Option<&'a RequirementHistory>,
    /// Why nothing can be decided for this copy (its advisories cannot be
    /// read), when that is so.
    pub unknown: Option<String>,
}

/// What one row gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowOut {
    pub standing: Standing,
    /// The release named: the smallest one, or the parent's own row target
    /// when it was lifted to it.
    pub to: Option<Found>,
}

impl RowOut {
    /// The smallest admitting release (rule a), before any lift.
    pub fn smallest(&self) -> Option<&Found> {
        match &self.standing {
            Standing::Release(f) => Some(f),
            _ => None,
        }
    }
}

/// Everything one parent's rows get, agreed (rules a, b, c, f).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The parent's own row target after the raise; `None` without an own row.
    pub row_target: Option<String>,
    pub rows: Vec<RowOut>,
}

/// One parent, every transitive row it blocks, and its own row's target:
/// each row's release, agreed with the others (the shared rule vectors run
/// exactly this).
pub fn decide(
    parent: &Releases,
    from: &str,
    own_target: Option<&str>,
    parent_advisories: &Advisories<'_>,
    rows: &[RowIn<'_>],
    extra_needs: &[String],
    dialect: Dialect,
) -> Decision {
    let eco = parent_advisories.ecosystem;
    let latest = parent_advisories.newest(parent);
    let own_clean = |v: &str| parent_advisories.usable(v, latest.as_deref(), Some(from));
    let blocked = parent_blocked(parent_advisories, from);
    let standings: Vec<Standing> = rows
        .iter()
        .map(|r| match (&blocked, &r.unknown) {
            (Some(b), _) => b.clone(),
            (None, Some(why)) => Standing::Undecided(why.clone()),
            (None, None) => {
                let s = standing(&r.child, parent, from, r.history, &own_clean, dialect);
                introduces(
                    s,
                    r,
                    parent,
                    from,
                    parent_advisories,
                    latest.as_deref(),
                    dialect,
                )
            }
        })
        .collect();
    let needs: Vec<&str> = standings
        .iter()
        .filter_map(|s| match s {
            Standing::Release(f) => Some(f.version.as_str()),
            _ => None,
        })
        .chain(extra_needs.iter().map(String::as_str))
        .collect();
    let target = row_target(own_target, &needs, &|v| parent_advisories.affect(v), eco);
    let rows = rows
        .iter()
        .zip(standings)
        .map(|(r, standing)| {
            let to = match (&standing, r.history) {
                (Standing::Release(f), Some(h)) => Some(
                    lift(f, target.as_deref(), &r.child, h, &own_clean, dialect)
                        .unwrap_or_else(|| f.clone()),
                ),
                _ => None,
            };
            RowOut { standing, to }
        })
        .collect();
    Decision {
        row_target: target,
        rows,
    }
}

/// Parity decision 2: when no release qualifies, but one would if it could
/// bring in an advisory the installed parent lacks, say which ones.
fn introduces(
    found: Standing,
    row: &RowIn<'_>,
    parent: &Releases,
    from: &str,
    advisories: &Advisories<'_>,
    latest: Option<&str>,
    dialect: Dialect,
) -> Standing {
    let (Standing::NoneAdmits { .. }, Some(history)) = (&found, row.history) else {
        return found;
    };
    // Every release above the installed one that admits a clean copy once
    // advisories the installed parent lacks are set aside, fixed or not
    // (review of #994, S2): what they bring in, unioned (S4).
    let relaxed = |v: &str| advisories.clean_but_introduced(v, latest, from);
    let eco = advisories.ecosystem;
    let admitting = ascending(&parent.versions, eco)
        .into_iter()
        .filter(|v| compare(v, from, eco) == Some(Ordering::Greater))
        .filter(|v| admits_at(&row.child, history, &relaxed, v, dialect).is_some())
        .map(String::as_str);
    let ids = advisories.introduced_union(admitting, latest, from);
    if ids.is_empty() {
        found
    } else {
        Standing::Introduces(ids)
    }
}

/// Why no release of the parent can be named for any row: the installed
/// parent is malicious, or one of its advisories cannot be read.
fn parent_blocked(advisories: &Advisories<'_>, from: &str) -> Option<Standing> {
    if let Some(id) = advisories.malware_at(from) {
        return Some(Standing::Malicious(id.to_string()));
    }
    advisories.unreadable().map(|id| {
        Standing::Undecided(format!(
            "the affected ranges of {id} cannot be read, so no release can be shown clear of it"
        ))
    })
}

#[cfg(test)]
#[path = "parent_release_tests.rs"]
mod tests;
