// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The one line of fix advice per copy, worded for both the prompt and the
//! floor (`fix_clause`).

use super::{FixPath, ProvenParent};

/// One line of fix advice, worded for both the prompt and the floor. Every
/// version sits right after the name of the package it belongs to: the model
/// copies this clause, and on 2026-10-03 the old wording ("transitive via
/// victauri-plugin 0.8.4, ... the fix (>= 2.1.0)") put rmcp's fix after
/// victauri-plugin's name, so the version check faulted the brief that quoted
/// it faithfully.
pub(crate) fn fix_clause(package: &str, path: &FixPath) -> String {
    match path {
        FixPath::Bump { to, fix } => match fix {
            Some(how) => format!("bump {package} to >= {to}: {how}"),
            None => format!("bump {package} to >= {to}"),
        },
        FixPath::Refresh {
            to,
            inferred,
            command,
        } => {
            let run = command
                .as_deref()
                .map(|c| format!(": run `{c}`"))
                .unwrap_or_default();
            if *inferred {
                format!(
                    "transitive; {package} >= {to} is semver-compatible with the installed copy, so \
                     refreshing the lockfile should reach it (no manifest change; the parent's \
                     requirement is not on disk){run}"
                )
            } else {
                format!("transitive; refreshing the lockfile reaches {package} >= {to} (no manifest change){run}")
            }
        }
        FixPath::Parent {
            parent,
            parent_version,
            to,
            by_requirement,
            proven,
            route,
        } => {
            let why = if *by_requirement {
                format!(
                    "{parent} {parent_version} requires a {package} line that excludes the fix \
                     ({package} >= {to}), so a lockfile refresh will NOT fix it"
                )
            } else {
                format!(
                    "{parent} {parent_version} stays on an older {package} line, and the fix \
                     ({package} >= {to}) is a semver-incompatible jump, so a lockfile refresh will \
                     NOT fix it"
                )
            };
            // A release proven on this machine is the move; otherwise only
            // what the registry rows support (never "upgrade" a parent with
            // no newer release on record as if one existed).
            let action = match proven {
                Some(p) => format!("upgrade {parent}{}", proven_clause(package, Some(p))),
                None => crate::evidence::parent_move(
                    package,
                    to,
                    parent,
                    parent_version,
                    &route.standing,
                    route.through.as_ref(),
                    route.override_hint.as_deref(),
                ),
            };
            format!("transitive; {why}: {action}")
        }
        FixPath::ParentUnknown { to } => format!(
            "transitive; the fix ({package} >= {to}) is a semver-incompatible jump, so a lockfile \
             refresh will NOT reach it: the dependency that pulls {package} in must move to a \
             release that admits it, or be replaced"
        ),
        FixPath::Reinstall { to } => format!(
            "the lockfile already pins {package} {to} but node_modules still runs the old copy: reinstall"
        ),
        FixPath::PerLine { lines } => format!(
            "fixed on each line: {}",
            lines
                .iter()
                .map(|l| format!("{package} {} needs >= {}", l.installed, l.to))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        FixPath::NoFix => "no fix published: pin, patch locally, replace, or accept the risk".to_string(),
        FixPath::Introduces { advisories } => format!(
            "no {package} release is named: {}; pin, patch locally, replace, or accept the risk",
            crate::osv::fix_target::introduces_text(advisories)
        ),
        FixPath::Update { to } => format!("update {package} to >= {to}"),
    }
}

/// The evidence that names the parent release to move to. Live 2026-10-03
/// (Screenshot_3845): "upgrade victauri-plugin to pull in rmcp >= 2.1.0" left
/// the user to find the release, while 4da/src-tauri already ran
/// victauri-plugin 0.9.0 against rmcp 3.4.1. It follows "upgrade {parent}",
/// so each version sits after its own package, as the version check
/// (`check_factual_claims`, nearest preceding package) requires.
fn proven_clause(package: &str, proven: Option<&ProvenParent>) -> String {
    proven.map_or_else(String::new, |p| {
        format!(
            " to {} ({} already runs it, resolved to {package} {})",
            p.parent_version, p.label, p.child_version
        )
    })
}

/// A copy the project declares: name how it moves where it is declared, read
/// from the project's own manifest (`osv::direct_upgrade`, audit E8). The
/// same rule the Upgrade Plan's direct rows use; nothing when it is not certain
/// (including when the declaration does not admit the installed copy).
pub(super) fn with_direct_fix(
    path: FixPath,
    project: &str,
    ecosystem: &str,
    package: &str,
    installed: Option<&str>,
) -> FixPath {
    let (FixPath::Bump { to, fix: None }, Some(installed)) = (&path, installed) else {
        return path;
    };
    let to = to.clone();
    let fix = crate::osv::exposure::canonical(ecosystem)
        .and_then(|eco| {
            crate::osv::direct_upgrade::direct_fix(
                std::path::Path::new(project),
                eco,
                package,
                installed,
                &to,
            )
        })
        .map(|f| f.brief_clause());
    let fix = with_major(installed, &to, fix);
    FixPath::Bump { to, fix }
}

/// A raise across a major (parity decision 3) is never silent: the bump
/// clause says so, before any command.
fn with_major(installed: &str, to: &str, fix: Option<String>) -> Option<String> {
    if !crate::osv::parent_release::is_major(installed, to) {
        return fix;
    }
    Some(fix.map_or_else(
        || "a major upgrade".to_string(),
        |f| format!("a major upgrade; {f}"),
    ))
}

/// A copy whose line has no target because every fixing release brings in
/// an advisory it does not have (parity decision 2) says so, with the same
/// ids the Upgrade Plan and Preemption name, instead of "no fix published".
pub(super) fn with_introduces(path: FixPath, introduces: &[String]) -> FixPath {
    if path != FixPath::NoFix || introduces.is_empty() {
        return path;
    }
    FixPath::Introduces {
        advisories: introduces.to_vec(),
    }
}
