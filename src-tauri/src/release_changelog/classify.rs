// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Changelog entry classification and sanitisation — a port of the MCP
//! server's `changelog-classify.ts` (github.com/4DA-Systems/4da-mcp-server),
//! whose breaking-change rules measured 77% -> 87% held-out precision against
//! a three-rater panel (2026-10-02/03). The rules are kept as they are there;
//! the fixtures in `classify_tests.rs` are that repository's, so a divergence
//! shows up as a failing test, not as a quieter feed.
//!
//! A changelog is third-party text that ends up on the user's screen, so every
//! entry is cleaned first: ASCII control characters and the zero-width /
//! bidi-override code points that make text render differently from how it
//! reads are stripped, whitespace is collapsed, and each entry is capped at 400
//! characters.
//!
//! Classification is keyword-based on purpose: changelogs follow no schema, and
//! a wrong guess is cheap because every entry carries its own text. An explicit
//! section heading ("### Breaking Changes", "### Removed", "### Security")
//! outranks keywords in the line itself.
//!
//! The MCP server's kinds are breaking / deprecation / security / change. The
//! release card also counts features and fixes, so a "change" is refined into
//! Feature / Fix / Other from its heading and its leading verb
//! ([`refine_change`]); the breaking, security and deprecation calls are the
//! MCP rules unchanged.
//!
//! The `regex` crate has no look-around, so the three look-around patterns
//! (`BREAKING` not followed by `-file`, negated "breaking change", `-breaking`)
//! are a plain match plus an explicit check of the text around it.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Longest entry kept, in characters (the MCP server's `MAX_ENTRY_CHARS`).
pub(crate) const MAX_ENTRY_CHARS: usize = 400;

/// What one changelog entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChangeKind {
    Breaking,
    Security,
    Deprecation,
    Feature,
    Fix,
    Other,
}

impl ChangeKind {
    /// The MCP server's four-way kind, for parity tests.
    #[cfg(test)]
    pub(crate) fn mcp_kind(self) -> &'static str {
        match self {
            Self::Breaking => "breaking",
            Self::Security => "security",
            Self::Deprecation => "deprecation",
            Self::Feature | Self::Fix | Self::Other => "change",
        }
    }
}

/// The MCP server's kind before Feature / Fix refinement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Breaking,
    Deprecation,
    Security,
    Change,
}

/// What a sub-heading or label-only bullet says about the entries under it.
/// `Removal` (a removal list) is breaking except for internal clean-up and
/// deprecations filed there; `Additive` (additions and fixes) is breaking only
/// on an explicit marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryContext {
    Kind(EntryKind),
    Removal,
    Additive,
}

/// Compile a literal pattern. Every pattern in this lane is a compile-time
/// literal exercised by its tests, so a failure is a bug caught there.
pub(crate) fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("release_changelog: literal regex compiles")
}

static CONTROL: LazyLock<Regex> = LazyLock::new(|| re(r"[\x00-\x09\x0B-\x1F\x7F]"));
static INVISIBLE: LazyLock<Regex> =
    LazyLock::new(|| re("[\u{200B}-\u{200F}\u{202A}-\u{202E}\u{2066}-\u{2069}\u{FEFF}]"));
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| re(r"\s+"));

/// Strip control and invisible/bidi characters, collapse whitespace, cap length.
pub(crate) fn sanitize_entry(text: &str, cap: usize) -> String {
    let no_invisible = INVISIBLE.replace_all(text, "");
    let no_control = CONTROL.replace_all(&no_invisible, " ");
    let clean = WHITESPACE.replace_all(&no_control, " ");
    let clean = clean.trim();
    if clean.chars().count() > cap {
        let mut out: String = clean.chars().take(cap.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    } else {
        clean.to_string()
    }
}

/// `BREAKING` (case-sensitive) — checked separately for "not followed by
/// `[-_.]\w`", so `BREAKING-CHANGES.md` is not a marker.
static BREAKING_WORD: LazyLock<Regex> = LazyLock::new(|| re(r"\bBREAKING\b"));
/// The rest of the MCP `EXPLICIT_BREAKING_MARKER` alternation (case-sensitive).
static MARKER_REST: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"⚠|^\s*\*{0,2}\w+(?:\([^)]*\))?!:|^\s*\*{0,2}[Bb]reaking\*{0,2}\s*(?::|[-–—]\s)|^\s*\[(?:[Bb]reaking|[Rr]emoved)\]",
    )
});

