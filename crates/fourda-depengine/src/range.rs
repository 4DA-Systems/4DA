// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV affected-window evaluation: is this version inside an advisory's
//! ranges? Moved out of the app's `osv/matching.rs` (single-engine C0.1); the
//! app's matcher reads `osv_advisories` and the dependency tables, then asks
//! [`check_version_affected`] about each copy.

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::version_order::{in_window, Lower, OrderedVersion, Upper};

/// The `type` of the synthetic range that carries an advisory's enumerated
/// `versions` list in the stored ranges JSON (events are the version
/// strings). Readers that only understand `SEMVER`/`ECOSYSTEM` skip it.
pub const ENUMERATED_RANGE: &str = "ENUMERATED";

/// One OSV range as the app stores it (`osv_advisories.affected_ranges` is a
/// JSON array of these).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Range {
    #[serde(rename = "type")]
    pub range_type: String,
    pub events: Option<Vec<serde_json::Value>>,
}

/// Check if a user's version is affected by the advisory's ranges.
/// Returns (is_affected, is_confirmed).
/// - is_affected: true if the version falls within affected ranges (or conservative fallback)
/// - is_confirmed: true only if we could definitively verify via semver
///
/// `ecosystem` is the advisory's: a PyPI version orders by PEP 440 even when
/// it also reads as semver (`version_order::OrderedVersion::parse_in`). An
/// empty or unknown ecosystem keeps the generic semver-first order.
///
/// The answer is a pure function of its three inputs, so it is memoized per
/// thread (see [`VERDICTS`]): the parent-release walk asks the same
/// (advisory, version) pair thousands of times per pass, and each uncached
/// call parses the ranges JSON and every bound again. Live 2026-11-06, a
/// fresh profile with 1,904 packages: those re-parses were most of a 156 s
/// (debug) Brief facts build.
pub fn check_version_affected(
    ecosystem: &str,
    user_version: Option<&str>,
    affected_ranges_json: &Option<String>,
) -> (bool, bool) {
    // Only a JSON-carrying call is worth remembering; the others return at once.
    let (Some(json), Some(version)) = (affected_ranges_json.as_deref(), user_version) else {
        return check_version_affected_uncached(ecosystem, user_version, affected_ranges_json);
    };
    let inner = format!("{ecosystem}\u{0}{version}");
    let hit = VERDICTS.with(|m| m.borrow().get(json).and_then(|v| v.get(&inner).copied()));
    if let Some(hit) = hit {
        return hit;
    }
    let verdict = check_version_affected_uncached(ecosystem, user_version, affected_ranges_json);
    VERDICTS.with(|m| {
        let mut m = m.borrow_mut();
        // Bounded: a long-lived worker thread never grows without limit.
        if m.len() >= VERDICT_CACHE_MAX_RANGES {
            m.clear();
        }
        let versions = m.entry(json.to_string()).or_default();
        if versions.len() >= VERDICT_CACHE_MAX_VERSIONS {
            versions.clear();
        }
        versions.insert(inner, verdict);
    });
    verdict
}

/// Distinct ranges documents remembered per thread before the memo starts over.
const VERDICT_CACHE_MAX_RANGES: usize = 4_096;
/// Versions remembered per ranges document.
const VERDICT_CACHE_MAX_VERSIONS: usize = 4_096;

type Verdicts = std::collections::HashMap<String, std::collections::HashMap<String, (bool, bool)>>;

