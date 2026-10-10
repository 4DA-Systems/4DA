// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The words for a transitive copy's fix: which installed parent brings it
//! in, whether a lockfile refresh is enough (and the command that does it),
//! and which newer parent release a project here already resolved to a clear
//! version (`osv::parent_hint`, `osv::fix_path`). Carried in the canonical
//! item — explanation text and `dependency-path` citations — never a new
//! type (doctrine rule 1). The Upgrade Plan and the Preemption alert print
//! the same sentence.

use crate::db::Database;
use crate::osv::fix_path::{Basis, Refresh};
use crate::osv::fix_target::LineTarget;
use crate::osv::parent_hint::{ParentLink, TransitiveRoutes, UnlinkedSite};

use super::types::EvidenceCitation;
use super::upgrade_plan::truncate;

/// Most links / resolutions spelled out in the explanation (the citations
/// carry the rest up to [`MAX_PATH_CITATIONS`]).
const MAX_LINKS_IN_TEXT: usize = 2;
const MAX_PATH_CITATIONS: usize = 4;
/// Most distinct refresh commands named.
const MAX_COMMANDS: usize = 2;

/// Generic text when nothing about the copies' route could be read.
pub(super) const GENERIC_UPSTREAM: &str =
    "Fixed only upstream — awaits a parent-package update or lockfile refresh";

/// The route sentence for a Preemption alert whose every named copy is
/// transitive and has a target: the same words the plan step prints.
/// `None` when some copy is direct or none has a target.
pub(crate) fn transitive_route_note(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    lines: &[LineTarget],
    multi_project: bool,
) -> Option<String> {
    let all_transitive = lines.iter().all(|l| l.sites.iter().all(|s| !s.is_direct));
    if !all_transitive || lines.iter().all(|l| l.target_version.is_none()) {
        return None;
    }
    let routes = crate::osv::parent_hint::load_routes(db, ecosystem_norm, package, lines);
    Some(upstream_note(package, &routes, multi_project))
}