pub(crate) fn explicit_breaking_marker(text: &str) -> bool {
    if MARKER_REST.is_match(text) {
        return true;
    }
    BREAKING_WORD.find_iter(text).any(|m| {
        let mut rest = text[m.end()..].chars();
        let followed_by_file = matches!(rest.next(), Some('-' | '_' | '.'))
            && rest.next().is_some_and(|c| c.is_alphanumeric() || c == '_');
        !followed_by_file
    })
}

static META_BREAKING: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b(?:list|lists|see|found|documented|described|summary|guide|doc|docs|documentation|label|labels|check|technically|despite|mentioning)\b[^.]{0,40}\bbreaking[- ]changes?\b|\bbreaking[- ]changes?\b[^.]{0,30}\b(?:can be found|are (?:listed|documented|described)|described|label|labels|doc|docs|documentation)\b|BREAKING[-_]CHANGES",
    )
});
static FUTURE_INTENT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b(?:reserve the right|will(?: be)?|may(?: be)?|might|plan(?:s|ning)? to|intend(?:s)? to|in (?:a|the) future|eventually)\b[^.]{0,40}\b(?:drop|dropp|remov|renam|deprecat)",
    )
});
static LEADING_FIX: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)^\W*(?:\*\*[^*]{1,40}\*\*:?\s*)?(?:fix|fixed|fixes|fixing)\b"));
/// "breaking change" in prose; the negation / compound / "release" checks
/// are in [`explicit_breaking_prose`].
static BREAKING_PROSE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\bbreaking[- ]changes?\b"));
static PROSE_NEGATION: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)(?:\bnot (?:a )?|\bnon[- ]|\bno |-)$"));

fn explicit_breaking_prose(text: &str) -> bool {
    BREAKING_PROSE.find_iter(text).any(|m| {
        let negated = PROSE_NEGATION.is_match(&text[..m.start()]);
        let names_release = text[m.end()..].to_lowercase().starts_with(" release");
        !negated && !names_release
    })
}

static YANK_NOTICE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\byanked\b"));
static NON_BREAKING_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^\s*\*{0,2}(?:fixed|fix|fixes|added|add|feat|features?|docs?|perf|chore|internal|improved|tests?|ci|build|refactor|style)(?:\([^)]*\))?\*{0,2}:\*{0,2}\s",
    )
});
static SECURITY: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)\bsecurity\b|\bCVE-\d{4}-\d+|\bGHSA-[\w-]+|\bRUSTSEC-\d{4}-\d+|vulnerab")
});
static REMOVAL_HEURISTIC: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "(?i){}",
        [
            r"\bremoved\b",
            r"\bremoves\b",
            r"\b(?:has|have|was|were) been dropped\b",
            r"\brenamed?\b",
            r"^\W*(?:remove|delete)\b(?:\s+[\w-]+){0,3}\s*\(?`",
            r"^\W*(?:remove|delete)\b(?:\s+[\w-]+){0,2}\s+[A-Za-z_]\w*(?:#|::|\.)[A-Za-z_]",
        ]
        .join("|")
    ))
});
static BREAKING_HEURISTIC: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        "(?i){}",
        [
            r"\bno longer (?:accepts?|returns?|supports?|exports?|exported|exposed|re-?exports?|available|provides?|allow(?:s|ed)?|implements?|public|includes?|ships?|compiles?|works? with)\b",
            r"\bcan no longer\b",
            r"\bnow (?:also )?returns?\b",
            r"\b(?:now )?marked (?:as )?`?#\[non_exhaustive\]",
            r"\bdrop(?:ped|s)? (?:support|compatibility)\b",
            r"\bdropped\b.*\b(?:support|compatibility)\b",
            r"\b(?:bump|bumped|raise|raised|increase|increased|update|updated|require|requires|now)\b.{0,60}\b(?:MSRV|minimum supported rust version|rust-version)\b.{0,30}\d+\.\d+",
            r"\b(?:MSRV|minimum supported rust version)\b.{0,60}?\b(?:is now|to|bumped to|raised to|increased to)\s+(?:rust\s+|rustc\s+)?v?\d",
            r"\b(?:MSRV|minimum supported rust version)\b.{0,60}\d+\.\d+.{0,20}\bor later\b",
            r"\bnow (?:requires?|required|returns?|takes?|accepts? only)\b",
            r"\b(?:function|method|type|the) signatures?\b",
            r"\bsignatures? (?:of|has|have|changed)\b",
            r"\b(?:changed?|new|different)\b[^.]{0,30}\b(?:return|argument|parameter) types?\b",
            r"\b(?:return|argument|parameter) types? (?:have |has )?(?:changed|change)\b",
            r"\btypes have changed\b",
            r"\b(?:implements?|returns?|takes?|accepts?|requires?|yields?|expects?)\b[^.]{0,40}`[^`]+`\s+instead of\s+`[^`]+`",
            r"\(instead of `",
            r"\bnow (?:receives?|uses?)\b[^.]{0,30}`",
            r"\bmoved (?:in)?to (?:the |a )?(?:new |separate )?`?[\w-]+`? (?:crate|package|module)\b",
            r"\bto keep the (?:old|previous|former) behaviou?r\b",
            r"\bincompatib",
        ]
        .join("|")
    ))
});
static INTERNAL_CHANGE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b(?:unused|unneeded|unnecessary|internal(?:ly)?|dead code|tests?|ci|lint(?:ing)?|typos?|comments?|docs?|documentation|readme|redundant|duplicated?|webpack|dev-?dependenc(?:y|ies)|dependency|warnings?|examples?|benchmarks?|release process|build process|tooling|usage|incorrect|wrong|erroneous|stray|spurious)\b",
    )
});
static DEPRECATION: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)deprecat"));
static LEADING_DEPRECATION: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)^\W*(?:\*\*[^*]{1,40}\*\*:?\s*)?(?:deprecated?|deprecates|deprecating)\b")
});
static RENAME: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\brenam"));
static REMOV: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\bremov"));

