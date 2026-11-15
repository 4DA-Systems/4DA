// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Held dependencies: what an npm release installs whatever the project's
//! lockfile holds (audit 6.2.5 D-4, the rule `@4da/mcp-server` 6.2.5 applies
//! in `src/live/held-dependencies.ts` / `held-targets.ts`).
//!
//! A release's own advisories are not the whole answer to "is this a clean
//! fix". express 4.20.0 exact-pins qs 6.11.0, cookie 0.6.0, body-parser
//! 1.20.3 and path-to-regexp 0.1.10; installing it installs them, and they
//! are affected. express 4.22.0 to 4.22.2 require qs `~6.14.0` / `~6.15.1`,
//! and every qs those ranges admit is affected by GHSA-4mjr-xmp4-gh2g (fixed
//! 6.16.0), so only 4.22.3 (`~6.16.0`) installs clean.
//!
//! So a release HOLDS a dependency (npm only; `dependencies` plus
//! `optionalDependencies`, the optional spec winning a name clash; dev and
//! peer dependencies are not read) when its spec is a registry spec: a
//! package name npm accepts and a plain version range ([`is_readable_npm_spec`];
//! dist-tags, protocols, paths and URLs install nothing this can judge). It is
//! - an exact pin: a bare version or `=version`, installed as written; or
//! - a range, narrow (`~1.2.3`, `^0.1.2`) or wide (`^6.0.2`, `>=1`, `*`),
//!   judged at its NEWEST ADMITTED release, what a fresh install or a lockfile
//!   refresh resolves it to: the `latest` dist-tag when the range admits it,
//!   else the highest release it admits (withdrawn releases included,
//!   pre-releases only when the spec names one).
//!
//! An npm alias (`"es": "npm:event-stream@3.3.6"`) installs the aliased
//! package and is judged as it (no spec after the name: `*`).
//!
//! Wide ranges were once left out (rule D-4, vector ft-086) on the premise
//! that a refresh reaches a clean release. That holds only when one exists:
//! mocha 10.6.0 to 10.8.2 require serialize-javascript `^6.0.2`, every 6.x is
//! affected (GHSA-5c6j-r48x-rmvq, fixed 7.0.3), and 10.6.0 was named a clean
//! target (pre-publish round 10). A wide range whose newest admitted release
//! is clean brings nothing, so the old rule's intent stands wherever its
//! premise does. A dependency whose releases cannot be read is NOT CHECKED,
//! never clean.
//!
//! A held dependency is affected when any advisory that is not a maintenance
//! notice covers its name at the judged version. The walk ([`judge_target`])
//! starts at the target the package's own advisories name and tries the
//! later stable releases on its caret line that those advisories leave clean,
//! [`HELD_CANDIDATES`] in all; the first holding nothing affected wins. None
//! clean: the target stays and is NOT a clean fix. The check could not run:
//! NOT CHECKED, and the target never moves on that.
//!
//! Pure: the host supplies the registry facts and the advisory lookup
//! ([`HeldSources`]).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::fix_target::AdvisoryKind;
use crate::npm_range::requirement_admits;
use crate::range::check_version_affected;
use crate::version_order::OrderedVersion;

/// How many releases a fix-target search judges at most: the target and the
/// next ones on its line.
pub const HELD_CANDIDATES: usize = 10;

const NPM: &str = "npm";

