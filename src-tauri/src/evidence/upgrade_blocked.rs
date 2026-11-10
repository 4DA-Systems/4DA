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
fn holder(l: &ParentLink) -> (String, String) {
    let newest = |s: &ParentStanding, installed: &str| match s {
        ParentStanding::NoneAdmits {
            latest: Some(latest),
        } => latest.clone(),
        _ => installed.to_string(),
    };
    if at_newest(&l.standing) {
        return (l.parent.clone(), newest(&l.standing, &l.parent_version));
    }
    match &l.through {
        Some(t) => (t.name.clone(), newest(&t.standing, &t.version)),
        None => (l.parent.clone(), l.parent_version.clone()),
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

/// The sentence a blocked move ends with: "no release fixes it yet. Options:
/// ...". Follows the move text (`parent_move`), so no version is named
/// before its own package.
pub(crate) fn blocked_suffix(
    package: &str,
    target: &str,
    parent: &str,
    standing: &ParentStanding,
    through: Option<&Through>,
    override_hint: Option<&str>,
) -> String {
    let held_by = if at_newest(standing) {
        parent
    } else {
        through.map_or(parent, |t| t.name.as_str())
    };
    let options = blocked_options(None, package, target, parent, held_by, override_hint);
    format!(
        "; blocked upstream: no release fixes it yet. Options: {}",
        options.join("; or ")
    )
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
        let (held_by, held_by_newest) = holder(l);
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
        });
    }
    out
}

/// The title of a `blocked_upstream` step: "zip 8.6.0 is held at ^8.6.0 by
/// docx-rs 0.4.22 (newest): no release fixes it yet". `None` without holds.
pub(super) fn blocked_title(package: &str, holds: &[BlockedHold]) -> Option<String> {
    let h = holds.first()?;
    let at = h
        .requirement
        .as_deref()
        .map_or_else(String::new, |r| format!(" at {r}"));
    let by = if h.held_by == h.parent && h.held_by_newest == h.parent_version {
        format!("{} {} (newest)", h.parent, h.parent_version)
    } else if h.held_by == h.parent {
        format!(
            "{} {} (no release up to {} {} admits it)",
            h.parent, h.parent_version, h.parent, h.held_by_newest
        )
    } else {
        format!(
            "{} {}, which {} {} (newest) brings in",
            h.parent, h.parent_version, h.held_by, h.held_by_newest
        )
    };
    let more = match holds.len() {
        1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    Some(format!(
        "{package} {} is held{at} by {by}: no release fixes it yet{more}",
        h.installed
    ))
}

#[cfg(test)]
#[path = "upgrade_blocked_tests.rs"]
mod tests;
