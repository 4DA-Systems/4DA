// SPDX-License-Identifier: Apache-2.0
/**
 * Advisory-database rows in the 4DA feed that name a package a task touches.
 *
 * Only the feed's advisory sources count (`cve`, `osv`): their titles name the
 * affected package. A headline that happens to contain a package name is not
 * an advisory — that is how OpenAI-the-company news became "Critical: Security
 * issue affects openai" (2026-10-01). Rows the relevance judge rejected
 * (`feed_relevant = 0`) are left out. These rows are NOT matched to installed
 * versions; the briefing says so, and with a ready scan keeps them only for
 * packages the scan confirms vulnerable.
 */

import type { FourDADatabase } from "../db.js";
import { normalizeStoredPriority } from "./signal-classifier.js";
import type { TaskPackage } from "./briefing-task-scope.js";

export interface StoredAdvisory {
  package: string;
  title: string;
  url: string | null;
  priority: string;
}

const ADVISORY_SOURCES = ["cve", "osv"];

export function storedAdvisoriesFor(db: FourDADatabase, packages: TaskPackage[], limit = 5): StoredAdvisory[] {
  if (packages.length === 0) return [];
  const raw = db.getRawDb();
  const hasFeedRelevant = (() => {
    try {
      return (raw.prepare("PRAGMA table_info(source_items)").all() as Array<{ name: string }>).some((c) => c.name === "feed_relevant");
    } catch {
      return false;
    }
  })();
  const out: StoredAdvisory[] = [];
  for (const pkg of packages) {
    let rows: Array<{ title: string; url: string | null; signal_priority: string | null }>;
    try {
      rows = raw
        .prepare(
          `SELECT title, url, signal_priority FROM source_items
           WHERE source_type IN (${ADVISORY_SOURCES.map(() => "?").join(", ")})
             AND created_at > datetime('now', '-90 days')
             AND LOWER(title) LIKE ?
             ${hasFeedRelevant ? "AND (feed_relevant IS NULL OR feed_relevant = 1)" : ""}
           ORDER BY created_at DESC LIMIT 20`,
        )
        .all(...ADVISORY_SOURCES, `%${pkg.name.toLowerCase()}%`) as typeof rows;
    } catch {
      continue;
    }
    const word = new RegExp(`(^|[^\\w@/-])${pkg.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![\\w-])`, "i");
    for (const row of rows) {
      if (!word.test(row.title)) continue;
      out.push({
        package: pkg.name,
        title: row.title,
        url: row.url,
        priority: row.signal_priority ? normalizeStoredPriority(row.signal_priority) : "medium",
      });
      if (out.length >= limit) return out;
    }
  }
  return out;
}
