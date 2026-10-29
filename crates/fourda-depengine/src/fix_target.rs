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
//! The pure half, moved out of the app's `osv/fix_target.rs` (single-engine
//! C0.1). The app keeps `line_targets` and friends, which aggregate its
//! `MatchedAdvisory` rows per project.

use std::cmp::Ordering;

use crate::range::{
    check_version_affected, is_unknown_bound, parse_version, Range, ENUMERATED_RANGE,
};
use crate::registry::{is_listed, RegistryReleases};
use crate::version_order::{in_window, Lower, OrderedVersion, Upper};

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
pub fn fix_for_version(
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
pub struct WalkTarget {
    pub version: String,
    /// No known advisory affects `version`. `false`: every advisory still
    /// affecting it has no way out (an open-ended window, or a
    /// `last_affected` bound with no release above it in the data); every
    /// advisory that HAS one is cleared.
    pub clears_all: bool,
}

/// How the walk treats one advisory (the host classifies; the crate stays
/// pure): a maintenance notice never blocks a target and is left out; a
/// malicious-package record with no way out is never "brought in", it makes
/// every fixing release a malicious one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdvisoryKind {
    #[serde(default)]
    pub malware: bool,
    #[serde(default)]
    pub informational: bool,
}

/// Where [`walk_target_in`] lands, and what it had to step over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalkOutcome {
    /// As [`walk_target`]; never a release the registry withdrew.
    pub target: Option<WalkTarget>,
    /// Withdrawn releases (yanked, deprecated) a step would otherwise have
    /// named. With no target, a non-empty list means a fix WAS published and
    /// then withdrawn, so the honest answer is "no installable fix", not "no
    /// fix". Empty when the walk could not evaluate an advisory.
    pub withheld: Vec<String>,
    /// Parity decision 2 (a target never brings in an advisory): indices
    /// into the walk's `ranges` of the advisories with no way out that affect
    /// where the walk lands but NOT the installed version. Non-empty means
    /// every fixing release brings them in, and `target` is `None`.
    pub introduces: Vec<usize>,
    /// With no target: indices of the malicious-package records that affect
    /// any release at or above where the walk lands (the MCP's
    /// `maliciousLanding`). Named as malware, never as `introduces`; set
    /// alongside `introduces` when both hold.
    pub malware: Vec<usize>,
}

/// A line fix as a release the registry still offers ([`installable_fix`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineFix {
    pub fix: Option<String>,
    /// The withdrawn releases passed over to reach `fix` (see
    /// [`WalkOutcome::withheld`]).
    pub withheld: Vec<String>,
}

/// The releases the walk can name, and the ones it must not.
///
/// `releases`: every stable release the advisory data names (every bound and
/// every enumerated version) plus every release the registry lists, when the
/// host supplied that list. Without it these are the only versions above a
/// `last_affected` bound the walk can name; a release the data never mentions
/// can only make the target higher than the true minimum, never vulnerable.
///
/// `excluded`: releases the registry withdrew. They stay in `releases` so a
/// step can tell it passed one, but no step lands on them, exactly as no step
/// lands on a pre-release.
struct Known {
    releases: Vec<Release>,
    excluded: Vec<OrderedVersion>,
    /// Every release the registry lists (pre-releases included), and the
    /// highest of them. Empty: no registry facts.
    published: Vec<OrderedVersion>,
    ceiling: Option<OrderedVersion>,
}

impl Known {
    fn new(
        ecosystem: &str,
        advisories: &[Option<Vec<Range>>],
        registry: &RegistryReleases,
    ) -> Self {
        let mut releases = advisory_releases(ecosystem, advisories);
        let published: Vec<OrderedVersion> = registry
            .published
            .iter()
            .filter_map(|v| OrderedVersion::parse_in(v, ecosystem))
            .collect();
        releases.extend(registry.published.iter().filter_map(|v| {
            OrderedVersion::parse_in(v, ecosystem)
                .filter(|p| !p.is_prerelease())
                .map(|p| (p, v.trim().to_string()))
        }));
        let ceiling = published
            .iter()
            .fold(None, |top: Option<&OrderedVersion>, v| match top {
                Some(t) if v.compare(t) != Some(Ordering::Greater) => Some(t),
                _ => Some(v),
            })
            .cloned();
        let mut known = Known {
            releases: Vec::new(),
            excluded: registry.excluded_in(ecosystem),
            published,
            ceiling,
        };
        // A bound the registry does not serve is no release at all: only a
        // `fixed` step that names one reports it (as withheld).
        releases.retain(|(v, _)| !known.unlisted(v));
        known.releases = releases;
        known
    }

