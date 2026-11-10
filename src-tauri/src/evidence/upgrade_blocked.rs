// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Blocked upstream (Y5, 2026-11-10): a transitive copy whose parent must
//! move, when no published release of the package the project can move
//! admits the fix. The plan used to say "Upgrade zip to >= 9.0.5 via
//! docx-rs" while docx-rs 0.4.22 was already its newest release and still
//! required zip ^8.6.0: a step nobody could follow. In 4DA's own repo four of
//! five such Rust steps were like that (zip via docx-rs, rsa via
//! jsonwebtoken, crypto-bigint via elliptic-curve under jsonwebtoken's p256,
//! the relay's libsqlite3-sys under sqlx-sqlite).
//!
//! Decided from the data the route already carries (`osv::parent_hint`),
//! never a new fetch: the registry standing of the parent and of every
//! declared dependency it arrives through. Blocked when the copy's parent
//! must move ([`Refresh::ParentMustMove`]) and either the parent itself is
//! at the newest release 4DA has seen (or every release was read and none
//! admits a clean copy), or every declared dependency it arrives through is.
//! A newer parent some project here already resolved clean is proof a move
//! exists, so it is never blocked.
//!
//! The same predicate words the Upgrade Plan step, the Preemption alert and
//! the Brief's fix clause (all through [`super::parent_move`]), and fills the
//! work order's `blocked_upstream` holds and its `mechanism`.

use serde::{Deserialize, Serialize};

use crate::osv::fix_path::Refresh;
use crate::osv::fix_target::LineTarget;
use crate::osv::parent_hint::{ParentLink, ParentStanding, Through, TransitiveRoutes};

use super::upgrade_steps::Mechanism;

/// One transitive copy no published release can fix yet: what holds it, and
/// the honest options. A field of the work order (`UpgradeStep::blocked_upstream`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedHold {
    /// The project path, as the step's sites name it (normalized).
    pub project: String,
    pub installed: String,
    /// The version that clears the advisories (the line's target).
    pub cleared_version: String,
    /// The package that pulls the copy in, and its installed version.
    pub parent: String,
    pub parent_version: String,
    /// The parent's requirement on the package as read; `None` when the
    /// hold was inferred from a semver-incompatible jump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// The package whose newest release keeps the copy: the parent itself,
    /// or the declared dependency it arrives through.
    pub held_by: String,
    /// `held_by`'s newest release 4DA has seen (its installed version when
    /// that is the newest).
    pub held_by_newest: String,
    /// `held_by`'s installed version. Equal to `held_by_newest` when it is
    /// already the newest; lower when every release was read and none admits
    /// a clean copy.
    pub held_by_version: String,
    /// What the reader can do instead, in order.
    pub options: Vec<String>,
}

/// How the fix reaches one transitive copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyRoute {
    Refresh,
    ParentUpdate,
    Blocked,
    Undecided,
}

fn at_newest(s: &ParentStanding) -> bool {
    matches!(
        s,
        ParentStanding::Newest { .. } | ParentStanding::NoneAdmits { .. }
    )
}

/// No published release of what the project can move admits the fix: the
/// parent is at its newest release (or no release of it admits a clean
/// copy), or every declared dependency it arrives through is at its newest.
pub(crate) fn standing_blocked(standing: &ParentStanding, through: Option<&Through>) -> bool {
    at_newest(standing) || through.is_some_and(|t| t.all().all(|l| at_newest(&l.standing)))
}

/// [`standing_blocked`] for one route link: its parent must move, it names a
/// target, and no project here resolved a newer parent clean.
pub(crate) fn link_blocked(l: &ParentLink) -> bool {
    !l.refresh.is_enough()
        && l.target.is_some()
        && l.resolutions.is_empty()
        && standing_blocked(&l.standing, l.through.as_ref())
}

