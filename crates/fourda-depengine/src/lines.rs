// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Upgrade targets per installed release line (dependency-handoff Phase 1.1):
//! the per-project aggregation of matched copies. A fix target is a fact
//! about ONE install, so the copies of a package are grouped by installed
//! version and each line gets its own target. Moved out of the app's
//! `osv/fix_target.rs` (single-engine C0.2); the app's `line_targets` wraps
//! [`line_targets`] over its `MatchedAdvisory` rows.

use std::collections::BTreeMap;

use semver::Version;

use crate::fix_target::{upgrade_type, UpgradeType};
use crate::matching::MatchedDependency;
use crate::range::parse_version;

/// One project holding one copy of the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSite {
    pub project_path: String,
    /// Declared directly in that project's manifest (bumpable now).
    pub is_direct: bool,
    /// Every recorded instance at this site is a dev dependency.
    pub is_dev: bool,
}

/// The target for one installed version across the projects a row names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineTarget {
    pub installed_version: String,
    /// The version to reach. `None` when no advisory on this copy has a fix.
    pub target_version: Option<String>,
    /// `target_version` clears every known advisory of the package (the
    /// clean walk succeeded). `false` means it clears only the advisories
    /// that HAVE a fix on this line — another one stays open upstream.
    pub clears_all_known: bool,
    pub upgrade_type: Option<UpgradeType>,
    pub sites: Vec<InstallSite>,
}

impl LineTarget {
    pub fn is_major(&self) -> bool {
        self.upgrade_type == Some(UpgradeType::Major)
    }
}

/// Per-line targets for the version-confirmed `copies` (every matched copy
/// of one package, across its advisories) in `projects`, sorted by installed
/// version. A copy in a project the caller does not speak for can neither
/// set nor raise a target here (AD-044).
pub fn line_targets<'a, I>(copies: I, projects: &[String]) -> Vec<LineTarget>
where
    I: IntoIterator<Item = &'a MatchedDependency>,
{
    struct Acc {
        clean: Option<String>,
        walked: Option<String>,
        partial: Option<(Version, String)>,
        sites: BTreeMap<String, InstallSite>,
    }
    let mut by_version: BTreeMap<String, Acc> = BTreeMap::new();
    let instances = copies
        .into_iter()
        .filter(|d| d.is_version_confirmed)
        .filter(|d| projects.iter().any(|p| p == &d.project_path));
    for d in instances {
        let Some(installed) = d.installed_version.as_deref() else {
            continue;
        };
        let acc = by_version
            .entry(installed.to_string())
            .or_insert_with(|| Acc {
                clean: None,
                walked: None,
                partial: None,
                sites: BTreeMap::new(),
            });
        if acc.clean.is_none() {
            acc.clean.clone_from(&d.clean_version);
        }
        if acc.walked.is_none() {
            acc.walked.clone_from(&d.target_version);
        }
        if let Some(fix) = d
            .fixed_version
            .as_deref()
            .and_then(|f| parse_version(f).map(|parsed| (parsed, f.to_string())))
        {
            if acc.partial.as_ref().is_none_or(|(best, _)| fix.0 > *best) {
                acc.partial = Some(fix);
            }
        }
        acc.sites
            .entry(d.project_path.clone())
            .and_modify(|s| {
                s.is_direct |= d.is_direct;
                s.is_dev &= d.is_dev;
            })
            .or_insert_with(|| InstallSite {
                project_path: d.project_path.clone(),
                is_direct: d.is_direct,
                is_dev: d.is_dev,
            });
    }

    let mut lines: Vec<LineTarget> = by_version
        .into_iter()
        .map(|(installed, acc)| {
            let clears_all_known = acc.clean.is_some();
            // Clean; else where the walk landed (past every window that has
            // a way out); else, when the walk could not run, the highest
            // own-line fix.
            let target_version = acc.clean.or(acc.walked).or(acc.partial.map(|(_, f)| f));
            let upgrade_type = target_version
                .as_deref()
                .and_then(|t| upgrade_type(&installed, t));
            LineTarget {
                installed_version: installed,
                target_version,
                clears_all_known,
                upgrade_type,
                sites: acc.sites.into_values().collect(),
            }
        })
        .collect();
    lines.sort_by(|a, b| {
        match (
            parse_version(&a.installed_version),
            parse_version(&b.installed_version),
        ) {
            (Some(va), Some(vb)) => va.cmp(&vb),
            _ => a.installed_version.cmp(&b.installed_version),
        }
    });
    lines
}

/// The distinct targets of `lines`, ascending — one entry means every copy
/// the row names converges on the same version.
pub fn distinct_targets(lines: &[LineTarget]) -> Vec<String> {
    let mut targets: Vec<String> = lines
        .iter()
        .filter_map(|l| l.target_version.clone())
        .collect();
    targets.sort_by(|a, b| match (parse_version(a), parse_version(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    });
    targets.dedup();
    targets
}

/// "1.1.18 -> 1.1.21, 5.0.9 -> 5.0.12": the per-line upgrade, for text that
/// must say where EACH copy goes. Lines without a target are left out.
pub fn describe_lines(lines: &[LineTarget]) -> String {
    lines
        .iter()
        .filter_map(|l| {
            l.target_version.as_deref().map(|t| {
                let major = if l.is_major() { " (major)" } else { "" };
                format!("{} -> {t}{major}", l.installed_version)
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}
