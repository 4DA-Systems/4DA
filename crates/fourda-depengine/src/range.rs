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
    let read = range_spans(ecosystem, range);
    let mut undecided = read.unread;
    for span in &read.spans {
        match span.contains(user) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => undecided = true,
        }
    }
    (!undecided).then_some(false)
}

/// One affected span of a SEMVER/ECOSYSTEM range: from `lower` up to `end`
/// (`fixed` exclusive, `last_affected` inclusive), or open-ended.
pub(crate) struct Span {
    pub(crate) lower: Lower,
    /// `(bound, as written, is_fixed)`; `None` for an open-ended span.
    pub(crate) end: Option<(OrderedVersion, String, bool)>,
}

impl Span {
    /// Whether `user` lies in the span; `None` when an edge cannot be
    /// compared with it.
    pub(crate) fn contains(&self, user: &OrderedVersion) -> Option<bool> {
        let upper = self.end.as_ref().map(|(version, _, fixed)| Upper {
            version,
            inclusive: !fixed,
        });
        in_window(user, &self.lower, upper)
    }
}

/// The spans one range reads as, and whether a window of it could not be read
/// (an unreadable bound): that window decides nothing, so a version no
/// readable span holds is undecided, never "not affected".
pub(crate) struct RangeSpans {
    pub(crate) spans: Vec<Span>,
    pub(crate) unread: bool,
}

/// One range event, read.
enum Event {
    /// `introduced`.
    Open(Lower),
    /// `fixed` (`true`) or `last_affected`, with the bound as written.
    Close(OrderedVersion, String, bool),
}

/// One event as written: `introduced` and the closing bound of one event
/// object, in that order (a hand-written object may carry both).
fn raw_events(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<(&str, Option<bool>)> {
    let mut out = Vec::new();
    if let Some(intro) = obj.get("introduced").and_then(|v| v.as_str()) {
        out.push((intro, None));
    }
    let close = obj
        .get("fixed")
        .map(|v| (v, true))
        .or_else(|| obj.get("last_affected").map(|v| (v, false)));
    if let Some((bound, fixed)) = close {
        out.push((bound.as_str().unwrap_or(""), Some(fixed)));
    }
    out
}

/// THE reading of one SEMVER/ECOSYSTEM range, for the matcher and the
/// fix-target walk alike: the OSV schema's evaluation. Its events sorted by
/// version (`introduced: "0"` first, equal versions in listed order), a
/// version is affected from an `introduced` at or below it until a `fixed` at
/// or below it or a `last_affected` below it. So an `introduced` inside an
/// open span changes nothing, and a closing event with no span open closes
/// nothing (api.osv.dev 2026-11-19: lunasvg 3.0.1 is clear of PYSEC-2025-128,
/// `introduced 0, last_affected 3.0.0, last_affected 3.1.0`).
///
/// Paired in listed order instead, npm derby's GHSA-82jv-9wjw-pqh6 range
/// `introduced 4.0.0-beta1, fixed 4.0.0-beta.11` (the fix sorts BELOW its
/// own start) was an empty window, and derby 4.0.0 and 4.2.3 read "not
/// affected" where OSV.dev reports both affected.
///
/// A range with a bound that cannot be read, or OSV's `-NA` ("boundary not
/// available") sentinel, cannot be put in order, so it is read in listed
/// order as before: the window an unreadable bound touches decides nothing
/// (`unread`), and a `-NA` bound closes its window without making any version
/// affected, which is OSV.dev's own answer for such records (PYSEC-2024-266,
/// `last_affected: "2.10.0-NA"`, is returned for no apache-airflow version
/// asked, 2026-11-19; see [`is_unknown_bound`]).
pub(crate) fn range_spans(ecosystem: &str, range: &Range) -> RangeSpans {
    let objects: Vec<&serde_json::Map<String, serde_json::Value>> = range
        .events
        .iter()
        .flatten()
        .filter_map(|e| e.as_object())
        .collect();
    let mut events = Vec::new();
    let mut readable = true;
    for (raw, close) in objects.iter().flat_map(|o| raw_events(o)) {
        let event = match close {
            None => Lower::parse_in(&coerce_bound(raw, ecosystem), ecosystem).map(Event::Open),
            Some(_) if is_unknown_bound(raw) => None,
            Some(fixed) => OrderedVersion::parse_in(&coerce_bound(raw, ecosystem), ecosystem)
                .map(|v| Event::Close(v, raw.trim().to_string(), fixed)),
        };
        match event {
            Some(e) => events.push(e),
            None => readable = false,
        }
    }
    if readable && sort_events(&mut events) {
        return RangeSpans {
            spans: sorted_spans(events),
            unread: false,
        };
    }
    listed_spans(ecosystem, &objects)
}

/// An event's version; `None` for `introduced: "0"`, below every version.
fn event_version(event: &Event) -> Option<&OrderedVersion> {
    match event {
        Event::Open(Lower::Unbounded) => None,
        Event::Open(Lower::At(v)) | Event::Close(v, _, _) => Some(v),
    }
}

/// The order of two events' versions.
fn event_order(a: &Event, b: &Event) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    match (event_version(a), event_version(b)) {
        (None, None) => Some(Ordering::Equal),
        (None, Some(_)) => Some(Ordering::Less),
        (Some(_), None) => Some(Ordering::Greater),
        (Some(x), Some(y)) => x.compare(y),
    }
}

