// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap grading: the pure rules that decide whether an unread item is
 * evidence about a dependency and how severe the gap is. The database walk
 * lives in knowledge-gaps.ts; advisory ranges in knowledge-gap-ranges.ts.
 */

import { comparePrecedence, parseSemverPrecedence, type SemverPrecedence } from "../live/semver-precedence.js";
import type { AdvisoryTier } from "./knowledge-gap-ranges.js";

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

/** A knowledge gap's severity. */
export type GapSeverity = "critical" | "high" | "medium" | "low";

/** The subset of an item this grading needs. */
export interface GradableItem {
  title: string | null;
  source_type?: string | null;
  content_type?: string | null;
  /**
   * Set by a caller that has already decided whether this item cites the
   * dependency: the dependency linker's structured proof says what a title
   * cannot. Unset, the title decides.
   */
  cites?: boolean;
}

/** Package-registry sources: every row is a published version of a package. */
export const REGISTRY_SOURCES = new Set(["crates_io", "npm_registry", "pypi", "go_modules"]);

/** Advisory sources: every row is a security advisory. */
export const ADVISORY_SOURCES = new Set(["osv", "cve"]);

/**
 * A registry advisory row (`knowledge_decay::is_advisory_row`). An editorial
 * story the pipeline typed `security_advisory` is not one: it cites by title
 * like any other story and is never proof of exposure (AD-040 rule 4).
 */
export function isAdvisoryRow(item: GradableItem): boolean {
  return ADVISORY_SOURCES.has(item.source_type ?? "");
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
 * Is an install inside a known advisory range?
 * - exposed: a readable same-ecosystem advisory range contains the version
 * - safe:    every readable same-ecosystem range excludes it
 * - unknown: nothing readable stored for its ecosystem, or no readable
 *            installed version — no claim either way
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
 * Does `item` cite `packageName` by its title alone? An advisory row cites
 * only its SUBJECT package and never falls back to a word in its title
 * ("[CVE-2026-63642] MagicMirror newsfeed Socket.IO notification ..." is not
 * a socket.io advisory); every other row names the package in its title.
 */
function citesByTitle(item: GradableItem, packageName: string): boolean {
  const title = item.title || "";
  if (isAdvisoryRow(item)) {
    const subject = advisorySubject(title);
    return subject !== null && samePackage(subject, packageName);
  }
  return mentionsPackage(title, packageName);
}

const RETIREMENT_KEYWORDS = ["breaking", "deprecated", "deprecation", "eol"];
const CONSEQUENCE_KEYWORDS = ["release", "released", "update", "upgrade"];

/**
 * Grade a knowledge gap by CONSEQUENCE, never by volume, on the tiers the
 * desktop app draws (`knowledge_decay::classify_severity`):
 *
 * - `critical`: an advisory citing the dependency still reaches an install,
 *   and the most severe advisory reaching an install is critical or high by
 *   its OWN grade — CVSS band, else the source's curated label (AD-040 rule 2).
 * - `high`: the same with a medium, low or ungraded advisory; or a title
 *   naming the dependency with a breaking change, deprecation or end of life.
 * - `medium`: a registry release newer than an install, or a title naming the
 *   dependency with a release, update or upgrade.
 * - `low`: everything else, including a large pile of passing mentions.
 *
 * `critical` used to mean "an advisory names the dependency", whatever the
 * advisory's own tier: the jsonwebtoken crate's GHSA-h395-gr6q-cpjc, graded
 * medium by its source and High by the app, was a critical gap here (measured
 * 2026-09-11). Earlier still, `medium` meant "3+ recent unread mentions"
 * matched on the content body: a `tracing` gap evidenced by "The Matrix:
 * Writing Code That Doesn't Need Comments", a `typescript` gap by a
 * Databricks job posting, fourteen of fifteen gaps noise. Two implementations
 * of one concept disagreeing is what this mirrors the app to avoid.
 */
export function gradeGap(
  items: GradableItem[],
  packageName: string,
  /**
   * An advisory citing this dependency still reaches an install, or cannot be
   * ruled out. False when every such advisory is fixed at or below the
   * installed versions: you cannot be "critically behind" on something you
   * already patched. Defaults to true so callers without version data keep
   * the conservative grade.
   */
  stillVulnerable = true,
  /**
   * The installed version(s), when known. A registry row is only a version
   * UPDATE when it is newer than an install (AD-041): the row for the version
   * every project already runs is not missed intelligence. Unknown keeps the
   * conservative grade.
   */
  installedVersion: string | readonly string[] | null = null,
  /** The tier of the most severe stored advisory reaching an install; null when ungraded. */
  advisoryTier: AdvisoryTier | null = null,
): GapSeverity {
  const cites = (item: GradableItem) => item.cites ?? citesByTitle(item, packageName);
  // The tiers below the security tier read the TITLE, as the app's do: it
  // must name the dependency (an advisory's subject counts).
  const titleNames = (item: GradableItem) =>
    mentionsPackage(item.title || "", packageName) || (isAdvisoryRow(item) && citesByTitle(item, packageName));

  // "Announcing <thing> <version>" is the canonical release phrasing and names
  // no other keyword. The version token is REQUIRED, matching the rule
  // `content_dna_classifiers` settled on: it keeps "Announcing axum 0.8.0" and
  // rejects "Announcing Toasty, an async ORM" and "Announcing our Series B".
  const announcesAVersion = (title: string) =>
    (hasWordBoundary(title, "announcing") || hasWordBoundary(title, "introducing")) && /\bv?\d+\.\d+/.test(title);
  const retires = (title: string) =>
    RETIREMENT_KEYWORDS.some((kw) => hasWordBoundary(title, kw)) || /\bend[\s-]of[\s-]life\b/i.test(title);
  const carriesConsequence = (title: string) =>
    CONSEQUENCE_KEYWORDS.some((kw) => hasWordBoundary(title, kw)) || announcesAVersion(title);

  // A registry row IS a version update: "crates.io: serde v1.0.220" names no
  // consequence keyword, but an unread release of a direct dependency is the
  // exact thing `knowledge_decay::gap_is_substantive` counts — provided it is
  // newer than an install (live: "npm: @tauri-apps/api v2.11.1" on 2.11.1).
  // Compared by semver precedence, so 0.10.0-rc.19 is newer than rc.18.
  const installed = (typeof installedVersion === "string" ? [installedVersion] : (installedVersion ?? []))
    .map((v) => parseSemverPrecedence(v))
    .filter((v): v is SemverPrecedence => v !== null);
  const isNewerRelease = (item: GradableItem) => {
    if (!REGISTRY_SOURCES.has(item.source_type ?? "")) return false;
    const released = parseSemverPrecedence(registryVersionFromTitle(item.title || "") ?? "");
    if (released === null || installed.length === 0) return true;
    return installed.some((v) => comparePrecedence(released, v) > 0);
  };

  if (stillVulnerable && items.some((item) => isAdvisoryRow(item) && cites(item))) {
    return advisoryTier === "critical" || advisoryTier === "high" ? "critical" : "high";
  }
  if (items.some((item) => cites(item) && titleNames(item) && retires(item.title || ""))) return "high";
  if (
    items.some(
      (item) => cites(item) && (isNewerRelease(item) || (titleNames(item) && carriesConsequence(item.title || ""))),
    )
  ) {
    return "medium";
  }
  return "low";
}
