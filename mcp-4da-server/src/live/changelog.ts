// SPDX-License-Identifier: Apache-2.0
/**
 * Changelog discovery and parsing for `upgrade_impact`.
 *
 * Changelogs follow no schema, so the parser recognises the version-heading
 * styles actually found in registry archives rather than one spec:
 *   keep-a-changelog      `## [1.2.3] - 2024-01-01`
 *   conventional          `## [7.0.0](https://…/compare/…) (2025-06-24)`
 *   plain / prefixed      `## 1.2.3`, `# v1.2.3 (2024-01-01)`, `### Version 1.2.3`, `## fastembed 5.0.0`
 *   setext                `1.2.3` underlined with `===` / `---`
 *   History.md (express)  `1.2.3 / 2020-01-01`
 * A heading only opens a section when the rest of it looks like a trailer
 * (date, link, "yanked") — `## Upgrading from 1.x` or `#### Rust 1.70 support`
 * must stay sub-headings, or a migration guide would split a release in two.
 *
 * Entries are bullets, numbered items and loose paragraph lines; indented
 * continuation lines join the entry above. Fenced code blocks and link
 * reference definitions are skipped: they are examples and URLs, not changes.
 */

import { compareVersionPrecedence } from "./semver-precedence.js";
import { classifyEntry, classifyHeading, sanitizeEntry, type EntryContext, type EntryKind } from "./changelog-classify.js";

export interface ChangelogEntry {
  kind: EntryKind;
  text: string;
  /**
   * The heading, label or parent bullet the entry sits under ("Removed", "remove",
   * "BREAKING: Functions that accept ..."). Without it "`rt::{Arbiter}` re-exports."
   * under actix-web's "Removed" reads as harmless, to an agent and to a rater alike.
   */
  under?: string;
}

export interface ChangelogSection {
  version: string;
  date: string | null;
  entries: ChangelogEntry[];
}

const NAME_STEMS = ["changelog", "changes", "history", "releases", "release-notes", "release_notes", "releasenotes", "news"];
const NAME_EXTS = ["", ".md", ".markdown", ".txt", ".rst"];

/** True for a root-level basename that names a changelog (case-insensitive). */
export function isChangelogName(basename: string): boolean {
  const lower = basename.toLowerCase();
  return NAME_STEMS.some((stem) => NAME_EXTS.some((ext) => lower === stem + ext));
}

/**
 * The changelog among archive paths, preferring CHANGELOG over the others and
 * Markdown over plain text. Null when there is none.
 */
export function findChangelogFile(paths: Iterable<string>): string | null {
  let best: { path: string; rank: number } | null = null;
  for (const path of paths) {
    const base = path.split("/").filter(Boolean).pop() ?? "";
    if (!isChangelogName(base)) continue;
    const lower = base.toLowerCase();
    const stem = NAME_STEMS.findIndex((s) => lower.startsWith(s));
    const ext = NAME_EXTS.indexOf(lower.slice(NAME_STEMS[stem].length));
    const rank = stem * 10 + (ext === 1 || ext === 2 ? 0 : ext + 1);
    if (!best || rank < best.rank) best = { path, rank };
  }
  return best?.path ?? null;
}

