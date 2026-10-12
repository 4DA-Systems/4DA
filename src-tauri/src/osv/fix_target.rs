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

use std::cmp::Ordering;
use std::collections::BTreeMap;

use semver::Version;

use super::matching::{check_version_affected, is_unknown_bound, parse_version};
use super::types::{MatchedAdvisory, Range, ENUMERATED_RANGE};
use super::version_order::{in_window, Lower, OrderedVersion, Upper};

/// One affected window of a SEMVER/ECOSYSTEM range: from `lower` up to
/// `end` (`fixed` exclusive, `last_affected` inclusive), or open-ended.
struct Window {
    lower: Lower,
    /// `(bound, as written, is_fixed)`; `None` for an open-ended window.
    end: Option<(OrderedVersion, String, bool)>,
}

impl Window {
    fn contains(&self, user: &OrderedVersion) -> Option<bool> {
        let upper = self.end.as_ref().map(|(version, _, fixed)| Upper {
            version,
            inclusive: !fixed,
        });
        in_window(user, &self.lower, upper)
    }
}

/// The readable windows of one advisory's ranges. A window whose end is an
/// unknown ("-NA") or unreadable bound is left out, as the matcher leaves it.
fn windows(ecosystem: &str, ranges: &[Range]) -> Vec<Window> {
    let mut out = Vec::new();
    for range in ranges {
        if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
            continue;
        }
        let mut lower: Option<Lower> = None;
        for obj in range.events.iter().flatten().filter_map(|e| e.as_object()) {
            if let Some(intro) = obj.get("introduced").and_then(|v| v.as_str()) {
                lower = Lower::parse_in(intro, ecosystem);
            }
            let bound = obj
                .get("fixed")
                .or_else(|| obj.get("last_affected"))
                .and_then(|v| v.as_str());
            if let Some(bound) = bound {
                let end =
                    OrderedVersion::parse_in(bound, ecosystem).filter(|_| !is_unknown_bound(bound));
                if let (Some(low), Some(end)) = (lower.take(), end) {
                    let fixed = obj.contains_key("fixed");
                    out.push(Window {
                        lower: low,
                        end: Some((end, bound.trim().to_string(), fixed)),
                    });
                }
            }
        }
        if let Some(low) = lower {
            out.push(Window {
                lower: low,
                end: None,
            });
        }
    }
    out
}

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
    ecosystem: &str,
    user_version: &str,
    affected_ranges_json: &Option<String>,
) -> Option<String> {
    let ranges: Vec<Range> = serde_json::from_str(affected_ranges_json.as_deref()?).ok()?;
    let user = OrderedVersion::parse_in(user_version, ecosystem)?;
    windows(ecosystem, &ranges)
        .into_iter()
        .find(|w| w.end.is_some() && w.contains(&user) == Some(true))
        .and_then(|w| w.end)
        .and_then(|(_, bound, fixed)| fixed.then_some(bound))
}

/// Successive steps before giving up. Every step moves strictly upward (a
/// window's exit is above every version inside it), so this only bounds
/// pathological data; real chains are a few steps.
const MAX_HOPS: usize = 16;

/// Where [`walk_target`] lands for one install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WalkTarget {
    pub version: String,
    /// No known advisory affects `version`. `false`: every advisory still
    /// affecting it has no way out (an open-ended window, or a
    /// `last_affected` bound with no release above it in the data); every
    /// advisory that HAS one is cleared.
    pub clears_all: bool,
}

/// The releases the advisory data names for this package (every bound and
/// every enumerated version, pre-releases left out): the only versions above
/// a `last_affected` bound the walk can name without a registry. Offline by
/// design; a release the data never mentions can only make the target higher
/// than the true minimum, never vulnerable.
fn known_releases(ecosystem: &str, advisories: &[Option<Vec<Range>>]) -> Vec<Release> {
    let mut out: Vec<Release> = Vec::new();
    for range in advisories.iter().flatten().flatten() {
        for event in range.events.iter().flatten() {
            let raw: Vec<&str> = if range.range_type == ENUMERATED_RANGE {
                event.as_str().into_iter().collect()
            } else if range.range_type == "SEMVER" || range.range_type == "ECOSYSTEM" {
                event
                    .as_object()
                    .into_iter()
                    .flat_map(|o| o.values())
                    .filter_map(|v| v.as_str())
                    .filter(|v| v.trim() != "0")
                    .collect()
            } else {
                Vec::new()
            };
            out.extend(
                raw.into_iter()
                    .filter(|v| !is_unknown_bound(v))
                    .filter_map(|v| {
                        OrderedVersion::parse_in(v, ecosystem).map(|p| (p, v.trim().to_string()))
                    })
                    .filter(|(v, _)| !v.is_prerelease()),
            );
        }
    }
    out
}

/// A version and its spelling in the advisory data.
type Release = (OrderedVersion, String);

/// Between two spellings of the SAME version, the one carrying build
/// metadata: Go publishes `v25.0.13+incompatible`, and an advisory that
/// spells the fix `25.0.13` would otherwise name a tag that does not exist.
/// (The pre-walk fallback took semver's maximum, which preferred it too.)
fn better_spelled(a: &Release, b: &Release) -> bool {
    a.1.contains('+') && !b.1.contains('+')
}

/// Whether `step` replaces `best` as the walk's next version: higher, or the
/// same version better spelled. `None` when the pair cannot be ordered.
fn outranks(step: &Release, best: &Release) -> Option<bool> {
    Some(match step.0.compare(&best.0)? {
        Ordering::Greater => true,
        Ordering::Equal => better_spelled(step, best),
        Ordering::Less => false,
    })
}

