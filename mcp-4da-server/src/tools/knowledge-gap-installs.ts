// SPDX-License-Identifier: Apache-2.0
/**
 * Knowledge-gap installs: the install each declaring project carries, with
 * its ecosystem (AD-045: a package is (ecosystem, name)). The MCP twin of the
 * version join in `temporal::get_all_dependencies` (tiers i and ii),
 * `knowledge_decay::installs_for` and `osv::exposure::canonical`.
 *
 * The first cut resolved ONE version per package name (first seen across the
 * machine) and displayed it beside whichever project row came first, so a gap
 * could judge one project's install and name another's.
 */

import type BetterSqlite3 from "better-sqlite3";
import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { ResolvedDependency } from "../live/types.js";
import { parseSemverPrecedence } from "../live/semver-precedence.js";
import { mapEcosystem } from "../live/version-resolver.js";
import { samePackage } from "./knowledge-gap-grading.js";
import { allRows, normPath, sharesRoot } from "./knowledge-gap-scope.js";

/** A `project_dependencies` row as the gap walk reads it. */
export interface DeclaringRow {
  package_name: string;
  version: string | null;
  project_path: string;
  language: string;
}

/** One declaring project's install of a dependency. */
export interface Install {
  projectPath: string;
  language: string;
  /** The OSV ecosystem ("npm", "crates.io", ...); null when the label names none the app knows. */
  ecosystem: string | null;
  version: string | null;
}

const NPM_LABELS = new Set(["npm", "javascript", "typescript", "node", "js", "ts"]);

/**
 * The OSV ecosystem a `project_dependencies.language` or
 * `user_dependencies.ecosystem` label names, or null when it names none
 * (`osv::exposure::canonical`). `mapEcosystem` falls back to npm for a label
 * it does not know; that fallback must not decide which advisories apply.
 */
export function osvEcosystem(label: string | null | undefined): string | null {
  const lower = (label ?? "").trim().toLowerCase();
  const mapped = mapEcosystem(lower);
  return mapped === "npm" && !NPM_LABELS.has(lower) ? null : mapped;
}

/** The family a lockfile join compares: an unknown label is its own family (`temporal::ecosystem_family`). */
function ecosystemFamily(label: string | null | undefined): string {
  return osvEcosystem(label) ?? `other:${(label ?? "").trim().toLowerCase()}`;
}

export const sameEcosystem = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Does a stored name denote this dependency? crates.io treats `-` and `_` as one namespace. */
export function denotes(storedName: string, ecosystem: string | null, depName: string): boolean {
  if (storedName.toLowerCase() === depName.toLowerCase()) return true;
  return ecosystem !== null && sameEcosystem(ecosystem, "crates.io") && samePackage(storedName, depName);
}

/** The other crates.io spelling of a name (`http_body_util` / `http-body-util`). */
export const otherSpelling = (name: string) =>
  name.includes("_") ? name.replace(/_/g, "-") : name.replace(/-/g, "_");

interface LockRow {
  project_path: string;
  package_name: string;
  version: string;
  ecosystem: string;
}

/**
 * Resolves the install each declaring project carries: the manifest's own
 * version when it is a real version; else the lockfile-resolved one in
 * `user_dependencies` (the same project and ecosystem family, then a lockfile
 * under the same active repo root — a workspace member resolved by the
 * workspace lockfile); else the live resolver's version for that project.
 * Never across repo roots or ecosystems: a project is never judged on a
 * sibling's version.
 */
export class InstallResolver {
  private readonly lockStmt: BetterSqlite3.Statement | null = null;
  private readonly lockRowsByName = new Map<string, LockRow[]>();
  private readonly resolved: ResolvedDependency[];
  private readonly activeRoots: readonly string[];

  constructor(
    db: FourDADatabase,
    liveIntel: Pick<LiveIntelligence, "getResolvedDeps" | "isInitialized"> | null | undefined,
    activeRoots: readonly string[],
  ) {
    this.activeRoots = activeRoots;
    this.resolved = liveIntel && liveIntel.isInitialized() ? liveIntel.getResolvedDeps() : [];
    const has = (column: string) => db.hasColumn("user_dependencies", column);
    if (!["project_path", "package_name", "version", "ecosystem"].every(has)) return;
    const order = [has("is_direct") ? "is_direct DESC" : "", has("last_seen_at") ? "last_seen_at DESC" : ""]
      .filter(Boolean)
      .join(", ");
    try {
      this.lockStmt = db.getRawDb().prepare(
        `SELECT project_path, package_name, version, ecosystem FROM user_dependencies
         WHERE lower(package_name) IN (lower(?), lower(?)) AND version IS NOT NULL AND version != ''
         ${order ? `ORDER BY ${order}` : ""}`,
      );
    } catch {
      // No readable lockfile table: fall through to the resolver.
    }
  }

  /** One install per declaring project and ecosystem. */
  installsFor(rows: DeclaringRow[]): Install[] {
    const seen = new Set<string>();
    const installs: Install[] = [];
    for (const row of rows) {
      const key = `${normPath(row.project_path)}\0${ecosystemFamily(row.language)}`;
      if (seen.has(key)) continue;
      seen.add(key);
      installs.push({
        projectPath: row.project_path,
        language: row.language,
        ecosystem: osvEcosystem(row.language),
        version: this.versionOf(row),
      });
    }
    return installs;
  }

  private versionOf(row: DeclaringRow): string | null {
    // A manifest records a RANGE when it records anything ("^4.1.5"); only a
    // real version is an install.
    if (row.version && parseSemverPrecedence(row.version) !== null) return row.version;
    const family = ecosystemFamily(row.language);
    const locked = this.lockRows(row.package_name).filter(
      (l) => ecosystemFamily(l.ecosystem) === family && denotes(l.package_name, family, row.package_name),
    );
    const here = normPath(row.project_path);
    const own = locked.find((l) => normPath(l.project_path) === here);
    if (own) return own.version;
    const workspace = locked.find((l) => sharesRoot(row.project_path, l.project_path, this.activeRoots));
    if (workspace) return workspace.version;
    return this.fromResolver(row);
  }

  private lockRows(name: string): LockRow[] {
    if (!this.lockStmt) return [];
    const key = name.toLowerCase();
    let rows = this.lockRowsByName.get(key);
    if (!rows) {
      rows = allRows<LockRow>(this.lockStmt, name, otherSpelling(name));
      this.lockRowsByName.set(key, rows);
    }
    return rows;
  }

  /**
   * The live resolver's version for THIS project: the resolution whose
   * manifest directory is the project. Without a lockfile table (standalone
   * mode, one project) the resolver's version for the package is the install.
   */
  private fromResolver(row: DeclaringRow): string | null {
    const ecosystem = osvEcosystem(row.language);
    if (ecosystem === null) return null;
    const matches = this.resolved.filter(
      (d) => d.version && sameEcosystem(d.ecosystem, ecosystem) && denotes(d.name, ecosystem, row.package_name),
    );
    const here = normPath(row.project_path);
    const own = matches.find((d) => (d.sourceDirs ?? []).some((dir) => normPath(dir) === here));
    if (own) return own.version;
    return this.lockStmt === null ? (matches[0]?.version ?? null) : null;
  }
}