fn pattern(cell: &'static OnceLock<Option<Regex>>, re: &str) -> Option<&'static Regex> {
    cell.get_or_init(|| Regex::new(re).ok()).as_ref()
}

fn matches(cell: &'static OnceLock<Option<Regex>>, re: &str, text: &str) -> bool {
    pattern(cell, re).is_some_and(|r| r.is_match(text))
}

/// The version an exact npm pin installs (`6.11.0`, `=6.11.0`, `v6.11.0`,
/// build metadata dropped); `None` for anything else.
pub fn exact_pin_version(spec: &str) -> Option<String> {
    static EXACT: OnceLock<Option<Regex>> = OnceLock::new();
    let re = pattern(
        &EXACT,
        r"^=?\s*v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)(?:\+[0-9A-Za-z.-]+)?$",
    )?;
    re.captures(spec.trim())
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// npm's package-name grammar (validate-npm-package-name, legacy capitals
/// included), at most 214 characters: the MCP's `isNpmPackageName`.
pub fn is_npm_package_name(name: &str) -> bool {
    static NAME: OnceLock<Option<Regex>> = OnceLock::new();
    !name.is_empty()
        && name.len() <= 214
        && matches(
            &NAME,
            r"(?i)^(?:@[a-z0-9~][a-z0-9._~-]*/)?[a-z0-9~][a-z0-9._~-]*$",
            name,
        )
}

/// Whether a spec is a plain npm version range (comparators, `-` and `||`
/// only), at most 80 characters: the MCP's `isReadableNpmSpec`. A dist-tag,
/// protocol, path, URL or prose names no range a registry install resolves.
pub fn is_readable_npm_spec(spec: &str) -> bool {
    static TOKEN: OnceLock<Option<Regex>> = OnceLock::new();
    static OP_SPACE: OnceLock<Option<Regex>> = OnceLock::new();
    let s = spec.trim();
    if s.is_empty() || s.len() > 80 {
        return false;
    }
    let Some(op_space) = pattern(&OP_SPACE, r"([<>=~^])\s+") else {
        return false;
    };
    let joined = op_space.replace_all(s, "$1");
    joined.split_whitespace().all(|t| {
        t == "-"
            || t == "||"
            || matches(
                &TOKEN,
                r"^(?:[~^]|[<>]=?|=)?v?(?:\d+|[xX*])(?:\.(?:\d+|[xX*])){0,2}(?:-[0-9A-Za-z.-]{1,40})?(?:\+[0-9A-Za-z.-]{1,40})?$",
                t,
            )
    })
}

/// The package and spec an npm alias installs: `npm:event-stream@3.3.6` ->
/// (`event-stream`, `3.3.6`), `npm:@scope/x` -> (`@scope/x`, `*`). `None`
/// for anything that is not an alias (the MCP's `npmAliasTarget`).
pub fn npm_alias_target(spec: &str) -> Option<(String, String)> {
    static ALIAS: OnceLock<Option<Regex>> = OnceLock::new();
    let re = pattern(&ALIAS, r"^npm:((?:@[^/@\s]+/)?[^/@\s]+)(?:@(.*))?$")?;
    let c = re.captures(spec.trim())?;
    let name = c.get(1)?.as_str().to_string();
    let range = c.get(2).map(|m| m.as_str().trim()).unwrap_or_default();
    Some((name, if range.is_empty() { "*" } else { range }.to_string()))
}

/// A release's held specs from `dependencies` and `optionalDependencies` (npm
/// installs both; the optional spec wins a name clash, as npm reads it):
/// every registry spec, exact or a range of any width, keyed by the package
/// it installs (an alias by its target). Non-string specs, names npm would
/// not accept and specs that are not plain ranges are skipped. An alias and a
/// plain entry on one package both install: one spec is kept, an exact pin
/// winning over a range (a pin is what a refresh can never move off an
/// affected copy), else the first read (the MCP keeps the document's order;
/// `serde_json` here reads keys in name order).
pub fn held_specs_of(
    dependencies: Option<&serde_json::Value>,
    optional: Option<&serde_json::Value>,
) -> BTreeMap<String, String> {
    let mut merged: Vec<(&String, &serde_json::Value)> = Vec::new();
    for section in [dependencies, optional].into_iter().flatten() {
        for (key, spec) in section.as_object().into_iter().flatten() {
            match merged.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = spec,
                None => merged.push((key, spec)),
            }
        }
    }
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (key, raw) in merged {
        let Some(raw) = raw.as_str() else { continue };
        let (name, spec) = npm_alias_target(raw).unwrap_or_else(|| (key.clone(), raw.to_string()));
        if !is_npm_package_name(key) || !is_npm_package_name(&name) || !is_readable_npm_spec(&spec)
        {
            continue;
        }
        let spec = spec.trim().to_string();
        let replace = out.get(&name).is_none_or(|have| {
            exact_pin_version(&spec).is_some() && exact_pin_version(have).is_none()
        });
        if replace {
            out.insert(name, spec);
        }
    }
    out
}

/// What the registry says about a held dependency a range names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldFacts {
    /// Every release the registry lists, withdrawn ones included.
    pub published: Vec<String>,
    /// The `latest` dist-tag.
    pub latest: Option<String>,
}