/// The lowest of `releases` above `bound` that `keep` accepts.
fn first_above(
    releases: &[Release],
    bound: &OrderedVersion,
    keep: impl Fn(&OrderedVersion) -> bool,
) -> Option<Release> {
    releases
        .iter()
        .filter(|(r, _)| r.compare(bound) == Some(Ordering::Greater) && keep(r))
        .fold(None, |best: Option<&Release>, r| match best {
            Some(b) => match r.0.compare(&b.0) {
                Some(Ordering::Less) => Some(r),
                Some(Ordering::Equal) if better_spelled(r, b) => Some(r),
                _ => Some(b),
            },
            None => Some(r),
        })
        .cloned()
}

/// How the walk leaves one advisory that affects the current version.
enum Exit {
    /// The lowest version above every window holding the current version.
    To(Release),
    /// A window holding it has no way out.
    Stuck,
}

/// The exit from every window of one advisory that holds `here`: a `fixed`
/// bound; above a `last_affected` bound, the first known release past it; for
/// an enumerated `versions` list, the first known release past `here` the list
/// does not name. The highest exit wins, since every window must be left.
fn exit(ecosystem: &str, here: &OrderedVersion, ranges: &[Range], releases: &[Release]) -> Exit {
    let mut exits: Vec<Option<Release>> = Vec::new();
    for window in windows(ecosystem, ranges) {
        if window.contains(here) != Some(true) {
            continue;
        }
        exits.push(match window.end {
            Some((fix, raw, true)) => Some((fix, raw)),
            Some((last, _, false)) => first_above(releases, &last, |_| true),
            None => None,
        });
    }
    for range in ranges.iter().filter(|r| r.range_type == ENUMERATED_RANGE) {
        let listed: Vec<OrderedVersion> = range
            .events
            .iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .filter_map(|v| OrderedVersion::parse_in(v, ecosystem))
            .collect();
        let named =
            |v: &OrderedVersion| listed.iter().any(|l| v.compare(l) == Some(Ordering::Equal));
        if named(here) {
            exits.push(first_above(releases, here, |r| !named(r)));
        }
    }
    let mut best: Option<Release> = None;
    for step in exits {
        let Some(step) = step else {
            return Exit::Stuck;
        };
        if best
            .as_ref()
            .is_none_or(|b| outranks(&step, b) == Some(true))
        {
            best = Some(step);
        }
    }
    // Affected, yet no window holds it: nothing to stand behind.
    best.map_or(Exit::Stuck, Exit::To)
}

/// Walk from `installed` over every known advisory of the package (`ranges`):
/// at each version, every advisory affecting it names its exit (see [`exit`]),
/// and the walk steps to the highest, until nothing with a way out affects
/// the version reached.
///
/// Taking each advisory's own line fix and the maximum is not enough: that
/// version can sit inside ANOTHER advisory's window (undici 5.28.4 must cross
/// into 6.x, where GHSA-3wwx-pv8p-q78v opens at 6.25.0; mistune 0.8.3's
/// highest fix, 3.3.0, opens GHSA-6m44-fpc8-c3rq, fixed in 3.3.3).
///
/// A `last_affected` window is left at the first release the advisory data
/// names above the bound (pyjwt's PYSEC-2025-183 ends at last_affected 2.10.1;
/// mistune's GHSA-8g87-j6q8-g93x at 3.2.0). It used to end the walk, so no
/// clean version was claimed and the plan fell back to the highest line fix,
/// which for mistune was a version still affected (rule vectors ft-043,
/// ft-052, 2026-10-12).
///
/// An advisory with no way out does not stop the walk; the result then says
/// it does not clear everything. `None` when nothing affects `installed` (or
/// nothing with a way out does, so there is nowhere to go), or an advisory
/// cannot be evaluated against a version on the path (no range data,
/// unparseable): no target is then a claim 4DA can stand behind.
pub(crate) fn walk_target(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
) -> Option<WalkTarget> {
    let parsed: Vec<Option<Vec<Range>>> = ranges
        .iter()
        .map(|r| r.as_deref().and_then(|j| serde_json::from_str(j).ok()))
        .collect();
    let releases = known_releases(ecosystem, &parsed);
    let start = installed.trim();
    let mut current = start.to_string();
    for _ in 0..MAX_HOPS {
        let here = OrderedVersion::parse_in(&current, ecosystem)?;
        let mut next: Option<Release> = None;
        let mut stuck = false;
        for (json, advisory) in ranges.iter().zip(&parsed) {
            match check_version_affected(ecosystem, Some(&current), json) {
                (false, _) => continue,
                (true, false) => return None,
                (true, true) => {}
            }
            match exit(ecosystem, &here, advisory.as_deref()?, &releases) {
                Exit::Stuck => stuck = true,
                Exit::To(step) => {
                    let replace = match next.as_ref() {
                        None => true,
                        Some(best) => outranks(&step, best)?,
                    };
                    if replace {
                        next = Some(step);
                    }
                }
            }
        }
        match next {
            Some((_, raw)) => current = raw,
            None => {
                return (current != start).then_some(WalkTarget {
                    version: current,
                    clears_all: !stuck,
                })
            }
        }
    }
    None
}

/// The lowest version reachable from `installed` that NO known advisory
/// affects: [`walk_target`] when it clears everything.
#[cfg(test)]
pub(crate) fn clean_version(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
) -> Option<String> {
    walk_target(ecosystem, installed, ranges)
        .filter(|w| w.clears_all)
        .map(|w| w.version)
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
        walked: Option<String>,
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

#[cfg(test)]
#[path = "fix_target_tests.rs"]
mod tests;