    /// The advisory data names `version`, the registry lists a later release
    /// and not this one.
    fn unlisted(&self, version: &OrderedVersion) -> bool {
        self.ceiling.as_ref().is_some_and(|top| {
            version.compare(top) != Some(Ordering::Greater) && !is_listed(&self.published, version)
        })
    }

    /// Never a step: a release the registry withdrew, or (audit A-1) a
    /// version the advisory data names that the registry does not list
    /// although it lists a later one. node-ipc's GHSA-97m3-w2cp-4xx6 is
    /// "fixed" at 10.1.3, which npm does not serve (10.1.0, then 11.0.0). A
    /// version above every listed release may be newer than the facts, so
    /// the advisory's word stands for it.
    fn allowed(&self, version: &OrderedVersion) -> bool {
        !self.unlisted(version) && !is_listed(&self.excluded, version)
    }

    /// The lowest release above `bound` that `keep` accepts and the registry
    /// has not withdrawn. Every withdrawn release `keep` accepts below it
    /// (all of them, when there is none) goes into `withheld`.
    fn next_above(
        &self,
        bound: &OrderedVersion,
        keep: impl Fn(&OrderedVersion) -> bool,
        withheld: &mut Vec<String>,
    ) -> Option<Release> {
        let next = first_above(&self.releases, bound, |r| keep(r) && self.allowed(r));
        for (version, raw) in &self.releases {
            let passed = version.compare(bound) == Some(Ordering::Greater)
                && keep(version)
                && !self.allowed(version)
                && next
                    .as_ref()
                    .is_none_or(|(n, _)| version.compare(n) == Some(Ordering::Less));
            if passed {
                withhold(withheld, raw);
            }
        }
        next
    }

    /// A `fixed` bound as a step: the bound itself, unless the registry
    /// withdrew it; then the lowest allowed release above it.
    fn fixed_step(&self, fix: Release, withheld: &mut Vec<String>) -> Option<Release> {
        if self.allowed(&fix.0) {
            return Some(fix);
        }
        withhold(withheld, &fix.1);
        self.next_above(&fix.0, |_| true, withheld)
    }
}

fn withhold(withheld: &mut Vec<String>, raw: &str) {
    if !withheld.iter().any(|w| w == raw) {
        withheld.push(raw.to_string());
    }
}

