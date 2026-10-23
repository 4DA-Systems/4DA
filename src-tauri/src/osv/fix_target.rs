// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Upgrade targets per installed release line (dependency-handoff Phase 1.1).
//!
//! A fix target is a fact about ONE install: the version that copy must reach.
//! The per-version rules (the line fix, the walk past every window, the
//! upgrade type) live in the pure engine crate,
//! `fourda_depengine::fix_target` (single-engine C0.1), and the per-project,
//! per-line aggregation in `fourda_depengine::lines` (C0.2). Both are
//! re-exported here under their old paths; [`line_targets`] reads the app's
//! `MatchedAdvisory` rows.

use super::types::MatchedAdvisory;

#[cfg(test)]
pub(crate) use fourda_depengine::fix_target::{
    clean_version, fix_for_version, walk_target, WalkTarget,
};
pub use fourda_depengine::fix_target::{upgrade_type, UpgradeType};
#[cfg(test)]
pub use fourda_depengine::lines::InstallSite;
pub use fourda_depengine::lines::{describe_lines, distinct_targets, LineTarget};

/// Parity decision 2, the MCP's words: why no target is named for a copy.
pub fn introduces_text(advisories: &[String]) -> String {
    format!(
        "every fixing release brings in {}, which the installed version does not have",
        advisories.join(", ")
    )
}

/// Every fixing release is flagged malicious (the MCP's `malware`, never
/// "brings in"): no release is named, and the first one is.
pub fn malicious_text(package: &str, m: &fourda_depengine::matching::Malicious) -> String {
    format!(
        "every fixing release of {package}, from {package} {}, is flagged malicious ({}): remove \
         or replace {package}",
        m.from,
        m.advisories.join(", ")
    )
}

/// The lines' malware block, when no line has a target.
pub fn malicious_of(lines: &[LineTarget]) -> Option<&fourda_depengine::matching::Malicious> {
    if lines.iter().any(|l| l.target_version.is_some()) {
        return None;
    }
    lines.iter().find_map(|l| l.malicious.as_ref())
}

/// The advisories every fixing release of these lines brings in, sorted and
/// deduped; empty when any line has a target.
pub fn introduced_by(lines: &[LineTarget]) -> Vec<String> {
    if lines.iter().any(|l| l.target_version.is_some()) {
        return Vec::new();
    }
    let mut out: Vec<String> = lines.iter().flat_map(|l| l.introduces.clone()).collect();
    out.sort();
    out.dedup();
    out
}

/// Per-line targets for the version-confirmed copies of a package in
/// `projects`, sorted by installed version. A copy in a project the caller
/// does not speak for can neither set nor raise a target here (AD-044).
pub fn line_targets(advisories: &[&MatchedAdvisory], projects: &[String]) -> Vec<LineTarget> {
    fourda_depengine::lines::line_targets(
        advisories
            .iter()
            .flat_map(|a| a.dependency_instances.iter()),
        projects,
    )
}

#[cfg(test)]
#[path = "fix_target_tests.rs"]
mod tests;
