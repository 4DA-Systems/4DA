// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Changelog discovery and parsing — a port of the MCP server's
//! `changelog.ts`, with its fixtures in `parse_tests.rs`.
//!
//! Changelogs follow no schema, so the parser recognises the version-heading
//! styles actually found in registry archives rather than one spec:
//!   keep-a-changelog      `## [1.2.3] - 2024-01-01`
//!   conventional          `## [7.0.0](https://…/compare/…) (2025-06-24)`
//!   plain / prefixed      `## 1.2.3`, `# v1.2.3 (2024-01-01)`, `### Version 1.2.3`, `## fastembed 5.0.0`
//!   setext                `1.2.3` underlined with `===` / `---`
//!   History.md (express)  `1.2.3 / 2020-01-01`
//! A heading only opens a section when the rest of it looks like a trailer
//! (date, link, "yanked") — `## Upgrading from 1.x` or `#### Rust 1.70 support`
//! must stay sub-headings, or a migration guide would split a release in two.
//!
//! Entries are bullets, numbered items and loose paragraph lines; indented
//! continuation lines join the entry above. Fenced code blocks, HTML comments
//! and link reference definitions are skipped: they are examples and URLs, not
//! changes.

use std::cmp::Ordering;
use std::sync::LazyLock;

use regex::Regex;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::classify::{
    classify_entry, classify_heading, explicit_breaking_marker, re, refine_change, sanitize_entry,
    ChangeKind, EntryContext, EntryKind, MAX_ENTRY_CHARS,
};

/// One change line, cleaned and classified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangelogEntry {
    pub kind: ChangeKind,
    pub text: String,
    /// The heading, label or parent bullet the entry sits under ("Removed",
    /// "BREAKING: Functions that accept ..."). Without it "`rt::{Arbiter}`
    /// re-exports." under actix-web's "Removed" reads as harmless.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
}

/// One release's section of a changelog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangelogSection {
    pub version: String,
    pub date: Option<String>,
    pub entries: Vec<ChangelogEntry>,
}

const NAME_STEMS: &[&str] = &[
    "changelog",
    "changes",
    "history",
    "releases",
    "release-notes",
    "release_notes",
    "releasenotes",
    "news",
];
const NAME_EXTS: &[&str] = &["", ".md", ".markdown", ".txt", ".rst"];

/// True for a basename that names a changelog (case-insensitive).
pub(crate) fn is_changelog_name(basename: &str) -> bool {
    let lower = basename.to_lowercase();
    NAME_STEMS
        .iter()
        .any(|stem| NAME_EXTS.iter().any(|ext| lower == format!("{stem}{ext}")))
}

/// Rank of a changelog basename: CHANGELOG before the others, Markdown before
/// plain text. Lower is better.
fn name_rank(basename: &str) -> Option<usize> {
    if !is_changelog_name(basename) {
        return None;
    }
    let lower = basename.to_lowercase();
    let stem = NAME_STEMS.iter().position(|s| lower.starts_with(s))?;
    let rest = lower.get(NAME_STEMS[stem].len()..).unwrap_or("");
    let ext = NAME_EXTS.iter().position(|e| *e == rest)?;
    Some(stem * 10 + if ext == 1 || ext == 2 { 0 } else { ext + 1 })
}

/// The changelog among archive paths, or `None` when there is none.
pub(crate) fn find_changelog_file<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for path in paths {
        let base = path.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
        let Some(rank) = name_rank(base) else {
            continue;
        };
        if best.is_none_or(|(_, r)| rank < r) {
            best = Some((path, rank));
        }
    }
    best.map(|(p, _)| p)
}

const VERSION: &str = r"\d+\.\d+(?:\.\d+)?(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?";
const MONTH_DATE_SRC: &str = r"(?i)\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\.? \d{1,2}(?:st|nd|rd|th)?,? \d{4}\b|\b\d{1,2} (?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*,? \d{4}\b";

