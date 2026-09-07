// SPDX-License-Identifier: Apache-2.0
/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps - dependencies with relevant content you haven't engaged with.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { DependencyWithProjectRow, SourceItemBriefRow } from "../types.js";
import { parseSemver } from "../live/semver-utils.js";
import { mapEcosystem } from "../live/version-resolver.js";
import {
  advisorySubject,
  gradeGap,
  isAdvisoryItem,
  mentionsPackage,
  parsePublishedAt,
  samePackage,
  versionInAnyRange,
  type AdvisoryRangeEvent,
  type Exposure,
} from "./knowledge-gap-grading.js";

// The grading rules live in knowledge-gap-grading.ts; re-exported for existing importers.
export {
  advisorySubject,
  gradeGap,
  registryVersionFromTitle,
  versionInAnyRange,
  type AdvisoryRangeEvent,
  type Exposure,
  type GradableItem,
} from "./knowledge-gap-grading.js";

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
  //
  // Scoped to the dependency's ecosystem when the table records one: the
  // sync stores the npm `jsonwebtoken` ([0, 9.0.0)) and the Rust crate
  // `jsonwebtoken` ([0, 10.3.0)) under one package name, and an unscoped
  // lookup graded the crate at 9.3.1 by the npm ranges.
  const hasOsvTable = db.hasColumn("osv_advisories", "affected_ranges");
  const hasOsvEcosystem = hasOsvTable && db.hasColumn("osv_advisories", "ecosystem");
  const advisoryRanges = hasOsvTable
    ? rawDb.prepare(
        `SELECT affected_ranges FROM osv_advisories
         WHERE lower(package_name) = lower(?) AND withdrawn_at IS NULL
         ${hasOsvEcosystem ? "AND ecosystem = ?" : ""}`,
      )
    : null;

  const exposureFor = (packageName: string, ecosystem: string, version: string | null): Exposure => {
    if (!advisoryRanges) return "unknown";
    let rows: Array<{ affected_ranges: string | null }>;
    try {
      rows = (
        hasOsvEcosystem ? advisoryRanges.all(packageName, ecosystem) : advisoryRanges.all(packageName)
      ) as Array<{ affected_ranges: string | null }>;
    } catch {
      return "unknown";
    }
    if (rows.length === 0) return "unknown"; // nothing known about this package

    const ranges: AdvisoryRangeEvent[][] = [];
    for (const row of rows) {
      if (!row.affected_ranges) continue;
      try {
        const parsed = JSON.parse(row.affected_ranges) as Array<{ events?: AdvisoryRangeEvent[] }>;
        for (const r of parsed) if (Array.isArray(r.events)) ranges.push(r.events);
      } catch {
        return "unknown"; // unreadable range — no claim
      }
    }
    if (ranges.length === 0) return "unknown";
    if (!version || parseSemver(version) === null) return "unknown";
    return versionInAnyRange(ranges, version) ? "exposed" : "safe";
  };
  const hasIsDirect = db.hasColumn("project_dependencies", "is_direct");

  // Direct dependencies only — a transitive dep's news is not the user's
  // reading backlog. Dev deps stay in: a vitest or eslint advisory is real.
  // Every direct dependency is scanned: a `LIMIT 100` here silently left the
  // 101st onward unexamined (143 direct deps live, 43 never looked at).
  const deps = rawDb
    .prepare(
      `SELECT package_name, version, project_path, language FROM project_dependencies ${hasIsDirect ? "WHERE is_direct = 1" : ""}`,
    )
    .all() as DependencyWithProjectRow[];

  if (deps.length === 0) {
    return {
      gaps: [],
      summary: "No project dependencies tracked. Add context directories to enable knowledge gap detection.",
    };
  }

  // Candidate source items, loaded ONCE and matched per dependency in JS.
  // Engagement is recorded by the app in interactions.item_id / .action_type
  // (the canonical columns; the older source_item_id / action columns are
  // unused), so the NOT-IN suppression must read those or it silently never
  // fires.
  //
  // Grounding (each guard killed an observed false-positive class):
  // - relevance_score >= 0.2: the scoring pipeline's above-noise band — drops
  //   off-topic chatter that merely contains the string (chocolate-bar class).
  // - 30-day window: a "gap" is something you MISSED, not archaeology — a
  //   2014 StackOverflow post is not missed intelligence.
  // - The real mention test is the word-boundary check below ("invite" must
  //   never evidence a vite gap); a cheap substring pre-check keeps the
  //   per-dependency pass fast.
  // - feed_relevant is deliberately NOT filtered here: an item the feed gate
  //   rejected can still be a legitimate unread dep mention — surfacing those
  //   is this tool's niche. The relevance floor already excludes noise.
  // - published_at is applied per dependency below, because its exemption
  //   depends on the dependency's installed version.
  type Candidate = SourceItemBriefRow & {
    content_type: string | null;
    relevance_score: number | null;
    published_at: string | null;
    content_head: string;
    haystack: string;
  };
  const candidates = (
    rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, ${hasContentType ? "si.content_type" : "NULL AS content_type"}, si.created_at,
               ${hasRelevance ? "si.relevance_score" : "NULL AS relevance_score"},
               ${hasPublishedAt ? "si.published_at" : "NULL AS published_at"},
               substr(COALESCE(si.content, ''), 1, 2000) AS content_head
        FROM source_items si
        WHERE si.created_at >= datetime('now', '-30 days')
        ${hasRelevance ? "AND si.relevance_score IS NOT NULL AND si.relevance_score >= 0.2" : ""}
        AND si.id NOT IN (SELECT item_id FROM interactions WHERE action_type IN ('click', 'save'))
        ORDER BY si.created_at DESC`)
      .all() as Omit<Candidate, "haystack">[]
  ).map((row) => ({ ...row, haystack: `${row.title || ""} ${row.content_head || ""}`.toLowerCase() }));

  // published_at guard: OSV/CVE backfills ingest decades-old advisories whose
  // created_at (discovery) is days old but whose published_at is ancient. A
  // 2021 advisory is not "missed intelligence" in 2026; keep NULL (many
  // sources never set it) and anything published recently. EXCEPT an advisory
  // the installed version is positively inside: a still-applying advisory is
  // missed intelligence no matter when it was published (live: the
  // jsonwebtoken crate advisory, published February, still open against
  // relay/'s 9.3.1 in September). "Unknown" exposure does not exempt — the
  // cut stays for advisories about the wrong ecosystem or an unpinned
  // version, which is where the false criticals came from.
  const publishedCutoff = Date.now() - 90 * 24 * 60 * 60 * 1000;
  const passesPublishedCut = (item: Candidate, exposure: Exposure): boolean => {
    const publishedAt = parsePublishedAt(item.published_at);
    if (publishedAt === null || publishedAt >= publishedCutoff) return true;
    return isAdvisoryItem(item) && exposure === "exposed";
  };

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

    // Word-boundary verification: the mention must be the package name as a
    // whole word in the title or the content head, not a substring. For deps
    // named by ordinary English words, the text must also carry an ecosystem
    // cue — "one-tap tower stacker" mentions the word, not the crate.
    //
    // An advisory row is a mention only when the dependency is its subject
    // package: a SurrealDB advisory that says "via URL path" is not `url`
    // intelligence, whatever its body goes on to mention.
    const isGenericName = GENERIC_WORD_DEPS.has(pkgKey);
    const spellings = [...new Set([pkgKey, pkgKey.replace(/_/g, "-"), pkgKey.replace(/-/g, "_")])];
    const mentioned = candidates.filter((item) => {
      if (!spellings.some((s) => item.haystack.includes(s))) return false;
      if (isAdvisoryItem(item)) {
        const subject = advisorySubject(item.title || "");
        if (subject !== null) return samePackage(subject, dep.package_name);
      }
      return (
        (mentionsPackage(item.title || "", dep.package_name) ||
          mentionsPackage(item.content_head || "", dep.package_name)) &&
        (!isGenericName || hasEcosystemCue(item))
      );
    });
    if (mentioned.length === 0) continue;

    const installedVersion = installedVersionFor(dep);
    const exposure = exposureFor(dep.package_name, mapEcosystem(dep.language || ""), installedVersion);
    const mentionedItems = mentioned
      .filter((item) => passesPublishedCut(item, exposure))
      .slice(0, 5);

    if (mentionedItems.length > 0) {
      const severity = gradeGap(
        mentionedItems,
        dep.package_name,
        exposure !== "safe",
        installedVersion,
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
