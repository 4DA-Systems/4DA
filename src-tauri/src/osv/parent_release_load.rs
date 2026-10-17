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

use std::collections::BTreeSet;
use std::path::Path;

use fourda_depengine::fix_target::walk_target_in;

use crate::db::Database;
use crate::osv::types::MatchedAdvisory;

use super::fix_path::{requirement_admits, Dialect, Manager};
use super::parent_hint::{normalize_path, ParentLink, ParentRelease, ParentStanding, Through};
use super::parent_release::{
    decide, is_major, newest, through_release, Advisories, Child, Decision, Found, Hop, HopParent,
    RowIn, RowOut, Standing,
};
use super::release_facts::ReleaseBook;

/// The only ecosystem whose parent releases are read.
const ECOSYSTEM: &str = "npm";

/// Most other vulnerable children of one parent weighed when agreeing its
/// release with its own row.
const MAX_SIBLING_ROWS: usize = 12;

fn advisories(db: &Database, name: &str) -> Advisories<'static> {
    Advisories {
        ecosystem: ECOSYSTEM,
        ranges: db
            .get_osv_advisories_for_package(name, ECOSYSTEM)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.affected_ranges)
            .collect(),
    }
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
            if let Some(t) = &link.through {
                out.insert((t.name.clone(), link.parent.clone()));
            }
        }
    }
    out.into_iter().collect()
}

/// A row's inputs, owned, so [`Child`] can borrow them.
struct RowData {
    package: String,
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
        Self {
            package: package.to_string(),
            installed: installed.to_string(),
            target: target.to_string(),
            newest: newest(&releases, ECOSYSTEM),
            releases: releases.versions,
            advisories: advisories(db, package),
        }
    }
}

/// The parent side of one decision.
struct ParentSide<'a> {
    project: &'a str,
    name: &'a str,
    version: &'a str,
    /// Declared by the project: it can have a plan row of its own.
    direct: bool,
}

/// The target of the parent's own row: the walk past its own advisories
/// from the installed release, when one affects it and it is declared.
fn own_target(book: &ReleaseBook, parent: &ParentSide<'_>, own: &Advisories<'_>) -> Option<String> {
    if !parent.direct || !own.affect(parent.version) {
        return None;
    }
    let refs: Vec<&Option<String>> = own.ranges.iter().collect();
    walk_target_in(
        ECOSYSTEM,
        parent.version,
        &refs,
        &book.registry(ECOSYSTEM, parent.name),
    )
    .target
    .map(|w| w.version)
}

/// The other vulnerable copies `parent` pins in its project: each child its
/// requirements were read for, at the installed version its edge leads to,
/// with a target that requirement excludes.
fn sibling_rows(
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
            let refs: Vec<&Option<String>> = own.ranges.iter().collect();
            let walk = walk_target_in(
                ECOSYSTEM,
                &installed,
                &refs,
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
/// releases were never read.
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
    let mut rows: Vec<RowData> = Vec::new();
    if let Some(f) = first {
        rows.push(RowData::new(book, db, &f.package, &f.installed, &f.target));
    }
    // Siblings matter only when the parent has its own row to agree with.
    if target.is_some() {
        let skip = first.map_or("", |f| f.package.as_str());
        rows.extend(sibling_rows(db, book, parent, skip));
    }
    let cleans: Vec<Box<dyn Fn(&str) -> bool + '_>> = rows
        .iter()
        .map(|r| {
            Box::new(move |v: &str| r.advisories.clean_of_fixable(v, r.newest.as_deref()))
                as Box<dyn Fn(&str) -> bool>
        })
        .collect();
    let inputs: Vec<RowIn<'_>> = rows
        .iter()
        .zip(&cleans)
        .map(|(r, clean)| RowIn {
            child: Child {
                ecosystem: ECOSYSTEM,
                installed: &r.installed,
                target: &r.target,
                releases: &r.releases,
                clean: clean.as_ref(),
            },
            history: book.history(ECOSYSTEM, parent.name, &r.package),
        })
        .collect();
    let decision = decide(
        &releases,
        parent.version,
        target.as_deref(),
        &own,
        &inputs,
        Dialect::Npm,
    );
    drop(inputs);
    drop(cleans);
    Some((decision, rows))
}

/// The plan row of a DECLARED vulnerable package, raised so one bump also
/// clears the transitive copies it pins (rule b): the highest release any of
/// them needs from it, when that stays in its major and no advisory of it
/// affects that release. `Some((to, packages))` only when it rises above
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
    let rises = super::parent_release::compare(&to, current, ECOSYSTEM)
        == Some(std::cmp::Ordering::Greater);
    if !rises {
        return None;
    }
    let packages: Vec<String> = rows
        .iter()
        .zip(&decision.rows)
        .filter(|(_, out)| {
            out.smallest().is_some_and(|f| {
                super::parent_release::compare(&f.version, current, ECOSYSTEM)
                    == Some(std::cmp::Ordering::Greater)
            })
        })
        .map(|(r, _)| r.package.clone())
        .collect();
    Some((to, packages))
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
        let Some(target) = link.target.clone() else {
            continue;
        };
        let parent = ParentSide {
            project: &link.project,
            name: &link.parent,
            version: &link.parent_version,
            direct: link.parent_is_direct,
        };
        let first = RowData::new(&book, db, package, &link.child_version, &target);
        let Some((decision, rows)) = decision_for(db, &book, &parent, Some(&first), None) else {
            continue;
        };
        let (Some(out), Some(row)) = (decision.rows.first(), rows.first()) else {
            continue;
        };
        let shared = others.iter().any(|(p, v, parent)| {
            *p == link.project
                && *v == link.child_version
                && !parent.eq_ignore_ascii_case(&link.parent)
        });
        let dir = raw_path(&link.project);
        let own = advisories(db, &link.parent);
        let releases = book.releases(ECOSYSTEM, &link.parent).unwrap_or_default();
        let own_checked = own.any_fixable(newest(&releases, ECOSYSTEM).as_deref());
        link.standing = standing_of(out, link, row, &dir, shared, own_checked);
        let parent_to = match &link.standing {
            ParentStanding::Admits(found) => Some(found.to.clone()),
            _ => None,
        };
        let installed_parent = (link.parent.clone(), link.parent_version.clone());
        if let (Some(t), Some(to)) = (link.through.as_mut(), parent_to) {
            hop(db, &book, t, installed_parent, &first, &to, &dir);
        }
    }
}

