// SPDX-License-Identifier: Apache-2.0
/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps - dependencies with relevant content you haven't engaged with.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { DependencyWithProjectRow, SourceItemBriefRow } from "../types.js";
import { compareSemver, parseSemver } from "../live/semver-utils.js";
import { mapEcosystem } from "../live/version-resolver.js";

// Word-boundary matching prevents "cve" matching inside "achieve", "receiver", etc.
function hasWordBoundary(text: string, term: string): boolean {
  const regex = new RegExp(`\\b${escapeRegExp(term)}\\b`, 'i');
  return regex.test(text);
}

// Package names can contain regex metacharacters (@scope/name, c++, next.js).
function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
}

// True when `text` mentions the package name as a whole word. Word characters
// for this purpose include - and _ (so dep "hono" does NOT match "hono-shim",
// and never matches "in HONOr of"). @scope/name matches as the full literal.
function mentionsPackage(text: string, pkg: string): boolean {
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
): string {
  const namesDep = (item: GradableItem) => mentionsPackage(item.title || "", packageName);

  const isAdvisory = (item: GradableItem) =>
    item.source_type === "cve" ||
    item.source_type === "osv" ||
    item.content_type === "security_advisory";

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

  if (stillVulnerable) {
    if (items.some((item) => isAdvisory(item) && namesDep(item))) return "critical";
    if (items.some((item) => namesDep(item) && securityKeywords(item.title || ""))) return "high";
  }
  if (items.some((item) => namesDep(item) && carriesConsequence(item.title || ""))) return "medium";
  return "low";
}

// Escape SQL LIKE wildcards so a package name containing % or _ (npm names may
// contain _) is matched literally rather than as a pattern. Paired with ESCAPE '\'.
function escapeLike(s: string): string {
  return s.replace(/[\\%_]/g, (c) => "\\" + c);
}

/**
 * Dependency names that are ordinary English words. A word-boundary match on
 * these is no evidence the item is about the PACKAGE: live 2026-08-30, a
 * `tower` gap was evidenced by a one-tap tower-stacker game and a drivable-car
 * post. For these names the item must also carry an ecosystem cue before it
 * counts as a mention.
 */
const GENERIC_WORD_DEPS = new Set([
  "tower", "base64", "image", "time", "rand", "log", "tracing", "url", "zip",
  "tar", "glob", "regex", "chrono", "notify", "either", "bytes", "flate",
]);

// A token that ties the text to software packaging rather than the English word:
// an ecosystem noun, a security noun, or a version number.
const ECOSYSTEM_CUE =
  /\b(crate|crates\.io|cargo|rust|npm|node|package|library|dependenc|version|release[ds]?|upgrad|deprecat|cve|advisory|vulnerab)|\bv?\d+\.\d+/i;

function hasEcosystemCue(item: { title: string | null; content_head?: string }): boolean {
  return ECOSYSTEM_CUE.test(`${item.title || ""} ${item.content_head || ""}`);
}

export interface KnowledgeGapsParams {
  min_severity?: string;
  limit?: number;
}

