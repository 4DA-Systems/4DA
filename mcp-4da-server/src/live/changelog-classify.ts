// SPDX-License-Identifier: Apache-2.0
/**
 * Changelog entry classification and sanitisation.
 *
 * A changelog is third-party text that ends up in an agent's context, so
 * every entry is cleaned before it leaves this module: ASCII control
 * characters and the zero-width / bidi-override code points that make text
 * render differently from how it reads (U+200B–U+200F, U+202A–U+202E,
 * U+2066–U+2069, U+FEFF) are stripped, whitespace is collapsed, and each
 * entry is capped at 400 characters so one runaway paragraph cannot crowd
 * out the rest of the report.
 *
 * Classification is keyword-based on purpose: changelogs follow no schema,
 * and a wrong guess is cheap here because every entry carries its text — the
 * kind decides ordering and what concise mode keeps, never whether an entry
 * exists. An explicit section heading ("### Breaking Changes", keep-a-
 * changelog's "### Removed" / "### Deprecated" / "### Security") outranks
 * keywords in the line itself.
 */

export type EntryKind = "breaking" | "deprecation" | "security" | "change";

export const MAX_ENTRY_CHARS = 400;

const CONTROL = /[\u0000-\u0009\u000B-\u001F\u007F]/g;
const INVISIBLE = /[​-‏‪-‮⁦-⁩﻿]/g;

/** Strip control and invisible/bidi characters, collapse whitespace, cap length. */
export function sanitizeEntry(text: string, cap: number = MAX_ENTRY_CHARS): string {
  const clean = text.replace(INVISIBLE, "").replace(CONTROL, " ").replace(/\s+/g, " ").trim();
  return clean.length > cap ? `${clean.slice(0, cap - 1)}…` : clean;
}

/**
 * Explicit markers: these win even over a security keyword in the same line.
 * Upper-case BREAKING and conventional-commit `feat!:` are case-sensitive
 * markers; the prose form must not be negated — tokio 1.44.0's entry
 * "We determined this change is not a breaking change" was counted as
 * breaking on the first live run.
 */
const EXPLICIT_BREAKING_MARKER = /\bBREAKING\b|⚠|^\s*\*{0,2}\w+(?:\([^)]*\))?!:|^\s*\*{0,2}[Bb]reaking\*{0,2}:/;
/**
 * "breaking change" in prose, except when negated, when it is a compound
 * ("reproducibility-breaking": rand 0.9's changed outputs, not its API), and
 * when it names another crate's release (regex 1.10: "`regex-syntax` breaking
 * change release"). Measured 2026-10-02 against a 3-rater panel.
 */
const EXPLICIT_BREAKING_PROSE = /(?<!\bnot (?:a )?|\bnon[- ]|\bno |-)\bbreaking[- ]changes?\b(?! release)/i;
/** A yank notice mentions breakage without being a change to the API ("Yanked ... due to unforeseen breaking change"). */
const YANK_NOTICE = /\byanked\b/i;
/**
 * Inline labels that say an entry is NOT a breaking change (axum's
 * `**fixed:** Removed the warning about breaking changes from README`,
 * conventional `fix:` / `docs:`). Such entries keep only their security and
 * deprecation signal; removal/rename keywords inside them are prose.
 */
const NON_BREAKING_LABEL =
  /^\s*\*{0,2}(?:fixed|fix|fixes|added|add|feat|features?|docs?|perf|chore|internal|improved|tests?|ci|build|refactor|style)(?:\([^)]*\))?\*{0,2}:\*{0,2}\s/i;
const SECURITY = /\bsecurity\b|\bCVE-\d{4}-\d+|\bGHSA-[\w-]+|\bRUSTSEC-\d{4}-\d+|vulnerab/i;
/**
 * Wording that changes the API a caller compiles or runs against. Each pattern
 * is narrower than the keyword it replaced: bare "no longer" also matched
 * "No longer panics when ..." (a fix), bare "MSRV" matched "Add
 * `Cargo.lock.msrv` file", and only past-tense "removed"/"renamed" were seen,
 * so rand 0.9's "Rename fn `thread_rng()` to `rng()`" was missed.
 *
 * Removal and rename wording is breaking unless what went is internal (see
 * INTERNAL_CHANGE); the other API wording stands whatever else the line mentions
 * ("MSRV is now 1.70 because of a dependency update").
 */
