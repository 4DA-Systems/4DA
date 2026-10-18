// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One bump per dependency (audit H1, rule b), on every surface: a declared
//! package whose own advisories are fixed in a release that still pins a
//! vulnerable transitive copy is raised to the release that also clears that
//! copy. NodeGoat: express 4.16.4's own fix is 4.20.0, which still pins
//! path-to-regexp 0.1.10 and qs 6.11.0; express 4.22.3 clears both, so the
//! express step, the express alert and the Brief's express fact all say
//! 4.22.3, as the path-to-regexp and qs route sentences do
//! (`osv::parent_release_load`). The Upgrade Plan, Preemption and the Brief
//! read their line targets here, never from `fix_target::line_targets` alone.

use crate::db::Database;
use crate::osv::fix_target::{line_targets, upgrade_type, LineTarget};
use crate::osv::types::MatchedAdvisory;

/// A package's line targets, raised, and the sentence that says why.
pub(crate) struct RaisedLines {
    pub lines: Vec<LineTarget>,
    /// Empty when nothing moved; otherwise starts with a space.
    pub note: String,
}

/// [`line_targets`] for the copies of `group` in `projects`, each declared
/// line raised by [`raise_declared_lines`].
pub(crate) fn raised_line_targets(
    db: &Database,
    group: &[&MatchedAdvisory],
    projects: &[String],
) -> RaisedLines {
    let mut lines = line_targets(group, projects);
    let note = match group
        .first()
        .and_then(|m| crate::osv::exposure::canonical(&m.ecosystem).map(|e| (e, m)))
    {
        Some((eco, m)) => raise_declared_lines(db, eco, &m.package_name, &mut lines),
        None => String::new(),
    };
    RaisedLines { lines, note }
}

/// Raise each line whose every site is declared, when the transitive copies
/// it pins need a higher release and every declaring project agrees on it.
/// The sentence for the explanation, empty when nothing moved.
pub(crate) fn raise_declared_lines(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    lines: &mut [LineTarget],
) -> String {
    let mut notes: Vec<String> = Vec::new();
    for line in lines.iter_mut() {
        let Some(current) = line.target_version.clone() else {
            continue;
        };
        if line.sites.is_empty() || line.sites.iter().any(|s| !s.is_direct) {
            continue;
        }
        let raises: Vec<Option<(String, Vec<String>)>> = line
            .sites
            .iter()
            .map(|s| {
                super::parent_release_load::raised_row_target(
                    db,
                    ecosystem_norm,
                    &s.project_path,
                    package,
                    &line.installed_version,
                    &current,
                )
            })
            .collect();
        let Some(Some((to, packages))) = raises.first().cloned() else {
            continue;
        };
        if raises
            .iter()
            .any(|r| r.as_ref().map(|(t, _)| t) != Some(&to))
        {
            continue; // the projects disagree: one line target cannot say both
        }
        notes.push(note(package, &current, &to, &packages));
        line.upgrade_type = upgrade_type(&line.installed_version, &to);
        line.target_version = Some(to);
    }
    notes.concat()
}

/// " Upgrade to express 4.22.3, not 4.20.0: express 4.20.0 still requires a
/// vulnerable path-to-regexp, qs; express 4.22.3 is the highest of the
/// express releases their steps need, and no advisory of express affects it."
fn note(package: &str, old: &str, to: &str, packages: &[String]) -> String {
    let which = packages.join(", ");
    let why = if packages.len() == 1 {
        format!(
            "{package} {to} is the smallest {package} release whose requirement admits a clean \
             {which}"
        )
    } else {
        format!("{package} {to} is the highest of the {package} releases they need")
    };
    // Across a major (parity decision 3): flagged, never silent.
    let breaking = if crate::osv::parent_release::is_major(old, to) {
        format!(
            " BREAKING: the only release of {package} that fixes {which} is {to}, a major \
             upgrade."
        )
    } else {
        String::new()
    };
    format!(
        " Upgrade to {package} {to}, not {old}: {package} {old} still requires a vulnerable \
         {which}; {why}, and no advisory of {package} affects it.{breaking}"
    )
}

#[cfg(test)]
mod tests {
    /// Parity decision 3 (pr-009's case, xsv, 2026-10-18): the raise across a
    /// major is flagged with its reason.
    #[test]
    fn a_raise_across_a_major_says_why() {
        let text = super::note(
            "crossbeam-channel",
            "0.4.4",
            "0.5.0",
            &["crossbeam-utils".to_string()],
        );
        assert!(
            text.contains(
                "BREAKING: the only release of crossbeam-channel that fixes crossbeam-utils is \
                 0.5.0, a major upgrade."
            ),
            "{text}"
        );
        let minor = super::note("express", "4.20.0", "4.22.3", &["qs".to_string()]);
        assert!(!minor.contains("BREAKING"), "{minor}");
    }
}
