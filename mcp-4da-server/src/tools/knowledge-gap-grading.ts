// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap grading: the pure rules that decide whether an unread item is
 * a gap for a dependency and how severe it is. Split out of knowledge-gaps.ts,
 * which owns the database walk.
 */

import { compareSemver, parseSemver } from "../live/semver-utils.js";

// Word-boundary matching prevents "cve" matching inside "achieve", "receiver", etc.
function hasWordBoundary(text: string, term: string): boolean {
  const regex = new RegExp(`\\b${escapeRegExp(term)}\\b`, "i");
  return regex.test(text);
}

// Package names can contain regex metacharacters (@scope/name, c++, next.js).
function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
}

// True when `text` mentions the package name as a whole word. Word characters
// for this purpose include - and _ (so dep "hono" does NOT match "hono-shim",
// and never matches "in HONOr of"). @scope/name matches as the full literal.
export function mentionsPackage(text: string, pkg: string): boolean {
  const regex = new RegExp(`(^|[^A-Za-z0-9_-])${escapeRegExp(pkg)}($|[^A-Za-z0-9_-])`, "i");
  return regex.test(text);
}

/** One `introduced`/`fixed` event pair from an OSV affected range. */
export interface AdvisoryRangeEvent {
  introduced?: string;
  fixed?: string;
}

/**
 * Is `installed` inside `[introduced, fixed)` for any of these advisory ranges?
 *
 * An advisory naming your dependency is only a gap if you are actually exposed.
 * Grading skipped this entirely: the live tool reported the three Hono CVEs as
 * a `critical` gap on hono **4.13.2**, when all three are fixed in **4.12.34** —
 * a version this repo had already pinned past via `pnpm.overrides`. The user
 * was told to worry about something they had already remediated.
 *
 * Conservative by construction: an unparseable installed version, or an
 * advisory whose range cannot be read, counts as AFFECTED. Never claim someone
 * is safe on missing information.
 */
export function versionInAnyRange(
  ranges: AdvisoryRangeEvent[][],
  installed: string | null | undefined,
): boolean {
  if (!installed || parseSemver(installed) === null) return true;

  for (const events of ranges) {
    let introduced: string | null = null;
    for (const event of events) {
      if (typeof event.introduced === "string") introduced = event.introduced;
      if (typeof event.fixed === "string" && introduced !== null) {
        const atOrAfterIntroduced =
          introduced === "0" || compareSemver(installed, introduced) >= 0;
        const beforeFix = compareSemver(installed, event.fixed) < 0;
        if (atOrAfterIntroduced && beforeFix) return true;
        introduced = null;
      }
    }
    // An `introduced` with no matching `fixed` means "affected from here on".
    if (introduced !== null) {
      if (introduced === "0" || compareSemver(installed, introduced) >= 0) return true;
    }
  }
  return false;
}

/** The subset of an item this grading needs. */
export interface GradableItem {
  title: string | null;
  source_type?: string | null;
  content_type?: string | null;
}

/** Package-registry sources: every row is a published version of a package. */
export const REGISTRY_SOURCES = new Set(["crates_io", "npm_registry", "pypi", "go_modules"]);

/** Advisory sources: every row is a security advisory. */
export const ADVISORY_SOURCES = new Set(["osv", "cve"]);

/** An advisory row: an advisory source, or anything the pipeline typed as one. */
export function isAdvisoryItem(item: GradableItem): boolean {
  return ADVISORY_SOURCES.has(item.source_type ?? "") || item.content_type === "security_advisory";
}

/**
 * The subject package of an advisory title in the ingested "[ID] package:
 * summary" shape ("[GHSA-h395-gr6q-cpjc] jsonwebtoken: ..."), or null when
 * the title has another shape and cannot be read that way.
 */
export function advisorySubject(title: string): string | null {
  const match = /^\s*\[[^\]]+\]\s*([^:\s]+)\s*:/.exec(title);
  return match ? match[1] : null;
}

/** Same package name, ignoring case and the crates.io `-`/`_` split. */
export function samePackage(a: string, b: string): boolean {
  const norm = (s: string) => s.toLowerCase().replace(/_/g, "-");
  return norm(a) === norm(b);
}

/** The released version a registry row announces ("npm: react v19.3.0" → "19.3.0"), if any. */
export function registryVersionFromTitle(title: string): string | null {
  const versions = title.match(/\bv?\d+\.\d+(?:\.\d+)?(?:[-+][0-9A-Za-z.-]+)?\b/g);
  if (!versions || versions.length === 0) return null;
  return versions[versions.length - 1].replace(/^v/, "");
}

/**
 * Is the installed version inside a known advisory range?
 * - exposed: the stored ranges contain the installed version
 * - safe:    the stored ranges are all fixed at or below it
 * - unknown: nothing stored for the package, an unreadable range, or no
 *            parseable installed version — no claim either way
 */
export type Exposure = "exposed" | "safe" | "unknown";

