// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The parent release of a nested advisory, read from the user's own data:
//! the stored registry facts (`osv::release_facts`), the advisories 4DA
//! holds, and the installed graph (`dependency_edges`, `dependency_instances`).
//! `osv::parent_release` decides; this module gathers its inputs, writes the
//! answer into each blocking [`ParentLink`] as [`ParentStanding::Admits`], and
//! names which parents' requirements the next registry read must keep.
//!
//! npm only: the reference rules were verified on npm 10.9.2, and npm is the
//! registry whose one document already carries every release's requirements.
//! crates.io (a per-version `/dependencies` read, or the sparse index) and
//! the other ecosystems keep the registry-row standing they had.
//!
//! Every surface that prints a transitive route (Upgrade Plan, Preemption,
//! Brief) reads [`ParentLink`]s from `parent_hint::load_routes`, so all three
//! name the same release. The parent's own plan row is raised to agree
//! ([`raised_row_target`]).

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::Path;

use fourda_depengine::fix_target::walk_target_kinds;

use crate::db::Database;
use crate::osv::types::MatchedAdvisory;

use super::fix_path::{requirement_admits, Dialect, Manager};
use super::parent_hint::{normalize_path, ParentLink, ParentRelease, ParentStanding};
use super::parent_release::{
    compare, decide, is_major, Advisories, Child, Decision, Found, RowIn, RowOut, Standing,
};
use super::release_facts::ReleaseBook;

/// The only ecosystem whose parent releases are read.
pub(super) const ECOSYSTEM: &str = "npm";

/// Most other vulnerable children of one parent weighed when agreeing its
/// release with its own row.
const MAX_SIBLING_ROWS: usize = 12;

pub(super) fn advisories(db: &Database, name: &str) -> Advisories<'static> {
    Advisories::stored(
        ECOSYSTEM,
        db.get_osv_advisories_for_package(name, ECOSYSTEM)
            .unwrap_or_default(),
    )
}

/// The parents (and the declared dependencies they arrive through) whose
/// per-release requirement on a vulnerable npm copy the registry read should
/// keep: `(package to read, package its requirement is read for)`.
pub(crate) fn wanted_parents(db: &Database, matches: &[MatchedAdvisory]) -> Vec<(String, String)> {
    let mut groups: std::collections::BTreeMap<String, Vec<&MatchedAdvisory>> =
        std::collections::BTreeMap::new();
    for m in matches.iter().filter(|m| m.is_version_confirmed) {
        if crate::osv::exposure::canonical(&m.ecosystem) == Some(ECOSYSTEM) {
            groups.entry(m.package_name.clone()).or_default().push(m);
        }
    }
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    for (package, advisories) in groups {
        let instances = || {
            advisories
                .iter()
                .flat_map(|a| a.dependency_instances.iter())
        };
        if !instances().any(|d| d.is_version_confirmed && !d.is_direct) {
            continue;
        }
        let mut projects: Vec<String> = instances().map(|d| d.project_path.clone()).collect();
        projects.sort();
        projects.dedup();
        let lines = super::fix_target::line_targets(&advisories, &projects);
        let routes = super::parent_hint::load_routes_with(db, ECOSYSTEM, &package, &lines, false);
        for link in routes.links.iter().filter(|l| !l.refresh.is_enough()) {
            out.insert((link.parent.clone(), package.clone()));
            for t in link.through.iter().flat_map(|t| t.all()) {
                out.insert((t.name.clone(), link.parent.clone()));
            }
        }
    }
    out.into_iter().collect()
}

/// A row's inputs, owned, so [`Child`] can borrow them.
pub(super) struct RowData {
    pub(super) package: String,
    installed: String,
    target: String,
    releases: Vec<String>,
    advisories: Advisories<'static>,
    newest: Option<String>,
}

impl RowData {
    fn new(
        book: &ReleaseBook,
        db: &Database,
        package: &str,
        installed: &str,
        target: &str,
    ) -> Self {
        let releases = book.releases(ECOSYSTEM, package).unwrap_or_default();
        let advisories = advisories(db, package);
        Self {
            package: package.to_string(),
            installed: installed.to_string(),
            target: target.to_string(),
            newest: advisories.newest(&releases),
            releases: releases.versions,
            advisories,
        }
    }

