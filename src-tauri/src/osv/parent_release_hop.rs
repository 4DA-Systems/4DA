// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A transitive parent's declared lever (audit H1; review of #981, S4): the
//! release of the dependency the project declares that brings in a parent
//! release admitting a clean copy, agreed with that dependency's own row.
//! NodeGoat: send 0.16.2 is pinned by serve-static 1.13.2, which arrives
//! through express 4.16.4; express 4.21.0 brings in serve-static 1.16.2, and
//! express's own row (raised for path-to-regexp and qs) is 4.22.3, which also
//! brings a fixed serve-static in, so the send row says express 4.22.3 too.
//! The MCP does the same through its lever map (`reconcileDirectTargets`).

use std::cmp::Ordering;

use crate::db::Database;

use super::fix_path::{requirement_admits, Dialect};
use super::parent_hint::{normalize_path, ParentRelease, ParentStanding, Through};
use super::parent_release::{
    admits, admits_clean, ascending, compare, is_major, latest_of, Advisories, Child, Releases,
    RequirementHistory,
};
use super::parent_release_load::{
    advisories, lever_command, own_row_target, refresh_command, sibling_rows, ParentSide, RowData,
    ECOSYSTEM,
};
use super::release_facts::ReleaseBook;

/// Everything one link's hops read.
pub(super) struct HopCtx<'a> {
    pub db: &'a Database,
    pub book: &'a ReleaseBook,
    pub project: &'a str,
    /// The transitive parent, as installed: (name, version).
    pub parent: &'a (String, String),
    /// The vulnerable copy the parent pins.
    pub child: &'a RowData,
    /// The parent release that admits a clean copy.
    pub parent_to: &'a str,
    pub dir: &'a str,
}

/// The parent's side of a hop, read from the stored registry facts.
struct ParentFacts<'a> {
    releases: Releases,
    history: &'a RequirementHistory,
    own: Advisories<'static>,
    newest: Option<String>,
}

fn parent_facts<'a>(
    db: &Database,
    book: &'a ReleaseBook,
    parent: &str,
    child: &str,
) -> Option<ParentFacts<'a>> {
    let releases = book.releases(ECOSYSTEM, parent)?;
    let history = book.history(ECOSYSTEM, parent, child)?;
    let own = advisories(db, parent);
    Some(ParentFacts {
        newest: own.newest(&releases),
        own,
        releases,
        history,
    })
}

/// The lever's release that brings in a fixed parent: the smallest, then
/// lifted to the lever's own row target when that brings one in too (with
/// the smallest it was lifted from).
fn lever_hop(
    hop: &HopCtx<'_>,
    through: &Through,
    lever_history: &RequirementHistory,
    parent: &ParentFacts<'_>,
) -> (Hop, Option<String>) {
    let lever = hop
        .book
        .releases(ECOSYSTEM, &through.name)
        .unwrap_or_default();
    let lever_own = advisories(hop.db, &through.name);
    let lever_newest = lever_own.newest(&lever);
    let lever_clean =
        |v: &str| lever_own.usable(v, lever_newest.as_deref(), Some(&through.version));
    let parent_clean = |v: &str| {
        parent
            .own
            .usable(v, parent.newest.as_deref(), Some(&hop.parent.1))
    };
    let clean = hop.child.clean();
    let child = hop.child.child(clean.as_ref());
    let hp = HopParent {
        installed: &hop.parent.1,
        releases: &parent.releases,
        history: parent.history,
        own_clean: &parent_clean,
    };
    let found = through_release(
        &child,
        &hp,
        &lever,
        &through.version,
        lever_history,
        &lever_clean,
        Dialect::Npm,
    );
    let Hop::Found { version, .. } = &found else {
        return (found, None);
    };
    // One release per declared dependency: its own row's, when that also
    // brings a fixed parent in and stays in the same major (rule b).
    let Some(row) = own_row_target(
        hop.db,
        hop.book,
        hop.project,
        &through.name,
        &through.version,
    ) else {
        return (found, None);
    };
    // A major step included: the lever's own row moves there anyway
    // (parity decision 3), and the release says it is a major upgrade.
    let lift = compare(version, &row, ECOSYSTEM) == Some(Ordering::Less);
    let smallest = version.clone();
    match hop_at(&child, &hp, lever_history, &lever_clean, &row, Dialect::Npm) {
        Some(lifted) if lift => (lifted, Some(smallest)),
        _ => (found, None),
    }
}