/// Kind of a single changelog line by its own wording (MCP `classifyText`).
pub(crate) fn classify_text(text: &str) -> EntryKind {
    if META_BREAKING.is_match(text) {
        return if SECURITY.is_match(text) {
            EntryKind::Security
        } else {
            EntryKind::Change
        };
    }
    if explicit_breaking_marker(text) {
        return EntryKind::Breaking;
    }
    if NON_BREAKING_LABEL.is_match(text) {
        if SECURITY.is_match(text) {
            return EntryKind::Security;
        }
        return deprecation_or_change(text);
    }
    if YANK_NOTICE.is_match(text) {
        return EntryKind::Change;
    }
    if LEADING_DEPRECATION.is_match(text) {
        return EntryKind::Deprecation;
    }
    if RENAME.is_match(text) && DEPRECATION.is_match(text) && !REMOV.is_match(text) {
        return EntryKind::Deprecation;
    }
    if explicit_breaking_prose(text) {
        return EntryKind::Breaking;
    }
    if SECURITY.is_match(text) {
        return EntryKind::Security;
    }
    if FUTURE_INTENT.is_match(text) {
        return deprecation_or_change(text);
    }
    if BREAKING_HEURISTIC.is_match(text) {
        return EntryKind::Breaking;
    }
    if REMOVAL_HEURISTIC.is_match(text)
        && !INTERNAL_CHANGE.is_match(text)
        && !LEADING_FIX.is_match(text)
    {
        return EntryKind::Breaking;
    }
    deprecation_or_change(text)
}

fn deprecation_or_change(text: &str) -> EntryKind {
    if DEPRECATION.is_match(text) {
        EntryKind::Deprecation
    } else {
        EntryKind::Change
    }
}

static ADDITIVE_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^\W*(?:added|adds|new|new features?|features?|enhancements?|improvements?|fixed|fixes|bug ?fixes|performance(?: improvements)?|perf|optimi[sz]ations?|docs?|documentation|tests?|testing|internal|chores?|ci|build|refactor(?:ing)?|misc(?:ellaneous)?|other(?: changes)?)\W*$",
    )
});
static HEADING_BREAKING_WORD: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\bbreaking\b"));
static HEADING_BREAKING_REST: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)⚠|incompatib|migration|\bmajor changes?\b"));
static HEADING_REMOVAL: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\bremov(?:e|ed|als?)\b|\bapi changes?\b"));
static HEADING_SECURITY: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)security|vulnerab"));

