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