/// The advisories the lever releases that would bring a fixed parent in
/// bring in, when only advisories the installed lever lacks (fixed or not)
/// block them: sorted, deduped, unioned over every such release (review of
/// #994, S3/S4). Empty when no release is blocked only so.
fn lever_introduces(
    hop: &HopCtx<'_>,
    through: &Through,
    lever_history: &RequirementHistory,
    parent: &ParentFacts<'_>,
) -> Vec<String> {
    let lever = hop
        .book
        .releases(ECOSYSTEM, &through.name)
        .unwrap_or_default();
    let lever_own = advisories(hop.db, &through.name);
    let newest = lever_own.newest(&lever);
    let relaxed = |v: &str| lever_own.clean_but_introduced(v, newest.as_deref(), &through.version);
    let parent_clean = |v: &str| {
        parent
            .own
            .usable(v, parent.newest.as_deref(), Some(&hop.parent.1))
    };
    let clean = hop.child.clean();
    let child = hop.child.child(clean.as_ref());
    let hp = HopParent {
        installed: &hop.parent.1,
        releases: &parent.releases,
        history: parent.history,
        own_clean: &parent_clean,
    };
    let candidates = ascending(&lever.versions, ECOSYSTEM)
        .into_iter()
        .filter(|v| compare(v, &through.version, ECOSYSTEM) == Some(Ordering::Greater))
        .filter(|v| hop_at(&child, &hp, lever_history, &relaxed, v, Dialect::Npm).is_some())
        .map(String::as_str);
    lever_own.introduced_union(candidates, newest.as_deref(), &through.version)
}

impl HopCtx<'_> {
    /// Fill `through.standing` with its release, when it requires the parent
    /// directly (adjacent) and every registry fact needed was read.
    pub(super) fn annotate(&self, through: &mut Through) {
        let parent = &self.parent.0;
        let Some(lever_history) = self.book.history(ECOSYSTEM, &through.name, parent) else {
            return;
        };
        let adjacent = lever_history
            .get(&through.version)
            .is_some_and(|r| !r.reqs.is_empty());
        let facts = parent_facts(self.db, self.book, parent, &self.child.package);
        let (true, Some(facts)) = (adjacent, facts) else {
            return;
        };
        let own = advisories(self.db, &through.name);
        if let Some(id) = own.malware_at(&through.version) {
            through.standing = ParentStanding::Malicious {
                package: through.name.clone(),
                advisory: id.to_string(),
            };
            return;
        }
        if let Some(id) = own.unreadable().or(facts.own.unreadable()) {
            through.standing = ParentStanding::Undecided {
                why: format!("the affected ranges of {id} cannot be read"),
            };
            return;
        }
        let (hop, smallest) = lever_hop(self, through, lever_history, &facts);
        through.standing = match hop {
            Hop::Found {
                version,
                brings,
                keeps_parent,
            } => ParentStanding::Admits(Box::new(self.release(
                through,
                (version, smallest),
                brings,
                keeps_parent,
            ))),
            Hop::Deprecated => ParentStanding::Deprecated,
            Hop::Latest => ParentStanding::Newest { deprecated: false },
            Hop::NoneBrings { latest } => {
                // Parity decision 2 for levers (the MCP's stuck lever,
                // "introduces"): a release would bring a fixed parent in, but
                // only advisories the installed lever lacks block it.
                let ids = lever_introduces(self, through, lever_history, &facts);
                if ids.is_empty() {
                    ParentStanding::NoneAdmits { latest }
                } else {
                    ParentStanding::Introduces { advisories: ids }
                }
            }
        };
    }

    /// The lever's release; `smallest` when it was lifted to its own row.
    fn release(
        &self,
        through: &Through,
        (version, smallest): (String, Option<String>),
        brings: String,
        keeps: bool,
    ) -> ParentRelease {
        let (parent, parent_version) = self.parent;
        let lever_releases = self
            .book
            .releases(ECOSYSTEM, &through.name)
            .unwrap_or_default();
        let lever_own = advisories(self.db, &through.name);
        let lever_newest = lever_own.newest(&lever_releases);
        ParentRelease {
            breaking: is_major(&through.version, &version),
            command: lever_command(self.dir, &through.name, &through.version, &version),
            kept: keeps.then(|| parent_version.clone()),
            refresh: keeps
                .then(|| refresh_command(self.dir, parent, parent_version, &brings))
                .flatten(),
            own_checked: lever_own.any_fixable(lever_newest.as_deref()),
            remains: lever_own.unfixed_at(&version, lever_newest.as_deref()),
            smallest,
            floor: (brings != self.parent_to).then(|| self.parent_to.to_string()),
            brings: Some(brings),
            to: version,
            ..ParentRelease::default()
        }
    }
}