const REMOVAL_HEURISTIC = new RegExp(
  [
    "\\bremoved\\b",
    "\\brenamed?\\b",
    // The removed thing is the object of "remove": a code name within a few words
    // ("Remove first parameter (`rng`) of ..."), not a backtick somewhere later
    // ("Remove a window when ... `cd`able", a security fix).
    "^\\W*(?:remove|delete)\\b(?:\\s+[\\w-]+){0,3}\\s*\\(?`",
    // ... or a qualified name without backticks ("remove Socket#rooms object").
    "^\\W*(?:remove|delete)\\b(?:\\s+[\\w-]+){0,2}\\s+[A-Za-z_]\\w*(?:#|::|\\.)[A-Za-z_]",
  ].join("|"),
  "i",
);
/** Wording that changes the API or its requirements, whatever else the line mentions. */
const BREAKING_HEURISTIC = new RegExp(
  [
    "\\bno longer (?:accepts?|returns?|supports?|exports?|exported|re-?exports?|available|provides?|allow(?:s|ed)?|implements?|public|includes?|ships?|compiles?|works? with|required?)\\b",
    "\\bdrop(?:ped|s)? (?:support|compatibility)\\b",
    "\\bdropped\\b.*\\b(?:support|compatibility)\\b",
    // A window of .{0,60}, not [^.]: versions carry dots ("MSRV: Rust 1.64.0 or later is now required").
    "\\b(?:bump|bumped|raise|raised|increase|increased|update|updated|require|requires|now)\\b.{0,60}\\b(?:MSRV|minimum supported rust version|rust-version)\\b",
    // A raise names the new version ("MSRV is now 1.70", "MSRV: Rust 1.64.0 or later");
    // "Move MSRV metadata to `Cargo.toml`" names none.
    "\\b(?:MSRV|minimum supported rust version)\\b.{0,60}?\\b(?:is now|to|bumped to|raised to|increased to)\\s+(?:rust\\s+|rustc\\s+)?v?\\d",
    "\\b(?:MSRV|minimum supported rust version)\\b.{0,60}\\d+\\.\\d+.{0,20}\\bor later\\b",
    "\\bnow (?:requires?|required|returns?|takes?|accepts? only)\\b",
    "\\b(?:function|method|type|the) signatures?\\b",
    "\\bsignatures? (?:of|has|have|changed)\\b",
    "\\b(?:changed?|new|different)\\b[^.]{0,30}\\b(?:return|argument|parameter) types?\\b",
    "\\b(?:return|argument|parameter) types? (?:have |has )?(?:changed|change)\\b",
    "\\btypes have changed\\b",
    // An API that now implements/returns/takes a different type; "Use `Cell` instead
    // of `RefCell` in `Format`" is an internal choice, not an API change.
    "\\b(?:implements?|returns?|takes?|accepts?|requires?|yields?|expects?)\\b[^.]{0,40}`[^`]+`\\s+instead of\\s+`[^`]+`",
    "\\bto keep the (?:old|previous|former) behaviou?r\\b",
    "\\bincompatib",
  ].join("|"),
  "i",
);
/** Removals that change nothing a caller sees: "Removed unused imports", "Removed Webpack". */
const INTERNAL_CHANGE =
  /\b(?:unused|unneeded|unnecessary|internal(?:ly)?|dead code|tests?|ci|lint(?:ing)?|typos?|comments?|docs?|documentation|readme|redundant|duplicated?|webpack|dev-?dependenc(?:y|ies)|dependency|warnings?|examples?|benchmarks?|release process|build process|tooling)\b/i;