    /// The shared predicate (`Advisories::usable`): a release that can be named.
    pub(super) fn clean(&self) -> Box<dyn Fn(&str) -> bool + '_> {
        Box::new(move |v: &str| {
            self.advisories
                .usable(v, self.newest.as_deref(), Some(&self.installed))
        })
    }

    /// Why nothing can be decided for this copy, when that is so.
    pub(super) fn unknown(&self) -> Option<String> {
        self.advisories.unreadable().map(|id| {
            format!(
                "the affected ranges of {id} ({}) cannot be read, so no {} release can be shown \
                 clear of it",
                self.package, self.package
            )
        })
    }

    pub(super) fn child<'a>(&'a self, clean: &'a dyn Fn(&str) -> bool) -> Child<'a> {
        Child {
            ecosystem: ECOSYSTEM,
            installed: &self.installed,
            target: &self.target,
            releases: &self.releases,
            clean,
        }
    }
}

/// The parent side of one decision.
pub(super) struct ParentSide<'a> {
    pub(super) project: &'a str,
    pub(super) name: &'a str,
    pub(super) version: &'a str,
    /// Declared by the project: it can have a plan row of its own.
    pub(super) direct: bool,
}

/// The target of the parent's own row: the walk past its own advisories
/// from the installed release, when one affects it and it is declared.
fn own_target(book: &ReleaseBook, parent: &ParentSide<'_>, own: &Advisories<'_>) -> Option<String> {
    if !parent.direct || !own.affect(parent.version) {
        return None;
    }
    let refs = own.ranges();
    walk_target_kinds(
        ECOSYSTEM,
        parent.version,
        &refs,
        &own.kinds(),
        &book.registry(ECOSYSTEM, parent.name),
    )
    .target
    .map(|w| w.version)
}

/// The other vulnerable copies `parent` pins in its project: each child its
/// requirements were read for, at the installed version its edge leads to,
/// with a target that requirement excludes.
pub(super) fn sibling_rows(
    db: &Database,
    book: &ReleaseBook,
    parent: &ParentSide<'_>,
    skip: &str,
) -> Vec<RowData> {
    let mut out = Vec::new();
    for child in book.children(ECOSYSTEM, parent.name) {
        if child.eq_ignore_ascii_case(skip) || out.len() >= MAX_SIBLING_ROWS {
            continue;
        }
        let Some(history) = book.history(ECOSYSTEM, parent.name, &child) else {
            continue;
        };
        let Some(req) = history.get(parent.version).and_then(|r| r.reqs.first()) else {
            continue;
        };
        let copies: BTreeSet<String> = db
            .get_package_instances(ECOSYSTEM, &child)
            .unwrap_or_default()
            .into_iter()
            .filter(|i| normalize_path(&i.project_path) == parent.project)
            .map(|i| i.version)
            .filter(|v| requirement_admits(req, v) == Some(true))
            .collect();
        let own = advisories(db, &child);
        for installed in copies.into_iter().filter(|v| own.affect(v)) {
            let refs = own.ranges();
            let walk = walk_target_kinds(
                ECOSYSTEM,
                &installed,
                &refs,
                &own.kinds(),
                &book.registry(ECOSYSTEM, &child),
            );
            let Some(target) = walk.target.map(|w| w.version) else {
                continue;
            };
            if requirement_admits(req, &target) == Some(true) {
                continue; // a refresh reaches it: not a row this parent blocks
            }
            out.push(RowData::new(book, db, &child, &installed, &target));
        }
    }
    out
}

/// One parent's decision over `first` (the row asked about) and its
/// siblings, with the parent's own row target. `None` when the parent's
/// releases were never read. When the parent has a row of its own, the
/// releases of it the other rows need come in too: the copies it pins
/// itself, and (as a declared lever) the copies a parent it requires pins.
fn decision_for(
    db: &Database,
    book: &ReleaseBook,
    parent: &ParentSide<'_>,
    first: Option<&RowData>,
    row_target: Option<&str>,
) -> Option<(Decision, Vec<RowData>)> {
    let releases = book.releases(ECOSYSTEM, parent.name)?;
    let own = advisories(db, parent.name);
    let target = row_target
        .map(str::to_string)
        .or_else(|| own_target(book, parent, &own));
    let mut rows: Vec<RowData> = first
        .map(|f| RowData::new(book, db, &f.package, &f.installed, &f.target))
        .into_iter()
        .collect();
    let mut extra: Vec<String> = Vec::new();
    // Siblings matter only when the parent has its own row to agree with.
    if target.is_some() {
        let skip = first.map_or("", |f| f.package.as_str());
        rows.extend(sibling_rows(db, book, parent, skip));
        extra = super::parent_release_hop::lever_needs(db, book, parent);
    }
    let cleans: Vec<Box<dyn Fn(&str) -> bool + '_>> = rows.iter().map(RowData::clean).collect();
    let inputs: Vec<RowIn<'_>> = rows
        .iter()
        .zip(&cleans)
        .map(|(r, clean)| RowIn {
            child: r.child(clean.as_ref()),
            history: book.history(ECOSYSTEM, parent.name, &r.package),
            unknown: r.unknown(),
        })
        .collect();
    let decision = decide(
        &releases,
        parent.version,
        target.as_deref(),
        &own,
        &inputs,
        &extra,
        Dialect::Npm,
    );
    drop(inputs);
    drop(cleans);
    Some((decision, rows))
}

