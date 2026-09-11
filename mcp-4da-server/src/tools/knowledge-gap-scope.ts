// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap scope: which projects count as active, and which items the
 * dependency linker bound to which packages. Mirrors the desktop app's
 * `temporal::{active_repo_roots, dep_within_active_root, shares_active_root}`
 * and `knowledge_decay::{get_active_project_paths, linked_to}`.
 */

import type BetterSqlite3 from "better-sqlite3";
import type { FourDADatabase } from "../db.js";

/** `stmt.all(...)`, or [] when the statement cannot run: a read here never throws out of the tool. */
export function allRows<T>(stmt: BetterSqlite3.Statement, ...args: unknown[]): T[] {
  try {
    return stmt.all(...args) as T[];
  } catch {
    return [];
  }
}

/** Lowercase, forward slashes, no trailing slash (`knowledge_decay::normalize_project_path`). */
export function normPath(p: string): string {
  return p.replace(/\\/g, "/").toLowerCase().replace(/\/+$/, "");
}

/**
 * Repo roots with git activity in the last `days` days, normalized. The
 * `git_signals.repo_path` column is stored raw ("D:\4DA"). Empty when the
 * table is absent or silent, which every caller reads as "unscoped".
 */
function activeRoots(db: FourDADatabase, days: number, requireCommit: boolean): string[] {
  if (!db.hasColumn("git_signals", "repo_path") || !db.hasColumn("git_signals", "timestamp")) return [];
  const commit =
    requireCommit && db.hasColumn("git_signals", "commit_hash")
      ? "AND commit_hash IS NOT NULL AND commit_hash != ''"
      : "";
  try {
    const rows = db
      .getRawDb()
      .prepare(`SELECT DISTINCT repo_path FROM git_signals WHERE timestamp > datetime('now', ?) ${commit}`)
      .all(`-${days} days`) as Array<{ repo_path: string | null }>;
    return [...new Set(rows.map((r) => normPath(r.repo_path ?? "")).filter((p) => p !== ""))];
  } catch {
    return [];
  }
}

/**
 * Path-BOUNDARY match in either direction: `d:/4da` covers `d:/4da/src-tauri`
 * (and a manifest recorded above its repo root still counts), but never
 * `d:/4da-experiments` (`temporal::dep_within_active_root`).
 */
export function withinRoots(path: string, roots: readonly string[]): boolean {
  const p = normPath(path);
  if (p === "") return false;
  return roots.some((root) => p === root || p.startsWith(`${root}/`) || root.startsWith(`${p}/`));
}

/** Do two project paths sit under one common active root (`temporal::shares_active_root`)? */
export function sharesRoot(a: string, b: string, roots: readonly string[]): boolean {
  return roots.some((root) => withinRoots(a, [root]) && withinRoots(b, [root]));
}

/** The app's two activity windows, loaded once per call. */
export interface ActiveScope {
  /** `temporal::active_repo_roots`: commits in the last 60 days. Scopes each project ROW. */
  rowRoots: string[];
  /** `knowledge_decay::get_active_project_paths`: activity in the last 30 days. Scopes each DEPENDENCY. */
  dependencyRoots: string[];
}

export function loadActiveScope(db: FourDADatabase): ActiveScope {
  return { rowRoots: activeRoots(db, 60, true), dependencyRoots: activeRoots(db, 30, false) };
}

/**
 * The project rows the app's dependency funnel keeps before knowledge decay
 * ever sees them (`temporal::get_all_dependencies`): rows inside an active repo
 * root. Without it a gap for a dependency the active repo shares with a
 * dormant one is judged on the dormant copy too — vitest in `d:/4da` (4.1.11)
 * beside navcal's untouched 3.2.4 would name navcal under a critical the app
 * never shows. No recorded git activity means unscoped, and a scope that
 * matches no row at all keeps every row rather than blanking the tool.
 */
export function scopeRows<T extends { project_path: string }>(rows: T[], scope: ActiveScope): T[] {
  if (scope.rowRoots.length === 0) return rows;
  const kept = rows.filter((r) => withinRoots(r.project_path, scope.rowRoots));
  return kept.length > 0 ? kept : rows;
}

/**
 * `knowledge_decay::detect_knowledge_gaps` active-project scoping: a
 * dependency whose declaring projects all sit outside every recently active
 * path is skipped. Measured 2026-09-11: axios and socket.io were reported
 * critical from `kairos-mvp/backend`, a project untouched since 2025-10.
 */
export function dependencyIsActive(rows: ReadonlyArray<{ project_path: string }>, scope: ActiveScope): boolean {
  return scope.dependencyRoots.length === 0 || rows.some((r) => withinRoots(r.project_path, scope.dependencyRoots));
}

/** Items the dependency linker bound to a package with STRUCTURED proof. */
export interface LinkerIndex {
  /** Ids of the items bound to this package (case- and `-`/`_`-folded). */
  itemsFor(packageName: string): ReadonlySet<number>;
}

const foldName = (name: string) => name.toLowerCase().replace(/_/g, "-");
const NO_ITEMS: ReadonlySet<number> = new Set<number>();

/**
 * `source_item_dependencies` rows carrying structured proof: `advisory` (the
 * advisory's own affected-package list) or `exact_registry`. A registry
 * advisory cites a dependency only through this proof, never through a title
 * word (#618; `knowledge_decay::linked_to`). Null when the table or its
 * `match_type` column is absent: callers then fall back to the advisory's
 * subject package, since heuristic rows cannot be told apart from proof.
 */
export function loadLinkerIndex(db: FourDADatabase): LinkerIndex | null {
  const has = (column: string) => db.hasColumn("source_item_dependencies", column);
  if (!has("source_item_id") || !has("package_name") || !has("match_type")) return null;
  const byPackage = new Map<string, Set<number>>();
  try {
    const rows = db
      .getRawDb()
      .prepare(
        "SELECT source_item_id, package_name FROM source_item_dependencies WHERE match_type IN ('advisory', 'exact_registry')",
      )
      .all() as Array<{ source_item_id: number; package_name: string | null }>;
    for (const row of rows) {
      if (!row.package_name) continue;
      const key = foldName(row.package_name);
      const ids = byPackage.get(key) ?? new Set<number>();
      ids.add(row.source_item_id);
      byPackage.set(key, ids);
    }
  } catch {
    return null;
  }
  return { itemsFor: (name) => byPackage.get(foldName(name)) ?? NO_ITEMS };
}
