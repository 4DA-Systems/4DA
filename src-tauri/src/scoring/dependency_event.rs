// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency EVENT claim — is this item about something happening TO one of
//! the user's dependencies (a release, a breaking change, a vulnerability),
//! or merely an article that uses the technology?
//!
//! `ScoreBreakdown.strongly_grounded` answers "does the item demonstrably
//! name a package the user depends on?". That is the right input for the
//! SCORE: a React tutorial is topically relevant to a React developer and the
//! dependency axis contributes to its relevance. It is the wrong input for the
//! CLAIM the Signal list makes with it — the "Affects You" pool and the
//! "Names your dependency X" explanation line. Live 2026-10-04: "Progressive
//! Hydration in React" and "The Native TypeScript Compiler Cut Our
//! Typecheck…" sat in Affects You labelled "Names your dependency react /
//! typescript"; the pool measured 59% useful, and requiring a dependency event
//! measured 77–87% with zero useful items lost.
//!
//! A previous attempt put the event requirement INSIDE grounding
//! (`dependencies::is_name_corroborated`), which also removed the dependency
//! axis's contribution to the score; the real-embedding calibration ratchet
//! rejected it ("React 20 ships with native signals…" fell 0.3–0.8 → 0.117).
//! So the event is a SEPARATE claim flag: it is computed after scoring and
//! feeds no score, rank, verdict, necessity or priority — display and claim
//! only (AD-034: no `PIPELINE_VERSION` bump; the in-memory results that the
//! Signal list reads are recomputed every cycle).

#![deny(clippy::string_slice)]

use super::dependencies::{
    is_strong_grounding_match, markers_in_window, normalize_package_name, package_name_positions,
    version_literal_at_start, DepMatch, MarkerFit,
};
use super::release_grade::ReleaseClass;

/// Everything the event verdict reads — all of it already computed by the
/// scorer for the same item.
pub(crate) struct EventInputs<'a> {
    pub source_type: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    /// The item is a registry advisory row (osv / cve).
    pub registry_advisory: bool,
    /// The OSV matcher placed an installed copy inside the advisory's range.
    pub security_confirmed: bool,
    /// `SourceRelevance.applicability` as the scorer decided it.
    pub applicability: Option<&'a str>,
    /// The grounding verdict came from the registry-subject route: the row
    /// IS a release of the user's own dependency.
    pub via_registry_subject: bool,
    /// The release grade's class, when the row was graded.
    pub release_class: Option<ReleaseClass>,
    /// Every project already runs this release (or only the projects that
    /// build the package carry it) — the v33/v37 "not news" rule.
    pub already_installed_release: bool,
    /// The canonical text-route grounding verdict (`GroundingVerdict.strong`).
    pub strongly_grounded: bool,
    /// The raw dependency matches; only strong grounding edges are consulted.
    pub deps: &'a [DepMatch],
}

/// The dependency-event claim for one scored item.
///
/// - **Registry advisory** (osv / cve): an event exactly when the scorer
///   decided the user is (likely) affected — the same applicability the
///   evidence pool already trusts.
/// - **Registry release** (crates.io / npm / PyPI / Go) of the user's own
///   dependency: an event when the grade says it is news — a breaking
///   upgrade, a new minor, or a yanked pin. A patch or a prerelease row is
///   not (it is gated below the line for the same reason), nor is a release
///   every project already runs. An UNGRADED release of the user's dependency
///   (no parseable pins — the grade cannot tell) keeps the claim: a registry
///   row is a release of its subject by construction, and the grade, not the
///   claim, is what is missing.
/// - **Editorial** (everything else but research papers): strongly grounded
///   AND dependency-event evidence at a full-name occurrence of a grounding
///   dependency ([`has_dependency_event_evidence`]); when the title names any
///   grounding dependency, only the title decides.
pub(crate) fn is_dependency_event(inp: &EventInputs<'_>) -> bool {
    if inp.registry_advisory {
        return inp.security_confirmed
            || matches!(inp.applicability, Some("affected" | "likely_affected"));
    }
    if crate::dep_linker::is_registry_source(inp.source_type) && inp.via_registry_subject {
        if inp.already_installed_release {
            return false;
        }
        return match inp.release_class {
            Some(ReleaseClass::Breaking | ReleaseClass::Minor | ReleaseClass::Yanked) => true,
            Some(ReleaseClass::Patch | ReleaseClass::Prerelease) => false,
            None => true,
        };
    }
    // A research paper is not news about a dependency: "A Function-level
    // Dataset of Vulnerable and Fixed Source Code in JavaScript and
    // TypeScript" (arXiv, live 2026-10-04) names the language as its corpus.
    if !inp.strongly_grounded || matches!(inp.source_type, "arxiv" | "papers_with_code") {
        return false;
    }
    let evidence: Vec<EventEvidence> = inp
        .deps
        .iter()
        .filter(|d| is_strong_grounding_match(d))
        .map(|d| {
            let written = d.raw_name.as_deref().unwrap_or(&d.package_name);
            event_evidence(
                inp.title,
                inp.content,
                written,
                &normalize_package_name(&d.package_name),
            )
        })
        .collect();
    // When the title names ANY grounding dependency, the item is about that
    // dependency and the title alone decides — for every dependency. Live
    // 2026-10-04: "The Native TypeScript Compiler Cut Our Typecheck…" (a
    // migration write-up) borrowed "native 7.0 ships no importable compiler
    // api" from its body through `typescript-eslint`, a second dependency the
    // title never names.
    if evidence.iter().any(|e| e.named_in_title) {
        evidence.iter().any(|e| e.title_event)
    } else {
        evidence.iter().any(|e| e.body_event)
    }
}