/// Context implied by a heading or label, or `None` when it carries no signal
/// ("### Changed") — MCP `classifyHeading`.
pub(crate) fn classify_heading(heading: &str) -> Option<EntryContext> {
    let breaking_word = HEADING_BREAKING_WORD
        .find_iter(heading)
        .any(|m| !heading[..m.start()].ends_with('-'));
    if breaking_word || HEADING_BREAKING_REST.is_match(heading) {
        return Some(EntryContext::Kind(EntryKind::Breaking));
    }
    if HEADING_REMOVAL.is_match(heading) {
        return Some(EntryContext::Removal);
    }
    if HEADING_SECURITY.is_match(heading) {
        return Some(EntryContext::Kind(EntryKind::Security));
    }
    if DEPRECATION.is_match(heading) {
        return Some(EntryContext::Kind(EntryKind::Deprecation));
    }
    if ADDITIVE_HEADING.is_match(heading) {
        return Some(EntryContext::Additive);
    }
    None
}

/// Final MCP kind: an explicit heading or bullet parent wins; removal and
/// additive contexts refine the line's wording (MCP `classifyEntry`).
pub(crate) fn classify_entry(text: &str, context: Option<EntryContext>) -> EntryKind {
    match context {
        Some(EntryContext::Kind(EntryKind::Change)) | None => classify_text(text),
        Some(EntryContext::Kind(kind)) => kind,
        Some(EntryContext::Removal) => {
            if META_BREAKING.is_match(text) {
                EntryKind::Change
            } else if explicit_breaking_marker(text) {
                EntryKind::Breaking
            } else if DEPRECATION.is_match(text) && !REMOV.is_match(text) {
                EntryKind::Deprecation
            } else if INTERNAL_CHANGE.is_match(text) {
                EntryKind::Change
            } else {
                EntryKind::Breaking
            }
        }
        Some(EntryContext::Additive) => {
            if !META_BREAKING.is_match(text) && explicit_breaking_marker(text) {
                EntryKind::Breaking
            } else if SECURITY.is_match(text) {
                EntryKind::Security
            } else {
                deprecation_or_change(text)
            }
        }
    }
}

/// A fix section. Not changesets' "Patch Changes": that names the semver
/// level, and live `ai` counted 2,116 "fixes" under it, dependency bumps
/// and features included (2026-10-04) — its entries are split by their verb.
static FIX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b(?:fix|fixes|fixed|bug ?fix(?:es)?|bugs?)\b"));
static FEATURE_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^\W*(?:added|adds|new|new features?|features?|enhancements?|improvements?|highlights|minor changes)\W*$",
    )
});
/// What may precede the verb: punctuation, a commit reference (changesets'
/// `8dd86a9: `, covector's ``[`9b29b601`](…) ``) and a bold scope label.
const LEAD: &str = r"^\W*(?:[0-9a-f]{7,40}`?\]?(?:\([^)]*\))?:?\s*)?(?:\*\*[^*]{1,40}\*\*:?\s*)?";
static FIX_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"(?i){LEAD}(?:fix|fixed|fixes|fixing|resolved?|resolves|correct(?:ed|s)?|prevent(?:ed|s)?|avoid(?:ed|s)?)\b|{LEAD}fix(?:\([^)]*\))?!?:"
    ))
});
static FEATURE_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"(?i){LEAD}(?:add|added|adds|adding|new|introduce[ds]?|implement(?:ed|s)?|support(?:s|ed)?|allow(?:s|ed)?|expose[ds]?)\b|{LEAD}feat(?:\([^)]*\))?!?:"
    ))
});

/// The release card's kind: the MCP kind, with a plain "change" split into
/// Feature / Fix / Other by the heading it sits under, then by its leading
/// verb. Never promotes anything to breaking.
pub(crate) fn refine_change(kind: EntryKind, text: &str, heading: Option<&str>) -> ChangeKind {
    match kind {
        EntryKind::Breaking => ChangeKind::Breaking,
        EntryKind::Security => ChangeKind::Security,
        EntryKind::Deprecation => ChangeKind::Deprecation,
        EntryKind::Change => {
            if let Some(h) = heading {
                if FIX_HEADING.is_match(h) {
                    return ChangeKind::Fix;
                }
                if FEATURE_HEADING.is_match(h) {
                    return ChangeKind::Feature;
                }
            }
            if FIX_TEXT.is_match(text) {
                ChangeKind::Fix
            } else if FEATURE_TEXT.is_match(text) {
                ChangeKind::Feature
            } else {
                ChangeKind::Other
            }
        }
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