/// The plan row of a DECLARED vulnerable package, raised so one bump also
/// clears the transitive copies it pins (rule b): the highest release any of
/// them needs from it, across a major too (flagged; parity decision 3), when
/// no advisory of it affects that release. `Some((to, packages))` only when it rises above
/// `current`; `packages` are the ones that needed more than `current`.
pub(crate) fn raised_row_target(
    db: &Database,
    ecosystem_norm: &str,
    project: &str,
    package: &str,
    installed: &str,
    current: &str,
) -> Option<(String, Vec<String>)> {
    if ecosystem_norm != ECOSYSTEM {
        return None;
    }
    let book = super::release_facts::book(db);
    let project = normalize_path(project);
    let parent = ParentSide {
        project: &project,
        name: package,
        version: installed,
        direct: true,
    };
    let (decision, rows) = decision_for(db, &book, &parent, None, Some(current))?;
    let to = decision.row_target?;
    let above = |v: &str| compare(v, current, ECOSYSTEM) == Some(Ordering::Greater);
    if !above(&to) {
        return None;
    }
    let mut packages: Vec<String> = rows
        .iter()
        .zip(&decision.rows)
        .filter(|(_, out)| out.smallest().is_some_and(|f| above(&f.version)))
        .map(|(r, _)| r.package.clone())
        .collect();
    if packages.is_empty() {
        // Only a parent it requires pins the copies (a declared lever).
        packages = super::parent_release_hop::lever_children(&book, &parent);
    }
    Some((to, packages))
}

/// The row target of a declared package `name` `version` in `project`, when
/// it has a row of its own (rule b), as [`raised_row_target`] works it out.
pub(super) fn own_row_target(
    db: &Database,
    book: &ReleaseBook,
    project: &str,
    name: &str,
    version: &str,
) -> Option<String> {
    let lever = ParentSide {
        project,
        name,
        version,
        direct: true,
    };
    decision_for(db, book, &lever, None, None)?.0.row_target
}

/// Write the registry-read release into every link whose parent must move.
/// A link whose parent's releases were never read keeps what it had.
pub(crate) fn annotate(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    links: &mut [ParentLink],
    raw_path: &dyn Fn(&str) -> String,
) {
    if ecosystem_norm != ECOSYSTEM {
        return;
    }
    let book = super::release_facts::book(db);
    let others: Vec<(String, String, String)> = links
        .iter()
        .map(|l| (l.project.clone(), l.child_version.clone(), l.parent.clone()))
        .collect();
    for link in links
        .iter_mut()
        .filter(|l| !l.refresh.is_enough() && !l.is_bundled())
    {
        let shared = others.iter().any(|(p, v, parent)| {
            *p == link.project
                && *v == link.child_version
                && !parent.eq_ignore_ascii_case(&link.parent)
        });
        let dir = raw_path(&link.project);
        annotate_link(db, &book, package, link, &dir, shared);
    }
}

