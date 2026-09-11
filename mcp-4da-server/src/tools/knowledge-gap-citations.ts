// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap citations: which unread items are evidence about which
 * dependency. Mirrors `knowledge_decay::{load_gap_candidates,
 * keyword_misses_from, normalize_gap_title}` in the desktop app.
 */

import type { FourDADatabase } from "../db.js";
import type { SourceItemBriefRow } from "../types.js";
import { advisorySubject, isAdvisoryRow, mentionsPackage, samePackage } from "./knowledge-gap-grading.js";
import type { LinkerIndex } from "./knowledge-gap-scope.js";

/** An unread item eligible to become a missed signal. */
export type GapCandidate = SourceItemBriefRow & {
  content_type: string | null;
  relevance_score: number | null;
  published_at: string | null;
  /** osv rows: the advisory id; cve rows: the CVE id. */
  source_id: string | null;
  content_head: string;
  title_lower: string;
};

/**
 * Content types the app never counts as missed intelligence
 * (`load_gap_candidates`): someone else's project, a tutorial, a question.
 */
const NON_GAP_CONTENT_TYPES = ["show_and_tell", "tutorial", "question", "help_request", "hiring", "clickbait"];

/**
 * Every unread candidate item, loaded ONCE and matched per dependency.
 * Grounding (each guard killed an observed false-positive class):
 * - relevance_score >= 0.2, the scoring pipeline's above-noise band, when the
 *   database has a scoring pipeline at all;
 * - a 30-day window: a gap is something you MISSED, not archaeology;
 * - engagement: an item clicked or saved here (`interactions.item_id` /
 *   `.action_type`, the canonical columns) or given feedback in the app;
 * - the content types above, as the app excludes them.
 * Optional columns are feature-detected: a standalone database may lack the
 * scoring pipeline, and the tool must degrade, never throw.
 */
export function loadCandidates(db: FourDADatabase): GapCandidate[] {
  const has = (column: string) => db.hasColumn("source_items", column);
  const hasContentType = has("content_type");
  const hasRelevance = has("relevance_score");
  const engaged =
    db.hasColumn("interactions", "item_id") && db.hasColumn("interactions", "action_type")
      ? "AND si.id NOT IN (SELECT item_id FROM interactions WHERE item_id IS NOT NULL AND action_type IN ('click', 'save'))"
      : "";
  const judged = db.hasColumn("feedback", "source_item_id")
    ? "AND si.id NOT IN (SELECT source_item_id FROM feedback WHERE source_item_id IS NOT NULL)"
    : "";
  const excludedTypes = NON_GAP_CONTENT_TYPES.map((t) => `'${t}'`).join(", ");
  const rows = db
    .getRawDb()
    .prepare(
      `SELECT si.id, si.title, si.url, si.source_type, si.created_at,
              ${hasContentType ? "si.content_type" : "NULL AS content_type"},
              ${hasRelevance ? "si.relevance_score" : "NULL AS relevance_score"},
              ${has("published_at") ? "si.published_at" : "NULL AS published_at"},
              ${has("source_id") ? "si.source_id" : "NULL AS source_id"},
              substr(COALESCE(si.content, ''), 1, 2000) AS content_head
         FROM source_items si
        WHERE si.created_at >= datetime('now', '-30 days')
          ${hasRelevance ? "AND si.relevance_score IS NOT NULL AND si.relevance_score >= 0.2" : ""}
          ${hasContentType ? `AND (si.content_type IS NULL OR si.content_type NOT IN (${excludedTypes}))` : ""}
          ${engaged}
          ${judged}
        ORDER BY si.created_at DESC, si.id DESC`,
    )
    .all() as Array<Omit<GapCandidate, "title_lower">>;
  return rows.map((row) => ({ ...row, title_lower: (row.title || "").toLowerCase() }));
}

/**
 * Dependency names that are ordinary English words. A word-boundary match on
 * these is no evidence the item is about the PACKAGE: live 2026-08-30, a
 * `tower` gap was evidenced by a one-tap tower-stacker game and a drivable-car
 * post. For these names the item must also carry an ecosystem cue.
 */
const GENERIC_WORD_DEPS = new Set([
  "tower", "base64", "image", "time", "rand", "log", "tracing", "url", "zip",
  "tar", "glob", "regex", "chrono", "notify", "either", "bytes", "flate",
]);

// A token that ties the text to software packaging rather than the English word:
// an ecosystem noun, a security noun, or a version number.
const ECOSYSTEM_CUE =
  /\b(crate|crates\.io|cargo|rust|npm|node|package|library|dependenc|version|release[ds]?|upgrad|deprecat|cve|advisory|vulnerab)|\bv?\d+\.\d+/i;

function hasEcosystemCue(item: GapCandidate): boolean {
  return ECOSYSTEM_CUE.test(`${item.title || ""} ${item.content_head || ""}`);
}

/**
 * Does this unread item cite `packageName`?
 *
 * An advisory row (osv/cve) cites a dependency only through the dependency
 * linker's structured proof when the linker table exists, and otherwise only
 * when the package is the advisory's SUBJECT ("[ID] package: ..."). Never
 * through a word in its title: "[CVE-2026-63642] MagicMirror newsfeed
 * Socket.IO notification allows blind server-side request forgery" has no
 * linker row and no parseable subject, and it minted a critical socket.io gap
 * on the word "Socket.IO" (measured 2026-09-11).
 *
 * Every other row cites a dependency when its TITLE names it at a word
 * boundary, as the app matches titles only. The body does not count: four
 * reposts of "I built 59 free browser-based dev tools in vanilla JS" evidenced
 * a critical `rsa` gap through a "RSA & ECC Key Generator" line 1,330
 * characters into the post. A package named by an ordinary English word also
 * needs an ecosystem cue, which may come from the body.
 */
export function citesDependency(item: GapCandidate, packageName: string, linker: LinkerIndex | null): boolean {
  if (isAdvisoryRow(item)) {
    if (linker) return linker.itemsFor(packageName).has(item.id);
    const subject = advisorySubject(item.title || "");
    return subject !== null && samePackage(subject, packageName);
  }
  if (!item.title_lower.includes(packageName.toLowerCase())) return false;
  if (!mentionsPackage(item.title || "", packageName)) return false;
  return !GENERIC_WORD_DEPS.has(packageName.toLowerCase()) || hasEcosystemCue(item);
}

/**
 * `knowledge_decay::normalize_gap_title`: lowercase, punctuation stripped,
 * first ten words. Reposts of one story share it.
 */
export function normalizeGapTitle(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^\p{Alphabetic}\p{N}\s]/gu, "")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 10)
    .join(" ");
}