/// The package that keeps the copy, and its newest release seen.
fn holder(l: &ParentLink) -> (String, String, String, bool) {
    let newest = |s: &ParentStanding, installed: &str| match s {
        ParentStanding::NoneAdmits {
            latest: Some(latest),
        } => latest.clone(),
        _ => installed.to_string(),
    };
    let none = |s: &ParentStanding| matches!(s, ParentStanding::NoneAdmits { .. });
    if at_newest(&l.standing) {
        return (
            l.parent.clone(),
            l.parent_version.clone(),
            newest(&l.standing, &l.parent_version),
            none(&l.standing),
        );
    }
    match &l.through {
        Some(t) => (
            t.name.clone(),
            t.version.clone(),
            newest(&t.standing, &t.version),
            none(&t.standing),
        ),
        None => (
            l.parent.clone(),
            l.parent_version.clone(),
            l.parent_version.clone(),
            false,
        ),
    }
}

/// The honest options for a blocked copy, by ecosystem (OSV-canonical) or,
/// when it is not known, by whether the project's tool can force a version
/// (`override_hint`, npm-family only).
pub(crate) fn blocked_options(
    ecosystem: Option<&str>,
    package: &str,
    target: &str,
    parent: &str,
    held_by: &str,
    override_hint: Option<&str>,
) -> Vec<String> {
    let fork = format!("fork {held_by} so it accepts {package} >= {target}");
    let accept = "accept the risk with a written review date".to_string();
    let npm = ecosystem.map_or(override_hint.is_some(), |e| e == "npm");
    if npm {
        let how = override_hint.unwrap_or(
            "an override in package.json (npm `overrides`, pnpm `pnpm.overrides` or yarn \
             `resolutions`)",
        );
        return vec![
            format!(
                "force {package} >= {target} with {how}; it runs outside the range {parent} \
                 declares, so test it"
            ),
            fork,
            accept,
        ];
    }
    match ecosystem {
        Some("crates.io") | None => vec![
            format!("{fork} and point `[patch.crates-io]` at the fork"),
            format!(
                "backport the fix onto the {package} line {parent} allows and point \
                 `[patch.crates-io]` at that fork"
            ),
            accept,
        ],
        Some("PyPI") => vec![
            format!("{fork} and install the fork (a pip constraint cannot exceed {parent}'s requirement)"),
            accept,
        ],
        _ => vec![fork, accept],
    }
}

/// One blocked copy, as the ONE sentence every surface prints (the plan step
/// and the Preemption alert through `upgrade_parent::link_sentence`, the
/// Brief through `brief_facts::fix_clause`):
/// "zip 8.6.0 comes in through docx-rs 0.4.22, the newest release, whose
/// requirement ^8.6.0 does not allow a fixed zip (>= 9.0.5) yet. Options:
/// ...". Every version follows its own package's name (the Brief's version
/// check reads the nearest preceding package).
pub(crate) struct BlockedCopy<'a> {
    pub package: &'a str,
    /// `None` where the surface does not hold it (the Brief's clause).
    pub installed: Option<&'a str>,
    /// " in <project>" when the step names several projects, else empty.
    pub place: &'a str,
    pub target: &'a str,
    pub parent: &'a str,
    pub parent_version: &'a str,
    /// The parent's requirement on the package, when read.
    pub requirement: Option<&'a str>,
    pub standing: &'a ParentStanding,
    pub through: Option<&'a Through>,
    pub override_hint: Option<&'a str>,
}

