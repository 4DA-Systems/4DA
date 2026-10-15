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
