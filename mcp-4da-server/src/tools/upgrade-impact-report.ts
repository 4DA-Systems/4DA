// SPDX-License-Identifier: Apache-2.0
/**
 * Output shaping for `upgrade_impact`: cross-referencing changelog entries
 * with the project's symbols, concise vs detailed trimming, and the one-line
 * summary an agent reads first.
 *
 * Concise is the default because a multi-major upgrade (vite 6 -> 7 crosses
 * dozens of releases) produces hundreds of "fix typo"-class entries that
 * would bury the five that matter. Concise keeps EVERY breaking, deprecation
 * and security entry; of the plain changes it keeps up to 20 per version that
 * sit under an API or neutral heading ("Changed", "Methods", "Types", or none)
 * and up to 3 under an additive one ("Added", "Fixed", "Docs"), and states how
 * many it left out — omission is always visible, never silent.
 *
 * The neutral allowance is where the classifier's misses live: on the
 * 2026-10-03 panel, 11 of 15 breaking changes it called plain sat under
 * actix-web 4's "Functions" / "Methods" / "Types" headings, and three-per-version
 * hid them from the agent entirely.
 */

import { classifyHeading, type EntryKind } from "../live/changelog-classify.js";
import type { ChangelogSection } from "../live/changelog.js";
import { compareVersionPrecedence, parseSemverPrecedence } from "../live/semver-precedence.js";
import { matchSymbols } from "./upgrade-impact-callsites.js";

export type ResponseFormat = "concise" | "detailed";

export interface ReportEntry {
  kind: EntryKind;
  text: string;
  /** The changelog heading or parent bullet the entry sits under ("Removed", "Breaking Changes"). */
  under?: string;
  touches_your_code?: boolean;
  matched_symbols?: string[];
  /** String literals in your code that use the syntax this entry retires (route patterns and the like). */
  matched_literals?: Array<{ file: string; line: number; literal: string }>;
}

export interface ReportSection {
  version: string;
  date: string | null;
  entries: ReportEntry[];
  /** Concise mode only: how many plain `change` entries were left out. */
  omitted_changes?: number;
}

export interface ShapedChangelog {
  sections: ReportSection[];
  breaking: number;
  deprecations: number;
  security: number;
  /** Breaking entries naming a symbol this project uses (deprecations are flagged per entry but not counted here). */
  touching: number;
  touchingSymbols: string[];
  truncated?: string;
  /** Entries left out because an earlier (newer) section already listed the same change. */
  duplicates?: number;
}

const CONCISE_NEUTRAL_PER_VERSION = 20;
const CONCISE_ADDITIVE_PER_VERSION = 3;
const DETAILED_ENTRY_CAP = 400;
const KIND_ORDER: Record<EntryKind, number> = { breaking: 0, security: 1, deprecation: 2, change: 3 };