impl BlockedCopy<'_> {
    /// The package that cannot move, and its newest release seen.
    /// The package that cannot move, its installed version, and whether
    /// every release of it was read and none admits a clean copy (then no
    /// release is named: the parent's own row names the one it moves to).
    fn holder(&self) -> (&str, &str, bool) {
        let none = |s: &ParentStanding| matches!(s, ParentStanding::NoneAdmits { .. });
        if at_newest(self.standing) {
            return (self.parent, self.parent_version, none(self.standing));
        }
        match self.through {
            Some(t) => (t.name.as_str(), t.version.as_str(), none(&t.standing)),
            None => (self.parent, self.parent_version, false),
        }
    }

    /// The sentence with its options: what the Brief prints per site.
    pub(crate) fn sentence(&self) -> String {
        format!(
            "{}. Options: {}",
            self.clause(),
            self.options().join("; or ")
        )
    }

    /// The honest options for this copy.
    pub(crate) fn options(&self) -> Vec<String> {
        let (held_by, ..) = self.holder();
        blocked_options(
            None,
            self.package,
            self.target,
            self.parent,
            held_by,
            self.override_hint,
        )
    }

    /// The sentence without its options (the plan states them once).
    pub(crate) fn clause(&self) -> String {
        let (package, target, parent) = (self.package, self.target, self.parent);
        let copy = self.installed.map_or_else(
            || format!("{package}{}", self.place),
            |v| format!("{package} {v}{}", self.place),
        );
        let fixed = format!("a fixed {package} (>= {target})");
        let (held_by, held_version, none_admits) = self.holder();
        let hold = if held_by == parent && !none_admits {
            let allow = self.requirement.map_or_else(
                || format!("which does not allow {fixed} yet"),
                |r| format!("whose requirement {r} does not allow {fixed} yet"),
            );
            format!(
                "{parent} {}, the newest release, {allow}",
                self.parent_version
            )
        } else if held_by == parent {
            format!(
                "{parent} {}, and no {parent} release up to its newest allows {fixed} yet",
                self.parent_version
            )
        } else if none_admits {
            format!(
                "{parent} {}, which arrives through {held_by} {held_version}, and no {held_by} \
                 release up to its newest allows {fixed} yet",
                self.parent_version
            )
        } else {
            format!(
                "{parent} {}, which arrives through {held_by} {held_version}, the newest {held_by} \
                 release, and no {held_by} release allows {fixed} yet",
                self.parent_version
            )
        };
        format!("{copy} comes in through {hold}")
    }
}

/// The marker a blocked link's `dependency-path` note ends with, read by the
/// hand-off prompt (`src/lib/agent-handoff.ts`): who holds the copy.
pub(crate) fn note_marker(l: &ParentLink) -> String {
    let (held_by, version, _, none_admits) = holder(l);
    if none_admits {
        format!("; blocked upstream: no {held_by} release up to its newest admits it")
    } else {
        format!("; blocked upstream: {held_by} {version} is the newest release")
    }
}

fn route_of_copy(links: &[&ParentLink]) -> CopyRoute {
    let movers: Vec<&&ParentLink> = links.iter().filter(|l| !l.refresh.is_enough()).collect();
    if movers.is_empty() {
        return CopyRoute::Refresh;
    }
    if movers.iter().any(|l| link_blocked(l)) {
        return CopyRoute::Blocked;
    }
    let proven = |l: &ParentLink| {
        matches!(l.standing, ParentStanding::Admits(_)) || !l.resolutions.is_empty()
    };
    if movers.iter().all(|l| proven(l)) {
        CopyRoute::ParentUpdate
    } else {
        CopyRoute::Undecided
    }
}

/// Every transitive copy with a target, with the route the data decides.
fn copy_routes(lines: &[LineTarget], routes: &TransitiveRoutes) -> Vec<CopyRoute> {
    let norm = fourda_depengine::parent_hint::normalize_path;
    let mut out = Vec::new();
    for line in lines.iter().filter(|l| l.target_version.is_some()) {
        for site in line.sites.iter().filter(|s| !s.is_direct) {
            let project = norm(&site.project_path);
            let links: Vec<&ParentLink> = routes
                .links
                .iter()
                .filter(|l| l.project == project && l.child_version == line.installed_version)
                .collect();
            let route = if !links.is_empty() {
                route_of_copy(&links)
            } else {
                match routes
                    .unlinked
                    .iter()
                    .find(|u| u.project == project && u.installed == line.installed_version)
                {
                    Some(u) if u.refresh.is_enough() => CopyRoute::Refresh,
                    _ => CopyRoute::Undecided,
                }
            };
            out.push(route);
        }
    }
    out
}