fn npm_version(v: &str) -> Option<OrderedVersion> {
    OrderedVersion::parse_in(v, NPM)
}

fn is_prerelease(v: &str) -> bool {
    npm_version(v).is_some_and(|p| p.is_prerelease())
}

fn compare(a: &str, b: &str) -> Option<Ordering> {
    npm_version(a)?.compare(&npm_version(b)?)
}

/// The release an install of `spec` gets: the pin, else (a range of any
/// width, from `facts`) `latest` when the range admits it, else the highest release
/// it admits (withdrawn included; pre-releases only when the spec has `-`).
/// `None` when it cannot be said.
pub fn judged_version(spec: &str, facts: Option<&HeldFacts>) -> Option<String> {
    if let Some(pinned) = exact_pin_version(spec) {
        return Some(pinned);
    }
    let facts = facts?;
    let admits = |v: &str| requirement_admits(spec, v) == Some(true);
    if let Some(latest) = facts.latest.as_deref().filter(|l| admits(l)) {
        return Some(latest.to_string());
    }
    let with_pre = spec.contains('-');
    facts
        .published
        .iter()
        .filter(|v| (with_pre || !is_prerelease(v)) && admits(v))
        .fold(None::<&String>, |best, v| match best {
            Some(b) if compare(v, b) != Some(Ordering::Greater) => Some(b),
            _ => Some(v),
        })
        .cloned()
}

/// Same compatibility line in npm's caret sense: same major, or the same
/// `0.minor` below 1.0.
pub fn same_npm_line(a: &str, b: &str) -> bool {
    static LINE: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(re) = pattern(&LINE, r"^v?(\d+)\.(\d+)\.") else {
        return false;
    };
    let (Some(pa), Some(pb)) = (re.captures(a.trim()), re.captures(b.trim())) else {
        return false;
    };
    let part = |c: &regex::Captures<'_>, i: usize| c.get(i).map(|m| m.as_str().to_string());
    part(&pa, 1) == part(&pb, 1)
        && (part(&pa, 1).as_deref() != Some("0") || part(&pa, 2) == part(&pb, 2))
}

/// The package's own advisories, as the candidate search reads them (the
/// MCP's `cleanReleases`): a release is clean when no malicious record
/// affects it and every other advisory that does is one with no fix
/// published (it also affects the newest release and the installed one).
/// Any advisory whose ranges cannot be read makes no release clean.
pub struct OwnAdvisories<'a> {
    benign: Vec<&'a Option<String>>,
    malicious: Vec<&'a Option<String>>,
    unfixed: Vec<usize>,
    unreadable: bool,
}