std::thread_local! {
    /// ranges JSON -> ("ecosystem\0version" -> verdict), for this thread.
    static VERDICTS: std::cell::RefCell<Verdicts> = std::cell::RefCell::new(Verdicts::new());
    static EVALUATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many uncached evaluations this thread has run: the memo's work bound,
/// for tests.
#[doc(hidden)]
pub fn range_evaluations() -> usize {
    EVALUATIONS.with(std::cell::Cell::get)
}

/// [`check_version_affected`] without the memo: the evaluation itself.
pub fn check_version_affected_uncached(
    ecosystem: &str,
    user_version: Option<&str>,
    affected_ranges_json: &Option<String>,
) -> (bool, bool) {
    EVALUATIONS.with(|n| n.set(n.get() + 1));
    let ranges_json = match affected_ranges_json {
        Some(json) if !json.is_empty() => json,
        _ => return (true, false), // No range info → conservative match
    };

    let ranges: Vec<Range> = match serde_json::from_str(ranges_json) {
        Ok(r) => r,
        Err(_) => return (true, false), // Can't parse → conservative
    };

    let user_ver_str = match user_version {
        Some(v) if !v.is_empty() => v,
        _ => return (true, false), // No version → conservative
    };

    let Some(user) = OrderedVersion::parse_in(user_ver_str, ecosystem) else {
        return (true, false); // Can't parse user version → conservative
    };

    // A bound the matcher cannot parse (or compare with this version) leaves
    // its window undecided. Reading that as "not in any window" answered
    // "confirmed NOT affected" about a window it never evaluated
    // (dependency-handoff Phase 1.4).
    let mut undecided = false;
    for range in &ranges {
        if range.range_type == ENUMERATED_RANGE {
            if enumerated_contains(ecosystem, &user, user_ver_str, range) {
                return (true, true);
            }
            continue;
        }
        if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
            continue;
        }
        match range_contains(ecosystem, &user, range) {
            Some(true) => return (true, true),
            Some(false) => {}
            None => undecided = true,
        }
    }

    if undecided {
        // Conservative, unconfirmed: the same answer as unparseable ranges JSON.
        return (true, false);
    }
    // Went through all ranges, version not in any affected window
    (false, true)
}

/// Whether an advisory's enumerated `versions` list (stored as an
/// [`ENUMERATED_RANGE`] range) names this version — exactly, or as the same
/// version under the ecosystem's ordering (`19.9` = `19.9.0`).
fn enumerated_contains(ecosystem: &str, user: &OrderedVersion, raw: &str, range: &Range) -> bool {
    range
        .events
        .iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .any(|listed| {
            listed == raw.trim()
                || OrderedVersion::parse_in(listed, ecosystem)
                    .and_then(|v| user.compare(&v))
                    .is_some_and(std::cmp::Ordering::is_eq)
        })
}

/// Whether `user` sits in any window of one range: `Some(true)` inside one,
/// `Some(false)` outside all of them, `None` when a window could not be
/// decided (an unreadable bound, or one no ordering compares with `user`).
fn range_contains(ecosystem: &str, user: &OrderedVersion, range: &Range) -> Option<bool> {
    let mut undecided = false;
    let mut lower: Option<Lower> = None;
    let mut note = |verdict: Option<bool>| match verdict {
        Some(true) => true,
        Some(false) => false,
        None => {
            undecided = true;
            false
        }
    };
    for obj in range.events.iter().flatten().filter_map(|e| e.as_object()) {
        if let Some(intro) = obj.get("introduced").and_then(|v| v.as_str()) {
            lower = Lower::parse_in(&coerce_bound(intro, ecosystem), ecosystem);
            if lower.is_none() && note(None) {
                return Some(true);
            }
        }
        let upper = obj
            .get("fixed")
            .map(|v| (v, false))
            .or_else(|| obj.get("last_affected").map(|v| (v, true)));
        if let Some((bound, inclusive)) = upper {
            let bound = bound.as_str().unwrap_or("");
            if let (Some(low), false) = (lower.as_ref(), is_unknown_bound(bound)) {
                let bound = coerce_bound(bound, ecosystem);
                let verdict = OrderedVersion::parse_in(&bound, ecosystem).and_then(|version| {
                    in_window(
                        user,
                        low,
                        Some(Upper {
                            version: &version,
                            inclusive,
                        }),
                    )
                });
                if note(verdict) {
                    return Some(true);
                }
            }
            lower = None;
        }
    }
    // introduced with no fix → every version from introduced onward
    if let Some(low) = lower.as_ref() {
        if note(in_window(user, low, None)) {
            return Some(true);
        }
    }
    (!undecided).then_some(false)
}

