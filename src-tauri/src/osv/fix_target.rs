// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Upgrade targets per installed release line (dependency-handoff Phase 1.1).
//!
//! A fix target is a fact about ONE install: the version that copy must reach.
//! It used to be computed machine-wide — the highest line fix across every
//! confirmed copy in every project — and the per-project upgrade plan then
//! printed it. Live 2026-10-01: navcal holds brace-expansion 1.1.12 only and
//! was told ">= 5.0.12" (a 4-major jump) because a different project held
//! 5.0.9; its real target is 1.1.21. navcal's undici 5.28.4 was told 7.29.1
//! because another project held 7.28.0; its own path is 6.28.1.
//!
//! Split out of `matching.rs` (file-size gate).

use std::collections::BTreeMap;

use semver::Version;

use super::matching::{check_version_affected, is_unknown_bound, parse_version};
use super::types::{MatchedAdvisory, Range};

/// The fix for the release line a version is on: the `fixed` bound of the
/// affected window that contains `user_version`. `None` when the version is
/// in no window, its window has no fix (`last_affected` / open-ended), or it
/// cannot be parsed.
///
/// An advisory fixed in several lines lists one fix per line. GHSA-p293-qw3h-jr36
/// (next) lists `["15.5.24", "16.3.3"]`, and taking the first told a 16.2.10
/// install that 15.5.24 fixed it; the group maximum then printed "update to
/// >= 16.2.11", which leaves both critical RCEs open (2026-09-26).
pub(crate) fn fix_for_version(
    user_version: &str,
    affected_ranges_json: &Option<String>,
) -> Option<String> {
    let ranges: Vec<Range> = serde_json::from_str(affected_ranges_json.as_deref()?).ok()?;
    let user = parse_version(user_version)?;
    for range in &ranges {
        if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
            continue;
        }
        let mut introduced: Option<Version> = None;
        for obj in range.events.iter().flatten().filter_map(|e| e.as_object()) {
            if let Some(intro) = obj.get("introduced").and_then(|v| v.as_str()) {
                introduced = if intro == "0" {
                    Some(Version::new(0, 0, 0))
                } else {
                    parse_version(intro)
                };
            }
            let bound = obj
                .get("fixed")
                .or_else(|| obj.get("last_affected"))
                .and_then(|v| v.as_str());
            if let Some(bound) = bound {
                let is_fix = obj.contains_key("fixed");
                if let (Some(intro), Some(end)) = (introduced.as_ref(), parse_version(bound)) {
                    let inside = if is_fix { user < end } else { user <= end };
                    if !is_unknown_bound(bound) && user >= *intro && inside {
                        return is_fix.then(|| bound.trim().to_string());
                    }
                }
                introduced = None;
            }
        }
    }
    None
}

/// Successive-fix steps before giving up. Every step moves strictly upward
/// (a window's fix is above every version inside it), so this only bounds
/// pathological data; real chains are one or two steps.
const MAX_HOPS: usize = 16;

/// The lowest version reachable from `installed` by successive fixes that no
/// advisory in `ranges` (every known advisory of the package) affects.
///
/// Taking each advisory's own line fix and the maximum is not enough: that
/// version can sit inside ANOTHER advisory's window (undici 5.28.4 must cross
/// into 6.x, where GHSA-3wwx-pv8p-q78v opens at 6.25.0). So walk: find every
/// advisory affecting the current version, step to the highest of their line
/// fixes, repeat until nothing affects it.
///
/// `None` when nothing affects `installed`, when the walk reaches a window
/// with no fix, or when an advisory cannot be evaluated against a version on
/// the path (no range data / unparseable) — a clean version is then not a
/// claim 4DA can stand behind.
pub(crate) fn clean_version(installed: &str, ranges: &[&Option<String>]) -> Option<String> {
    let mut current = installed.trim().to_string();
    for _ in 0..MAX_HOPS {
        let mut next: Option<(Version, String)> = None;
        for r in ranges {
            match check_version_affected(Some(&current), r) {
                (false, _) => continue,
                (true, false) => return None,
                (true, true) => {}
            }
            let fix = fix_for_version(&current, r)
                .and_then(|f| parse_version(&f).map(|parsed| (parsed, f)))?;
            if next.as_ref().is_none_or(|(best, _)| fix.0 > *best) {
                next = Some(fix);
            }
        }
        match next {
            Some((_, fix)) => current = fix,
            None => return (current != installed.trim()).then_some(current),
        }
    }
    None
}

/// How far an upgrade moves, by semver compatibility: a step the package
/// manager's default caret range would NOT take is `Major` (for `0.y.z`, a
/// minor bump breaks; for `0.0.z`, any bump does). Serialized as
/// `"patch"` / `"minor"` / `"major"` in the plan's work order (AD-049).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeType {
    Patch,
    Minor,
    Major,
}

/// Classify `installed -> target`. `None` when either side is not semver.
pub fn upgrade_type(installed: &str, target: &str) -> Option<UpgradeType> {
    let from = parse_version(installed)?;
    let to = parse_version(target)?;
    let breaking = if from.major > 0 {
        to.major != from.major
    } else if from.minor > 0 {
        to.major != 0 || to.minor != from.minor
    } else {
        to.major != 0 || to.minor != 0 || to.patch != from.patch
    };
    Some(if breaking {
        UpgradeType::Major
    } else if to.minor != from.minor {
        UpgradeType::Minor
    } else {
        UpgradeType::Patch
    })
}

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

/// Per-line targets for the version-confirmed copies of a package in
/// `projects`, sorted by installed version. A copy in a project the caller
/// does not speak for can neither set nor raise a target here (AD-044).
pub fn line_targets(advisories: &[&MatchedAdvisory], projects: &[String]) -> Vec<LineTarget> {
    struct Acc {
        clean: Option<String>,
        partial: Option<(Version, String)>,
        sites: BTreeMap<String, InstallSite>,
    }
    let mut by_version: BTreeMap<String, Acc> = BTreeMap::new();
    let instances = advisories
        .iter()
        .flat_map(|a| a.dependency_instances.iter())
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
                partial: None,
                sites: BTreeMap::new(),
            });
        if acc.clean.is_none() {
            acc.clean.clone_from(&d.clean_version);
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
            let target_version = acc.clean.or(acc.partial.map(|(_, f)| f));
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

#[cfg(test)]
#[path = "fix_target_tests.rs"]
mod tests;
