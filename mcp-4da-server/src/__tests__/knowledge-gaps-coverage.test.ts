// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for knowledge_gaps coverage and the advisory publish-date cut.
 *
 * Live 2026-09-07: `project_dependencies` held 143 distinct direct
 * dependencies and the tool scanned `LIMIT 100` of them — 43 were never
 * examined. In the same corpus the jsonwebtoken crate advisory (published
 * February, still open against relay/'s 9.3.1) was cut by the 90-day
 * published_at guard, and `osv_advisories` was consulted by package name
 * alone, so the Rust crate was graded by the npm package's ranges.
 *
 * This fixture's `osv_advisories` carries no CVSS score or severity label, so
 * every advisory here is ungraded: a still-reaching one is `high`, never
 * `critical` (AD-040 rule 2 — the tier comes from the advisory itself).
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeKnowledgeGaps } from "../tools/knowledge-gaps.js";

const SCHEMA = `
  CREATE TABLE source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, url TEXT, source_type TEXT,
    content TEXT, content_type TEXT, created_at TEXT, published_at TEXT, relevance_score REAL
  );
  CREATE TABLE project_dependencies (
    package_name TEXT, version TEXT, project_path TEXT, language TEXT,
    is_direct INTEGER DEFAULT 1, is_dev INTEGER DEFAULT 0
  );
  CREATE TABLE interactions (item_id INTEGER, action_type TEXT);
  CREATE TABLE osv_advisories (package_name TEXT, ecosystem TEXT, affected_ranges TEXT, withdrawn_at TEXT);
`;

function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.exec(SCHEMA);
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