/// One link: its parent's release, then (when the parent arrives through
/// declared dependencies) each of theirs.
fn annotate_link(
    db: &Database,
    book: &ReleaseBook,
    package: &str,
    link: &mut ParentLink,
    dir: &str,
    shared: bool,
) {
    let Some(target) = link.target.clone() else {
        return;
    };
    let parent = ParentSide {
        project: &link.project,
        name: &link.parent,
        version: &link.parent_version,
        direct: link.parent_is_direct,
    };
    let first = RowData::new(book, db, package, &link.child_version, &target);
    // A malicious copy is removed or replaced, never moved to (review of #981).
    if let Some(id) = first.advisories.malware_at(&link.child_version) {
        link.standing = ParentStanding::Malicious {
            package: package.to_string(),
            advisory: id.to_string(),
        };
        return;
    }
    let Some((decision, rows)) = decision_for(db, book, &parent, Some(&first), None) else {
        return;
    };
    let (Some(out), Some(row)) = (decision.rows.first(), rows.first()) else {
        return;
    };
    let own = advisories(db, &link.parent);
    let releases = book.releases(ECOSYSTEM, &link.parent).unwrap_or_default();
    let newest = own.newest(&releases);
    let own_checked = own.any_fixable(newest.as_deref());
    link.standing = standing_of(out, link, row, dir, shared, own_checked);
    if let ParentStanding::Admits(r) = &mut link.standing {
        r.remains = own.unfixed_at(&r.to, newest.as_deref());
    }
    let ParentStanding::Admits(found) = &link.standing else {
        return;
    };
    let parent_to = found.to.clone();
    let installed_parent = (link.parent.clone(), link.parent_version.clone());
    let project = link.project.clone();
    if let Some(t) = link.through.as_mut() {
        let hop = super::parent_release_hop::HopCtx {
            db,
            book,
            project: &project,
            parent: &installed_parent,
            child: &first,
            parent_to: &parent_to,
            dir,
        };
        hop.annotate(t);
        for also in &mut t.also {
            hop.annotate(also);
        }
    }
}

/// The command that moves a declared npm dependency: the in-range refresh when
/// its declared range already admits the release (pr-001: `npm update
/// express`), else the install that keeps the operator.
pub(super) fn lever_command(dir: &str, name: &str, installed: &str, to: &str) -> Option<String> {
    match super::direct_upgrade::direct_fix(Path::new(dir), ECOSYSTEM, name, installed, to)? {
        super::direct_upgrade::DirectFix::Command(c)
        | super::direct_upgrade::DirectFix::Refresh(c) => Some(c),
        super::direct_upgrade::DirectFix::Instruction(_) => None,
    }
}

pub(super) fn refresh_command(
    dir: &str,
    package: &str,
    installed: &str,
    target: &str,
) -> Option<String> {
    Manager::detect(Path::new(dir), ECOSYSTEM)?.refresh_command(package, installed, target)
}

/// The link's standing from its row of the decision.
fn standing_of(
    out: &RowOut,
    link: &ParentLink,
    row: &RowData,
    dir: &str,
    shared: bool,
    own_checked: bool,
) -> ParentStanding {
    match (&out.standing, &out.to) {
        (Standing::Release(smallest), Some(to)) => ParentStanding::Admits(Box::new(release(
            smallest,
            to,
            link,
            row,
            dir,
            shared,
            own_checked,
        ))),
        (Standing::Deprecated, _) => ParentStanding::Deprecated,
        (Standing::Latest, _) => ParentStanding::Newest { deprecated: false },
        (Standing::NoneAdmits { latest }, _) => ParentStanding::NoneAdmits {
            latest: latest.clone(),
        },
        (Standing::Undecided(why), _) => ParentStanding::Undecided { why: why.clone() },
        (Standing::Introduces(ids), _) => ParentStanding::Introduces {
            advisories: ids.clone(),
        },
        (Standing::Malicious(id), _) => ParentStanding::Malicious {
            package: link.parent.clone(),
            advisory: id.clone(),
        },
        _ => link.standing.clone(),
    }
}

fn release(
    smallest: &Found,
    to: &Found,
    link: &ParentLink,
    row: &RowData,
    dir: &str,
    shared: bool,
    own_checked: bool,
) -> ParentRelease {
    // An install keeps a copy the new requirement still admits (npm 10.9.2);
    // with no requirement left, another parent may still hold it.
    let keeps = to.keeps_installed || (to.requirement.is_none() && shared);
    let lever_unknown = (!link.parent_is_direct && link.through.is_none()).then(|| {
        link.lever_unknown.clone().unwrap_or_else(|| {
            format!("no declared dependency above {} could be read", link.parent)
        })
    });
    ParentRelease {
        to: to.version.clone(),
        smallest: (smallest.version != to.version).then(|| smallest.version.clone()),
        breaking: is_major(&link.parent_version, &to.version),
        requirement: to.requirement.clone(),
        peer: to.peer,
        resolves_to: to.resolves_to.clone(),
        own_checked,
        command: link
            .parent_is_direct
            .then(|| lever_command(dir, &link.parent, &link.parent_version, &to.version))
            .flatten(),
        kept: keeps.then(|| row.installed.clone()),
        refresh: keeps
            .then(|| refresh_command(dir, &row.package, &row.installed, &row.target))
            .flatten(),
        lever_unknown,
        ..ParentRelease::default()
    }
}

#[cfg(test)]
#[path = "parent_release_load_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "parent_release_levers_tests.rs"]
mod levers_tests;
