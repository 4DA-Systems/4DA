// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Brief facts, per-copy fix targets and parent routes: which copies of a
//! package a project holds, the fix each copy's own release line takes, and
//! what 4DA's data says about moving the parent that holds a copy back.
//! Split from `brief_facts_build.rs` (file-size gate).

use std::collections::BTreeMap;

use crate::db::Database;
use crate::osv::fix_path::Manager;

use super::{norm_path, FixPath, LineFix, ParentRoute};

/// Every distinct version-confirmed copy of the package in one project (a
/// project can hold brace-expansion 1.1.12 beside 2.0.2, each on its own
/// line and its own route), a declared instance preferred per version.
/// `[None]` when the matcher recorded no confirmed copy there.
pub(crate) fn project_copies<'a>(
    group: &[&'a crate::osv::types::MatchedAdvisory],
    pnorm: &str,
) -> Vec<Option<&'a crate::osv::types::MatchedDependency>> {
    let mut by_version: BTreeMap<&str, &crate::osv::types::MatchedDependency> = BTreeMap::new();
    for d in group
        .iter()
        .flat_map(|m| m.dependency_instances.iter())
        .filter(|d| d.is_version_confirmed && norm_path(&d.project_path) == pnorm)
    {
        let Some(v) = d.installed_version.as_deref() else {
            continue;
        };
        by_version
            .entry(v)
            .and_modify(|kept| {
                if d.is_direct && !kept.is_direct {
                    *kept = d;
                }
            })
            .or_insert(d);
    }
    if by_version.is_empty() {
        return vec![None];
    }
    by_version.into_values().map(Some).collect()
}

/// What one copy's fix is.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CopyFix {
    /// The copy's own line target (`None`: no way out on its line).
    Target(Option<String>),
    /// The copy is not identifiable and the lines' targets differ.
    PerLine(Vec<LineFix>),
}

/// The fix target of the copy at `installed` in `pnorm`: its own release
/// line's target. With no line for it (a name-only match), the alert's
/// single target (`alert_fix`); with no installed version to place it on a
/// line, the lines' targets when they disagree.
pub(crate) fn copy_fix(
    lines: &[crate::osv::fix_target::LineTarget],
    pnorm: &str,
    installed: Option<&str>,
    alert_fix: Option<&str>,
) -> CopyFix {
    let in_project = |l: &&crate::osv::fix_target::LineTarget| {
        l.sites.iter().any(|s| norm_path(&s.project_path) == pnorm)
    };
    if let Some(v) = installed {
        if let Some(line) = lines
            .iter()
            .filter(in_project)
            .find(|l| l.installed_version == v)
        {
            return CopyFix::Target(line.target_version.clone());
        }
    }
    if let Some(fix) = alert_fix {
        return CopyFix::Target(Some(fix.to_string()));
    }
    let fixes: Vec<LineFix> = lines
        .iter()
        .filter(in_project)
        .filter_map(|l| {
            l.target_version.as_ref().map(|to| LineFix {
                installed: l.installed_version.clone(),
                to: to.clone(),
            })
        })
        .collect();
    match fixes.as_slice() {
        [] => CopyFix::Target(None),
        [one] => CopyFix::Target(Some(one.to.clone())),
        _ => CopyFix::PerLine(fixes),
    }
}

