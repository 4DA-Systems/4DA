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

/// A malicious landing, in the MCP's words (`malwareText`, fix-paths.ts):
/// "<pkg> is flagged malicious (<ids>): remove or replace <pkg>". No version
/// is named, so the Brief's version check has nothing to fault.
pub fn malicious_text(package: &str, advisories: &[String]) -> String {
    format!(
        "{package} is flagged malicious ({}): remove or replace {package}",
        advisories.join(", ")
    )
}

/// The malicious records a fixing release of these lines falls in, sorted
/// and deduped; empty when any line has a target.
pub fn malicious_of(lines: &[LineTarget]) -> Vec<String> {
    if lines.iter().any(|l| l.target_version.is_some()) {
        return Vec::new();
    }
    let mut out: Vec<String> = lines.iter().flat_map(|l| l.malicious.clone()).collect();
    out.sort();
    out.dedup();
    out
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

/// npm (audit 6.2.5 D-4): the target of `line` may be called a clean fix as
/// far as the dependencies it holds go (judged clean, or nothing to judge).
pub fn held_clean(line: &LineTarget) -> bool {
    line.held
        .as_ref()
        .is_none_or(fourda_depengine::held::HeldOutcome::is_clean)
}

/// What the held-dependency rule said about each line's target, one sentence
/// per line that has something to say, each starting with a space:
/// " express 4.16.4: 4.20.0 exact-pins qs 6.11.0 (GHSA-...); so 4.22.3, ...".
/// Empty when no line has a note.
pub fn held_sentences(package: &str, lines: &[LineTarget]) -> String {
    lines
        .iter()
        .filter_map(|l| {
            let note = fourda_depengine::held::held_note(l.held.as_ref()?);
            (!note.is_empty()).then(|| format!(" {package} {}: {note}.", l.installed_version))
        })
        .collect()
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