static ISO_DATE: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d{4})[-/.](\d{2})[-/.](\d{2})\b"));
static MONTH_DATE: LazyLock<Regex> = LazyLock::new(|| re(MONTH_DATE_SRC));
static HEADING: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"(?i)^\[?(?:(version|release)\s+|([A-Za-z@][\w@/.-]*)(?:\s+|@))?\[?v?({VERSION})\]?(.*)$"
    ))
});
static HTML_ANCHOR: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(?:<a\b[^>]*>(?:\s*</a>)?\s*)+"));
static FULL_TRIPLE: LazyLock<Regex> = LazyLock::new(|| re(r"^\d+\.\d+\.\d+"));
static TRAILER_LINK: LazyLock<Regex> = LazyLock::new(|| re(r"\]?\([^)]*\)"));
static TRAILER_BRACKET: LazyLock<Regex> = LazyLock::new(|| re(r"\[[^\]]*\]"));
static TRAILER_WORDS: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b(?:yanked|unreleased|released|latest|stable|on)\b"));
static TRAILER_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"[-–—/:,.*_#()\[\]<>|]"));
static DAY: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d{1,2})(?:st|nd|rd|th)?\b"));
static YEAR: LazyLock<Regex> = LazyLock::new(|| re(r"\d{4}"));

const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// ISO date from a heading trailer: `2024-01-01`, `2024/01/01`,
/// `August 1st, 2025`, `1 Aug 2025`.
fn extract_date(text: &str) -> Option<String> {
    if let Some(c) = ISO_DATE.captures(text) {
        return Some(format!("{}-{}-{}", &c[1], &c[2], &c[3]));
    }
    let named = MONTH_DATE.find(text)?.as_str();
    let lower = named.to_lowercase();
    let month = MONTHS.iter().position(|m| lower.contains(m)).map(|i| i + 1);
    let without_year = YEAR.replace(named, "");
    let day = DAY.captures(&without_year).map(|c| c[1].to_string());
    let year = YEAR.find(named).map(|m| m.as_str().to_string());
    match (month, day, year) {
        (Some(m), Some(d), Some(y)) => Some(format!("{y}-{m:02}-{d:0>2}")),
        _ => Some(named.to_string()),
    }
}

/// Version and date from a heading's text, or `None` when it is not a
/// release heading.
pub(crate) fn parse_version_heading(raw: &str) -> Option<(String, Option<String>)> {
    let trimmed = raw.trim();
    let text = trimmed.strip_prefix("**").unwrap_or(trimmed);
    let text = text.strip_suffix("**").unwrap_or(text);
    // An HTML anchor before the version (stripe's generated changelog:
    // `## <a id="23-0-0"></a>23.0.0 - 2026-09-30`) is a link target, not text.
    let text = HTML_ANCHOR.replace(text, "");
    let c = HEADING.captures(&text)?;
    let keyword = c.get(1);
    let name_prefix = c.get(2);
    let version = c.get(3)?.as_str();
    let rest = c.get(4).map_or("", |m| m.as_str());
    // A free-form name prefix needs a full x.y.z — "Rust 1.70" is prose.
    if name_prefix.is_some() && keyword.is_none() && !FULL_TRIPLE.is_match(version) {
        return None;
    }
    let trailer = TRAILER_LINK.replace_all(rest, " ");
    let trailer = TRAILER_BRACKET.replace_all(&trailer, " ");
    let trailer = ISO_DATE.replace(&trailer, " ");
    let trailer = MONTH_DATE.replace(&trailer, " ");
    let trailer = TRAILER_WORDS.replace_all(&trailer, " ");
    let trailer = TRAILER_PUNCT.replace_all(&trailer, " ");
    if trailer.split_whitespace().count() > 2 {
        return None;
    }
    Some((version.to_string(), extract_date(rest)))
}

