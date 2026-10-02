// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The upgrade step's words for a transitive copy: which installed parent
//! brings it in, whether a lockfile refresh is enough, and which newer parent
//! release a project here already resolved to a clear version
//! (`osv::parent_hint`). Carried in the canonical item — explanation text and
//! `dependency-path` citations — never a new type (doctrine rule 1).

use crate::osv::parent_hint::ParentLink;

use super::types::EvidenceCitation;
use super::upgrade_plan::truncate;

/// Most links / resolutions spelled out in the explanation (the citations
/// carry the rest up to [`MAX_PATH_CITATIONS`]).
const MAX_LINKS_IN_TEXT: usize = 2;
const MAX_PATH_CITATIONS: usize = 4;

/// Generic text when no parent could be read from the lockfiles.
pub(super) const GENERIC_UPSTREAM: &str =
    "Fixed only upstream — awaits a parent-package update or lockfile refresh";

/// The scope sentence for a step that is NOT fixable by a direct bump. No
/// trailing period (the caller adds it).
pub(super) fn upstream_note(package: &str, links: &[ParentLink], multi_project: bool) -> String {
    if links.is_empty() {
        return GENERIC_UPSTREAM.to_string();
    }
    let mut parts: Vec<String> = links
        .iter()
        .take(MAX_LINKS_IN_TEXT)
        .map(|l| link_sentence(package, l, multi_project))
        .collect();
    if links.len() > MAX_LINKS_IN_TEXT {
        parts.push(format!(
            "{} more parent {}",
            links.len() - MAX_LINKS_IN_TEXT,
            if links.len() - MAX_LINKS_IN_TEXT == 1 {
                "path"
            } else {
                "paths"
            }
        ));
    }
    let mut out = format!("Fixed only upstream: {}", parts.join("; "));
    if let Some(seen) = seen_sentence(package, links) {
        out.push_str(". ");
        out.push_str(&seen);
    }
    out
}

fn link_sentence(package: &str, l: &ParentLink, multi_project: bool) -> String {
    let direct = if l.parent_is_direct {
        " (a direct dependency)"
    } else {
        ""
    };
    let place = if multi_project {
        format!(" in {}", l.project)
    } else {
        String::new()
    };
    let head = format!(
        "{package} {} comes in through {} {}{direct}{place}",
        l.child_version, l.parent, l.parent_version
    );
    let Some(target) = l.target.as_deref() else {
        return head;
    };
    match (l.requirement.as_deref(), l.admits_target) {
        (Some(req), Some(true)) => format!(
            "{head}, whose requirement {req} already admits {target} — a lockfile refresh is enough"
        ),
        (Some(req), Some(false)) => format!(
            "{head}, whose requirement {req} excludes {target} — {} itself must be updated",
            l.parent
        ),
        _ => format!(
            "{head} — update {}, or refresh the lockfile if its requirement admits {package} {target}",
            l.parent
        ),
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
/// already known to be enough.
fn needs_parent_move(l: &ParentLink) -> bool {
    l.admits_target != Some(true)
}

/// The single parent every link goes through, for the step title — `None`
/// when there are several, or when a lockfile refresh alone clears every link
/// (then the parent is not the move).
pub(super) fn single_parent(links: &[ParentLink]) -> Option<&str> {
    let first = links.first()?;
    if links.iter().any(|l| l.parent != first.parent) {
        return None;
    }
    if links.iter().all(|l| l.admits_target == Some(true)) {
        return None;
    }
    Some(first.parent.as_str())
}

/// One `dependency-path` citation per link, then one per newer-parent
/// observation, capped.
pub(super) fn path_citations(package: &str, links: &[ParentLink]) -> Vec<EvidenceCitation> {
    let mut out: Vec<EvidenceCitation> = Vec::new();
    for l in links {
        let requirement = match (
            l.requirement.as_deref(),
            l.admits_target,
            l.target.as_deref(),
        ) {
            (Some(req), Some(true), Some(t)) => format!("requires {req}, which admits {t}"),
            (Some(req), Some(false), Some(t)) => format!("requires {req}, which excludes {t}"),
            (Some(req), _, _) => format!("requires {req}"),
            (None, _, _) => "requirement not recorded in the lockfile".to_string(),
        };
        out.push(citation(
            format!(
                "{package} {} <- {} {}",
                l.child_version, l.parent, l.parent_version
            ),
            format!(
                "In {}; {} is a {} dependency; {requirement}",
                l.project,
                l.parent,
                if l.parent_is_direct {
                    "direct"
                } else {
                    "transitive"
                }
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