/// The command that moves a declared npm dependency, the operator kept.
fn lever_command(dir: &str, name: &str, installed: &str, to: &str) -> Option<String> {
    match super::direct_upgrade::direct_fix(Path::new(dir), ECOSYSTEM, name, installed, to)? {
        super::direct_upgrade::DirectFix::Command(c) => Some(c),
        super::direct_upgrade::DirectFix::Instruction(_) => None,
    }
}

fn refresh_command(dir: &str, package: &str, installed: &str, target: &str) -> Option<String> {
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
    let lever_unknown = !link.parent_is_direct && link.through.is_none();
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
        brings: None,
        lever_unknown,
    }
}

/// The declared dependency the parent arrives through: when it requires the
/// parent directly, the smallest release of it that brings in a parent
/// release admitting a clean copy.
fn hop(
    db: &Database,
    book: &ReleaseBook,
    through: &mut Through,
    (parent, parent_version): (String, String),
    child: &RowData,
    parent_to: &str,
    dir: &str,
) {
    let (Some(lever), Some(lever_history), Some(parent_releases), Some(parent_history)) = (
        book.releases(ECOSYSTEM, &through.name),
        book.history(ECOSYSTEM, &through.name, &parent),
        book.releases(ECOSYSTEM, &parent),
        book.history(ECOSYSTEM, &parent, &child.package),
    ) else {
        return;
    };
    // Adjacent only: the installed lever requires the parent itself.
    let adjacent = lever_history
        .get(&through.version)
        .is_some_and(|r| !r.reqs.is_empty());
    if !adjacent {
        return;
    }
    let parent_own = advisories(db, &parent);
    let parent_newest = newest(&parent_releases, ECOSYSTEM);
    let parent_clean = |v: &str| parent_own.clean_of_fixable(v, parent_newest.as_deref());
    let lever_own = advisories(db, &through.name);
    let lever_newest = newest(&lever, ECOSYSTEM);
    let lever_clean = |v: &str| lever_own.clean_of_fixable(v, lever_newest.as_deref());
    let clean = |v: &str| {
        child
            .advisories
            .clean_of_fixable(v, child.newest.as_deref())
    };
    let c = Child {
        ecosystem: ECOSYSTEM,
        installed: &child.installed,
        target: &child.target,
        releases: &child.releases,
        clean: &clean,
    };
    let hp = HopParent {
        installed: &parent_version,
        releases: &parent_releases,
        history: parent_history,
        own_clean: &parent_clean,
    };
    through.standing = match through_release(
        &c,
        &hp,
        &lever,
        &through.version,
        lever_history,
        &lever_clean,
        Dialect::Npm,
    ) {
        Hop::Found {
            version,
            brings,
            keeps_parent,
        } => ParentStanding::Admits(Box::new(ParentRelease {
            breaking: is_major(&through.version, &version),
            command: lever_command(dir, &through.name, &through.version, &version),
            kept: keeps_parent.then(|| parent_version.clone()),
            refresh: keeps_parent
                .then(|| refresh_command(dir, &parent, &parent_version, &brings))
                .flatten(),
            own_checked: lever_own.any_fixable(lever_newest.as_deref()),
            smallest: (brings != parent_to).then(|| parent_to.to_string()),
            brings: Some(brings),
            to: version,
            ..ParentRelease::default()
        })),
        Hop::Deprecated => ParentStanding::Deprecated,
        Hop::Latest => ParentStanding::Newest { deprecated: false },
        Hop::NoneBrings { latest } => ParentStanding::NoneAdmits { latest },
    };
}

#[cfg(test)]
#[path = "parent_release_load_tests.rs"]
mod tests;