const VERSION = String.raw`\d+\.\d+(?:\.\d+)?(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?`;
const ISO_DATE = /\b(\d{4})[-/.](\d{2})[-/.](\d{2})\b/;
const MONTH_DATE =
  /\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\.? \d{1,2}(?:st|nd|rd|th)?,? \d{4}\b|\b\d{1,2} (?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*,? \d{4}\b/i;

/** `[optional prefix] version rest`: prefix is a keyword, a `v`, or a package name. */
const HEADING = new RegExp(
  String.raw`^\[?(?:(version|release)\s+|([A-Za-z@][\w@/.-]*)(?:\s+|@))?\[?v?(${VERSION})\]?(.*)$`,
  "i",
);

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/** ISO date from a heading trailer: `2024-01-01`, `2024/01/01`, `August 1st, 2025`, `1 Aug 2025`. */
function extractDate(text: string): string | null {
  const iso = ISO_DATE.exec(text);
  if (iso) return `${iso[1]}-${iso[2]}-${iso[3]}`;
  const named = MONTH_DATE.exec(text)?.[0];
  if (!named) return null;
  const month = MONTHS.findIndex((m) => named.toLowerCase().includes(m)) + 1;
  const day = /\b(\d{1,2})(?:st|nd|rd|th)?\b(?!\d)/.exec(named.replace(/\d{4}/, ""))?.[1];
  const year = /\d{4}/.exec(named)?.[0];
  if (!month || !day || !year) return named;
  return `${year}-${String(month).padStart(2, "0")}-${day.padStart(2, "0")}`;
}

/**
 * Version and date from a heading's text, or null when it is not a release
 * heading. Exported for tests.
 */
export function parseVersionHeading(raw: string): { version: string; date: string | null } | null {
  const text = raw.trim().replace(/^\*\*|\*\*$/g, "");
  const m = HEADING.exec(text);
  if (!m) return null;
  const [, keyword, namePrefix, version, rest] = m;
  // A free-form name prefix needs a full x.y.z — "Rust 1.70" is prose, not a release.
  if (namePrefix && !keyword && !/^\d+\.\d+\.\d+/.test(version)) return null;
  const trailer = rest
    .replace(/\]?\([^)]*\)/g, " ")
    .replace(/\[[^\]]*\]/g, " ")
    .replace(ISO_DATE, " ")
    .replace(MONTH_DATE, " ")
    .replace(/\b(?:yanked|unreleased|released|latest|stable|on)\b/gi, " ")
    .replace(/[-–—/:,.*_#()[\]<>|]/g, " ")
    .trim();
  if (trailer.split(/\s+/).filter(Boolean).length > 2) return null;
  return { version, date: extractDate(rest) };
}

const ATX = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const SETEXT_RULE = /^\s*(=+|-+)\s*$/;
const BULLET = /^(\s*)(?:[-*+•]|\d+[.)])\s+(.*)$/;
const LINK_DEF = /^\s*\[[^\]]+\]:\s*\S+/;
const FENCE = /^\s*(```|~~~)/;
const RULE = /^\s*(?:[-=*_]\s*){3,}$/;
/** A whole-line bold label ("**Breaking Changes:**") acts as a sub-heading. */
const BOLD_LABEL = /^\s*\*\*([^*]+)\*\*:?\s*$/;
/** A short unbulleted line ending in a colon: a label for what follows ("Security:", "Breaking changes:"). */
const PLAIN_LABEL = /^\s{0,3}([A-Za-z⚠][\w /&,'()⚠️-]{1,58}?)\s*:\s*$/;
/** A line that opens or closes layout markup rather than describing a change. */
const HTML_LAYOUT = /^<\/?(?:details|summary|div|p|br|hr|img|picture|source|table|tr|td|th|thead|tbody|center|sup|sub)\b/i;
/** A bullet that is only a label: "remove:", "**Breaking**:", "deps:". */
const LABEL_BULLET = /^\W*([A-Za-z][\w -]{0,30}?)\W*:\W*$/;

interface ParseState {
  sections: ChangelogSection[];
  current: ChangelogSection | null;
  /** ATX level of the heading that opened `current`; a non-release heading at or above it ends the section. */
  sectionLevel: number;
  headingKind: EntryContext | null;
  /** Context a parent bullet lends to the bullets nested under it, and that parent's indent. */
  parentKind: EntryContext | null;
  parentIndent: number | null;
  /** Text of the current sub-heading and of the parent bullet, for ChangelogEntry.under. */
  headingText: string | null;
  parentText: string | null;
  pending: { text: string; context: EntryContext | null; indent: number; under: string | null } | null;
}

function flush(state: ParseState): void {
  if (state.pending && state.current) {
    const text = sanitizeEntry(state.pending.text);
    const under = state.pending.under;
    // Layout markup is not a change: actix-web 4 wraps pre-release notes in
    // `<details> <summary>...</summary>`, and both lines were counted as removals.
    if (text && !HTML_LAYOUT.test(text)) {
      state.current.entries.push({ kind: classifyEntry(text, state.pending.context), text, ...(under ? { under } : {}) });
    }
  }
  state.pending = null;
}

/**
 * Headings that name a kind of change (keep-a-changelog and conventional-commit
 * categories), as opposed to a new document part ("Migration guide", "Older releases").
 */
const CATEGORY_HEADING =
  /^\W*(added|changed|changes|deprecated|deprecations|removed|removals|fixed|fixes|bug ?fixes|security|features?|new features|enhancements?|improvements?|performance( improvements)?|breaking( changes?)?|⚠️?\s*breaking( changes?)?|dependencies|dependency updates|documentation|docs|internal|misc(ellaneous)?|other( changes)?|chores?|refactor(ing)?|reverts?|build|tests?)\W*$/i;

function isCategoryHeading(raw: string): boolean {
  return CATEGORY_HEADING.test(raw.trim());
}

function openSection(state: ParseState, heading: { version: string; date: string | null }, level: number): void {
  flush(state);
  state.sectionLevel = level;
  state.current = { version: heading.version, date: heading.date, entries: [] };
  state.sections.push(state.current);
  state.headingKind = null;
  state.parentKind = null;
  state.parentIndent = null;
  state.headingText = null;
  state.parentText = null;
}

/** A heading or parent bullet as context text: emphasis and code marks dropped, one line, at most 80 characters. */
function contextText(raw: string): string | null {
  return sanitizeEntry(raw.replace(/\*\*|__|`/g, "").replace(/:\s*$/, ""), 80) || null;
}