/// What the text says about one dependency.
#[derive(Debug, Clone, Copy, Default)]
struct EventEvidence {
    /// The title names the package (a full-name occurrence).
    named_in_title: bool,
    /// The title carries event evidence at the name.
    title_event: bool,
    /// A body occurrence carries event vocabulary beside it.
    body_event: bool,
}

/// Bytes either side of a TITLE name occurrence searched for event
/// vocabulary. A title is one sentence about one subject, but a long title can
/// carry two: "UUID v4 vs. UUID v7 vs. ULID in 2026: Database Index
/// Performance, B-Tree Fragmentation & When to Migrate" ends on "Migrate" ~90
/// bytes after the last "UUID", and the migration is not of the `uuid` crate.
/// Every positive headline measured keeps its event word within this span:
/// "Announcing Tauri 2.12", "OpenAI Node SDK v5 breaking changes", "openai
/// npm package compromised", "React 20 ships with native signals",
/// "TypeScript 7 Is Up to 10x Faster. Should You Upgrade Now?" (34 bytes).
const EVENT_TITLE_WINDOW: usize = 48;

/// Bytes either side of a BODY name occurrence (body hits decide only when the
/// title does not name the package at all).
const EVENT_BODY_WINDOW: usize = 60;

/// At most this many other words may share a title with "<name> <version>"
/// for the version literal alone to make it a release headline ("Tokio 1.45",
/// "tauri v2.12.0", "Vite 8.1 is out"). A longer title with a version but no
/// event word is a tutorial about that version — "React 19.3 ViewTransition:
/// Animate State Without Losing It", "React.js ~New feature in React 19.3~".
const HEADLINE_EXTRA_WORDS: usize = 2;

/// Release / change vocabulary: what makes a mention of a package a dependency
/// EVENT. Word-start stems unless the short form collides with prose.
const DEPENDENCY_EVENT_MARKERS: &[(&str, MarkerFit)] = &[
    ("releas", MarkerFit::Stem),
    ("announc", MarkerFit::Stem),
    ("changelog", MarkerFit::Stem),
    ("deprecat", MarkerFit::Stem),
    ("breaking", MarkerFit::Stem),
    ("migrat", MarkerFit::Stem),
    ("upgrad", MarkerFit::Stem),
    ("regression", MarkerFit::Stem),
    ("sunset", MarkerFit::Stem),
    ("end-of-life", MarkerFit::Stem),
    ("end of life", MarkerFit::Stem),
    ("yanked", MarkerFit::Token),
    ("eol", MarkerFit::Token),
    ("lts", MarkerFit::Token),
    ("rc", MarkerFit::Token),
    ("beta", MarkerFit::Token),
    ("alpha", MarkerFit::Token),
    ("drops support", MarkerFit::Stem),
];

/// Headline verbs: an event in a TITLE ("React 20 ships with native signals",
/// "Vite 8 is out"), ordinary prose in a body ("everything reachable from a
/// 'use client' module ships to the browser, plus the react runtime" — live
/// 2026-10-04, a bundle-size write-up).
const HEADLINE_EVENT_MARKERS: &[(&str, MarkerFit)] = &[
    ("ships", MarkerFit::Token),
    ("shipped", MarkerFit::Token),
    ("lands", MarkerFit::Token),
    ("landed", MarkerFit::Token),
    ("is out", MarkerFit::Token),
    ("now available", MarkerFit::Token),
    ("what's new", MarkerFit::Token),
    ("what\u{2019}s new", MarkerFit::Token),
];

/// Security vocabulary that makes a package mention a vulnerability EVENT.
/// The scoring-side `SECURITY_CONTEXT_MARKERS` keeps bare "patch" (HTTP PATCH
/// requests corroborated a TypeScript utility-types tutorial, live
/// 2026-10-02); the claim takes only the release senses of it.
const SECURITY_EVENT_MARKERS: &[(&str, MarkerFit)] = &[
    ("cve-", MarkerFit::Stem),
    ("rustsec-", MarkerFit::Stem),
    ("ghsa-", MarkerFit::Stem),
    ("osv-", MarkerFit::Stem),
    ("vulnerab", MarkerFit::Stem),
    ("advisor", MarkerFit::Stem),
    ("security", MarkerFit::Stem),
    ("exploit", MarkerFit::Stem),
    ("malware", MarkerFit::Stem),
    ("malicious", MarkerFit::Stem),
    ("supply chain", MarkerFit::Stem),
    ("supply-chain", MarkerFit::Stem),
    // Not the "compromis" stem: "replacing serde means making some painful
    // compromises" grounded a serialization essay (live 2026-10-04).
    ("compromised", MarkerFit::Token),
    ("backdoor", MarkerFit::Stem),
    ("hijack", MarkerFit::Stem),
    ("typosquat", MarkerFit::Stem),
    ("0-day", MarkerFit::Stem),
    ("zero-day", MarkerFit::Stem),
    ("rce", MarkerFit::Token),
    ("patched", MarkerFit::Token),
    ("patches", MarkerFit::Token),
    ("patch release", MarkerFit::Stem),
];

