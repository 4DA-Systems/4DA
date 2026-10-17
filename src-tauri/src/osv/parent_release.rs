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

/// One package's known advisories, as stored ranges JSON.
pub struct Advisories<'a> {
    pub ecosystem: &'a str,
    pub ranges: Vec<Option<String>>,
}

impl Advisories<'_> {
    /// Some advisory affects `version`.
    pub fn affect(&self, version: &str) -> bool {
        self.ranges
            .iter()
            .any(|r| check_version_affected(self.ecosystem, Some(version), r).0)
    }

    /// No advisory WITH A WAY OUT affects `version`: one that also affects
    /// `newest` has no published fix and cannot make any release unclean.
    pub fn clean_of_fixable(&self, version: &str, newest: Option<&str>) -> bool {
        self.ranges.iter().all(|r| {
            let unfixed =
                newest.is_some_and(|n| check_version_affected(self.ecosystem, Some(n), r).0);
            unfixed || !check_version_affected(self.ecosystem, Some(version), r).0
        })
    }

    /// Some advisory has a way out (does not affect `newest`).
    pub fn any_fixable(&self, newest: Option<&str>) -> bool {
        self.ranges
            .iter()
            .any(|r| !newest.is_some_and(|n| check_version_affected(self.ecosystem, Some(n), r).0))
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
fn ascending<'a>(versions: &'a [String], eco: &str) -> Vec<&'a String> {
    let mut out: Vec<&String> = versions.iter().filter(|v| stable(v, eco)).collect();
    out.sort_by(|a, b| compare(a, b, eco).unwrap_or(Ordering::Equal));
    out
}

/// The newest stable release of `releases`, or `latest` when it names one.
pub fn newest(releases: &Releases, eco: &str) -> Option<String> {
    releases.latest.clone().or_else(|| {
        ascending(&releases.versions, eco)
            .last()
            .map(|v| (*v).clone())
    })
}

/// A move from `from` to `to` crosses a major version (a `0.x` minor counts).
pub fn is_major(from: &str, to: &str) -> bool {
    super::fix_target::upgrade_type(from, to) == Some(super::fix_target::UpgradeType::Major)
}

fn admits(req: &str, version: &str, dialect: Dialect) -> Option<bool> {
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
    let latest = newest(parent, eco);
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
/// the transitive rows need from it, unless that crosses a major its own
/// target does not or one of its advisories affects it. `None` when the
/// parent has no row of its own.
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
    let raise = compare(highest, own, eco) == Some(Ordering::Greater)
        && !is_major(own, highest)
        && !parent_affected(highest);
    Some(if raise { highest } else { own }.to_string())
}

/// Rule (b), a transitive row: a smallest release below the parent's own
/// row target moves up to it when that target also admits a clean copy and
/// the step does not cross a major.
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
    if !below || is_major(&found.version, target) {
        return None;
    }
    admits_at(child, history, own_clean, target, dialect)
}

/// One transitive row a parent blocks: the copy, and the parent's
/// per-release requirement on it (`None` when not read).
pub struct RowIn<'a> {
    pub child: Child<'a>,
    pub history: Option<&'a RequirementHistory>,
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
    dialect: Dialect,
) -> Decision {
    let eco = parent_advisories.ecosystem;
    let latest = newest(parent, eco);
    let own_clean = |v: &str| parent_advisories.clean_of_fixable(v, latest.as_deref());
    let standings: Vec<Standing> = rows
        .iter()
        .map(|r| standing(&r.child, parent, from, r.history, &own_clean, dialect))
        .collect();
    let needs: Vec<&str> = standings
        .iter()
        .filter_map(|s| match s {
            Standing::Release(f) => Some(f.version.as_str()),
            _ => None,
        })
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

/// A lever that is not the parent: the declared dependency the parent
/// arrives through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hop {
    /// `version` of the lever brings in parent `brings`, a release that
    /// admits a clean copy; `keeps_parent` when the lever's requirement still
    /// admits the installed parent (an install keeps it; a refresh of the
    /// parent follows).
    Found {
        version: String,
        brings: String,
        keeps_parent: bool,
    },
    Deprecated,
    Latest,
    /// Every release was read and none brings in a parent that admits it.
    NoneBrings {
        latest: Option<String>,
    },
}

/// The parent side of a hop.
pub struct HopParent<'a> {
    pub installed: &'a str,
    pub releases: &'a Releases,
    pub history: &'a RequirementHistory,
    pub own_clean: &'a dyn Fn(&str) -> bool,
}

/// The smallest release of the lever, clear of its own advisories, whose one
/// requirement on the parent resolves to a parent release above the
/// installed one, clear of the parent's advisories, that admits a clean
/// copy. `lever_history` is the lever's requirement on the parent.
pub fn through_release(
    child: &Child<'_>,
    parent: &HopParent<'_>,
    lever: &Releases,
    lever_from: &str,
    lever_history: &RequirementHistory,
    lever_clean: &dyn Fn(&str) -> bool,
    dialect: Dialect,
) -> Hop {
    let eco = child.ecosystem;
    if lever.deprecated {
        return Hop::Deprecated;
    }
    let latest = newest(lever, eco);
    if latest
        .as_deref()
        .is_some_and(|l| compare(lever_from, l, eco).is_some_and(|o| o != Ordering::Less))
    {
        return Hop::Latest;
    }
    let parent_versions = ascending(&parent.releases.versions, eco);
    let brought = |v: &str| -> Option<(String, bool)> {
        let entry = lever_history.get(v)?;
        let [req] = entry.reqs.as_slice() else {
            return None;
        };
        if !lever_clean(v) {
            return None;
        }
        let brings = parent_versions
            .iter()
            .rev()
            .find(|p| admits(req, p, dialect) == Some(true))?;
        let above = compare(brings, parent.installed, eco) == Some(Ordering::Greater);
        if !above || !(parent.own_clean)(brings) {
            return None;
        }
        admits_clean(child, &parent.history.get(brings.as_str())?.reqs, dialect)?;
        Some((
            (*brings).clone(),
            admits(req, parent.installed, dialect) == Some(true),
        ))
    };
    ascending(&lever.versions, eco)
        .into_iter()
        .filter(|v| compare(v, lever_from, eco) == Some(Ordering::Greater))
        .find_map(|v| {
            brought(v).map(|(brings, keeps_parent)| Hop::Found {
                version: v.clone(),
                brings,
                keeps_parent,
            })
        })
        .unwrap_or(Hop::NoneBrings { latest })
}

#[cfg(test)]
#[path = "parent_release_tests.rs"]
mod tests;