function addLine(state: ParseState, line: string): void {
  const bullet = BULLET.exec(line);
  if (bullet) {
    flush(state);
    const indent = bullet[1].length;
    const text = bullet[2];
    // The outermost bullet level of a list, wherever it is indented (express's
    // History.md lists sit at two spaces), lends context to bullets nested under it.
    const outer = state.parentIndent === null || indent <= state.parentIndent;
    if (outer) {
      state.parentIndent = indent;
      // A label-only bullet ("* remove:", "- Breaking:") is a heading for its children, not an entry.
      const label = LABEL_BULLET.exec(text);
      if (label) {
        state.parentKind = classifyHeading(label[1]) ?? state.headingKind;
        state.parentText = contextText(label[1]);
        return;
      }
      // A breaking bullet lends "breaking" to its sub-points (date-fns 3.0: "**BREAKING**: Functions
      // that accept Interval arguments ..." with one nested bullet per affected function); one
      // ending in ":" lends any signalling kind.
      const own = classifyEntry(text, state.headingKind);
      state.parentKind = own === "breaking" || (/:\s*$/.test(text) && own !== "change") ? own : null;
      state.parentText = contextText(text);
    }
    const context = !outer && state.parentKind ? state.parentKind : state.headingKind;
    const under = !outer && state.parentText ? state.parentText : state.headingText;
    state.pending = { text, context, indent, under };
    return;
  }
  if (line.trim() === "") {
    flush(state);
    return;
  }
  if (state.pending && /^\s+\S/.test(line)) {
    state.pending.text += ` ${line.trim()}`;
    return;
  }
  if (state.pending && state.pending.indent === -1) {
    state.pending.text += ` ${line.trim()}`; // wrapped paragraph line
    return;
  }
  flush(state);
  state.pending = { text: line, context: state.headingKind, indent: -1, under: state.headingText };
}

