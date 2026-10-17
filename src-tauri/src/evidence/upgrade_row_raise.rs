// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One bump per dependency (audit H1, rule b): a declared package whose own
//! advisories are fixed in a release that still pins a vulnerable transitive
//! copy is raised to the release that also clears that copy. NodeGoat:
//! express 4.16.4's own fix is 4.20.0, which still pins path-to-regexp 0.1.10
//! and qs 6.11.0; express 4.22.3 clears both, so the express row and the
//! path-to-regexp and qs rows all name 4.22.3 (`osv::parent_release_load`).

use crate::db::Database;
use crate::osv::fix_target::{upgrade_type, LineTarget};

/// Raise each line whose every site is declared, when the transitive rows it
/// pins need a higher release and every declaring project agrees on it. The
/// sentence for the explanation, empty when nothing moved.
pub(super) fn raise_declared_lines(
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
                crate::osv::parent_release_load::raised_row_target(
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

/// "4.22.3, not 4.20.0: express 4.20.0 still requires a vulnerable
/// path-to-regexp, qs; 4.22.3 is the highest of the express releases their
/// steps need, and no advisory of express affects it."
fn note(package: &str, old: &str, to: &str, packages: &[String]) -> String {
    let which = packages.join(", ");
    let why = if packages.len() == 1 {
        format!("{package} {to} is the smallest {package} release whose requirement admits a clean {which} (see its step)")
    } else {
        format!("{package} {to} is the highest of the {package} releases their steps need")
    };
    format!(
        " Upgrade to {package} {to}, not {old}: {package} {old} still requires a vulnerable {which}; \
         {why}, and no advisory of {package} affects it."
    )
}