/// Release/change or security vocabulary in `text[start..end]`; headline
/// verbs only when `headline` (a title window).
fn event_words_in(text: &str, start: usize, end: usize, headline: bool) -> bool {
    markers_in_window(text, start, end, DEPENDENCY_EVENT_MARKERS)
        || markers_in_window(text, start, end, SECURITY_EVENT_MARKERS)
        || (headline && markers_in_window(text, start, end, HEADLINE_EVENT_MARKERS))
}

/// Is the item about the package AS A DEPENDENCY — a release, an API change,
/// a migration, a vulnerability — rather than any article that uses it?
///
/// When the TITLE names the package, the title alone decides: an event word
/// within [`EVENT_TITLE_WINDOW`] bytes of the name, or a version literal right
/// after the name in a bare release headline ([`HEADLINE_EXTRA_WORDS`]). A
/// version literal in a longer title needs the event word beside the name
/// too: "UUID v4 vs. UUID v7 vs. ULID in 2026: … When to Migrate" is a guide
/// to UUID versions, not a `uuid` crate migration. A tutorial whose title
/// names React is about React whatever its body says about React's release
/// history.
///
/// When only the BODY names it, an event word within [`EVENT_BODY_WINDOW`]
/// bytes of a body occurrence ("…the axios maintainers published a security
/// advisory…"). A bare body version literal ("we use react 18.2") is not an
/// event. (Across several grounding dependencies, a title naming any of them
/// decides for all — see [`is_dependency_event`], which production calls; this
/// single-dependency view exists for the title-by-title tests.)
#[cfg(test)]
pub(crate) fn has_dependency_event_evidence(
    title: &str,
    content: &str,
    package_name: &str,
    normalized_name: &str,
) -> bool {
    let e = event_evidence(title, content, package_name, normalized_name);
    if e.named_in_title {
        e.title_event
    } else {
        e.body_event
    }
}

fn event_evidence(
    title: &str,
    content: &str,
    package_name: &str,
    normalized_name: &str,
) -> EventEvidence {
    let title_lower = title.to_lowercase();
    let title_len = title_lower.len();
    let text_lower = format!("{title_lower} {}", content.to_lowercase());
    let positions = package_name_positions(&text_lower, package_name, normalized_name);
    let (in_title, in_body): (Vec<_>, Vec<_>) =
        positions.into_iter().partition(|&(pos, _)| pos < title_len);

    let title_event = in_title.iter().any(|&(pos, len)| {
        let end = pos + len;
        event_words_in(
            &title_lower,
            pos.saturating_sub(EVENT_TITLE_WINDOW),
            (end + EVENT_TITLE_WINDOW).min(title_len),
            true,
        ) || (title_lower.get(end..).is_some_and(version_literal_at_start)
            && is_release_headline(&title_lower, normalized_name))
    });
    let body_event = in_body.iter().any(|&(pos, len)| {
        event_words_in(
            &text_lower,
            pos.saturating_sub(EVENT_BODY_WINDOW).max(title_len),
            pos + len + EVENT_BODY_WINDOW,
            false,
        )
    });
    EventEvidence {
        named_in_title: !in_title.is_empty(),
        title_event,
        body_event,
    }
}

/// A title that is little more than "<name> <version>": every word other than
/// the name and version-shaped tokens counts, and at most
/// [`HEADLINE_EXTRA_WORDS`] may remain.
fn is_release_headline(title_lower: &str, normalized_name: &str) -> bool {
    let extra = title_lower
        .split(|c: char| {
            c.is_whitespace() || matches!(c, ':' | '~' | '|' | '\u{2014}' | '\u{2013}')
        })
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.'))
        .map(|w| w.trim_end_matches('.'))
        .filter(|w| !w.is_empty())
        .filter(|w| {
            let bare = w.strip_suffix(".js").unwrap_or(w);
            let is_name = bare == normalized_name || w.replace('_', "-") == normalized_name;
            let is_version = w
                .trim_start_matches(['v', 'V'])
                .starts_with(|c: char| c.is_ascii_digit());
            !is_name && !is_version
        })
        .count();
    extra <= HEADLINE_EXTRA_WORDS
}

#[cfg(test)]
#[path = "dependency_event_tests.rs"]
mod tests;
