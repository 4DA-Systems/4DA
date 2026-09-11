// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap exposure: which stored advisories reach the installs a gap
 * names, and each advisory ROW's live verdict. The MCP twin of the app's
 * `osv::exposure::advisory_row_reaches` and `knowledge_decay::{
 * installs_still_vulnerable, affected_project_paths, advisory_tier_for}`
 * (AD-045: an advisory is judged against the install it names, in the
 * install's own ecosystem).
 *
 * The first cut read one package-wide verdict from every stored advisory at
 * once, so one unreadable row, one sibling project's version, or another
 * ecosystem's same-named package decided the gap for every project in it.
 */

import type BetterSqlite3 from "better-sqlite3";
import type { FourDADatabase } from "../db.js";
import { parseSemverPrecedence } from "../live/semver-precedence.js";
import { samePackage, type Exposure } from "./knowledge-gap-grading.js";
import { denotes, otherSpelling, sameEcosystem, type Install } from "./knowledge-gap-installs.js";
import {
  advisoryTier,
  moreSevereTier,
  parseAffectedRanges,
  rangesContain,
  type AdvisoryRangeEvent,
  type AdvisoryTier,
} from "./knowledge-gap-ranges.js";
import { allRows } from "./knowledge-gap-scope.js";

/** One stored advisory record for one (package, ecosystem). */
export interface AdvisoryEntry {
  packageName: string;
  /** Null when the table predates its ecosystem column. */
  ecosystem: string | null;
  /** Null when `affected_ranges` is missing or unreadable. */
  ranges: AdvisoryRangeEvent[][] | null;
  tier: AdvisoryTier | null;
}

/**
 * An advisory row's verdict against the installs its gap names.
 * - `reached`: some install sits inside one of the row's advisories.
 * - `clear`: every install is outside them — fixed at or below the installed
 *   version, or a package of the same name in another ecosystem.
 * - `unknown`: an install whose version (or the row's ranges) cannot be read.
 *   Kept conservatively: an unknown install stays exposed (AD-040 rule 3).
 */
export type RowVerdict = "reached" | "clear" | "unknown";

/**
 * An advisory counts against an install only for the install's own
 * ecosystem; when either side's ecosystem is unknown it is judged against
 * every record, as `advisory_row_reaches` does.
 */
function appliesTo(entry: AdvisoryEntry, install: Install): boolean {
  return entry.ecosystem === null || install.ecosystem === null || sameEcosystem(entry.ecosystem, install.ecosystem);
}

/** Inside the entry's readable ranges: true/false; null when either side cannot be read. */
function contains(entry: AdvisoryEntry, install: Install): boolean | null {
  if (entry.ranges === null || install.version === null) return null;
  return rangesContain(entry.ranges, install.version);
}

/**
 * One install against every stored advisory for its package, judged advisory
 * by advisory: exposed if any readable advisory of its ecosystem contains it,
 * safe if every readable one excludes it, unknown only when nothing readable
 * exists (or its version cannot be read). An unreadable row is simply not
 * evidence: it no longer turns a patched package into an unknown one, and
 * unknown into "vulnerable".
 */
export function installExposure(install: Install, entries: AdvisoryEntry[]): Exposure {
  if (install.version === null || parseSemverPrecedence(install.version) === null) return "unknown";
  const readable = entries.filter((e) => appliesTo(e, install) && e.ranges !== null);
  if (readable.length === 0) return "unknown";
  return readable.some((e) => contains(e, install) === true) ? "exposed" : "safe";
}

/** A gap's exposure: exposed if any install is, unknown if any install is, else safe. */
export function gapExposure(installs: Install[], entries: AdvisoryEntry[]): Exposure {
  const verdicts = installs.map((install) => installExposure(install, entries));
  if (verdicts.includes("exposed")) return "exposed";
  return verdicts.length === 0 || verdicts.includes("unknown") ? "unknown" : "safe";
}

/** An advisory row's entries against the installs its gap names (`advisory_row_reaches`). */
export function rowVerdict(entries: AdvisoryEntry[], installs: Install[]): RowVerdict {
  let unknown = installs.length === 0;
  for (const install of installs) {
    const applicable = entries.filter((e) => appliesTo(e, install));
    if (applicable.length === 0) continue; // another ecosystem's package of the same name
    let judged = false;
    for (const entry of applicable) {
      const inside = contains(entry, install);
      if (inside === true) return "reached";
      if (inside === false) judged = true;
    }
    if (!judged) unknown = true;
  }
  return unknown ? "unknown" : "clear";
}

/**
 * The tier of the most severe stored advisory that contains an install the
 * gap names, in that install's ecosystem (`knowledge_decay::advisory_tier_for`),
 * or null when none of the reaching advisories is graded.
 */
export function reachingTier(entries: AdvisoryEntry[], installs: Install[]): AdvisoryTier | null {
  let best: AdvisoryTier | null = null;
  for (const entry of entries) {
    if (entry.tier === null) continue;
    if (installs.some((install) => appliesTo(entry, install) && contains(entry, install) === true)) {
      best = moreSevereTier(best, entry.tier);
    }
  }
  return best;
}

interface AdvisoryRow {
  package_name: string;
  ecosystem: string | null;
  affected_ranges: string | null;
  cvss_score: number | null;
  severity_label: string | null;
}

function toEntry(row: AdvisoryRow): AdvisoryEntry {
  return {
    packageName: row.package_name,
    ecosystem: row.ecosystem,
    ranges: parseAffectedRanges(row.affected_ranges),
    tier: advisoryTier(row.cvss_score, row.severity_label),
  };
}

/** Read access to the OSV mirror (`osv_advisories`), feature-detected column by column. */
export class AdvisoryStore {
  private readonly byPackage: BetterSqlite3.Statement | null = null;
  private readonly byId: BetterSqlite3.Statement | null = null;
  private readonly idMatchesAliases: boolean = false;
  private readonly rowsById = new Map<string, AdvisoryRow[]>();

  constructor(db: FourDADatabase) {
    const has = (column: string) => db.hasColumn("osv_advisories", column);
    if (!has("package_name") || !has("affected_ranges")) return;
    const columns = [
      "package_name",
      has("ecosystem") ? "ecosystem" : "NULL AS ecosystem",
      "affected_ranges",
      has("cvss_score") ? "cvss_score" : "NULL AS cvss_score",
      has("severity_label") ? "severity_label" : "NULL AS severity_label",
    ].join(", ");
    const live = has("withdrawn_at") ? "AND withdrawn_at IS NULL" : "";
    const raw = db.getRawDb();
    try {
      this.byPackage = raw.prepare(
        `SELECT ${columns} FROM osv_advisories WHERE lower(package_name) IN (lower(?), lower(?)) ${live}`,
      );
      if (has("advisory_id")) {
        this.idMatchesAliases = has("aliases");
        const match = this.idMatchesAliases ? "(advisory_id = ? OR aliases LIKE ? ESCAPE '\\')" : "advisory_id = ?";
        this.byId = raw.prepare(`SELECT ${columns} FROM osv_advisories WHERE ${match} ${live}`);
      }
    } catch {
      // An unreadable mirror is no mirror: every verdict stays conservative.
    }
  }

  /** Every live advisory stored for this dependency, in any ecosystem. */
  forPackage(name: string): AdvisoryEntry[] {
    if (!this.byPackage) return [];
    return allRows<AdvisoryRow>(this.byPackage, name, otherSpelling(name))
      .filter((row) => denotes(row.package_name, row.ecosystem, name))
      .map(toEntry);
  }

  /**
   * The entries for dependency `name` that an advisory ROW stands for
   * (`osv::exposure::advisory_records`). An osv row carries its advisory id in
   * `source_items.source_id`; a cve row carries the CVE id, which the mirror
   * keeps in `aliases` (a JSON array). Names compare case-insensitively with
   * `-` equal to `_`. Null when the row resolves to no stored entry for this
   * dependency: the caller keeps its conservative fallback.
   */
  forRow(sourceId: string | null | undefined, name: string): AdvisoryEntry[] | null {
    const id = (sourceId ?? "").trim();
    if (!this.byId || id === "") return null;
    let rows = this.rowsById.get(id);
    if (!rows) {
      const args = this.idMatchesAliases ? [id, `%"${id.replace(/[\\%_]/g, "\\$&")}"%`] : [id];
      rows = allRows<AdvisoryRow>(this.byId, ...args);
      this.rowsById.set(id, rows);
    }
    const mine = rows.filter((row) => samePackage(row.package_name, name));
    return mine.length > 0 ? mine.map(toEntry) : null;
  }
}