export const knowledgeGapsTool = {
  name: "knowledge_gaps",
  description: `Detect knowledge gaps by cross-referencing your project dependencies with source items you haven't engaged with. Identifies things you should know about but might have missed.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      min_severity: {
        type: "string",
        enum: ["critical", "high", "medium", "low"],
        description: "Minimum gap severity to include. Default: medium",
        default: "medium",
      },
      limit: {
        type: "number",
        description: "Maximum gaps to return. Default: 15",
        default: 15,
      },
    },
  },
};

export interface KnowledgeGap {
  dependency: string;
  version: string | null;
  project_path: string;
  language: string;
  missed_items: SourceItemBriefRow[];
  gap_severity: string;
  missed_count: number;
}

export function executeKnowledgeGaps(
  db: FourDADatabase,
  params: KnowledgeGapsParams,
  liveIntel?: Pick<LiveIntelligence, "getResolvedDeps" | "isInitialized"> | null,
) {
  const rawDb = db.getRawDb();

  // The desktop DB's project_dependencies rows often carry `version: NULL`
  // (manifest scrapes record presence, not pins). The advisory range check then
  // stays conservative and grades decade-old, long-fixed advisories as a
  // critical gap — observed live: tokio graded critical on RUSTSEC-2021/2023
  // advisories while every project ran tokio 1.50+. The lockfile-resolved dep
  // set knows the installed version; use it as the fallback.
  const lockfileVersions = new Map<string, string>();
  if (liveIntel && liveIntel.isInitialized()) {
    for (const dep of liveIntel.getResolvedDeps()) {
      if (!dep.version) continue;
      const key = `${dep.ecosystem}\0${dep.name.toLowerCase()}`;
      if (!lockfileVersions.has(key)) lockfileVersions.set(key, dep.version);
      // crates.io: `-` and `_` are one namespace; index both spellings.
      if (dep.ecosystem === "crates.io") {
        const swapped = dep.name.includes("_")
          ? dep.name.replace(/_/g, "-")
          : dep.name.replace(/-/g, "_");
        const altKey = `${dep.ecosystem}\0${swapped.toLowerCase()}`;
        if (!lockfileVersions.has(altKey)) lockfileVersions.set(altKey, dep.version);
      }
    }
  }
  const installedVersionFor = (dep: DependencyWithProjectRow): string | null => {
    if (dep.version) return dep.version;
    const eco = mapEcosystem(dep.language || "");
    return lockfileVersions.get(`${eco}\0${dep.package_name.toLowerCase()}`) ?? null;
  };

  // Feature-detect optional columns: a pure-standalone database has no scoring
  // pipeline (no relevance_score) and may predate content_type — the tool must
  // degrade to word-boundary + recency grounding there, never throw.
  const hasRelevance = db.hasColumn("source_items", "relevance_score");
  const hasContentType = db.hasColumn("source_items", "content_type");
  const hasPublishedAt = db.hasColumn("source_items", "published_at");

  // Advisory-driven grades must not fire on a dependency the user already
  // patched. `osv_advisories` carries the affected ranges the OSV sync stored;
  // when it is absent or silent about a package, stay conservative and grade as
  // if still exposed — never claim someone is safe on missing data.
  const hasOsvTable = db.hasColumn("osv_advisories", "affected_ranges");
  const advisoryRanges = hasOsvTable
    ? rawDb.prepare(
        `SELECT affected_ranges FROM osv_advisories
         WHERE lower(package_name) = lower(?) AND withdrawn_at IS NULL`,
      )
    : null;

  const stillVulnerable = (packageName: string, version: string | null): boolean => {
    if (!advisoryRanges) return true;
    let rows: Array<{ affected_ranges: string | null }>;
    try {
      rows = advisoryRanges.all(packageName) as Array<{ affected_ranges: string | null }>;
    } catch {
      return true;
    }
    if (rows.length === 0) return true; // nothing known about this package

    const ranges: AdvisoryRangeEvent[][] = [];
    for (const row of rows) {
      if (!row.affected_ranges) continue;
      try {
        const parsed = JSON.parse(row.affected_ranges) as Array<{ events?: AdvisoryRangeEvent[] }>;
        for (const r of parsed) if (Array.isArray(r.events)) ranges.push(r.events);
      } catch {
        return true; // unreadable range — assume exposed
      }
    }
    if (ranges.length === 0) return true;
    return versionInAnyRange(ranges, version);
  };
  const hasIsDirect = db.hasColumn("project_dependencies", "is_direct");

  // Direct dependencies only — a transitive dep's news is not the user's
  // reading backlog. Dev deps stay in: a vitest or eslint advisory is real.
  const deps = rawDb
    .prepare(
      `SELECT package_name, version, project_path, language FROM project_dependencies ${hasIsDirect ? "WHERE is_direct = 1 " : ""}LIMIT 100`,
    )
    .all() as DependencyWithProjectRow[];

  if (deps.length === 0) {
    return {
      gaps: [],
      summary: "No project dependencies tracked. Add context directories to enable knowledge gap detection.",
    };
  }

  const gaps: KnowledgeGap[] = [];
  const seenPackages = new Set<string>();

  for (const dep of deps) {
    // Names shorter than 3 chars (e.g. "c", "go", "ws") match too many unrelated
    // items via substring LIKE to be a trustworthy "mention" signal — skip them.
    if (!dep.package_name || dep.package_name.length < 3) continue;
    // One gap per package: the same dep declared by several projects would
    // otherwise repeat identical missed_items once per project_path.
    const pkgKey = dep.package_name.toLowerCase();
    if (seenPackages.has(pkgKey)) continue;
    seenPackages.add(pkgKey);

    // Find source items mentioning this dependency. Engagement is recorded by the
    // app in interactions.item_id / .action_type (the canonical columns; the older
    // source_item_id / action columns are unused), so the NOT-IN suppression must
    // read those or it silently never fires.
    //
    // Grounding (each guard killed an observed false-positive class):
    // - relevance_score >= 0.2: the scoring pipeline's above-noise band — drops
    //   off-topic chatter that merely contains the string (chocolate-bar class).
    // - 30-day window: a "gap" is something you MISSED, not archaeology — a
    //   2014 StackOverflow post is not missed intelligence.
    // - LIKE is only a cheap candidate pre-filter; the real test is the
    //   word-boundary check below ("invite" must never evidence a vite gap).
    // - feed_relevant is deliberately NOT filtered here: an item the feed gate
    //   rejected can still be a legitimate unread dep mention — surfacing those
    //   is this tool's niche. The relevance floor already excludes noise.
    const pattern = `%${escapeLike(dep.package_name)}%`;
    // - published_at guard: OSV/CVE backfills ingest decades-old advisories
    //   whose created_at (discovery) is days old but whose published_at is
    //   ancient. A 2021 advisory is not "missed intelligence" in 2026; keep
    //   NULL (many sources never set it) and anything published recently.
    const candidates = rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, ${hasContentType ? "si.content_type" : "NULL AS content_type"}, si.created_at,
               ${hasRelevance ? "si.relevance_score" : "NULL AS relevance_score"}, substr(COALESCE(si.content, ''), 1, 2000) AS content_head
        FROM source_items si
        WHERE (si.title LIKE ? ESCAPE '\\' OR si.content LIKE ? ESCAPE '\\')
        ${hasRelevance ? "AND si.relevance_score IS NOT NULL AND si.relevance_score >= 0.2" : ""}
        AND si.created_at >= datetime('now', '-30 days')
        ${hasPublishedAt ? "AND (si.published_at IS NULL OR datetime(si.published_at) >= datetime('now', '-90 days'))" : ""}
        AND si.id NOT IN (SELECT item_id FROM interactions WHERE action_type IN ('click', 'save'))
        ORDER BY si.created_at DESC LIMIT 25`)
      .all(pattern, pattern) as Array<SourceItemBriefRow & {
        content_type: string | null;
        relevance_score: number | null;
        content_head: string;
      }>;

    // Word-boundary verification: the mention must be the package name as a
    // whole word in the title or the content head, not a substring. For deps
    // named by ordinary English words, the text must also carry an ecosystem
    // cue — "one-tap tower stacker" mentions the word, not the crate.
    const isGenericName = GENERIC_WORD_DEPS.has(dep.package_name.toLowerCase());
    const mentionedItems = candidates
      .filter(
        (item) =>
          mentionsPackage(item.title || "", dep.package_name) ||
          mentionsPackage(item.content_head || "", dep.package_name),
      )
      .filter((item) => !isGenericName || hasEcosystemCue(item))
      .slice(0, 5);

    if (mentionedItems.length > 0) {
      const installedVersion = installedVersionFor(dep);
      const severity = gradeGap(
        mentionedItems,
        dep.package_name,
        stillVulnerable(dep.package_name, installedVersion),
      );

      gaps.push({
        dependency: dep.package_name,
        version: installedVersion,
        project_path: dep.project_path,
        language: dep.language,
        missed_items: mentionedItems.map((item) => ({
          id: item.id,
          title:
            item.title && item.title.length > 120
              ? item.title.substring(0, 120) + "..."
              : item.title,
          url: item.url,
          source_type: item.source_type,
          created_at: item.created_at,
          relevance_score: item.relevance_score,
        })) as SourceItemBriefRow[],
        gap_severity: severity,
        missed_count: mentionedItems.length,
      });
    }
  }

  // Filter by severity
  const severityOrder: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
  const minLevel = severityOrder[params.min_severity || "medium"] || 2;
  const filtered = gaps.filter(
    (g) => (severityOrder[g.gap_severity] || 0) >= minLevel,
  );

  const maxGaps = Math.min(Math.max(1, params.limit || 15), 50);

  return {
    gaps: filtered.sort(
      (a, b) =>
        (severityOrder[b.gap_severity] || 0) - (severityOrder[a.gap_severity] || 0),
    ).slice(0, maxGaps),
    total_dependencies: deps.length,
    gaps_found: filtered.length,
    gaps_returned: Math.min(filtered.length, maxGaps),
    summary: `${filtered.length} knowledge gaps across ${deps.length} tracked dependencies (showing top ${Math.min(filtered.length, maxGaps)})`,
  };
}