describe("knowledge_gaps — coverage and advisory freshness", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  const addDep = (name: string, version: string | null, language: string) =>
    db
      .getRawDb()
      .prepare("INSERT INTO project_dependencies (package_name, version, project_path, language) VALUES (?, ?, 'd:/proj', ?)")
      .run(name, version, language);

  const addItem = (over: {
    title: string;
    source_type: string;
    daysAgo?: number;
    published?: string | null;
    relevance?: number;
  }) =>
    db
      .getRawDb()
      .prepare(
        `INSERT INTO source_items (title, url, source_type, content, created_at, published_at, relevance_score)
         VALUES (?, 'https://x', ?, ?, datetime('now', ?), ?, ?)`,
      )
      .run(over.title, over.source_type, over.title, `-${over.daysAgo ?? 2} days`, over.published ?? null, over.relevance ?? 0.9);

  const addAdvisory = (pkg: string, ecosystem: string, ranges: string) =>
    db
      .getRawDb()
      .prepare("INSERT INTO osv_advisories (package_name, ecosystem, affected_ranges, withdrawn_at) VALUES (?, ?, ?, NULL)")
      .run(pkg, ecosystem, ranges);

  it("scans every direct dependency, not the first hundred", () => {
    for (let i = 1; i <= 120; i++) addDep(`pkg-${String(i).padStart(3, "0")}`, "1.0.0", "javascript");
    addItem({ title: "pkg-115 2.0.0 released with breaking changes", source_type: "hackernews" });

    const result = executeKnowledgeGaps(db, { min_severity: "low" });

    expect(result.total_dependencies).toBe(120);
    expect(result.gaps?.map((g) => g.dependency)).toContain("pkg-115");
  });

  it("keeps an old-published advisory the installed version is positively inside", () => {
    // Published five years ago, discovered this week, and 4.12.0 is still in
    // [3.8.0, 4.12.34): missed intelligence no matter the publish date.
    addDep("hono", "4.12.0", "javascript");
    addAdvisory("hono", "npm", '[{"events":[{"introduced":"3.8.0"},{"fixed":"4.12.34"}]}]');
    addItem({ title: "[GHSA-f23p-vx2j-j53r] hono: memo() retains SSR output across requests", source_type: "osv", published: "2020-09-04 15:00:00" });

    const result = executeKnowledgeGaps(db, {});
    const gap = result.gaps?.find((g) => g.dependency === "hono");

    expect(gap).toBeDefined();
    // Ungraded here; the real GHSA-f23p is CVSS 4.8 (medium), which is `high` too.
    expect(gap!.gap_severity).toBe("high");
    expect(gap!.missed_items[0].title).toContain("GHSA-f23p");
  });

  it("still cuts an old-published advisory the installed version is past", () => {
    addDep("hono", "4.13.2", "javascript");
    addAdvisory("hono", "npm", '[{"events":[{"introduced":"3.8.0"},{"fixed":"4.12.34"}]}]');
    addItem({ title: "[GHSA-f23p-vx2j-j53r] hono: memo() retains SSR output across requests", source_type: "osv", published: "2020-09-04 15:00:00" });

    const result = executeKnowledgeGaps(db, { min_severity: "low" });
    expect(result.gaps?.find((g) => g.dependency === "hono")).toBeUndefined();
  });

  it("does not exempt an old-published advisory on unknown exposure", () => {
    // No osv_advisories rows: nothing positive is known, so the cut stands —
    // this is where the `url`-evidenced-by-axios class of false critical lives.
    addDep("hono", "4.12.0", "javascript");
    addItem({ title: "[GHSA-f23p-vx2j-j53r] hono: memo() retains SSR output across requests", source_type: "osv", published: "2020-09-04 15:00:00" });

    const result = executeKnowledgeGaps(db, { min_severity: "low" });
    expect(result.gaps?.find((g) => g.dependency === "hono")).toBeUndefined();
  });

  it("consults advisory ranges for the dependency's own ecosystem", () => {
    // The live collision: npm `jsonwebtoken` is fixed at 9.0.0, the Rust crate
    // at 10.3.0. The crate at 9.3.1 must be graded by the crates.io ranges.
    addDep("jsonwebtoken", "9.3.1", "rust");
    addAdvisory("jsonwebtoken", "npm", '[{"events":[{"introduced":"0"},{"fixed":"9.0.0"}]}]');
    addAdvisory("jsonwebtoken", "crates.io", '[{"events":[{"introduced":"0"},{"fixed":"10.3.0"}]}]');
    addItem({
      title: "[GHSA-h395-gr6q-cpjc] jsonwebtoken: jsonwebtoken has Type Confusion that leads to potential authorization bypass",
      source_type: "osv",
      published: "2026-02-03 18:47:40",
    });

    const result = executeKnowledgeGaps(db, {});
    const gap = result.gaps?.find((g) => g.dependency === "jsonwebtoken");
    expect(gap).toBeDefined();
    // Exposed, at an ungraded tier: `high`. The live advisory is medium, and
    // the app grades this gap High; it was `critical` here (measured 2026-09-11).
    expect(gap!.gap_severity).toBe("high");
  });

  it("never grades a crate safe on another ecosystem's ranges", () => {
    // Only the npm ranges are stored; for the Rust crate that is no data,
    // and no data keeps the conservative security grade for a recent advisory.
    addDep("jsonwebtoken", "9.3.1", "rust");
    addAdvisory("jsonwebtoken", "npm", '[{"events":[{"introduced":"0"},{"fixed":"9.0.0"}]}]');
    addItem({
      title: "[GHSA-h395-gr6q-cpjc] jsonwebtoken: Type Confusion leads to authorization bypass",
      source_type: "osv",
      daysAgo: 2,
      published: null,
    });

    const result = executeKnowledgeGaps(db, {});
    const gap = result.gaps?.find((g) => g.dependency === "jsonwebtoken");
    expect(gap).toBeDefined();
    expect(gap!.gap_severity).toBe("high");
  });

  it("an advisory about another package is not a mention of a generic-word dependency", () => {
    // Live: the `url` crate's missed_items carried a SurrealDB advisory
    // ("... via URL path") and graded critical on it.
    addDep("url", "2.5.8", "rust");
    addItem({
      title: "[CVE-2026-63735] SurrealDB: Custom API route lets authenticated callers override namespace/database scope via URL path",
      source_type: "cve",
      published: null,
    });

    const result = executeKnowledgeGaps(db, { min_severity: "low" });
    expect(result.gaps?.find((g) => g.dependency === "url")).toBeUndefined();
  });

  it("a registry row for the installed version stays below the default floor", () => {
    addDep("@tauri-apps/api", "2.11.1", "javascript");
    addItem({ title: "npm: @tauri-apps/api v2.11.1", source_type: "npm_registry", relevance: 0.37 });

    const result = executeKnowledgeGaps(db, {});
    expect(result.gaps?.find((g) => g.dependency === "@tauri-apps/api")).toBeUndefined();
  });

  it("surfaces an unread registry release of a direct dependency at the default floor", () => {
    addDep("serde", "1.0.210", "rust");
    addItem({ title: "crates.io: serde v1.0.220", source_type: "crates_io", relevance: 0.37 });

    const result = executeKnowledgeGaps(db, {});
    const gap = result.gaps?.find((g) => g.dependency === "serde");
    expect(gap).toBeDefined();
    expect(gap!.gap_severity).toBe("medium");
  });
});
