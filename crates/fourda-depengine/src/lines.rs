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
    /// Parity decision 2: every fixing release brings in these advisories,
    /// which the installed version does not have, so `target_version` is
    /// `None` (no line fix is offered in its place either). Sorted, deduped.
    pub introduces: Vec<String>,
    /// With no target: the malicious records a fixing release falls in
    /// (sorted, deduped); `target_version` is `None` (no line fix either).
    pub malicious: Vec<String>,
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
        introduces: Vec<String>,
        malicious: Vec<String>,
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
                introduces: Vec::new(),
                malicious: Vec::new(),
            });
        if acc.clean.is_none() {
            acc.clean.clone_from(&d.clean_version);
        }
        if acc.walked.is_none() {
            acc.walked.clone_from(&d.target_version);
        }
        acc.introduces.extend(d.introduces.iter().cloned());
        acc.malicious.extend(d.malicious.iter().cloned());
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
            // A target never brings an advisory in (parity decision 2).
            let mut introduces = acc.introduces;
            introduces.sort();
            introduces.dedup();
            let mut malicious = acc.malicious;
            malicious.sort();
            malicious.dedup();
            let target_version = if introduces.is_empty() && malicious.is_empty() {
                acc.clean.or(acc.walked).or(acc.partial.map(|(_, f)| f))
            } else {
                None
            };
            let upgrade_type = target_version
                .as_deref()
                .and_then(|t| upgrade_type(&installed, t));
            LineTarget {
                installed_version: installed,
                target_version,
                clears_all_known,
                upgrade_type,
                sites: acc.sites.into_values().collect(),
                introduces,
                malicious,
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
mod tests {
    use super::*;

    fn copy(
        project: &str,
        installed: &str,
        fixed: Option<&str>,
        clean: Option<&str>,
        walked: Option<&str>,
        direct: bool,
        dev: bool,
    ) -> MatchedDependency {
        MatchedDependency {
            project_path: project.to_string(),
            installed_version: Some(installed.to_string()),
            is_direct: direct,
            is_dev: dev,
            is_version_confirmed: true,
            fixed_version: fixed.map(str::to_string),
            clean_version: clean.map(str::to_string),
            target_version: walked.map(str::to_string),
            withheld: Vec::new(),
            introduces: Vec::new(),
            malicious: Vec::new(),
        }
    }

    #[test]
    fn one_target_per_installed_line_sorted_by_version() {
        let copies = [
            copy(
                "/a",
                "5.0.9",
                Some("5.0.12"),
                Some("5.0.12"),
                Some("5.0.12"),
                false,
                true,
            ),
            copy(
                "/a",
                "1.1.18",
                Some("1.1.21"),
                Some("1.1.21"),
                Some("1.1.21"),
                true,
                false,
            ),
            copy("/b", "1.1.18", Some("1.1.20"), None, None, false, true),
            // A project the caller does not speak for sets nothing (AD-044).
            copy(
                "/other",
                "0.9.0",
                Some("2.0.0"),
                Some("2.0.0"),
                Some("2.0.0"),
                true,
                false,
            ),
            // An unconfirmed copy sets nothing either.
            MatchedDependency {
                is_version_confirmed: false,
                ..copy("/a", "0.1.0", Some("9.0.0"), None, None, true, false)
            },
        ];
        let projects = vec!["/a".to_string(), "/b".to_string()];
        let lines = line_targets(&copies, &projects);
        let installed: Vec<&str> = lines.iter().map(|l| l.installed_version.as_str()).collect();
        assert_eq!(installed, ["1.1.18", "5.0.9"]);

        let l = &lines[0];
        assert_eq!(l.target_version.as_deref(), Some("1.1.21"));
        assert!(l.clears_all_known);
        assert_eq!(l.upgrade_type, Some(UpgradeType::Patch));
        assert_eq!(
            l.sites,
            vec![
                InstallSite {
                    project_path: "/a".to_string(),
                    is_direct: true,
                    is_dev: false
                },
                InstallSite {
                    project_path: "/b".to_string(),
                    is_direct: false,
                    is_dev: true
                },
            ]
        );
        assert_eq!(distinct_targets(&lines), ["1.1.21", "5.0.12"]);
        assert_eq!(describe_lines(&lines), "1.1.18 -> 1.1.21, 5.0.9 -> 5.0.12");
    }

    #[test]
    fn without_a_walk_the_highest_line_fix_is_the_partial_target() {
        let copies = [
            copy("/a", "1.0.0", Some("1.0.5"), None, None, true, false),
            copy("/a", "1.0.0", Some("2.0.1"), None, None, true, false),
        ];
        let lines = line_targets(&copies, &["/a".to_string()]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].target_version.as_deref(), Some("2.0.1"));
        assert!(!lines[0].clears_all_known);
        assert!(lines[0].is_major());
        assert_eq!(describe_lines(&lines), "1.0.0 -> 2.0.1 (major)");
    }

    /// Parity decision 2: a copy whose every fixing release brings in an
    /// advisory gets no target, not even its line fix.
    #[test]
    fn a_line_whose_fixes_bring_in_an_advisory_has_no_target() {
        let copies = [MatchedDependency {
            introduces: vec!["B".to_string()],
            ..copy("/a", "1.0.0", Some("1.2.0"), None, None, true, false)
        }];
        let lines = line_targets(&copies, &["/a".to_string()]);
        assert_eq!(lines[0].target_version, None);
        assert_eq!(lines[0].introduces, ["B"]);
        assert!(distinct_targets(&lines).is_empty());
    }
}