static ATX: LazyLock<Regex> = LazyLock::new(|| re(r"^(#{1,6})\s+(.*?)\s*#*\s*$"));
static SETEXT_RULE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*(=+|-+)\s*$"));
static BULLET: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)(?:[-*+•]|\d+[.)])\s+(.*)$"));
static LINK_DEF: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*\[[^\]]+\]:\s*\S+"));
static FENCE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*(?:```|~~~)"));
static RULE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*(?:[-=*_]\s*){3,}$"));
static BOLD_LABEL: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*\*\*([^*]+)\*\*:?\s*$"));
static PLAIN_LABEL: LazyLock<Regex> =
    LazyLock::new(|| re(r"^\s{0,3}([A-Za-z⚠][\w /&,'()⚠\u{FE0F}-]{1,58}?)\s*:\s*$"));
static HTML_LAYOUT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^</?(?:details|summary|div|p|br|hr|img|picture|source|table|tr|td|th|thead|tbody|center|sup|sub)\b",
    )
});
static LABEL_BULLET: LazyLock<Regex> = LazyLock::new(|| re(r"^\W*([A-Za-z][\w -]{0,30}?)\W*:\W*$"));
static CATEGORY_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^\W*(added|changed|changes|deprecated|deprecations|removed|removals|fixed|fixes|bug ?fixes|security|features?|new features|enhancements?|improvements?|performance( improvements)?|breaking( changes?)?|⚠\u{FE0F}?\s*breaking( changes?)?|dependencies|dependency updates|documentation|docs|internal|misc(ellaneous)?|other( changes)?|chores?|refactor(ing)?|reverts?|build|tests?)\W*$",
    )
});
static HISTORY_LINE: LazyLock<Regex> =
    LazyLock::new(|| re(&format!(r"^v?({VERSION})\s+/\s+(.+)$")));
static VERSION_BULLET_START: LazyLock<Regex> = LazyLock::new(|| re(r"^\[?v?\d"));
static TWO_WORDS: LazyLock<Regex> = LazyLock::new(|| re(r"\s\w+\s\w+"));
static EMPHASIS: LazyLock<Regex> = LazyLock::new(|| re(r"\*\*|__|`"));
static TRAILING_COLON: LazyLock<Regex> = LazyLock::new(|| re(r":\s*$"));
static CONTINUATION: LazyLock<Regex> = LazyLock::new(|| re(r"^\s+\S"));

struct Pending {
    text: String,
    context: Option<EntryContext>,
    indent: Option<usize>,
    under: Option<String>,
    /// The sub-heading in force, for the Feature / Fix split.
    heading: Option<String>,
    /// The context came from a parent bullet; `own_context` is the heading's.
    lent: bool,
    own_context: Option<EntryContext>,
}

/// One child entry that took its kind from a parent bullet.
struct LentEntry {
    section: usize,
    entry: usize,
    /// The kind its own wording (under the heading alone) gives it.
    own: ChangeKind,
    /// It carries its own explicit breaking marker (⚠, BREAKING, `feat!:`).
    marked: bool,
}

#[derive(Default)]
struct ParseState {
    /// Children of the current parent bullet that took the parent's kind.
    lent: Vec<LentEntry>,
    sections: Vec<ChangelogSection>,
    /// Index into `sections` of the section being filled.
    current: Option<usize>,
    section_level: usize,
    heading_kind: Option<EntryContext>,
    parent_kind: Option<EntryContext>,
    parent_indent: Option<usize>,
    heading_text: Option<String>,
    parent_text: Option<String>,
    pending: Option<Pending>,
    /// Indent of the bullet the last pushed entry came from (`None` after a
    /// paragraph entry or a new heading).
    last_bullet_indent: Option<usize>,
}

impl ParseState {
    fn flush(&mut self) {
        let Some(p) = self.pending.take() else { return };
        let Some(idx) = self.current else { return };
        self.last_bullet_indent = p.indent;
        let text = sanitize_entry(&p.text, MAX_ENTRY_CHARS);
        // Layout markup is not a change: actix-web 4 wraps pre-release notes
        // in `<details> <summary>`, and both lines were counted as removals.
        if text.is_empty() || HTML_LAYOUT.is_match(&text) {
            return;
        }
        let mcp = classify_entry(&text, p.context);
        let kind = refine_change(mcp, &text, p.heading.as_deref());
        let Some(section) = self.sections.get_mut(idx) else {
            return;
        };
        if p.lent {
            let own = classify_entry(&text, p.own_context);
            self.lent.push(LentEntry {
                section: idx,
                entry: section.entries.len(),
                own: refine_change(own, &text, p.heading.as_deref()),
                marked: explicit_breaking_marker(&text),
            });
        }
        section.entries.push(ChangelogEntry {
            kind,
            text,
            under: p.under,
        });
    }

    /// Close the current parent bullet's group. A parent lends its kind to
    /// children that say nothing themselves (date-fns 3.0: "**BREAKING**:
    /// Functions that accept Interval ..." over one bullet per function). But
    /// when the author marks breaking children one by one, the marks are the
    /// signal and the rest are what their own words say: stripe 23.0.0's
    /// "⚠️ Update generated code" holds ~60 "Add support for ..." lines and a
    /// few "⚠️ Remove support for ...", and lending made all of them breaking
    /// (387 across 20.3.1 → 23.0.0, live 2026-10-04).
    fn settle_lent(&mut self) {
        let group = std::mem::take(&mut self.lent);
        if !group.iter().any(|e| e.marked) {
            return;
        }
        for e in group {
            if let Some(entry) = self
                .sections
                .get_mut(e.section)
                .and_then(|s| s.entries.get_mut(e.entry))
            {
                entry.kind = e.own;
            }
        }
    }

    fn open_section(&mut self, version: String, date: Option<String>, level: usize) {
        self.flush();
        self.settle_lent();
        self.section_level = level;
        self.sections.push(ChangelogSection {
            version,
            date,
            entries: Vec::new(),
        });
        self.current = Some(self.sections.len() - 1);
        self.set_heading(None, None);
    }

    /// A new sub-heading (or label) is in force; parent-bullet context resets.
    fn set_heading(&mut self, kind: Option<EntryContext>, text: Option<String>) {
        self.settle_lent();
        self.last_bullet_indent = None;
        self.heading_kind = kind;
        self.heading_text = text;
        self.parent_kind = None;
        self.parent_indent = None;
        self.parent_text = None;
    }
}

/// A heading or parent bullet as context text: emphasis and code marks
/// dropped, one line, at most 80 characters.
fn context_text(raw: &str) -> Option<String> {
    let no_marks = EMPHASIS.replace_all(raw, "");
    let no_colon = TRAILING_COLON.replace(&no_marks, "");
    let out = sanitize_entry(&no_colon, 80);
    (!out.is_empty()).then_some(out)
}

fn is_category_heading(raw: &str) -> bool {
    CATEGORY_HEADING.is_match(raw.trim())
}

/// A bullet line: its context comes from the heading, or from the outermost
/// bullet above it when nested.
fn add_bullet(state: &mut ParseState, indent: usize, text: &str) {
    state.flush();
    // The outermost bullet level of a list, wherever it is indented (express's
    // History.md lists sit at two spaces), lends context to bullets under it.
    let outer = state.parent_indent.is_none_or(|p| indent <= p);
    if outer {
        state.settle_lent();
        state.parent_indent = Some(indent);
        // A label-only bullet ("* remove:", "- Breaking:") heads its children.
        if let Some(label) = LABEL_BULLET.captures(text) {
            state.parent_kind = classify_heading(&label[1]).or(state.heading_kind);
            state.parent_text = context_text(&label[1]);
            return;
        }
        // A breaking bullet lends "breaking" to its sub-points (date-fns 3.0);
        // one ending in ":" lends any signalling kind.
        let own = classify_entry(text, state.heading_kind);
        let lends = own == EntryKind::Breaking
            || (TRAILING_COLON.is_match(text) && own != EntryKind::Change);
        state.parent_kind = lends.then_some(EntryContext::Kind(own));
        state.parent_text = context_text(text);
    }
    let (context, under) = if outer {
        (state.heading_kind, state.heading_text.clone())
    } else {
        (
            state.parent_kind.or(state.heading_kind),
            state
                .parent_text
                .clone()
                .or_else(|| state.heading_text.clone()),
        )
    };
    state.pending = Some(Pending {
        text: text.to_string(),
        context,
        indent: Some(indent),
        under,
        heading: state.heading_text.clone(),
        lent: !outer && state.parent_kind.is_some(),
        own_context: state.heading_kind,
    });
}

/// A paragraph indented under the bullet just above it, after a blank line,
/// describes that bullet: stripe writes "* ⚠️ Remove `ErrorType` export",
/// a blank line, then "  Remove the ErrorType interface ...", and the
/// paragraph was a second "breaking" entry for the same change (live
/// 2026-10-04). It is appended to the bullet's text (still capped), not
/// counted. Returns true when the line was consumed.
fn extend_last_bullet(state: &mut ParseState, line: &str) -> bool {
    let indent = line.len() - line.trim_start().len();
    let Some(bullet_indent) = state.last_bullet_indent else {
        return false;
    };
    if indent <= bullet_indent {
        return false;
    }
    let Some(entry) = state
        .current
        .and_then(|i| state.sections.get_mut(i))
        .and_then(|s| s.entries.last_mut())
    else {
        return false;
    };
    let joined = format!("{} \u{2014} {}", entry.text, line.trim());
    entry.text = sanitize_entry(&joined, MAX_ENTRY_CHARS);
    true
}

fn add_line(state: &mut ParseState, line: &str) {
    if let Some(b) = BULLET.captures(line) {
        let indent = b[1].len();
        add_bullet(state, indent, &b[2]);
        return;
    }
    if line.trim().is_empty() {
        state.flush();
        return;
    }
    if state.pending.is_none() && extend_last_bullet(state, line) {
        return;
    }
    if let Some(p) = state.pending.as_mut() {
        // An indented continuation, or a wrapped paragraph line.
        if CONTINUATION.is_match(line) || p.indent.is_none() {
            p.text.push(' ');
            p.text.push_str(line.trim());
            return;
        }
    }
    state.flush();
    state.pending = Some(Pending {
        text: line.to_string(),
        context: state.heading_kind,
        indent: None,
        under: state.heading_text.clone(),
        heading: state.heading_text.clone(),
        lent: false,
        own_context: state.heading_kind,
    });
}

/// Skip state for fences and multi-line HTML comments. Returns true when the
/// line is consumed.
fn skip_line(
    state: &mut ParseState,
    line: &str,
    in_fence: &mut bool,
    in_comment: &mut bool,
) -> bool {
    if FENCE.is_match(line) {
        state.flush();
        *in_fence = !*in_fence;
        return true;
    }
    if *in_fence {
        return true;
    }
    let trimmed = line.trim();
    if *in_comment {
        if trimmed.contains("-->") {
            *in_comment = false;
        }
        return true;
    }
    if let Some(after) = trimmed.strip_prefix("<!--") {
        if !after.contains("-->") {
            *in_comment = true;
        }
        return true;
    }
    LINK_DEF.is_match(line)
}

/// An ATX heading: a release opens a section; anything else is a sub-heading,
/// and a non-category heading at or above the release's level ends it.
fn handle_atx(state: &mut ParseState, level: usize, text: &str) {
    if let Some((version, date)) = parse_version_heading(text) {
        state.open_section(version, date, level);
        return;
    }
    state.flush();
    // "## Migration guide" after the last release is not part of it; a
    // category heading ("## Changed") at the release's own level still is
    // (date-fns 3.0.0 writes "## v3.0.0" then "## Changed").
    if state.current.is_some() && level <= state.section_level && !is_category_heading(text) {
        state.current = None;
    }
    state.set_heading(classify_heading(text), context_text(text));
}

/// A line that opens a release without an ATX heading: setext, History.md,
/// or a top-level version bullet. Returns how many lines it consumed.
fn release_line(state: &mut ParseState, line: &str, next: Option<&str>) -> usize {
    if let Some(next) = next {
        if SETEXT_RULE.is_match(next) && !line.trim().is_empty() && !BULLET.is_match(line) {
            if let Some((version, date)) = parse_version_heading(line) {
                let level = if next.trim().starts_with('=') { 1 } else { 2 };
                state.open_section(version, date, level);
                return 2;
            }
        }
    }
    // History.md without an underline: `1.2.3 / 2020-01-01` alone on a line.
    if let Some(h) = HISTORY_LINE.captures(line.trim()) {
        if let Some(date) = extract_date(&h[2]) {
            state.open_section(h[1].to_string(), Some(date), 2);
            return 1;
        }
    }
    0
}

/// A release written as a top-level bullet with its changes nested under it
/// (indexmap's RELEASES.md: "- 2.0.0" then "  - **MSRV**: ...").
fn version_bullet(state: &mut ParseState, line: &str) -> bool {
    let Some(b) = BULLET.captures(line) else {
        return false;
    };
    if !b[1].is_empty() || !VERSION_BULLET_START.is_match(&b[2]) {
        return false;
    }
    let Some((version, date)) = parse_version_heading(&b[2]) else {
        return false;
    };
    let no_iso = ISO_DATE.replace(&b[2], "");
    let bare = MONTH_DATE.replace(&no_iso, "");
    if TWO_WORDS.is_match(&bare) {
        return false;
    }
    state.open_section(version, date, 7);
    true
}

/// Parse a changelog into version sections, in document order (usually
/// newest first).
pub(crate) fn parse_changelog(text: &str) -> Vec<ChangelogSection> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut state = ParseState::default();
    let (mut in_fence, mut in_comment) = (false, false);
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if skip_line(&mut state, line, &mut in_fence, &mut in_comment) {
            continue;
        }
        if let Some(atx) = ATX.captures(line) {
            handle_atx(&mut state, atx[1].len(), &atx[2]);
            continue;
        }
        let consumed = release_line(&mut state, line, lines.get(i).copied());
        if consumed > 0 {
            i += consumed - 1;
            continue;
        }
        if RULE.is_match(line) {
            state.flush();
            continue;
        }
        if version_bullet(&mut state, line) {
            continue;
        }
        // A whole-line label, bold or plain ("**Breaking Changes**",
        // "BREAKING CHANGES:") is a sub-heading, not an entry.
        let label = BOLD_LABEL
            .captures(line)
            .or_else(|| PLAIN_LABEL.captures(line));
        if let (Some(label), Some(_)) = (label, state.current) {
            state.flush();
            state.set_heading(classify_heading(&label[1]), context_text(&label[1]));
            continue;
        }
        if state.current.is_some() {
            add_line(&mut state, line);
        }
    }
    state.flush();
    state.settle_lent();
    state.sections
}