impl<'a> OwnAdvisories<'a> {
    /// `advisories`: the package's records (ranges JSON and kind);
    /// `releases`: its installable releases (withdrawn left out).
    pub fn new(
        installed: Option<&str>,
        releases: &[String],
        advisories: &[(&'a Option<String>, AdvisoryKind)],
    ) -> Self {
        let real: Vec<&(&Option<String>, AdvisoryKind)> = advisories
            .iter()
            .filter(|(_, k)| !k.informational)
            .collect();
        let unreadable = real.iter().any(|(r, _)| {
            r.as_deref()
                .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                .is_none()
        });
        let malicious: Vec<&Option<String>> = real
            .iter()
            .filter(|(_, k)| k.malware)
            .map(|(r, _)| *r)
            .collect();
        let benign: Vec<&Option<String>> = real
            .iter()
            .filter(|(_, k)| !k.malware)
            .map(|(r, _)| *r)
            .collect();
        let mut stable: Vec<&String> = releases.iter().filter(|v| !is_prerelease(v)).collect();
        stable.sort_by(|a, b| compare(a, b).unwrap_or(Ordering::Equal));
        let newest = stable
            .iter()
            .rev()
            .find(|v| !malicious.iter().any(|r| affects(r, v)))
            .copied();
        let unfixed = newest
            .map(|n| {
                (0..benign.len())
                    .filter(|&i| {
                        affects(benign[i], n) && installed.is_none_or(|v| affects(benign[i], v))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            benign,
            malicious,
            unfixed,
            unreadable,
        }
    }

    /// The package's own advisories leave `version` clean.
    pub fn clean(&self, version: &str) -> bool {
        !self.unreadable
            && !self.malicious.iter().any(|r| affects(r, version))
            && self
                .benign
                .iter()
                .enumerate()
                .all(|(i, r)| !affects(r, version) || self.unfixed.contains(&i))
    }

    /// No advisory of the package affects `version` at all.
    pub fn clears_all(&self, version: &str) -> bool {
        !self.unreadable
            && !self
                .malicious
                .iter()
                .chain(self.benign.iter())
                .any(|r| affects(r, version))
    }
}

/// An advisory covers `version` (a window the matcher cannot evaluate counts:
/// the conservative answer every "this version is clear" check gives).
#[allow(clippy::ref_option)] // the shape `check_version_affected` takes
fn affects(ranges: &Option<String>, version: &str) -> bool {
    check_version_affected(NPM, Some(version), ranges).0
}

/// The releases a fix-target search judges, smallest first: `target`, then
/// the later stable releases on its caret line that `clean` accepts (the
/// package's own advisories, [`OwnAdvisories::clean`]), [`HELD_CANDIDATES`]
/// in all at most.
pub fn held_candidates(
    target: &str,
    releases: &[String],
    clean: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut later: Vec<&String> = releases
        .iter()
        .filter(|v| {
            !is_prerelease(v)
                && same_npm_line(v, target)
                && compare(v, target) == Some(Ordering::Greater)
                && clean(v)
        })
        .collect();
    later.sort_by(|a, b| compare(a, b).unwrap_or(Ordering::Equal));
    later.dedup();
    std::iter::once(target.to_string())
        .chain(later.into_iter().cloned())
        .take(HELD_CANDIDATES)
        .collect()
}

/// One held dependency an advisory affects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldDependency {
    pub name: String,
    /// The release's own spec for it, as published.
    pub spec: String,
    /// An exact pin; `false` for a range (narrow or wide).
    pub exact: bool,
    /// What an install of the release gets.
    pub version: String,
    /// The advisories that affect `version`, one id per alias group.
    pub ids: Vec<String>,
}

/// Whether a release holds an affected dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldCheck {
    /// Judged: the affected held dependencies (empty: it installs clean).
    Checked(Vec<HeldDependency>),
    /// Something it holds could not be judged, and why.
    Unchecked(String),
}

/// What the host reads for the rule.
pub trait HeldSources {
    /// The registry facts of a dependency a range names; `None` when they
    /// were not read.
    fn facts(&self, name: &str) -> Option<HeldFacts>;
    /// The advisories (one id per alias group, maintenance notices left out)
    /// that affect `name` at `version`; `None` when that package's advisories
    /// cannot be read.
    fn affected(&self, name: &str, version: &str) -> Option<Vec<String>>;
    /// The release an install of the range `spec` on `name` gets
    /// ([`judged_version`] over [`HeldSources::facts`]). A host judging many
    /// releases memoizes it per (name, spec).
    fn judged(&self, name: &str, spec: &str) -> Option<String> {
        judged_version(spec, self.facts(name).as_ref())
    }
}

/// "a, b and 3 more" for a reason.
fn listed(items: &[String]) -> String {
    let shown = items.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    match items.len() {
        n if n > 3 => format!("{shown} and {} more", n - 3),
        _ => shown,
    }
}

/// Judge one release by its held `specs`. A concrete finding stands even
/// when something else it holds could not be judged; otherwise anything
/// unjudged makes the release unchecked.
pub fn check_release(specs: &BTreeMap<String, String>, sources: &dyn HeldSources) -> HeldCheck {
    let mut affected = Vec::new();
    let mut unread_ranges = Vec::new();
    let mut unread_advisories = Vec::new();
    for (name, spec) in specs {
        let pinned = exact_pin_version(spec);
        let exact = pinned.is_some();
        let Some(version) = pinned.or_else(|| sources.judged(name, spec)) else {
            unread_ranges.push(format!("{name} \"{spec}\""));
            continue;
        };
        match sources.affected(name, &version) {
            Some(ids) if !ids.is_empty() => affected.push(HeldDependency {
                name: name.clone(),
                spec: spec.clone(),
                exact,
                version,
                ids,
            }),
            Some(_) => {}
            None => unread_advisories.push(name.clone()),
        }
    }
    if !affected.is_empty() {
        HeldCheck::Checked(affected)
    } else if !unread_ranges.is_empty() {
        HeldCheck::Unchecked(format!(
            "the releases {} admit could not be read from the npm registry",
            listed(&unread_ranges)
        ))
    } else if !unread_advisories.is_empty() {
        HeldCheck::Unchecked(format!(
            "the advisories of {} are not in the local advisory mirror yet",
            listed(&unread_advisories)
        ))
    } else {
        HeldCheck::Checked(Vec::new())
    }
}

/// One release the walk passed, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldPast {
    pub version: String,
    pub affected: Vec<HeldDependency>,
}

/// What the held-dependency rule says about a fix target. `None` from
/// [`judge_target`]: the target holds nothing affected (or nothing at all).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HeldOutcome {
    /// `from` (the advisories' target) holds an affected dependency; `to`,
    /// the smallest later release on its line whose own advisories and held
    /// dependencies are clean, is the target. `past` names each release
    /// judged and passed, `from` first.
    Moved {
        from: String,
        to: String,
        past: Vec<HeldPast>,
    },
    /// The target holds `affected` and no release `searched` that could be
    /// checked clears it: the target stays, NOT a clean fix. `unchecked`:
    /// the releases among `searched` whose held dependencies could not be
    /// judged (passed, never called clean or affected).
    Blocked {
        target: String,
        affected: Vec<HeldDependency>,
        searched: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unchecked: Vec<String>,
    },
    /// Whether the target's held dependencies are affected could not be
    /// checked: the target stays, and is not called a clean fix.
    Unchecked { target: String, reason: String },
}

impl HeldOutcome {
    /// The release the outcome leaves as the target.
    pub fn target(&self) -> &str {
        match self {
            Self::Moved { to, .. } => to,
            Self::Blocked { target, .. } | Self::Unchecked { target, .. } => target,
        }
    }