/// The releases the advisory data names for this package (every bound and
/// every enumerated version, pre-releases left out).
fn advisory_releases(ecosystem: &str, advisories: &[Option<Vec<Range>>]) -> Vec<Release> {
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

/// The versions one enumerated `versions` list names.
fn enumerated(ecosystem: &str, range: &Range) -> Vec<OrderedVersion> {
    range
        .events
        .iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter_map(|v| OrderedVersion::parse_in(v, ecosystem))
        .collect()
}

/// Whether one advisory's `ranges` hold `version` (a readable window, or an
/// enumerated list naming it).
fn affects(ecosystem: &str, ranges: &[Range], version: &OrderedVersion) -> bool {
    windows(ecosystem, ranges)
        .iter()
        .any(|w| w.contains(version) == Some(true))
        || ranges
            .iter()
            .filter(|r| r.range_type == ENUMERATED_RANGE)
            .any(|r| is_listed(&enumerated(ecosystem, r), version))
}

/// The exit from every window of one advisory that holds `here`: a `fixed`
/// bound; above a `last_affected` bound, the first known release past it; for
/// an enumerated `versions` list, the first known release past `here` the list
/// does not name. A release the registry withdrew is never an exit: the next
/// allowed one is, and the withdrawn one goes into `withheld`. The highest
/// exit wins, since every window must be left.
fn exit(
    ecosystem: &str,
    here: &OrderedVersion,
    ranges: &[Range],
    known: &Known,
    withheld: &mut Vec<String>,
) -> Exit {
    let mut exits: Vec<Option<Release>> = Vec::new();
    for window in windows(ecosystem, ranges) {
        if window.contains(here) != Some(true) {
            continue;
        }
        exits.push(match window.end {
            Some((fix, raw, true)) => known.fixed_step((fix, raw), withheld),
            Some((last, _, false)) => known.next_above(&last, |_| true, withheld),
            None => None,
        });
    }
    for range in ranges.iter().filter(|r| r.range_type == ENUMERATED_RANGE) {
        let listed = enumerated(ecosystem, range);
        if is_listed(&listed, here) {
            exits.push(known.next_above(here, |r| !is_listed(&listed, r), withheld));
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
///
/// No registry facts: [`walk_target_in`] with none.
pub fn walk_target(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
) -> Option<WalkTarget> {
    walk_target_in(ecosystem, installed, ranges, &RegistryReleases::default()).target
}

/// [`walk_target`] with what the registry says about the package's releases:
/// every release it lists can be a step, and none it withdrew (yanked,
/// deprecated) can. A window whose every exit is withdrawn has no way out,
/// like an open-ended one, and the outcome names the withdrawn releases.
pub fn walk_target_in(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
    registry: &RegistryReleases,
) -> WalkOutcome {
    walk_target_kinds(ecosystem, installed, ranges, &[], registry)
}

/// [`walk_target_in`] with each advisory's [`AdvisoryKind`] (parallel to
/// `ranges`; a missing entry is an ordinary advisory). Indices in the outcome
/// are into `ranges`.
pub fn walk_target_kinds(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
    kinds: &[AdvisoryKind],
    registry: &RegistryReleases,
) -> WalkOutcome {
    let mut withheld = Vec::new();
    let kind = |i: usize| kinds.get(i).copied().unwrap_or_default();
    match walk(ecosystem, installed, ranges, &kind, registry, &mut withheld) {
        Ok(Landing::At(target)) => WalkOutcome {
            target,
            withheld,
            ..WalkOutcome::default()
        },
        Ok(Landing::Blocked {
            introduces,
            malware,
        }) => WalkOutcome {
            target: None,
            withheld,
            introduces,
            malware,
        },
        Err(Unreadable) => WalkOutcome::default(),
    }
}

/// The walk met an advisory it cannot evaluate on its path.
struct Unreadable;

/// Where the walk ends.
enum Landing {
    At(Option<WalkTarget>),
    /// No target: it lands inside advisories with no way out that the
    /// installed version does not have (`introduces`), or inside a malicious
    /// record; `malware` names every malicious record any release from the
    /// landing on falls in.
    Blocked {
        introduces: Vec<usize>,
        malware: Vec<usize>,
    },
}

fn walk(
    ecosystem: &str,
    installed: &str,
    ranges: &[&Option<String>],
    kind: &dyn Fn(usize) -> AdvisoryKind,
    registry: &RegistryReleases,
    withheld: &mut Vec<String>,
) -> Result<Landing, Unreadable> {
    let parsed: Vec<Option<Vec<Range>>> = ranges
        .iter()
        .map(|r| r.as_deref().and_then(|j| serde_json::from_str(j).ok()))
        .collect();
    let known = Known::new(ecosystem, &parsed, registry);
    let start = installed.trim();
    let mut current = start.to_string();
    for _ in 0..MAX_HOPS {
        let here = OrderedVersion::parse_in(&current, ecosystem).ok_or(Unreadable)?;
        let mut next: Option<Release> = None;
        let mut stuck: Vec<usize> = Vec::new();
        for (index, (json, advisory)) in ranges.iter().zip(&parsed).enumerate() {
            // A maintenance notice never blocks a target (decided for both
            // engines, 2026-10-19).
            if kind(index).informational {
                continue;
            }
            match check_version_affected(ecosystem, Some(&current), json) {
                (false, _) => continue,
                (true, false) => return Err(Unreadable),
                (true, true) => {}
            }
            let advisory = advisory.as_deref().ok_or(Unreadable)?;
            match exit(ecosystem, &here, advisory, &known, withheld) {
                Exit::Stuck => stuck.push(index),
                Exit::To(step) => {
                    let replace = match next.as_ref() {
                        None => true,
                        Some(best) => outranks(&step, best).ok_or(Unreadable)?,
                    };
                    if replace {
                        next = Some(step);
                    }
                }
            }
        }
        match next {
            Some((_, raw)) => current = raw,
            None if current == start => return Ok(Landing::At(None)),
            None => {
                // Never bring an advisory in (parity decision 2): a benign one
                // with no way out that the installed version does not have
                // blocks the target. A malicious record is never "brought
                // in": it blocks too, and is named as malware. The MCP's
                // `smallestCleanVersion(installed)` agrees.
                let blocks_malware = stuck.iter().any(|&i| kind(i).malware);
                let introduced: Vec<usize> = stuck
                    .iter()
                    .copied()
                    .filter(|&i| !kind(i).malware)
                    .filter(|&i| !check_version_affected(ecosystem, Some(start), ranges[i]).0)
                    .collect();
                if !blocks_malware && introduced.is_empty() {
                    return Ok(Landing::At(Some(WalkTarget {
                        version: current,
                        clears_all: stuck.is_empty(),
                    })));
                }
                let malware = malicious_landing(ecosystem, &here, ranges, kind, &known);
                return Ok(Landing::Blocked {
                    introduces: introduced,
                    malware,
                });
            }
        }
    }
    Err(Unreadable)
}

/// The malicious records (by index) that affect any release the walk could
/// name from `from` on: known, not withdrawn, not a pre-release (the MCP's
/// `maliciousLanding` over its candidates).
fn malicious_landing(
    ecosystem: &str,
    from: &OrderedVersion,
    ranges: &[&Option<String>],
    kind: &dyn Fn(usize) -> AdvisoryKind,
    known: &Known,
) -> Vec<usize> {
    let candidates: Vec<&str> = known
        .releases
        .iter()
        .filter(|(v, _)| v.compare(from) != Some(Ordering::Less) && known.allowed(v))
        .map(|(_, raw)| raw.as_str())
        .collect();
    (0..ranges.len())
        .filter(|&i| kind(i).malware && !kind(i).informational)
        .filter(|&i| {
            candidates
                .iter()
                .any(|v| check_version_affected(ecosystem, Some(v), ranges[i]).0)
        })
        .collect()
}

/// A fix bound (a line fix, or an advisory's listed fix) as a release the
/// registry still offers: `fix` itself, or, when the registry withdrew it or
/// does not list it below a release it does list, the lowest allowed release
/// above it that this advisory (`ranges_json`) does not affect. `fix: None`
/// with `withheld` set: the fix is not installable, and no installable
/// release past it is known.
pub fn installable_fix(
    ecosystem: &str,
    fix: &str,
    ranges_json: &Option<String>,
    registry: &RegistryReleases,
) -> LineFix {
    let parsed: [Option<Vec<Range>>; 1] = [ranges_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok())];
    let known = Known::new(ecosystem, &parsed, registry);
    let bound = OrderedVersion::parse_in(fix, ecosystem);
    let Some(bound) = bound.filter(|b| !known.allowed(b)) else {
        return LineFix {
            fix: Some(fix.to_string()),
            withheld: Vec::new(),
        };
    };
    let ranges: &[Range] = parsed[0].as_deref().unwrap_or_default();
    let mut withheld = vec![fix.trim().to_string()];
    let next = known.next_above(&bound, |v| !affects(ecosystem, ranges, v), &mut withheld);
    LineFix {
        fix: next.map(|(_, raw)| raw),
        withheld,
    }
}

/// [`fix_for_version`] through [`installable_fix`]. `None` fix and no
/// `withheld`: the line has no fix at all.
pub fn installable_line_fix(
    ecosystem: &str,
    user_version: &str,
    affected_ranges_json: &Option<String>,
    registry: &RegistryReleases,
) -> LineFix {
    fix_for_version(ecosystem, user_version, affected_ranges_json)
        .map(|fix| installable_fix(ecosystem, &fix, affected_ranges_json, registry))
        .unwrap_or_default()
}

/// The lowest version reachable from `installed` that NO known advisory
/// affects: [`walk_target`] when it clears everything.
pub fn clean_version(
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

#[cfg(test)]
#[path = "fix_target_tests.rs"]
mod tests;
