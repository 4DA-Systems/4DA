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
const EXPLICIT_BREAKING_PROSE = /(?<!\bnot (?:a )?|\bnon[- ]|\bno )\bbreaking[- ]changes?\b/i;
/**
 * Inline labels that say an entry is NOT a breaking change (axum's
 * `**fixed:** Removed the warning about breaking changes from README`,
 * conventional `fix:` / `docs:`). Such entries keep only their security and
 * deprecation signal; removal/rename keywords inside them are prose.
 */
const NON_BREAKING_LABEL =
  /^\s*\*{0,2}(?:fixed|fix|fixes|added|add|feat|features?|docs?|perf|chore|internal|improved|tests?|ci|build|refactor|style)(?:\([^)]*\))?\*{0,2}:\*{0,2}\s/i;
const SECURITY = /\bsecurity\b|\bCVE-\d{4}-\d+|\bGHSA-[\w-]+|\bRUSTSEC-\d{4}-\d+|vulnerab/i;
const BREAKING_HEURISTIC = new RegExp(
  [
    "\\bno longer\\b",
    "\\bremoved\\b",
    "\\brenamed\\b",
    "\\bdrop(?:ped|s)? support\\b",
    "\\bdropped\\b.*\\bsupport\\b",
    "minimum supported rust version",
    "\\bMSRV\\b",
    "\\bnow requires?\\b",
    "\\bincompatib",
  ].join("|"),
  "i",
);
const DEPRECATION = /deprecat/i;

/** Kind of a single changelog line by its own wording. */
export function classifyText(text: string): EntryKind {
  if (EXPLICIT_BREAKING_MARKER.test(text)) return "breaking";
  if (NON_BREAKING_LABEL.test(text)) {
    if (SECURITY.test(text)) return "security";
    return DEPRECATION.test(text) ? "deprecation" : "change";
  }
  if (EXPLICIT_BREAKING_PROSE.test(text)) return "breaking";
  if (SECURITY.test(text)) return "security";
  if (BREAKING_HEURISTIC.test(text)) return "breaking";
  if (DEPRECATION.test(text)) return "deprecation";
  return "change";
}

/**
 * Kind implied by a sub-heading inside a version section, or null when the
 * heading carries no signal ("### Features", "### Bug Fixes").
 */
export function classifyHeading(heading: string): EntryKind | null {
  if (/breaking|⚠|\bremoved\b|incompatib|migration/i.test(heading)) return "breaking";
  if (/security|vulnerab/i.test(heading)) return "security";
  if (/deprecat/i.test(heading)) return "deprecation";
  return null;
}

/** Final kind: a signalling heading (or bullet parent) wins; otherwise the line's own wording. */
export function classifyEntry(text: string, context: EntryKind | null): EntryKind {
  if (context && context !== "change") {
    // A security line under "Breaking" stays breaking: the heading is the author's own call.
    return context;
  }
  return classifyText(text);
}