/** Flag, order and trim changelog sections. Counts always cover every entry, trimmed or not. */
export function shapeChangelog(
  sections: ChangelogSection[],
  symbols: string[],
  format: ResponseFormat,
): ShapedChangelog {
  const out: ShapedChangelog = {
    sections: [], breaking: 0, deprecations: 0, security: 0, touching: 0, touchingSymbols: [],
  };
  const touched = new Set<string>();
  let emitted = 0;
  let dropped = 0;
  // A release's notes often repeat its prereleases' (axum 0.8.0 repeats the
  // 0.8.0-alpha/rc entries): read once, in the newest section, and counted
  // once. Measured 2026-10-02 in an agent eval: axum 0.7.9 -> 0.8.4 reported
  // "20 breaking changes" for about ten distinct ones.
  const seen = new Set<string>();
  const key = (text: string) =>
    text.toLowerCase().replace(/\(\[?#\d+\]?\)|\[#\d+\]|#\d+/g, "").replace(/[^a-z0-9]+/g, " ").trim();

  for (const section of sections) {
    const unique = section.entries.filter((e) => {
      const k = key(e.text);
      if (!k || seen.has(k)) {
        if (k) out.duplicates = (out.duplicates ?? 0) + 1;
        return !k;
      }
      seen.add(k);
      return true;
    });
    const entries: ReportEntry[] = unique.map((e) => {
      const entry: ReportEntry = { kind: e.kind, text: e.text, ...(e.under ? { under: e.under } : {}) };
      if (e.kind === "breaking" || e.kind === "deprecation") {
        const matched = matchSymbols(e.text, symbols);
        if (matched.length > 0) {
          entry.touches_your_code = true;
          entry.matched_symbols = matched;
          if (e.kind === "breaking") {
            out.touching++;
            matched.forEach((s) => touched.add(s));
          }
        }
      }
      if (e.kind === "breaking") out.breaking++;
      else if (e.kind === "deprecation") out.deprecations++;
      else if (e.kind === "security") out.security++;
      return entry;
    });
    // Stable sort: entries touching your code first, then by kind, document order within.
    entries.sort(
      (a, b) =>
        Number(Boolean(b.touches_your_code)) - Number(Boolean(a.touches_your_code)) ||
        KIND_ORDER[a.kind] - KIND_ORDER[b.kind],
    );

    let kept = entries;
    const shaped: ReportSection = { version: section.version, date: section.date, entries: [] };
    if (format === "concise") {
      const important = entries.filter((e) => e.kind !== "change");
      const others = entries.filter((e) => e.kind === "change");
      const additive = (e: ReportEntry) => (e.under ? classifyHeading(e.under) === "additive" : false);
      const neutral = others.filter((e) => !additive(e)).slice(0, CONCISE_NEUTRAL_PER_VERSION);
      const minor = others.filter(additive).slice(0, CONCISE_ADDITIVE_PER_VERSION);
      // Keep document order among the kept plain changes.
      const keptOthers = others.filter((e) => neutral.includes(e) || minor.includes(e));
      kept = [...important, ...keptOthers];
      if (others.length > keptOthers.length) {
        shaped.omitted_changes = others.length - keptOthers.length;
      }
    } else {
      const room = Math.max(0, DETAILED_ENTRY_CAP - emitted);
      if (kept.length > room) {
        dropped += kept.length - room;
        kept = kept.slice(0, room);
      }
    }
    emitted += kept.length;
    shaped.entries = kept;
    out.sections.push(shaped);
  }

  out.touchingSymbols = [...touched].sort();
  if (dropped > 0) {
    out.truncated = `detailed output is capped at ${DETAILED_ENTRY_CAP} entries; ${dropped} later entries were left out (counts still include them)`;
  }
  return out;
}

export type UpgradeType = "patch" | "minor" | "major" | "prerelease" | "unknown";

/**
 * Upgrade class by semver. A 0.x minor bump is reported as "major": under
 * caret rules (npm and Cargo alike) 0.32 -> 0.37 is as breaking as 1 -> 2.
 */
export function upgradeType(from: string, to: string): UpgradeType {
  const a = parseSemverPrecedence(from);
  const b = parseSemverPrecedence(to);
  if (!a || !b) return "unknown";
  if (b.prerelease.length > 0) return "prerelease";
  if (a.major !== b.major) return "major";
  if (a.major === 0 && a.minor !== b.minor) return "major";
  if (a.minor !== b.minor) return "minor";
  return "patch";
}

/** Major versions crossed (0.x minors count as majors, matching `upgradeType`). */
export function majorsCrossed(from: string, to: string): number {
  const a = parseSemverPrecedence(from);
  const b = parseSemverPrecedence(to);
  if (!a || !b) return 0;
  if (a.major === 0 && b.major === 0) return Math.max(0, b.minor - a.minor);
  return Math.max(0, b.major - a.major);
}

/** Up to `count` published versions nearest to `wanted` by precedence, for "version not found" errors. */
export function nearestVersions(published: string[], requested: string, count = 5): string[] {
  // "7" / "7.x" / "7.1" are not semver: read them as the start of that line,
  // or every comparison is unreadable and the suggestion list comes back empty.
  const parts = requested.replace(/\.[x*]$/i, "").split(".");
  const wanted = /^\d+(\.\d+){0,2}$/.test(parts.join(".")) ? [...parts, "0", "0"].slice(0, 3).join(".") : requested;
  const below = published.filter((v) => (compareVersionPrecedence(v, wanted) ?? 1) < 0);
  const above = published.filter((v) => (compareVersionPrecedence(v, wanted) ?? -1) > 0);
  // Prefer an even split; let either side fill in when the other runs short.
  const fromAbove = Math.min(above.length, count - Math.min(below.length, Math.floor(count / 2)));
  const fromBelow = Math.min(below.length, count - fromAbove);
  return [...below.slice(below.length - fromBelow), ...above.slice(0, fromAbove)];
}

export interface SummaryInput {
  pkg: string;
  from: string;
  to: string;
  releases: number;
  breaking: number;
  touching: number;
  touchingSymbols: string[];
  /** missing = no changelog file; no_entries = a file with no section for these releases; partial = some of them. */
  changelog: "missing" | "no_entries" | "partial" | "complete";
  advisoriesFixed: number | null;
}

/** The one sentence a human or agent reads first. */
export function summarize(input: SummaryInput): string {
  const majors = majorsCrossed(input.from, input.to);
  const parts = [
    majors > 0 ? `${majors} major version${majors === 1 ? "" : "s"}` : null,
    `${input.releases} release${input.releases === 1 ? "" : "s"}`,
  ].filter(Boolean);
  let breaking: string;
  const atLeast = input.changelog === "partial" ? "at least " : "";
  if (input.changelog === "missing") breaking = "no changelog in the package archive (see release_notes_url)";
  else if (input.changelog === "no_entries") {
    breaking = "the package's changelog has no entries for these releases, so breaking changes are unknown (see release_notes_url)";
  } else {
    // "flagged", not "N breaking changes": the flags come from headings and
    // wording and are neither exhaustive nor always right (_meta.classification).
    const flagged = `${atLeast}${input.breaking} entr${input.breaking === 1 ? "y" : "ies"} flagged breaking`;
    if (input.touching > 0) {
      const names = input.touchingSymbols.slice(0, 5).join(", ");
      const verb = input.breaking === 1 ? "it touches" : `${input.touching} touch`;
      breaking = `${flagged} (${verb} your code: ${names})`;
    } else breaking = flagged;
  }
  const advisories =
    input.advisoriesFixed === null ? "advisories unknown (OSV unreachable)" : `${input.advisoriesFixed} advisories fixed`;
  return `${input.pkg} ${input.from} -> ${input.to}: ${parts.join(", ")}, ${breaking}, ${advisories}.`;
}
