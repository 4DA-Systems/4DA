// SPDX-License-Identifier: Apache-2.0
/**
 * OSV affected-range evaluation for knowledge gaps: the TypeScript twin of
 * `src-tauri/src/osv/matching.rs::check_version_affected`, so the desktop app
 * and this server answer "is this install inside that advisory?" the same way.
 *
 * The first version here understood only `introduced`/`fixed`. GHSA-9crc-q9x8-hgqq
 * stores vitest's pre-1.0 window as `[{"introduced":"0"},{"last_affected":"0.0.125"}]`;
 * the loop never saw a `fixed`, read the trailing `introduced: "0"` as
 * "affected from 0 onward", and put EVERY vitest version inside a CVSS 9.6
 * advisory (measured 2026-09-11 against vitest 4.1.11, whose one real exposure
 * is fixed at exactly 4.1.11).
 */

import { comparePrecedence, parseSemverPrecedence, type SemverPrecedence } from "../live/semver-precedence.js";

/** One event of an OSV affected range; OSV sets exactly one key per event. */
export interface AdvisoryRangeEvent {
  introduced?: string;
  fixed?: string;
  last_affected?: string;
  limit?: string;
}

/** An advisory's own severity tier. */
export type AdvisoryTier = "critical" | "high" | "medium" | "low";

/**
 * The SEMVER/ECOSYSTEM ranges of a stored `osv_advisories.affected_ranges`
 * value as event lists, or null when the value is missing or unreadable.
 * `[]` is readable and contains nothing. GIT ranges carry commit hashes and
 * are skipped, as the Rust reference skips them; a range with no `type` is
 * read as SEMVER.
 */
export function parseAffectedRanges(json: string | null | undefined): AdvisoryRangeEvent[][] | null {
  if (typeof json !== "string" || json.trim() === "") return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return null;
  }
  if (!Array.isArray(parsed)) return null;
  const ranges: AdvisoryRangeEvent[][] = [];
  for (const range of parsed) {
    if (typeof range !== "object" || range === null) continue;
    const { type, events } = range as { type?: unknown; events?: unknown };
    if (typeof type === "string" && type !== "SEMVER" && type !== "ECOSYSTEM") continue;
    if (!Array.isArray(events)) continue;
    ranges.push(events.filter((e): e is AdvisoryRangeEvent => typeof e === "object" && e !== null));
  }
  return ranges;
}

/**
 * OSV's marker for a boundary nobody knows ("2.5.0-NA", from PYSEC imports).
 * `matching.rs::is_unknown_bound`: such a bound grounds no match.
 */
function isUnknownBound(v: string): boolean {
  const t = v.trim();
  return t.endsWith("-NA") || t.endsWith("-na");
}

type Lower = SemverPrecedence | "zero";
interface Upper {
  /** null: unknown or unreadable, matches nothing. "unbounded": OSV's `limit: "*"`. */
  bound: SemverPrecedence | "unbounded" | null;
  inclusive: boolean;
}

function closingEvent(event: AdvisoryRangeEvent): Upper | null {
  const read = (v: string) => (isUnknownBound(v) ? null : parseSemverPrecedence(v));
  if (typeof event.fixed === "string") return { bound: read(event.fixed), inclusive: false };
  if (typeof event.last_affected === "string") return { bound: read(event.last_affected), inclusive: true };
  if (typeof event.limit === "string") {
    return { bound: event.limit.trim() === "*" ? "unbounded" : read(event.limit), inclusive: false };
  }
  return null;
}

/**
 * Is `version` inside this one range? Events pair up in order: `introduced`
 * opens, `fixed` and `limit` close exclusively, `last_affected` closes
 * inclusively, and one range may hold several pairs. A closing event always
 * ends the pair it closes, even when its bound is unknown or unreadable: such
 * a bound matches nothing and leaves no open range behind. Only an
 * `introduced` that nothing closed means "affected from there on".
 */
function eventsContain(events: AdvisoryRangeEvent[], version: SemverPrecedence): boolean {
  const atOrAbove = (lower: Lower) => lower === "zero" || comparePrecedence(version, lower) >= 0;
  let lower: Lower | null = null;
  for (const event of events) {
    if (typeof event.introduced === "string") {
      lower = event.introduced.trim() === "0" ? "zero" : parseSemverPrecedence(event.introduced);
    }
    const upper = closingEvent(event);
    if (upper === null) continue;
    if (lower !== null && upper.bound !== null && atOrAbove(lower)) {
      if (upper.bound === "unbounded") return true;
      const cmp = comparePrecedence(version, upper.bound);
      if (upper.inclusive ? cmp <= 0 : cmp < 0) return true;
    }
    lower = null;
  }
  return lower !== null && atOrAbove(lower);
}

/** Is `installed` inside any of `ranges`? Null when the installed version is unreadable. */
export function rangesContain(ranges: AdvisoryRangeEvent[][], installed: string): boolean | null {
  const version = parseSemverPrecedence(installed);
  if (version === null) return null;
  return ranges.some((events) => eventsContain(events, version));
}

/**
 * Is `installed` inside any of these advisory ranges?
 *
 * An advisory naming your dependency is only a gap if you are actually
 * exposed: the live tool once graded three Hono CVEs `critical` on hono
 * 4.13.2 when all three are fixed in 4.12.34, a version this repo had already
 * pinned past via `pnpm.overrides`.
 *
 * Conservative by construction: a missing or unreadable installed version
 * counts as AFFECTED. Never claim someone is safe on missing information.
 */
export function versionInAnyRange(
  ranges: AdvisoryRangeEvent[][],
  installed: string | null | undefined,
): boolean {
  if (!installed) return true;
  return rangesContain(ranges, installed) ?? true;
}

const TIERS = new Set<string>(["critical", "high", "medium", "low"]);
const isTier = (s: string): s is AdvisoryTier => TIERS.has(s);

/**
 * An advisory's tier: its CVSS band first (>= 9 critical, >= 7 high, >= 4
 * medium, else low: `osv::types::cvss_band`), else the source's curated label,
 * else null (ungraded). Mirrors `knowledge_decay::advisory_tier_for`.
 */
export function advisoryTier(
  cvssScore: number | null | undefined,
  label: string | null | undefined,
): AdvisoryTier | null {
  if (typeof cvssScore === "number" && Number.isFinite(cvssScore)) {
    if (cvssScore >= 9) return "critical";
    if (cvssScore >= 7) return "high";
    if (cvssScore >= 4) return "medium";
    return "low";
  }
  const l = typeof label === "string" ? label.trim().toLowerCase() : "";
  return isTier(l) ? l : null;
}

const TIER_RANK: Record<AdvisoryTier, number> = { critical: 4, high: 3, medium: 2, low: 1 };

/** The more severe of two tiers; null (ungraded) ranks below every tier. */
export function moreSevereTier(a: AdvisoryTier | null, b: AdvisoryTier | null): AdvisoryTier | null {
  if (a === null) return b;
  if (b === null) return a;
  return TIER_RANK[b] > TIER_RANK[a] ? b : a;
}
