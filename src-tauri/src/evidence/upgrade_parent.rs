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
use crate::osv::parent_hint::{
    ParentLink, ParentStanding, Through, TransitiveRoutes, UnlinkedSite,
};

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
    if l.is_bundled() {
        // The edge records the shipped version, not a requirement: say what
        // it is, never print the internal marker.
        return format!(
            "{head}, which ships it bundled inside its own package, so neither a lockfile \
             refresh nor an override can move it to {target} — {}",
            link_move(package, target, l)
        );
    }
    let req = l.requirement.as_deref().unwrap_or("");
    match l.refresh {
        Refresh::Enough(Basis::Requirement) => format!(
            "{head}, whose requirement {req} already admits {target} — a lockfile refresh is enough"
        ),
        Refresh::ParentMustMove(Basis::Requirement) => format!(
            "{head}, whose requirement {req} excludes {target} — {}",
            link_move(package, target, l)
        ),
        Refresh::Enough(Basis::Semver) => format!(
            "{head}; its requirement is not on disk, but {target} is semver-compatible with {}, \
             so a lockfile refresh should reach it",
            l.child_version
        ),
        Refresh::ParentMustMove(Basis::Semver) => format!(
            "{head}; {target} is a semver-incompatible jump from {}, so {}",
            l.child_version,
            link_move(package, target, l)
        ),
        Refresh::Unknown => format!(
            "{head} — update {}, or refresh the lockfile if its requirement admits {package} {target}",
            l.parent
        ),
    }
}

/// The move for a link whose parent's line excludes the target. A newer
/// parent a project here already resolved clear is proof an update exists
/// ("Seen in your projects" follows); otherwise the registry rows decide
/// what may be claimed ([`parent_move`]).
fn link_move(package: &str, target: &str, l: &ParentLink) -> String {
    // A release read from the registry's own requirements (audit H1) is the
    // move itself; a newer parent seen on this machine only proves one exists.
    let registry_read = matches!(
        l.standing,
        ParentStanding::Admits(_) | ParentStanding::NoneAdmits { .. }
    );
    if !l.resolutions.is_empty() && !registry_read {
        return format!("{} itself must be updated", l.parent);
    }
    parent_move(
        package,
        target,
        &l.parent,
        &l.parent_version,
        &l.standing,
        l.through.as_ref(),
        l.override_hint.as_deref(),
    )
}

/// What the reader can do when `parent` `parent_version` keeps `package` off
/// `target`, saying only what 4DA's own data supports. The lever is the
/// package the project declares: `through` when the parent arrives through
/// a direct dependency, else the parent itself. Its registry standing
/// decides the words:
/// - a newer release seen: upgrade it, and check that it admits the target
///   (its requirement is not on disk);
/// - already the newest release seen (or deprecated): no upgrade is known
///   to help, so replace it or force the copy;
/// - nothing on record: it must move, and whether a release that admits the
///   target exists is left open.
///
/// Every version follows its own package's name (the Brief's version check,
/// `briefing_groundedness::check_factual_claims`, reads the nearest
/// preceding package). Shared by the Upgrade Plan, Preemption and the Brief
/// (`brief_facts::fix_clause`), so the three name the same parent and the
/// same move.
pub(crate) fn parent_move(
    package: &str,
    target: &str,
    parent: &str,
    parent_version: &str,
    standing: &ParentStanding,
    through: Option<&Through>,
    override_hint: Option<&str>,
) -> String {
    let force = override_hint.map_or_else(String::new, |h| {
        format!(", or force {package} >= {target} with {h}")
    });
    match standing {
        ParentStanding::Admits(r) => {
            super::upgrade_parent_release::admits_move(package, target, parent, r, through, &force)
        }
        ParentStanding::Malicious { package, advisory } => format!(
            "{package} is flagged malicious ({advisory}): remove or replace {package}; no \
             {parent} release is named as the fix"
        ),
        ParentStanding::Introduces {
            advisories,
            deprecated,
        } => format!(
            "{}{force}",
            introduces_clause(parent, advisories, *deprecated)
        ),
        ParentStanding::Undecided { why } => {
            format!("no {parent} release is named because {why}, so no command is named{force}")
        }
        ParentStanding::NoneAdmits { .. } => {
            // No release is named here, not even the newest: the parent's own
            // row (and its other rows) name the one release it moves to.
            none_admits_move(package, target, parent, through, &force)
        }
        _ => row_move(
            package,
            target,
            parent,
            parent_version,
            standing,
            through,
            &force,
        ),
    }
}