/// A range bound as OSV.dev evaluates it, for the semver ecosystems (npm,
/// crates.io, Go). Advisory data is hand-written and sometimes not semver:
/// GHSA-92cp-5422-2mw7 (go-redis v9) opens a range at `9.6.0b1`. Unread, the
/// window was undecided; OSV's own coercion (missing minor and patch are 0,
/// a bare suffix is a prerelease) puts it just below 9.6.0, which is what
/// api.osv.dev answers per version (9.6.0 to 9.6.2 affected, 9.5.5 and 9.6.3
/// not, 2026-11-10). A bound that already reads as semver, `0`, a bound of
/// another ecosystem, or one even the coercion cannot read is kept as
/// written. The MCP server's `coerceBound` (osv-ranges.ts) is the same rule
/// (Y4: one reading on every surface).
pub fn coerce_bound<'a>(bound: &'a str, ecosystem: &str) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;
    let semver_family = matches!(
        ecosystem.to_ascii_lowercase().as_str(),
        "npm" | "crates.io" | "go"
    );
    let t = bound.trim();
    if t == "0" || !semver_family || parse_version(t).is_some() {
        return Cow::Borrowed(bound);
    }
    let (v, body) = t.strip_prefix('v').map_or(("", t), |b| ("v", b));
    // major[.minor[.patch]], then an optional separated or bare suffix.
    let mut parts: Vec<&str> = Vec::new();
    let mut rest = body;
    while parts.len() < 3 {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            break;
        }
        parts.push(&rest[..digits]);
        rest = &rest[digits..];
        match rest.strip_prefix('.') {
            Some(r) if parts.len() < 3 && r.starts_with(|c: char| c.is_ascii_digit()) => rest = r,
            _ => break,
        }
    }
    if parts.is_empty() {
        return Cow::Borrowed(bound);
    }
    let core = format!(
        "{v}{}.{}.{}",
        parts[0],
        parts.get(1).unwrap_or(&"0"),
        parts.get(2).unwrap_or(&"0")
    );
    let coerced = match rest.chars().next() {
        None => core,
        Some('+' | '-') => format!("{core}{rest}"),
        Some(c) if c.is_ascii_alphanumeric() => format!("{core}-{rest}"),
        Some(_) => return Cow::Borrowed(bound),
    };
    if parse_version(&coerced).is_some() {
        Cow::Owned(coerced)
    } else {
        Cow::Borrowed(bound)
    }
}

/// Whether an OSV version boundary is an unknown ("not available") sentinel rather than a
/// concrete version. OSV's PYSEC import appends "-NA" when the exact affected boundary is
/// unknown; `semver` parses "2.5.0-NA" as a 2.5.0 prerelease, so a naive comparison would
/// over-match (e.g. torch 2.3.0 <= "2.5.0-NA" => affected) — yet OSV's own `/v1/query`
/// matcher does NOT place a version inside such a range. Treat it as no usable bound so the
/// engine stays conservative and consistent with OSV (verified 2026-06-18 via the ledger's
/// external accuracy audit).
pub fn is_unknown_bound(v: &str) -> bool {
    let t = v.trim();
    t.ends_with("-NA") || t.ends_with("-na")
}

/// Parse a version string, handling common non-semver formats.
pub fn parse_version(ver: &str) -> Option<Version> {
    let v = ver.trim().trim_start_matches('v');
    if v.is_empty() {
        return None;
    }

    if let Ok(version) = Version::parse(v) {
        return Some(version);
    }

    // Two-part version: "1.2" → "1.2.0"
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() == 2 {
        if let Ok(version) = Version::parse(&format!("{v}.0")) {
            return Some(version);
        }
    }

    // Leading zeros in the numeric core ("17.06.0-ce", a Docker-style bound a
    // Go advisory carries): semver rejects them, every ecosystem reads them
    // as the number. Left unparsed, the window was undecided and the copy
    // stayed an unconfirmed match forever.
    let split = v.find(['-', '+']).unwrap_or(v.len());
    let (core, rest) = v.split_at(split);
    let numbers: Vec<&str> = core.split('.').collect();
    if numbers.len() == 3
        && numbers
            .iter()
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        let trimmed: Vec<&str> = numbers
            .iter()
            .map(|n| match n.trim_start_matches('0') {
                "" => "0",
                t => t,
            })
            .collect();
        return Version::parse(&format!("{}{rest}", trimmed.join("."))).ok();
    }

    None
}

#[cfg(test)]
#[path = "range_bound_tests.rs"]
mod bound_tests;
