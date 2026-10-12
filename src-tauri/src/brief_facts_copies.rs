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
                override_hint: crate::osv::fix_path::override_hint(
                    Manager::detect(std::path::Path::new(project), eco),
                    eco,
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