/// No `name` release is named because every fixing one brings in an
/// advisory the installed version lacks (parity decision 2); a deprecated
/// package says so too (review of #994, B3).
pub(super) fn introduces_clause(name: &str, advisories: &[String], deprecated: bool) -> String {
    let dep = if deprecated {
        format!(", and {name} is deprecated")
    } else {
        String::new()
    };
    format!(
        "no {name} release is named: {}{dep}",
        crate::osv::fix_target::introduces_text(advisories)
    )
}

/// Every release of the parent was read and none admits a clean copy.
fn none_admits_move(
    package: &str,
    target: &str,
    parent: &str,
    through: Option<&Through>,
    force: &str,
) -> String {
    let replace = through.map_or_else(
        || format!("replace {parent}"),
        |t| {
            let names: Vec<&str> = t.all().map(|l| l.name.as_str()).collect();
            format!(
                "{parent} arrives through {}, so replace {}",
                lever_names(t),
                names.join(" or ")
            )
        },
    );
    format!(
        "no {parent} release up to its newest admits a clean {package} (>= {target}) while clear of \
         {parent}'s own advisories, so no {parent} upgrade fixes it; {replace}{force}"
    )
}

/// "a 1.0.0 and b 1.0.0" (and N more).
fn lever_names(t: &Through) -> String {
    let names: Vec<String> = t
        .all()
        .map(|l| format!("{} {}", l.name, l.version))
        .collect();
    let more = if t.more > 0 {
        format!(" (and {} more)", t.more)
    } else {
        String::new()
    };
    format!("{}{more}", names.join(" and "))
}

/// The move from the registry rows alone (no per-release requirements read).
fn row_move(
    package: &str,
    target: &str,
    parent: &str,
    parent_version: &str,
    standing: &ParentStanding,
    through: Option<&Through>,
    force: &str,
) -> String {
    let (lever, lever_version, lever_standing) = match through {
        Some(t) => (t.name.as_str(), t.version.as_str(), &t.standing),
        None => (parent, parent_version, standing),
    };
    // When the parent is not the lever, what is known about the parent
    // itself still tells the reader why moving it alone is not the answer.
    let via = through.map_or_else(String::new, |t| {
        let own = match standing {
            ParentStanding::Newest { deprecated } => format!(
                "{parent} {parent_version} is already the newest {parent} release 4DA has seen{}, and it ",
                if *deprecated { " and is deprecated" } else { "" }
            ),
            ParentStanding::Deprecated => format!("{parent} is deprecated, and it "),
            _ => format!("{parent} "),
        };
        let kind = if t.also.is_empty() && t.more == 0 {
            "a direct dependency"
        } else {
            "direct dependencies, each of which must move"
        };
        format!("{own}arrives through {}, {kind}; ", lever_names(t))
    });
    match lever_standing {
        ParentStanding::Newer { latest } => format!(
            "{via}upgrade {lever} ({lever} {latest} is the newest release 4DA has seen; check \
             that it admits {package} >= {target}){force}"
        ),
        ParentStanding::Newest { deprecated } => {
            format!(
            "{via}{lever} {lever_version} is already the newest {lever} release 4DA has seen{}, \
             so no {lever} upgrade is known to admit it: replace {lever}{force}",
            if *deprecated { " and is deprecated" } else { "" }
        )
        }
        ParentStanding::Deprecated => {
            format!("{via}{lever} is deprecated: replace {lever}{force}")
        }
        ParentStanding::Introduces {
            advisories,
            deprecated,
        } => format!(
            "{via}{}{force}",
            introduces_clause(lever, advisories, *deprecated)
        ),
        // A lever's registry read runs only under a parent release
        // (`admits_move`); reaching here it says nothing more than Unknown.
        ParentStanding::Unknown
        | ParentStanding::Admits(_)
        | ParentStanding::NoneAdmits { .. }
        | ParentStanding::Undecided { .. }
        | ParentStanding::Malicious { .. } => {
            format!(
                "{via}{lever} must move (4DA holds no record of a {lever} release that admits \
             {package} >= {target}): upgrade {lever} if one does, or replace it{force}"
            )
        }
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
            "{head}; {} is a semver-incompatible jump, so the dependency that pulls it in must move to a release that admits it, or be replaced",
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
    // The registry fact behind each named parent release (audit H1).
    for (title, note) in links
        .iter()
        .filter_map(|l| super::upgrade_parent_release::release_citation(package, l))
    {
        if !out.iter().any(|c| c.title == title) {
            out.push(citation(title, note));
        }
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
    if l.is_bundled() {
        return format!(
            "ships {} bundled inside its own package; only a new {} release moves it",
            l.child_version, l.parent
        );
    }
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