/// The installed versions of `child` in `project` that `requirement` admits.
fn installed_under(db: &Database, project: &str, child: &str, requirement: &str) -> Vec<String> {
    let mut out: Vec<String> = db
        .get_package_instances(ECOSYSTEM, child)
        .unwrap_or_default()
        .into_iter()
        .filter(|i| normalize_path(&i.project_path) == project)
        .map(|i| i.version)
        .filter(|v| requirement_admits(requirement, v) == Some(true))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The parents `lever` requires that pin vulnerable copies (their
/// requirements were read for them): (name, installed version).
fn pinned_parents(
    db: &Database,
    book: &ReleaseBook,
    lever: &ParentSide<'_>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for parent in book.children(ECOSYSTEM, lever.name) {
        if book.children(ECOSYSTEM, &parent).is_empty() {
            continue;
        }
        let Some(req) = book
            .history(ECOSYSTEM, lever.name, &parent)
            .and_then(|h| h.get(lever.version))
            .and_then(|r| r.reqs.first())
        else {
            continue;
        };
        for v in installed_under(db, lever.project, &parent, req) {
            out.push((parent.clone(), v));
        }
    }
    out
}

/// The releases of a declared `lever` that the copies pinned by the parents
/// it requires need (each the smallest lever release that brings a fixed
/// parent in): they raise the lever's own row like its direct copies do.
pub(super) fn lever_needs(
    db: &Database,
    book: &ReleaseBook,
    lever: &ParentSide<'_>,
) -> Vec<String> {
    let mut needs = Vec::new();
    let Some(lever_releases) = book.releases(ECOSYSTEM, lever.name) else {
        return needs;
    };
    let lever_own = advisories(db, lever.name);
    let lever_newest = lever_own.newest(&lever_releases);
    let lever_clean = |v: &str| lever_own.usable(v, lever_newest.as_deref(), Some(lever.version));
    for (parent, version) in pinned_parents(db, book, lever) {
        let side = ParentSide {
            project: lever.project,
            name: &parent,
            version: &version,
            direct: false,
        };
        let Some(lever_history) = book.history(ECOSYSTEM, lever.name, &parent) else {
            continue;
        };
        for row in sibling_rows(db, book, &side, "") {
            let Some(facts) = parent_facts(db, book, &parent, &row.package) else {
                continue;
            };
            let parent_clean =
                |v: &str| facts.own.usable(v, facts.newest.as_deref(), Some(&version));
            let clean = row.clean();
            let hp = HopParent {
                installed: &version,
                releases: &facts.releases,
                history: facts.history,
                own_clean: &parent_clean,
            };
            let hop = through_release(
                &row.child(clean.as_ref()),
                &hp,
                &lever_releases,
                lever.version,
                lever_history,
                &lever_clean,
                Dialect::Npm,
            );
            if let Hop::Found { version, .. } = hop {
                needs.push(version);
            }
        }
    }
    needs
}

/// The packages the parents a declared `lever` requires were read for: what
/// "still requires a vulnerable ..." names when only those raise its row.
pub(super) fn lever_children(book: &ReleaseBook, lever: &ParentSide<'_>) -> Vec<String> {
    let mut out: Vec<String> = book
        .children(ECOSYSTEM, lever.name)
        .iter()
        .flat_map(|p| book.children(ECOSYSTEM, p))
        .collect();
    out.sort();
    out.dedup();
    out
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
    let latest = latest_of(lever, eco);
    if latest
        .as_deref()
        .is_some_and(|l| compare(lever_from, l, eco).is_some_and(|o| o != Ordering::Less))
    {
        return Hop::Latest;
    }
    ascending(&lever.versions, eco)
        .into_iter()
        .filter(|v| compare(v, lever_from, eco) == Some(Ordering::Greater))
        .find_map(|v| hop_at(child, parent, lever_history, lever_clean, v, dialect))
        .unwrap_or(Hop::NoneBrings { latest })
}

/// Lever release `version`, clear of the lever's own advisories, brings in
/// (its one requirement on the parent resolves to) a parent release above the
/// installed one, clear of the parent's advisories, that admits a clean copy.
pub fn hop_at(
    child: &Child<'_>,
    parent: &HopParent<'_>,
    lever_history: &RequirementHistory,
    lever_clean: &dyn Fn(&str) -> bool,
    version: &str,
    dialect: Dialect,
) -> Option<Hop> {
    let eco = child.ecosystem;
    let entry = lever_history.get(version)?;
    let [req] = entry.reqs.as_slice() else {
        return None;
    };
    if !lever_clean(version) {
        return None;
    }
    let brings = ascending(&parent.releases.versions, eco)
        .into_iter()
        .rev()
        .find(|p| admits(req, p, dialect) == Some(true))?;
    let above = compare(brings, parent.installed, eco) == Some(Ordering::Greater);
    if !above || !(parent.own_clean)(brings) {
        return None;
    }
    admits_clean(child, &parent.history.get(brings.as_str())?.reqs, dialect)?;
    Some(Hop::Found {
        version: version.to_string(),
        brings: brings.clone(),
        keeps_parent: admits(req, parent.installed, dialect) == Some(true),
    })
}