/// The mechanism of an all-transitive step: `lockfile_refresh` when every
/// copy's requirement admits the fix, `parent_update` when the rest need a
/// parent release the registry (or a project here) shows admits it,
/// `blocked_upstream` when no release fixes any of them yet, otherwise the
/// undecided `lockfile_or_parent_update`.
pub(super) fn transitive_mechanism(lines: &[LineTarget], routes: &TransitiveRoutes) -> Mechanism {
    let all = copy_routes(lines, routes);
    if all.is_empty() || all.contains(&CopyRoute::Undecided) {
        return Mechanism::LockfileOrParentUpdate;
    }
    if all.iter().all(|r| *r == CopyRoute::Refresh) {
        Mechanism::LockfileRefresh
    } else if all.iter().all(|r| *r == CopyRoute::Blocked) {
        Mechanism::BlockedUpstream
    } else if all.contains(&CopyRoute::Blocked) {
        Mechanism::LockfileOrParentUpdate
    } else {
        Mechanism::ParentUpdate
    }
}

/// The holds of every blocked link, one per (project, installed, parent).
pub(super) fn holds(ecosystem: &str, package: &str, routes: &TransitiveRoutes) -> Vec<BlockedHold> {
    let mut out: Vec<BlockedHold> = Vec::new();
    for l in routes.links.iter().filter(|l| link_blocked(l)) {
        let Some(target) = l.target.clone() else {
            continue;
        };
        if out.iter().any(|h| {
            h.project == l.project && h.installed == l.child_version && h.parent == l.parent
        }) {
            continue;
        }
        let (held_by, held_by_version, held_by_newest, _) = holder(l);
        let requirement = match l.refresh {
            Refresh::ParentMustMove(crate::osv::fix_path::Basis::Requirement) => {
                l.requirement.clone().filter(|_| !l.is_bundled())
            }
            _ => None,
        };
        out.push(BlockedHold {
            options: blocked_options(
                Some(ecosystem),
                package,
                &target,
                &l.parent,
                &held_by,
                l.override_hint.as_deref(),
            ),
            project: l.project.clone(),
            installed: l.child_version.clone(),
            cleared_version: target,
            parent: l.parent.clone(),
            parent_version: l.parent_version.clone(),
            requirement,
            held_by,
            held_by_newest,
            held_by_version,
        });
    }
    out
}

/// The title of a `blocked_upstream` step: "zip 8.6.0: blocked by docx-rs
/// 0.4.22 (newest), no fix yet". Short by construction (live 2026-11-10 the
/// longer "is held ... by elliptic-curve 0.13.8, which jsonwebtoken 11.1.0
/// (newest) brings in" form ran past the 120-byte cap and was cut
/// mid-word): the requirement and the parent are in the explanation. Other
/// holds count as "(+N more)" (distinct copy and holder, not one per
/// project); parts drop, last first, until it fits. `None` without holds.
pub(super) fn blocked_title(package: &str, holds: &[BlockedHold]) -> Option<String> {
    const MAX: usize = 120;
    let h = holds.first()?;
    let mut distinct: Vec<(&str, &str)> = holds
        .iter()
        .map(|h| (h.installed.as_str(), h.held_by.as_str()))
        .collect();
    distinct.sort_unstable();
    distinct.dedup();
    let more = match distinct.len() {
        1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    // "(newest)" only when the holder IS at its newest; when no release up
    // to the newest admits it, no release is named (the parent's own row
    // names the one it moves to).
    let by = if h.held_by_version == h.held_by_newest {
        format!("{} {} (newest)", h.held_by, h.held_by_newest)
    } else {
        h.held_by.clone()
    };
    let candidates = [
        format!(
            "{package} {}: blocked by {by}, no fix yet{more}",
            h.installed
        ),
        format!("{package} {}: blocked by {by}, no fix yet", h.installed),
        format!(
            "{package} {}: blocked by {}, no fix yet",
            h.installed, h.held_by
        ),
        format!("{package}: blocked by {}, no fix yet", h.held_by),
        format!("{package}: blocked upstream, no fix yet"),
    ];
    let fits = candidates.iter().find(|t| t.len() <= MAX).cloned();
    Some(fits.unwrap_or_else(|| candidates[candidates.len() - 1].clone()))
}

#[cfg(test)]
#[path = "upgrade_blocked_tests.rs"]
mod tests;