/// A version for precedence comparison: a leading `v` dropped, `MAJOR.MINOR`
/// read as `MAJOR.MINOR.0`, anything else that is not semver unreadable.
pub(crate) fn precedence(version: &str) -> Option<Version> {
    let v = version.trim().trim_start_matches('v');
    if let Ok(parsed) = Version::parse(v) {
        return Some(parsed);
    }
    let mut parts = v.split('.');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if numeric(a) && numeric(b) {
        Version::parse(&format!("{a}.{b}.0")).ok()
    } else {
        None
    }
}

/// Semver precedence of two version strings; `None` if either is unreadable.
pub(crate) fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    Some(precedence(a)?.cmp_precedence(&precedence(b)?))
}

/// True when `version` lies in (from, to]; unreadable versions are out.
pub(crate) fn in_range(version: &str, from: &str, to: &str) -> bool {
    matches!(compare_versions(version, from), Some(Ordering::Greater))
        && matches!(
            compare_versions(version, to),
            Some(Ordering::Less | Ordering::Equal)
        )
}

/// Sections in (from, to], plus whether the changelog reaches both ends of
/// the range: its newest section is at or past `to`, and its oldest at or
/// before `first_in_range` (the earliest release the upgrade crosses). A
/// changelog that stopped being maintained two majors ago is reported as not
/// covering the range instead of presenting a partial history as the whole.
pub(crate) fn select_range<'a>(
    sections: &'a [ChangelogSection],
    from: &str,
    to: &str,
    first_in_range: &str,
) -> (Vec<&'a ChangelogSection>, bool) {
    let readable: Vec<&ChangelogSection> = sections
        .iter()
        .filter(|s| precedence(&s.version).is_some())
        .collect();
    let selected: Vec<&ChangelogSection> = readable
        .iter()
        .copied()
        .filter(|s| in_range(&s.version, from, to))
        .collect();
    let reaches_top = readable.iter().any(|s| {
        matches!(
            compare_versions(&s.version, to),
            Some(Ordering::Greater | Ordering::Equal)
        )
    });
    let reaches_bottom = readable.iter().any(|s| {
        matches!(
            compare_versions(&s.version, first_in_range),
            Some(Ordering::Less | Ordering::Equal)
        )
    });
    let covers = !selected.is_empty() && reaches_top && reaches_bottom;
    (selected, covers)
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