/// Sorts `events` by version, keeping equal versions in listed order (an
/// insertion sort: ranges hold a handful of events). `false` when a pair it
/// had to order cannot be ordered.
fn sort_events(events: &mut [Event]) -> bool {
    for i in 1..events.len() {
        let mut j = i;
        while j > 0 {
            match event_order(&events[j - 1], &events[j]) {
                Some(std::cmp::Ordering::Greater) => {
                    events.swap(j - 1, j);
                    j -= 1;
                }
                Some(_) => break,
                None => return false,
            }
        }
    }
    true
}

/// Sorted events as spans: each `introduced` with nothing open opens one, the
/// next closing event ends it, and a closing event with nothing open is no-op.
fn sorted_spans(events: Vec<Event>) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut open: Option<Lower> = None;
    for event in events {
        match event {
            Event::Open(lower) => {
                if open.is_none() {
                    open = Some(lower);
                }
            }
            Event::Close(version, raw, fixed) => {
                if let Some(lower) = open.take() {
                    spans.push(Span {
                        lower,
                        end: Some((version, raw, fixed)),
                    });
                }
            }
        }
    }
    if let Some(lower) = open {
        spans.push(Span { lower, end: None });
    }
    spans
}

/// A range that cannot be sorted, read in listed order: each `introduced`
/// starts a window that the next closing event ends. A window whose bound
/// cannot be read is left out and marks the range `unread`; one a `-NA`
/// bound ends is left out (it holds no version, see [`range_spans`]).
fn listed_spans(
    ecosystem: &str,
    objects: &[&serde_json::Map<String, serde_json::Value>],
) -> RangeSpans {
    let mut spans = Vec::new();
    let mut unread = false;
    let mut lower: Option<Lower> = None;
    for (raw, close) in objects.iter().flat_map(|o| raw_events(o)) {
        let Some(fixed) = close else {
            lower = Lower::parse_in(&coerce_bound(raw, ecosystem), ecosystem);
            unread |= lower.is_none();
            continue;
        };
        let Some(low) = lower.take() else {
            continue;
        };
        if is_unknown_bound(raw) {
            continue;
        }
        match OrderedVersion::parse_in(&coerce_bound(raw, ecosystem), ecosystem) {
            Some(version) => spans.push(Span {
                lower: low,
                end: Some((version, raw.trim().to_string(), fixed)),
            }),
            None => unread = true,
        }
    }
    if let Some(lower) = lower {
        spans.push(Span { lower, end: None });
    }
    RangeSpans { spans, unread }
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

#[cfg(test)]
#[path = "range_order_tests.rs"]
mod order_tests;
