// SPDX-License-Identifier: Apache-2.0
/**
 * Output shaping for `upgrade_impact`: cross-referencing changelog entries
 * with the project's symbols, concise vs detailed trimming, and the one-line
 * summary an agent reads first.
 *
 * Concise is the default because a multi-major upgrade (vite 6 -> 7 crosses
 * dozens of releases) produces hundreds of "fix typo"-class entries that
 * would bury the five that matter. Concise keeps EVERY breaking, deprecation
 * and security entry, plus up to three others per version, and states how
 * many it left out — omission is always visible, never silent.
 */

import type { EntryKind } from "../live/changelog-classify.js";
import type { ChangelogSection } from "../live/changelog.js";
import { compareVersionPrecedence, parseSemverPrecedence } from "../live/semver-precedence.js";
import { matchSymbols } from "./upgrade-impact-callsites.js";

export type ResponseFormat = "concise" | "detailed";

export interface ReportEntry {
  kind: EntryKind;
  text: string;
  touches_your_code?: boolean;
  matched_symbols?: string[];
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
}

const CONCISE_OTHER_PER_VERSION = 3;
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

  for (const section of sections) {
    const entries: ReportEntry[] = section.entries.map((e) => {
      const entry: ReportEntry = { kind: e.kind, text: e.text };
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
      kept = [...important, ...others.slice(0, CONCISE_OTHER_PER_VERSION)];
      if (others.length > CONCISE_OTHER_PER_VERSION) {
        shaped.omitted_changes = others.length - CONCISE_OTHER_PER_VERSION;
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
export function nearestVersions(published: string[], wanted: string, count = 5): string[] {
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
  changelogFound: boolean;
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
  if (!input.changelogFound) breaking = "no changelog in the package archive (see release_notes_url)";
  else if (input.touching > 0) {
    const names = input.touchingSymbols.slice(0, 5).join(", ");
    const verb = input.breaking === 1 ? "it touches" : `${input.touching} of them touch`;
    breaking = `${input.breaking} breaking change${input.breaking === 1 ? "" : "s"} (${verb} symbols you use: ${names})`;
  } else breaking = `${input.breaking} breaking change${input.breaking === 1 ? "" : "s"}`;
  const advisories =
    input.advisoriesFixed === null ? "advisories unknown (OSV unreachable)" : `${input.advisoriesFixed} advisories fixed`;
  return `${input.pkg} ${input.from} -> ${input.to}: ${parts.join(", ")}, ${breaking}, ${advisories}.`;
}