/// The scope sentence for a step that is NOT fixable by a direct bump. No
/// trailing period (the caller adds it).
pub(super) fn upstream_note(
    package: &str,
    routes: &TransitiveRoutes,
    multi_project: bool,
) -> String {
    let unlinked = &routes.unlinked;
    if routes.links.is_empty() && unlinked.is_empty() {
        return GENERIC_UPSTREAM.to_string();
    }
    // A copy any of whose parents excludes the target is not fixed by a
    // refresh, whatever its other parents allow (traefik/webui's js-yaml
    // 3.13.1: three parents take ^3.13, mocha pins 3.13.1). Those links lead.
    let mut links: Vec<&ParentLink> = routes.links.iter().collect();
    links.sort_by_key(|l| (l.refresh.is_enough(), !blocked(&routes.links, l)));
    let mut parts: Vec<String> = links
        .iter()
        .map(|l| {
            let sentence = link_sentence(package, l, multi_project);
            if l.refresh.is_enough() && blocked(&routes.links, l) {
                // This parent allows the target; another parent of the same
                // copy does not, so it is not "a refresh is enough".
                let cut = sentence
                    .find(" — a lockfile refresh")
                    .unwrap_or(sentence.len());
                let cut = cut.min(sentence.find(", so a lockfile refresh").unwrap_or(cut));
                format!("{} (another parent of this copy pins it)", &sentence[..cut])
            } else {
                sentence
            }
        })
        .chain(
            unlinked
                .iter()
                .map(|u| unlinked_sentence(package, u, multi_project)),
        )
        .collect();
    let total = parts.len();
    parts.truncate(MAX_LINKS_IN_TEXT);
    if total > MAX_LINKS_IN_TEXT {
        let more = total - MAX_LINKS_IN_TEXT;
        parts.push(format!(
            "{more} more parent {}",
            if more == 1 { "path" } else { "paths" }
        ));
    }
    let refreshable = links.iter().filter(|l| !blocked(&routes.links, l)).count()
        + unlinked.iter().filter(|u| u.refresh.is_enough()).count();
    let head = if refreshable == links.len() + unlinked.len() {
        "No manifest change needed"
    } else if refreshable == 0 {
        "Fixed only upstream"
    } else {
        "Transitive"
    };
    let mut out = format!("{head}: {}", parts.join("; "));
    let commands = commands(routes);
    if !commands.is_empty() {
        out.push_str(&format!(
            ". Run: {}",
            commands
                .iter()
                .map(|(c, projects)| {
                    if multi_project {
                        format!("`{c}` (in {})", projects.join(", "))
                    } else {
                        format!("`{c}`")
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        ));
        // `--precise` moves only that crate; when the target needs a newer
        // copy of something else that is locked, Cargo stops and names it
        // (nushell's openssl 0.10.80 needs a newer quote, 2026-10-10 oracle).
        if commands.iter().any(|(c, _)| c.starts_with("cargo update")) {
            out.push_str(" (if Cargo reports a conflict, `cargo update -p` the locked crate it names, then rerun)");
        }
    }
    if let Some(seen) = seen_sentence(package, &routes.links) {
        out.push_str(". ");
        out.push_str(&seen);
    }
    out
}

/// Some link of the same copy (project, installed version) does not admit a
/// refresh — so no refresh fixes that copy, whatever this link allows.
fn blocked(all: &[ParentLink], link: &ParentLink) -> bool {
    all.iter().any(|l| {
        l.project == link.project && l.child_version == link.child_version && !l.refresh.is_enough()
    })
}

/// The distinct refresh commands of the copies a refresh fixes, in route
/// order, each with the projects it applies to (at most two named).
fn commands(routes: &TransitiveRoutes) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let all = routes
        .links
        .iter()
        .filter(|l| !blocked(&routes.links, l))
        .filter_map(|l| l.command.clone().map(|c| (c, &l.project)))
        .chain(
            routes
                .unlinked
                .iter()
                .filter_map(|u| u.command.clone().map(|c| (c, &u.project))),
        );
    for (c, project) in all {
        match out.iter_mut().find(|(seen, _)| *seen == c) {
            Some((_, projects)) => {
                if !projects.contains(project) && projects.len() < MAX_COMMANDS {
                    projects.push(project.clone());
                }
            }
            None => out.push((c, vec![project.clone()])),
        }
    }
    out.truncate(MAX_COMMANDS);
    out
}

fn place(project: &str, multi_project: bool) -> String {
    if multi_project {
        format!(" in {project}")
    } else {
        String::new()
    }
}

fn link_sentence(package: &str, l: &ParentLink, multi_project: bool) -> String {
    let direct = if l.parent_is_direct {
        " (a direct dependency)"
    } else {
        ""
    };
    let head = format!(
        "{package} {} comes in through {} {}{direct}{}",
        l.child_version,
        l.parent,
        l.parent_version,
        place(&l.project, multi_project)
    );
    let Some(target) = l.target.as_deref() else {
        return head;
    };
    let req = l.requirement.as_deref().unwrap_or("");
    match l.refresh {
        Refresh::Enough(Basis::Requirement) => format!(
            "{head}, whose requirement {req} already admits {target} — a lockfile refresh is enough"
        ),
        Refresh::ParentMustMove(Basis::Requirement) => format!(
            "{head}, whose requirement {req} excludes {target} — {} itself must be updated",
            l.parent
        ),
        Refresh::Enough(Basis::Semver) => format!(
            "{head}; its requirement is not on disk, but {target} is semver-compatible with {}, \
             so a lockfile refresh should reach it",
            l.child_version
        ),
        Refresh::ParentMustMove(Basis::Semver) => format!(
            "{head}; {target} is a semver-incompatible jump from {}, so {} itself must be updated",
            l.child_version, l.parent
        ),
        Refresh::Unknown => format!(
            "{head} — update {}, or refresh the lockfile if its requirement admits {package} {target}",
            l.parent
        ),
    }
}

fn unlinked_sentence(package: &str, u: &UnlinkedSite, multi_project: bool) -> String {
    let head = format!(
        "{package} {}{} is transitive (the lockfile names no parent 4DA can read)",
        u.installed,
        place(&u.project, multi_project)
    );
    match u.refresh {
        Refresh::Enough(_) => format!(
            "{head}; {} is semver-compatible with {}, so a lockfile refresh should reach it",
            u.target, u.installed
        ),
        Refresh::ParentMustMove(_) => format!(
            "{head}; {} is a semver-incompatible jump, so the dependency that pulls it in must be updated",
            u.target
        ),
        Refresh::Unknown => format!("{head}; it awaits a parent-package update or lockfile refresh"),
    }
}

/// "Seen in your projects: victauri-plugin 0.9.0 resolves rmcp 3.4.1
/// (d:/4da/src-tauri) — no advisory 4DA holds affects that version".
fn seen_sentence(package: &str, links: &[ParentLink]) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for l in links.iter().filter(|l| needs_parent_move(l)) {
        for r in &l.resolutions {
            let s = format!(
                "{} {} resolves {package} {} ({})",
                l.parent, r.parent_version, r.child_version, r.project
            );
            if !seen.contains(&s) {
                seen.push(s);
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    let n = seen.len();
    seen.truncate(MAX_LINKS_IN_TEXT);
    Some(format!(
        "Seen in your projects: {} — no advisory 4DA holds affects {}",
        seen.join("; "),
        if n == 1 {
            "that version"
        } else {
            "those versions"
        }
    ))
}

/// A newer parent is only worth citing where a lockfile refresh is not
/// already enough.
fn needs_parent_move(l: &ParentLink) -> bool {
    !l.refresh.is_enough()
}

/// The single parent every link goes through, for the step title — `None`
/// when there are several, or when a lockfile refresh alone clears every link
/// (then the parent is not the move).
pub(super) fn single_parent(links: &[ParentLink]) -> Option<&str> {
    let first = links.first()?;
    if links.iter().any(|l| l.parent != first.parent) {
        return None;
    }
    if links.iter().all(|l| l.refresh.is_enough()) {
        return None;
    }
    Some(first.parent.as_str())
}

/// One `dependency-path` citation per link, then one per newer-parent
/// observation, capped.
pub(super) fn path_citations(package: &str, links: &[ParentLink]) -> Vec<EvidenceCitation> {
    let mut out: Vec<EvidenceCitation> = Vec::new();
    for l in links {
        out.push(citation(
            format!(
                "{package} {} <- {} {}",
                l.child_version, l.parent, l.parent_version
            ),
            format!(
                "In {}; {} is a {} dependency; {}",
                l.project,
                l.parent,
                if l.parent_is_direct {
                    "direct"
                } else {
                    "transitive"
                },
                requirement_note(l)
            ),
        ));
    }
    for l in links.iter().filter(|l| needs_parent_move(l)) {
        for r in &l.resolutions {
            let title = format!(
                "{} {} -> {package} {}",
                l.parent, r.parent_version, r.child_version
            );
            if out.iter().any(|c| c.title == title) {
                continue;
            }
            out.push(citation(
                title,
                format!(
                    "Resolved in {}; no advisory 4DA holds affects {package} {}",
                    r.project, r.child_version
                ),
            ));
        }
    }
    out.truncate(MAX_PATH_CITATIONS);
    out
}

fn requirement_note(l: &ParentLink) -> String {
    let req = l.requirement.as_deref();
    match (l.refresh, req, l.target.as_deref()) {
        (Refresh::Enough(Basis::Requirement), Some(r), Some(t)) => {
            format!("requires {r}, which admits {t}")
        }
        (Refresh::ParentMustMove(Basis::Requirement), Some(r), Some(t)) => {
            format!("requires {r}, which excludes {t}")
        }
        (Refresh::Enough(Basis::Semver), _, Some(t)) => format!(
            "requirement not on disk; {t} is semver-compatible with {}",
            l.child_version
        ),
        (Refresh::ParentMustMove(Basis::Semver), _, Some(t)) => format!(
            "requirement not on disk; {t} is a semver-incompatible jump from {}",
            l.child_version
        ),
        (_, Some(r), _) => format!("requires {r}"),
        (_, None, _) => "requirement not recorded in the lockfile".to_string(),
    }
}

fn citation(title: String, note: String) -> EvidenceCitation {
    EvidenceCitation {
        source: "dependency-path".to_string(),
        title: truncate(&title, 160),
        url: None,
        freshness_days: 0.0,
        relevance_note: truncate(&note, 200),
    }
}

#[cfg(test)]
#[path = "upgrade_parent_tests.rs"]
mod tests;
