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
pub fn check_version_affected(
    ecosystem: &str,
    user_version: Option<&str>,
    affected_ranges_json: &Option<String>,
) -> (bool, bool) {
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
            lower = Lower::parse_in(intro, ecosystem);
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
                let verdict = OrderedVersion::parse_in(bound, ecosystem).and_then(|version| {
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