/// Fill a parent path's route — the direct dependency the parent arrives
/// through, the registry's record of newer parent releases, the override
/// mechanism — from the Upgrade Plan's own link for the same parent, so the
/// two surfaces say the same thing; read directly when the plan has none.
pub(crate) fn with_parent_route(
    path: FixPath,
    links: &[crate::osv::parent_hint::ParentLink],
    pnorm: &str,
    project: &str,
    eco_norm: Option<&str>,
    db: &Database,
    installed: Option<&str>,
) -> FixPath {
    let FixPath::Parent {
        parent,
        parent_version,
        to,
        by_requirement,
        proven,
        route,
    } = path
    else {
        return path;
    };
    let planned = links.iter().find(|l| {
        l.project == pnorm
            && l.parent.eq_ignore_ascii_case(&parent)
            && l.parent_version == parent_version
            && !l.refresh.is_enough()
    });
    let route = match (planned, eco_norm) {
        (Some(l), _) => ParentRoute {
            through: l.through.clone().or(route.through),
            standing: l.standing.clone(),
            override_hint: l.override_hint.clone(),
        },
        (None, Some(eco)) => {
            let standing_of =
                |name: &str, v: &str| crate::osv::parent_hint::parent_standing(db, eco, name, v);
            ParentRoute {
                standing: standing_of(&parent, &parent_version),
                through: route.through.map(|t| crate::osv::parent_hint::Through {
                    standing: standing_of(&t.name, &t.version),
                    ..t
                }),
                // No plan link names this parent (the Brief's immediate
                // parent comes from its own edge walk). If the plan holds a
                // blocking link for THIS copy through a bundle, no override
                // reaches it; another copy's bundle says nothing about this one.
                override_hint: crate::osv::fix_path::override_hint(
                    Manager::detect(std::path::Path::new(project), eco),
                    eco,
                    links.iter().any(|l| {
                        l.project == pnorm
                            && installed.is_none_or(|v| l.child_version == v)
                            && !l.refresh.is_enough()
                            && l.is_bundled()
                    }),
                ),
            }
        }
        (None, None) => route,
    };
    FixPath::Parent {
        parent,
        parent_version,
        to,
        by_requirement,
        proven,
        route,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osv::fix_path::{Basis, Refresh};
    use crate::osv::parent_hint::{ParentLink, ParentStanding};

    const P: &str = "d:/work/web";

    fn link(parent: &str, requirement: &str) -> ParentLink {
        ParentLink {
            project: P.to_string(),
            child_version: "4.1.1".to_string(),
            target: Some("4.3.1".to_string()),
            parent: parent.to_string(),
            parent_version: "1.2.13".to_string(),
            parent_is_direct: false,
            requirement: Some(requirement.to_string()),
            refresh: Refresh::ParentMustMove(Basis::Requirement),
            command: None,
            resolutions: Vec::new(),
            through: None,
            standing: ParentStanding::Unknown,
            override_hint: None,
            lever_unknown: None,
        }
    }

    /// The Brief's parent for debug 4.1.1 is chokidar; the plan's link names
    /// fsevents, the bundle owner. No plan link matches chokidar, so the route
    /// is read directly — and must not offer an npm override for a bundled
    /// copy (audit E8: `overrides` do not reach bundled dependencies).
    fn route_with(links: &[ParentLink]) -> ParentRoute {
        let path = FixPath::Parent {
            parent: "chokidar".to_string(),
            parent_version: "2.1.8".to_string(),
            to: "4.3.1".to_string(),
            by_requirement: true,
            proven: None,
            route: ParentRoute::default(),
        };
        let db = crate::test_utils::test_db();
        match with_parent_route(path, links, P, P, Some("npm"), &db, Some("4.1.1")) {
            FixPath::Parent { route, .. } => route,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_bundled_copy_gets_no_override_on_the_brief_route() {
        assert_eq!(
            route_with(&[link("fsevents", "4.1.1 (bundled)")]).override_hint,
            None
        );
        // An ordinary pin still gets the override.
        assert!(route_with(&[link("needle", "4.1.1")])
            .override_hint
            .is_some());
        // ANOTHER copy's bundle (debug 2.2.0 inside nyc) says nothing about
        // 4.1.1: its override advice stays.
        let other_copy = ParentLink {
            child_version: "2.2.0".to_string(),
            ..link("nyc", "2.2.0 (bundled)")
        };
        assert!(route_with(&[other_copy]).override_hint.is_some());
    }
}
