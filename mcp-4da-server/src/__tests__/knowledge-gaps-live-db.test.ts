// SPDX-License-Identifier: Apache-2.0
/**
 * Opt-in: run knowledge_gaps READ-ONLY against a real 4DA database and print
 * what it reports. Skipped unless FOURDA_VERIFY_DB names a database file:
 *
 *   FOURDA_VERIFY_DB=D:/4DA/data/4da.db FOURDA_VERIFY_ROOT=d:/4da \
 *     pnpm exec vitest run src/__tests__/knowledge-gaps-live-db.test.ts
 *
 * The connection is opened `readonly` and handed to the tool directly: the
 * FourDADatabase constructor sets a journal pragma, which is a write. The
 * live resolver is stood in for by the database's own lockfile table
 * (`user_dependencies`, direct rows, scoped to FOURDA_VERIFY_ROOT when set),
 * so no lockfile walk or `cargo` run touches the checkout being inspected.
 */
import { describe, it, expect } from "vitest";
import { writeFileSync } from "node:fs";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeKnowledgeGaps } from "../tools/knowledge-gaps.js";
import { mapEcosystem } from "../live/version-resolver.js";
import type { ResolvedDependency } from "../live/types.js";

const dbPath = process.env.FOURDA_VERIFY_DB;
const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase().replace(/\/+$/, "");
const root = norm(process.env.FOURDA_VERIFY_ROOT ?? "");

interface LockRow {
  project_path: string;
  package_name: string;
  version: string;
  ecosystem: string;
}

function lockfileStandIn(raw: Database.Database) {
  let rows: LockRow[] = [];
  try {
    rows = raw
      .prepare(
        "SELECT project_path, package_name, version, ecosystem FROM user_dependencies WHERE version IS NOT NULL AND version != '' AND is_direct = 1",
      )
      .all() as LockRow[];
  } catch {
    rows = [];
  }
  const deps = rows
    .filter((r) => !root || norm(r.project_path) === root || norm(r.project_path).startsWith(`${root}/`))
    .map(
      (r) =>
        ({
          name: r.package_name,
          version: r.version,
          ecosystem: mapEcosystem(r.ecosystem),
          sourceDirs: [r.project_path],
        }) as unknown as ResolvedDependency,
    );
  return { isInitialized: () => deps.length > 0, getResolvedDeps: () => deps };
}

describe.skipIf(!dbPath)("knowledge_gaps on a real database (opt-in, read-only)", () => {
  it("reports its gaps without writing", () => {
    const raw = new Database(dbPath as string, { readonly: true, fileMustExist: true });
    try {
      const db = Object.create(FourDADatabase.prototype) as FourDADatabase;
      (db as unknown as { db: Database.Database }).db = raw;
      const minSeverity = process.env.FOURDA_VERIFY_MIN_SEVERITY || "low";
      const result = executeKnowledgeGaps(db, { min_severity: minSeverity, limit: 50 }, lockfileStandIn(raw));
      const lines = (result.gaps ?? []).map((g) =>
        [
          `${g.gap_severity.padEnd(8)} ${g.dependency} @ ${g.version ?? "?"} (${g.language}) - ${g.project_path}`,
          ...g.missed_items.map((m) => `           [${m.id}] ${m.source_type}: ${m.title}`),
        ].join("\n"),
      );
      const report = `${result.summary}\n${lines.join("\n")}\n`;
      console.log(report);
      // Vitest can swallow a passing test's console; FOURDA_VERIFY_OUT keeps the report.
      if (process.env.FOURDA_VERIFY_OUT) writeFileSync(process.env.FOURDA_VERIFY_OUT, report);
      expect(Array.isArray(result.gaps)).toBe(true);
    } finally {
      raw.close();
    }
  });
});
