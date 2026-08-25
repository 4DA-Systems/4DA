// SPDX-License-Identifier: Apache-2.0
/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps - dependencies with relevant content you haven't engaged with.
 */

import type { FourDADatabase } from "../db.js";
import type { DependencyWithProjectRow, SourceItemBriefRow } from "../types.js";

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
export function gradeGap(items: GradableItem[], packageName: string): string {
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

  if (items.some((item) => isAdvisory(item) && namesDep(item))) return "critical";
  if (items.some((item) => namesDep(item) && securityKeywords(item.title || ""))) return "high";
  if (items.some((item) => namesDep(item) && carriesConsequence(item.title || ""))) return "medium";
  return "low";
}

// Escape SQL LIKE wildcards so a package name containing % or _ (npm names may
// contain _) is matched literally rather than as a pattern. Paired with ESCAPE '\'.
function escapeLike(s: string): string {
  return s.replace(/[\\%_]/g, (c) => "\\" + c);
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
  version: string;
  project_path: string;
  language: string;
  missed_items: SourceItemBriefRow[];
  gap_severity: string;
  missed_count: number;
}

export function executeKnowledgeGaps(
  db: FourDADatabase,
  params: KnowledgeGapsParams,
) {
  const rawDb = db.getRawDb();

  // Feature-detect optional columns: a pure-standalone database has no scoring
  // pipeline (no relevance_score) and may predate content_type — the tool must
  // degrade to word-boundary + recency grounding there, never throw.
  const hasRelevance = db.hasColumn("source_items", "relevance_score");
  const hasContentType = db.hasColumn("source_items", "content_type");
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
    const candidates = rawDb
      .prepare(`SELECT si.id, si.title, si.url, si.source_type, ${hasContentType ? "si.content_type" : "NULL AS content_type"}, si.created_at,
               ${hasRelevance ? "si.relevance_score" : "NULL AS relevance_score"}, substr(COALESCE(si.content, ''), 1, 2000) AS content_head
        FROM source_items si
        WHERE (si.title LIKE ? ESCAPE '\\' OR si.content LIKE ? ESCAPE '\\')
        ${hasRelevance ? "AND si.relevance_score IS NOT NULL AND si.relevance_score >= 0.2" : ""}
        AND si.created_at >= datetime('now', '-30 days')
        AND si.id NOT IN (SELECT item_id FROM interactions WHERE action_type IN ('click', 'save'))
        ORDER BY si.created_at DESC LIMIT 25`)
      .all(pattern, pattern) as Array<SourceItemBriefRow & {
        content_type: string | null;
        relevance_score: number | null;
        content_head: string;
      }>;

    // Word-boundary verification: the mention must be the package name as a
    // whole word in the title or the content head, not a substring.
    const mentionedItems = candidates
      .filter(
        (item) =>
          mentionsPackage(item.title || "", dep.package_name) ||
          mentionsPackage(item.content_head || "", dep.package_name),
      )
      .slice(0, 5);

    if (mentionedItems.length > 0) {
      const severity = gradeGap(mentionedItems, dep.package_name);

      gaps.push({
        dependency: dep.package_name,
        version: dep.version,
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