const DEPRECATION = /deprecat/i;
/**
 * An entry whose main verb is "deprecate" is a deprecation even when it names
 * the replacement ("Deprecated `Itertools::group_by` (renamed `chunk_by`)"):
 * the old name still works.
 */
const LEADING_DEPRECATION = /^\W*(?:\*\*[^*]{1,40}\*\*:?\s*)?(?:deprecated?|deprecates|deprecating)\b/i;

/** Kind of a single changelog line by its own wording. */
export function classifyText(text: string): EntryKind {
  if (EXPLICIT_BREAKING_MARKER.test(text)) return "breaking";
  if (NON_BREAKING_LABEL.test(text)) {
    if (SECURITY.test(text)) return "security";
    return DEPRECATION.test(text) ? "deprecation" : "change";
  }
  if (YANK_NOTICE.test(text)) return "change";
  if (LEADING_DEPRECATION.test(text)) return "deprecation";
  if (EXPLICIT_BREAKING_PROSE.test(text)) return "breaking";
  if (SECURITY.test(text)) return "security";
  if (BREAKING_HEURISTIC.test(text)) return "breaking";
  if (REMOVAL_HEURISTIC.test(text) && !INTERNAL_CHANGE.test(text)) return "breaking";
  if (DEPRECATION.test(text)) return "deprecation";
  return "change";
}

/**
 * What a sub-heading (or a label-only bullet such as express's "* remove:")
 * says about the entries under it:
 * - an EntryKind: the author's explicit call ("BREAKING CHANGES", "⚠",
 *   changesets' "Major Changes", "Security", "Deprecated");
 * - "removal": a removal list ("Removed", "remove:", "API changes"), breaking
 *   except for internal clean-up and deprecation notices filed under it (axios
 *   1.0 removed "unused imports" there; nix files deprecations under Removed);
 * - "additive": additions and fixes ("Added", "Fixed", "Features", "Other"),
 *   where only an explicit marker makes an entry breaking ("Added a clear()
 *   function ... removed from an axios instance" is not a removal).
 */
export type EntryContext = EntryKind | "removal" | "additive";

const ADDITIVE_HEADING =
  /^\W*(?:added|adds|new|new features?|features?|enhancements?|improvements?|fixed|fixes|bug ?fixes|performance(?: improvements)?|perf|optimi[sz]ations?|docs?|documentation|tests?|testing|internal|chores?|ci|build|refactor(?:ing)?|misc(?:ellaneous)?|other(?: changes)?)\W*$/i;

/** Context implied by a heading or label, or null when it carries no signal ("### Changed"). */
export function classifyHeading(heading: string): EntryContext | null {
  if (/(?<!-)\bbreaking\b|⚠|incompatib|migration|\bmajor changes?\b/i.test(heading)) return "breaking";
  if (/\bremov(?:e|ed|als?)\b|\bapi changes?\b/i.test(heading)) return "removal";
  if (/security|vulnerab/i.test(heading)) return "security";
  if (/deprecat/i.test(heading)) return "deprecation";
  if (ADDITIVE_HEADING.test(heading)) return "additive";
  return null;
}

/** Final kind: an explicit heading or bullet parent wins; removal and additive contexts refine the line's wording. */
export function classifyEntry(text: string, context: EntryContext | null): EntryKind {
  if (context === "breaking" || context === "security" || context === "deprecation") {
    // A security line under "Breaking" stays breaking: the heading is the author's own call.
    return context;
  }
  if (context === "removal") {
    if (EXPLICIT_BREAKING_MARKER.test(text)) return "breaking";
    if (DEPRECATION.test(text) && !/\bremov/i.test(text)) return "deprecation";
    return INTERNAL_CHANGE.test(text) ? "change" : "breaking";
  }
  if (context === "additive") {
    if (EXPLICIT_BREAKING_MARKER.test(text)) return "breaking";
    if (SECURITY.test(text)) return "security";
    return DEPRECATION.test(text) ? "deprecation" : "change";
  }
  return classifyText(text);
}
