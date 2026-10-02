// SPDX-License-Identifier: Apache-2.0
/**
 * What this project's lockfiles hold for a package someone asked about by name.
 *
 * `vulnerability_scan` and `upgrade_planner` both take `package`. When either
 * has nothing to show, "no findings" must not read as "not a dependency": in
 * the 2026-10-03 agent eval the planner told an agent that `diff`, a
 * transitive devDependency with two advisories, was "not a dependency of this
 * project (or is a devDependency: pass include_dev)" while include_dev was set.
 */

import type { LiveIntelligence } from "../live/index.js";
import { namesPackage } from "../live/version-compare.js";

export interface PackagePresence {
  /** The registry name as the lockfile spells it, or null when no lockfile holds it. */
  name: string | null;
  versions: string[];
  scope: "direct" | "transitive" | null;
  /** Every copy is a direct devDependency (left out of runtime-scope scans and plans). */
  directDevOnly: boolean;
}

export function packagePresence(query: string, liveIntel: LiveIntelligence): PackagePresence {
  const copies = liveIntel.getAuditDeps().filter((d) => namesPackage(query, d.name, d.ecosystem));
  if (copies.length === 0) return { name: null, versions: [], scope: null, directDevOnly: false };
  return {
    name: copies[0].name,
    versions: [...new Set(copies.map((d) => d.version ?? "unknown"))],
    scope: copies.some((d) => d.isDirect) ? "direct" : "transitive",
    directDevOnly: copies.every((d) => d.isDirect && d.isDev),
  };
}

/**
 * One sentence for a `package` query that produced no rows.
 * `checked` says what an empty answer means for this tool ("no known advisories", "nothing to upgrade").
 */
export function emptyAnswerNote(query: string, liveIntel: LiveIntelligence, includeDev: boolean, checked: string): string {
  const p = packagePresence(query, liveIntel);
  if (!p.name) return `"${query}" is not in this project's lockfiles`;
  const held = `${p.name} ${p.versions.join(", ")} (${p.scope})`;
  if (!includeDev && p.directDevOnly) return `${held} is a direct devDependency, left out at this scope: pass include_dev: true`;
  return `${held}: ${checked}`;
}