    /// The target is a clean fix as far as what it holds goes.
    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Moved { .. })
    }
}

/// The held-dependency walk over `candidates` (target first,
/// [`held_candidates`]), judging each with `check`.
pub fn judge_target(
    candidates: &[String],
    check: impl Fn(&str) -> HeldCheck,
) -> Option<HeldOutcome> {
    let target = candidates.first()?;
    let own = match check(target) {
        HeldCheck::Unchecked(reason) => {
            return Some(HeldOutcome::Unchecked {
                target: target.clone(),
                reason,
            })
        }
        HeldCheck::Checked(affected) if affected.is_empty() => return None,
        HeldCheck::Checked(affected) => affected,
    };
    let mut unchecked = Vec::new();
    let mut past = vec![HeldPast {
        version: target.clone(),
        affected: own.clone(),
    }];
    for v in &candidates[1..] {
        match check(v) {
            HeldCheck::Checked(a) if a.is_empty() => {
                return Some(HeldOutcome::Moved {
                    from: target.clone(),
                    to: v.clone(),
                    past,
                })
            }
            HeldCheck::Checked(a) => past.push(HeldPast {
                version: v.clone(),
                affected: a,
            }),
            // Passed: the one chosen after it is itself judged clean; with
            // none chosen, the note names it NOT CHECKED.
            HeldCheck::Unchecked(_) => unchecked.push(v.clone()),
        }
    }
    Some(HeldOutcome::Blocked {
        target: target.clone(),
        affected: own,
        searched: candidates.to_vec(),
        unchecked,
    })
}

