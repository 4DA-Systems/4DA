// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The Upgrade Plan's machine-readable work order (AD-049).
//!
//! The plan's [`EvidenceItem`](super::EvidenceItem)s are written for a person:
//! the target version is inside the title, and ecosystem, per-project installs,
//! the size of the jump and how the fix lands are in prose. A coding agent that
//! pulls the plan (`4da plan --json`, the MCP `upgrade_planner`) needs those as
//! fields. An [`UpgradeStep`] is that handoff record. It is keyed by the id of
//! the item it belongs to and travels ONLY in the persisted snapshot envelope
//! (`UpgradePlanSnapshot::steps`) — `EvidenceItem` is unchanged and no UI
//! renders it.
//!
//! One computation, two renderings: `upgrade_plan` builds each step from the
//! same `PackageGroup` (the same [`LineTarget`]s) it builds the item's title
//! from, at the same moment, and a step is dropped together with its item when
//! the item fails validation. Nothing here re-derives a target.

use serde::{Deserialize, Serialize};

use crate::osv::fix_target::{LineTarget, UpgradeType};
use crate::osv::types::MatchedAdvisory;

/// The fixed instruction every step carries: how the agent proves the change
/// worked. The snapshot is only rewritten when 4DA recomputes the plan, so the
/// check is against a NEWER `generated_at`.
pub const VERIFICATION: &str = "After the change, wait for 4DA to rescan the project and \
     recompute the plan (a newer generated_at), then re-run `4da plan --json`: this step's \
     item_id must no longer appear in `steps`. If it is still there, the change did not take.";

/// How the fix reaches the installs a step names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism {
    /// Every install site declares the package directly: bump it in each
    /// project's manifest.
    ManifestBump,
    /// The package is transitive at every site: the fix arrives by updating
    /// the parent that pulls it in, or by refreshing the lockfile.
    LockfileOrParentUpdate,
    /// Direct at some sites, transitive at others: both of the above.
    Mixed,
    /// No installed line has a fixed version to move to.
    NoFix,
}

/// One project holding the installed version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSite {
    /// The project path, exactly as the item's `affected_projects` names it.
    pub project: String,
    /// Declared in that project's manifest.
    pub direct: bool,
    /// Every recorded copy at this site is a dev dependency.
    pub dev: bool,
}

/// One installed version and where it must go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLine {
    pub installed: String,
    /// The minimum version this line must reach (`osv::fix_target`); `None`
    /// when no advisory on this copy has a fix.
    pub target: Option<String>,
    /// `None` when either side is not semver, or there is no target.
    pub upgrade_type: Option<UpgradeType>,
    /// `target` clears every known advisory of the package. `false` means it
    /// clears only those with a fix on this line.
    pub clears_all_known: bool,
    pub sites: Vec<StepSite>,
}

/// One package's work order, keyed by the plan item it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeStep {
    /// Equals the `id` of the `EvidenceItem` this step belongs to. Unique,
    /// except for the plan's single maintenance-notice item
    /// (`upgrade-plan:informational`), which carries one `no_fix` step per
    /// package it names; `(item_id, ecosystem, package)` is always unique.
    pub item_id: String,
    /// OSV-canonical ecosystem (`npm`, `crates.io`, `PyPI`, `Go`, ...).
    pub ecosystem: String,
    pub package: String,
    /// Sorted by installed version.
    pub lines: Vec<StepLine>,
    pub mechanism: Mechanism,
    /// Every advisory id behind the step, aliases included, sorted.
    pub advisory_ids: Vec<String>,
    /// [`VERIFICATION`].
    pub verification: String,
}

/// Build the step for one plan row from the row's own lines.
pub(super) fn step_for(
    item_id: &str,
    ecosystem: &str,
    package: &str,
    lines: &[LineTarget],
    advisories: &[&MatchedAdvisory],
) -> UpgradeStep {
    let mut advisory_ids: Vec<String> = advisories.iter().map(|a| a.advisory_id.clone()).collect();
    advisory_ids.sort_unstable();
    advisory_ids.dedup();
    UpgradeStep {
        item_id: item_id.to_string(),
        ecosystem: ecosystem.to_string(),
        package: package.to_string(),
        lines: lines.iter().map(step_line).collect(),
        mechanism: mechanism(lines),
        advisory_ids,
        verification: VERIFICATION.to_string(),
    }
}

/// Fold steps that name the same package (a package whose rows split by
/// exposure, inside the one maintenance-notice item) into one: their lines
/// merged by installed version, their sites and advisory ids unioned.
pub(super) fn merge_same_package(steps: Vec<UpgradeStep>) -> Vec<UpgradeStep> {
    let mut out: Vec<UpgradeStep> = Vec::with_capacity(steps.len());
    for step in steps {
        let Some(existing) = out
            .iter_mut()
            .find(|s| s.ecosystem == step.ecosystem && s.package == step.package)
        else {
            out.push(step);
            continue;
        };
        for line in step.lines {
            match existing
                .lines
                .iter_mut()
                .find(|l| l.installed == line.installed)
            {
                Some(l) => {
                    for site in line.sites {
                        if !l.sites.iter().any(|s| s.project == site.project) {
                            l.sites.push(site);
                        }
                    }
                    l.sites.sort_by(|a, b| a.project.cmp(&b.project));
                }
                None => existing.lines.push(line),
            }
        }
        existing.advisory_ids.extend(step.advisory_ids);
        existing.advisory_ids.sort_unstable();
        existing.advisory_ids.dedup();
    }
    out
}

fn step_line(line: &LineTarget) -> StepLine {
    StepLine {
        installed: line.installed_version.clone(),
        target: line.target_version.clone(),
        upgrade_type: line.upgrade_type,
        clears_all_known: line.target_version.is_some() && line.clears_all_known,
        sites: line
            .sites
            .iter()
            .map(|s| StepSite {
                project: s.project_path.clone(),
                direct: s.is_direct,
                dev: s.is_dev,
            })
            .collect(),
    }
}

/// `no_fix` when no line has a target (the same test that makes the item say
/// "No fix published"); otherwise by where the package is declared.
fn mechanism(lines: &[LineTarget]) -> Mechanism {
    if lines.iter().all(|l| l.target_version.is_none()) {
        return Mechanism::NoFix;
    }
    let sites = || lines.iter().flat_map(|l| l.sites.iter());
    if sites().all(|s| s.is_direct) {
        Mechanism::ManifestBump
    } else if sites().all(|s| !s.is_direct) {
        Mechanism::LockfileOrParentUpdate
    } else {
        Mechanism::Mixed
    }
}

#[cfg(test)]
#[path = "upgrade_steps_tests.rs"]
mod tests;