/** Parse a source_items.published_at value (SQLite "YYYY-MM-DD HH:MM:SS" or ISO). */
export function parsePublishedAt(value: string | null | undefined): number | null {
  if (!value) return null;
  const iso = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const ms = Date.parse(iso);
  return Number.isNaN(ms) ? null : ms;
}

/**
 * Grade a knowledge gap by CONSEQUENCE, never by volume.
 *
 * - `critical` — a real advisory whose TITLE names this dependency. The
 *   advisory is about the dep, not merely co-mentioning it in a body.
 * - `high` — a security-keyword item whose title names the dep.
 * - `medium` — an unread item that names the dep in its title and carries
 *   consequence: a breaking change, a deprecation, or a release.
 * - `low` — everything else, including a large pile of passing mentions.
 *
 * `medium` used to mean "3+ recent unread mentions", and a mention could match
 * on the content body rather than the title. That graded unread VOLUME as a
 * knowledge gap, and since `min_severity` defaults to medium it shipped: a
 * `tracing` gap evidenced by "The Matrix: Writing Code That Doesn't Need
 * Comments", a `typescript` gap evidenced by a Databricks job posting, a `uuid`
 * gap evidenced by Go's standard library, a `vite` gap evidenced by a
 * period-tracker app. Fourteen of fifteen gaps were noise.
 *
 * The Rust surface already draws exactly this line —
 * `knowledge_decay::gap_is_substantive` requires a security advisory, breaking
 * change, or version update, and calls anything else "unread VOLUME, not a
 * knowledge gap". Two implementations of one concept disagreeing is what let
 * this tool report 18 gaps while the app reported none.
 */
export function gradeGap(
  items: GradableItem[],
  packageName: string,
  /**
   * False when every advisory for this package is already fixed at or below the
   * installed version. Security tiers then cannot apply — you cannot be
   * "critically behind" on something you have already patched. Defaults to
   * `true` so callers without version data keep the conservative grade.
   */
  stillVulnerable = true,
  /**
   * The installed version, when known. A registry row is only a version
   * UPDATE when it is newer than this; the row for the version you already
   * run is not missed intelligence. Unknown keeps the conservative grade.
   */
  installedVersion: string | null = null,
): string {
  // An advisory names the dependency when the dependency is its SUBJECT
  // package. Word matching on advisory titles minted critical gaps for `url`
  // from "[CVE-2026-63735] SurrealDB: ... via URL path" and for `hmac` from
  // "[CVE-2026-54736] Phalcon: Non-constant-time HMAC verification" — real
  // advisories, about other packages. Titles of another shape keep the
  // word-boundary test.
  const namesDep = (item: GradableItem) => {
    const title = item.title || "";
    if (isAdvisoryItem(item)) {
      const subject = advisorySubject(title);
      if (subject !== null) return samePackage(subject, packageName);
    }
    return mentionsPackage(title, packageName);
  };

  const securityKeywords = (title: string) =>
    hasWordBoundary(title, "cve") ||
    hasWordBoundary(title, "security") ||
    hasWordBoundary(title, "vulnerability");

  // "Announcing <thing> <version>" is the canonical release phrasing and names
  // no other keyword. The version token is REQUIRED, matching the rule
  // `content_dna_classifiers` settled on: it keeps "Announcing axum 0.8.0" and
  // rejects "Announcing Toasty, an async ORM" and "Announcing our Series B".
  const announcesAVersion = (title: string) =>
    (hasWordBoundary(title, "announcing") || hasWordBoundary(title, "introducing")) &&
    /\bv?\d+\.\d+/.test(title);

  const carriesConsequence = (title: string) =>
    ["breaking", "deprecated", "eol", "release", "released", "update", "upgrade"].some((kw) =>
      hasWordBoundary(title, kw),
    ) || announcesAVersion(title);

  // A registry row IS a version update — "crates.io: serde v1.0.220" names no
  // consequence keyword, but an unread release of a direct dependency is the
  // exact thing `knowledge_decay::gap_is_substantive` counts. Graded on the
  // source, not on the title's vocabulary — provided the row is newer than
  // what is installed (live: "npm: @tauri-apps/api v2.11.1" on 2.11.1).
  const isRegistryRelease = (item: GradableItem) => {
    if (!REGISTRY_SOURCES.has(item.source_type ?? "")) return false;
    const released = registryVersionFromTitle(item.title || "");
    if (!installedVersion || !released) return true;
    if (parseSemver(installedVersion) === null || parseSemver(released) === null) return true;
    return compareSemver(released, installedVersion) > 0;
  };

  if (stillVulnerable) {
    if (items.some((item) => isAdvisoryItem(item) && namesDep(item))) return "critical";
    if (items.some((item) => namesDep(item) && securityKeywords(item.title || ""))) return "high";
  }
  if (
    items.some(
      (item) => namesDep(item) && (isRegistryRelease(item) || carriesConsequence(item.title || "")),
    )
  ) {
    return "medium";
  }
  return "low";
}