fn and(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// "4.20.0 exact-pins qs 6.11.0 (GHSA-a) and cookie 0.6.0 (GHSA-c)", and for a
/// range "4.22.0 requires qs \"~6.14.0\", whose newest admitted release
/// 6.14.2 is affected (GHSA-d)". At most three dependencies are named.
pub fn held_text(release: &str, affected: &[HeldDependency]) -> String {
    let shown = &affected[..affected.len().min(3)];
    let exact: Vec<String> = shown
        .iter()
        .filter(|d| d.exact)
        .map(|d| format!("{} {} ({})", d.name, d.version, d.ids.join(", ")))
        .collect();
    let ranged: Vec<String> = shown
        .iter()
        .filter(|d| !d.exact)
        .map(|d| {
            format!(
                "{} \"{}\", whose newest admitted release {} is affected ({})",
                d.name,
                d.spec,
                d.version,
                d.ids.join(", ")
            )
        })
        .collect();
    let mut clauses = Vec::new();
    if !exact.is_empty() {
        clauses.push(format!("exact-pins {}", and(&exact)));
    }
    if !ranged.is_empty() {
        clauses.push(format!("requires {}", and(&ranged)));
    }
    let rest = affected.len() - shown.len();
    let more = match rest {
        0 => String::new(),
        1 => ", and 1 more affected dependency".to_string(),
        n => format!(", and {n} more affected dependencies"),
    };
    format!("{release} {}{more}", clauses.join("; it also "))
}

/// The sentence a surface prints for the outcome (the MCP's `heldNote`).
pub fn held_note(outcome: &HeldOutcome) -> String {
    match outcome {
        HeldOutcome::Moved { to, past, .. } => {
            let Some((first, rest)) = past.split_first() else {
                return String::new();
            };
            let mut names: Vec<&str> = Vec::new();
            for d in rest.iter().flat_map(|p| p.affected.iter()) {
                if !names.contains(&d.name.as_str()) {
                    names.push(&d.name);
                }
            }
            let later = match rest {
                [] => String::new(),
                [one] => format!(
                    "; {} still brings in an affected {}",
                    one.version,
                    names.join(", ")
                ),
                [lo, .., hi] => format!(
                    "; {} later releases ({} to {}) still bring in an affected {}",
                    rest.len(),
                    lo.version,
                    hi.version,
                    names.join(", ")
                ),
            };
            format!(
                "{}{later}; so {to}, the smallest later release on its line whose own advisories \
                 and held dependencies are clean",
                held_text(&first.version, &first.affected)
            )
        }
        HeldOutcome::Blocked {
            target,
            affected,
            searched,
            unchecked,
        } => {
            let up_to = searched
                .last()
                .filter(|l| *l != target)
                .map(|l| format!("up to {l} "))
                .unwrap_or_default();
            let them = if affected.len() == 1 { "it" } else { "them" };
            let partial = if unchecked.is_empty() {
                String::new()
            } else {
                format!(
                    ", of the releases that could be checked ({} NOT CHECKED)",
                    and(unchecked)
                )
            };
            format!(
                "{}; no release on its line {up_to}that its own advisories leave clean clears \
                 {them}{partial}, so {target} is NOT a clean fix",
                held_text(target, affected)
            )
        }
        HeldOutcome::Unchecked { target, reason } => format!(
            "whether the dependencies {target} exact-pins or narrowly ranges are affected was NOT \
             CHECKED ({reason}), so {target} is not confirmed as a clean fix"
        ),
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