/** Parse a changelog into version sections, in document order (usually newest first). */
export function parseChangelog(text: string): ChangelogSection[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const state: ParseState = {
    sections: [],
    current: null,
    sectionLevel: 0,
    headingKind: null,
    parentKind: null,
    parentIndent: null,
    headingText: null,
    parentText: null,
    pending: null,
  };
  let inFence = false;
  let inComment = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (FENCE.test(line)) {
      flush(state);
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;
    // HTML comments are skipped whole, including ones that span lines
    // (release tooling leaves `<!--\n  template notes\n-->` blocks in changelogs).
    const trimmed = line.trim();
    if (inComment) {
      if (trimmed.includes("-->")) inComment = false;
      continue;
    }
    if (trimmed.startsWith("<!--")) {
      if (!trimmed.slice(4).includes("-->")) inComment = true;
      continue;
    }
    if (LINK_DEF.test(line)) continue;

    const atx = ATX.exec(line);
    if (atx) {
      const heading = parseVersionHeading(atx[2]);
      const level = atx[1].length;
      if (heading) openSection(state, heading, level);
      else {
        flush(state);
        // "## Migration guide" after the last release must not be read as part of it.
        // A change-category heading ("## Changed", "## Breaking Changes") at the
        // release's own level is still inside it: date-fns 3.0.0 writes
        // "## v3.0.0" then "## Changed", and its ten BREAKING entries were dropped.
        if (state.current && level <= state.sectionLevel && !isCategoryHeading(atx[2])) state.current = null;
        state.headingKind = classifyHeading(atx[2]);
        state.headingText = contextText(atx[2]);
        state.parentKind = null;
        state.parentIndent = null;
        state.parentText = null;
      }
      continue;
    }
    const next = lines[i + 1];
    if (next !== undefined && SETEXT_RULE.test(next) && line.trim() !== "" && !BULLET.test(line)) {
      const heading = parseVersionHeading(line);
      if (heading) {
        openSection(state, heading, next.trim().startsWith("=") ? 1 : 2);
        i++;
        continue;
      }
    }
    // History.md style without an underline: `1.2.3 / 2020-01-01` alone on a line.
    const history = new RegExp(String.raw`^v?(${VERSION})\s+/\s+(.+)$`).exec(line.trim());
    if (history && extractDate(history[2])) {
      openSection(state, { version: history[1], date: extractDate(history[2]) }, 2);
      continue;
    }
    if (RULE.test(line)) {
      flush(state);
      continue;
    }
    // A release as a top-level bullet with its changes nested under it
    // (indexmap's RELEASES.md: "- 2.0.0" then "  - **MSRV**: Rust 1.64.0 ...").
    const versionBullet = BULLET.exec(line);
    if (versionBullet && versionBullet[1].length === 0 && /^\[?v?\d/.test(versionBullet[2])) {
      const heading = parseVersionHeading(versionBullet[2]);
      if (heading && !/\s\w+\s\w+/.test(versionBullet[2].replace(ISO_DATE, "").replace(MONTH_DATE, ""))) {
        openSection(state, heading, 7);
        continue;
      }
    }
    // A whole-line label, bold or plain ("**Breaking Changes**", "BREAKING CHANGES:",
    // "Deprecations / Removals / API Changes:") is a sub-heading, not an entry.
    // Plain ones were entries until 2026-10-03: 6 of 17 false "breaking" flags
    // on the fresh corpus-4 panel (highlight.js 11, handlebars 4.6, ts-loader 9).
    const bold = BOLD_LABEL.exec(line) ?? PLAIN_LABEL.exec(line);
    if (bold && state.current) {
      flush(state);
      state.headingKind = classifyHeading(bold[1]);
      state.headingText = contextText(bold[1]);
      state.parentKind = null;
      state.parentIndent = null;
      state.parentText = null;
      continue;
    }
    if (state.current) addLine(state, line);
  }
  flush(state);
  return state.sections;
}

/** True when `version` lies in (from, to] by semver precedence; unreadable versions are out. */
export function inRange(version: string, from: string, to: string): boolean {
  const lower = compareVersionPrecedence(version, from);
  const upper = compareVersionPrecedence(version, to);
  return lower !== null && upper !== null && lower > 0 && upper <= 0;
}

/**
 * Sections in (from, to], plus whether the changelog reaches both ends of the
 * range: its newest section is at or past `to`, and its oldest at or before
 * `firstInRange` (the earliest release the upgrade crosses). A changelog that
 * stopped being maintained two majors ago is reported as not covering the
 * range instead of silently presenting a partial history as the whole one.
 */
export function selectRange(
  sections: ChangelogSection[],
  from: string,
  to: string,
  firstInRange: string,
): { sections: ChangelogSection[]; coversRange: boolean } {
  const readable = sections.filter((s) => compareVersionPrecedence(s.version, s.version) !== null);
  const selected = readable.filter((s) => inRange(s.version, from, to));
  const reachesTop = readable.some((s) => (compareVersionPrecedence(s.version, to) ?? -1) >= 0);
  const reachesBottom = readable.some((s) => (compareVersionPrecedence(s.version, firstInRange) ?? 1) <= 0);
  return { sections: selected, coversRange: selected.length > 0 && reachesTop && reachesBottom };
}
