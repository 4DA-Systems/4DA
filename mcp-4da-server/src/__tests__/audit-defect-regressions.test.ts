// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for the 2026-08-30 live-audit defects.
 *
 * Each block pins a defect observed against the live database that night:
 * 1. upgrade_planner attached a transitive instance's CVE to the same-named
 *    direct dep at a NON-affected version (anyhow 1.0.104 charged with
 *    RUSTSEC-2026-0190, fixed in 1.0.103), while hiding the actually
 *    vulnerable transitive instance (1.0.102).
 * 2. Cached OSV entries served yesterday's dep context: `glib` stayed
 *    platform-active on Windows and `sandbox` lost its source dirs for a full
 *    cache TTL after the resolver was fixed.
 * 3. Registry records cached per NAME embedded the first querying instance's
 *    version — two better-sqlite3 instances (11.10.0, 12.11.1) both rendered
 *    as 12.11.1.
 * 4. Rust deps reported under their IMPORT spelling (http_body_util) missed
 *    the lockfile's canonical name (http-body-util) and surfaced twice, once
 *    with `version: null`.
 * 5. knowledge_gaps graded long-fixed advisories critical for version-null
 *    deps, and evidenced a `tower` gap with a tower-stacker game.
 */
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { executeUpgradePlanner } from "../tools/upgrade-planner.js";
import { executeKnowledgeGaps } from "../tools/knowledge-gaps.js";
import { restampDepContext } from "../live/osv-scanner.js";
import { resolveVersions } from "../live/version-resolver.js";
import { FourDADatabase } from "../db.js";
import type { RegistryPackageInfo, ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

let root: string;
let webDir: string;
let rustDir: string;
let priorOffline: string | undefined;

const noDb = null as unknown as FourDADatabase;

function makeEntry(overrides: Partial<VulnerabilityEntry>): VulnerabilityEntry {
  return {
    package: "anyhow",
    currentVersion: "1.0.102",
    ecosystem: "crates.io",
    isDev: false,
    isDirect: false,
    devScopeKnown: false,
    vulnId: "RUSTSEC-2026-0190",
    aliases: [],
    severity: "medium",
    cvssScore: null,
    summary: "Unsoundness in Error::downcast_mut()",
    fixedVersion: "1.0.103",
    published: "2026-06-25T00:00:00Z",
    references: [],
    target: null,
    platformActive: true,
    sourceDirs: [],
    ...overrides,
  };
}

function makeScan(entries: VulnerabilityEntry[], projectPath: string): VulnerabilityScanResult {
  return {
    scannedAt: new Date().toISOString(),
    projectPath,
    ecosystemsScanned: ["crates.io"],
    totalScanned: 10,
    totalVulnerable: entries.length,
    platformInactiveVulnerable: 0,
    bySeverity: { critical: 0, high: 0, medium: entries.length, low: 0, unknown: 0 },
    vulnerabilities: entries,
    cleanCount: 10 - entries.length,
    scanDurationMs: 5,
    cached: true,
    offline: true,
  };
}

function makeIntel(scan: VulnerabilityScanResult | null): LiveIntelligence {
  const li = new LiveIntelligence(new Database(":memory:"));
  li.initFromDependencyGroups([
    { dir: rustDir, language: "rust", deps: ["anyhow"], devDeps: [] },
  ]);
  if (scan) (li as unknown as { lastVulnScan: VulnerabilityScanResult }).lastVulnScan = scan;
  return li;
}

beforeAll(() => {
  priorOffline = process.env.FOURDA_OFFLINE;
  process.env.FOURDA_OFFLINE = "true";

  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-audit-defects-"));
  webDir = path.join(root, "web");
  fs.mkdirSync(webDir);
  fs.writeFileSync(
    path.join(webDir, "package-lock.json"),
    JSON.stringify({
      packages: {
        "node_modules/better-sqlite3": { version: "12.11.1" },
      },
    }),
  );
  rustDir = path.join(root, "src-tauri");
  fs.mkdirSync(rustDir);
  fs.writeFileSync(
    path.join(rustDir, "Cargo.lock"),
    [
      "[[package]]",
      'name = "anyhow"',
      'version = "1.0.104"',
      "",
      "[[package]]",
      'name = "http-body-util"',
      'version = "0.1.5"',
      "",
    ].join("\n"),
  );
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
  fs.rmSync(root, { recursive: true, force: true });
});

describe("upgrade_planner — CVEs attach to the scanned INSTANCE, not the name", () => {
  it("does not charge a direct dep with a transitive instance's CVE, and surfaces the transitive instance", async () => {
    // Direct anyhow resolves to 1.0.104 from the lockfile fixture; the CVE was
    // scanned against the transitive 1.0.102 instance.
    const scan = makeScan([makeEntry({})], rustDir);
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    const directRow = result.recommendations.find(
      (r) => r.package === "anyhow" && r.scope === "direct",
    );
    // The direct 1.0.104 instance has no reasons (no CVE at its version, not
    // behind, not deprecated in offline mode) — it must not appear at all.
    expect(directRow).toBeUndefined();

    const transitiveRow = result.recommendations.find(
      (r) => r.package === "anyhow" && r.scope === "transitive",
    );
    expect(transitiveRow).toBeDefined();
    expect(transitiveRow!.currentVersion).toBe("1.0.102");
    expect(transitiveRow!.action).toBe("waiting_on_upstream");
    expect(transitiveRow!.reasons.some((r) => r.includes("RUSTSEC-2026-0190"))).toBe(true);
  });

  it("still attaches a CVE scanned at the direct dep's own version", async () => {
    const scan = makeScan(
      [makeEntry({ currentVersion: "1.0.104", isDirect: true, devScopeKnown: true, fixedVersion: "1.0.105" })],
      rustDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    const directRow = result.recommendations.find(
      (r) => r.package === "anyhow" && r.scope === "direct",
    );
    expect(directRow).toBeDefined();
    expect(directRow!.currentVersion).toBe("1.0.104");
    expect(directRow!.reasons.some((r) => r.includes("RUSTSEC-2026-0190"))).toBe(true);
  });
});

describe("osv-scanner — cached advisory entries wear TODAY's dep context", () => {
  it("restamps platform activity, provenance, and scope from the current dep", () => {
    const cached = makeEntry({
      package: "glib",
      currentVersion: "0.18.5",
      platformActive: true,
      target: null,
      sourceDirs: [],
      isDirect: true,
    });
    const dep: ResolvedDependency = {
      name: "glib",
      version: "0.18.5",
      ecosystem: "crates.io",
      isDev: false,
      isDirect: false,
      devScopeKnown: false,
      target: "not built for x86_64-pc-windows-msvc",
      platformActive: false,
      sourceDirs: ["D:/repo/src-tauri"],
    };

    const restamped = restampDepContext(cached, dep);
    expect(restamped.platformActive).toBe(false);
    expect(restamped.target).toBe("not built for x86_64-pc-windows-msvc");
    expect(restamped.sourceDirs).toEqual(["D:/repo/src-tauri"]);
    expect(restamped.isDirect).toBe(false);
    // Advisory facts survive untouched.
    expect(restamped.vulnId).toBe("RUSTSEC-2026-0190");
    expect(restamped.fixedVersion).toBe("1.0.103");
  });
});

describe("registry health — per-instance fields are re-stamped after the name-keyed cache", () => {
  it("returns each instance's own version and distance, not the first caller's", async () => {
    process.env.FOURDA_OFFLINE = "false";
    const li = new LiveIntelligence(new Database(":memory:"));
    process.env.FOURDA_OFFLINE = "true";

    // A name-keyed cache would hand instance #2 the record of instance #1.
    const staleRecord: RegistryPackageInfo = {
      name: "better-sqlite3", ecosystem: "npm", currentVersion: "12.11.1",
      latestVersion: "13.0.3", latestStableVersion: "13.0.3",
      versionsBehind: { major: 1, minor: 0, patch: 0, label: "major" },
      deprecated: false, deprecationMessage: null, lastPublished: null,
      license: null, weeklyDownloads: null, isDev: false, fetchError: null,
    };
    (li as unknown as { npmRegistry: unknown }).npmRegistry = {
      getPackageInfo: async () => ({ ...staleRecord }),
      getBulkDownloads: async () => new Map<string, number>(),
    };

    const dep = (version: string): ResolvedDependency => ({
      name: "better-sqlite3", version, ecosystem: "npm", isDev: false,
      isDirect: true, devScopeKnown: true, target: null, platformActive: true,
      sourceDirs: [],
    });

    const results = await li.fetchRegistryHealth([dep("12.11.1"), dep("11.10.0")]);
    expect(results.map((r) => r.currentVersion)).toEqual(["12.11.1", "11.10.0"]);
    expect(results[1].versionsBehind?.major).toBe(2);
  });
});

describe("version-resolver — import spellings resolve to the lockfile's canonical crate", () => {
  it("canonicalizes underscore import names and finds the version", () => {
    const resolved = resolveVersions(rustDir, ["http_body_util"], [], "rust");
    expect(resolved).toHaveLength(1);
    expect(resolved[0].name).toBe("http-body-util");
    expect(resolved[0].version).toBe("0.1.5");
  });

  it("merges both spellings into one dependency", () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([
      { dir: rustDir, language: "rust", deps: ["http-body-util", "http_body_util"], devDeps: [] },
    ]);
    const matches = li.getResolvedDeps().filter((d) => d.name.replace(/_/g, "-") === "http-body-util");
    expect(matches).toHaveLength(1);
    expect(matches[0].version).toBe("0.1.5");
  });
});

describe("knowledge_gaps — version fallback and generic-word evidence", () => {
  function makeGapsDb(): { db: FourDADatabase; dir: string } {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "4da-gaps-"));
    const dbPath = path.join(dir, "test.db");
    const raw = new Database(dbPath);
    raw.exec(`
      CREATE TABLE source_items (
        id INTEGER PRIMARY KEY, title TEXT, url TEXT, source_type TEXT,
        content TEXT, content_type TEXT, created_at TEXT, published_at TEXT,
        relevance_score REAL
      );
      CREATE TABLE project_dependencies (
        package_name TEXT, version TEXT, project_path TEXT, language TEXT,
        is_direct INTEGER DEFAULT 1, is_dev INTEGER DEFAULT 0
      );
      CREATE TABLE interactions (item_id INTEGER, action_type TEXT);
      CREATE TABLE osv_advisories (
        package_name TEXT, affected_ranges TEXT, withdrawn_at TEXT
      );
    `);
    raw.close();
    return { db: new FourDADatabase(dbPath), dir };
  }

  it("resolves a null DB version from the lockfile set and downgrades a patched advisory", () => {
    const { db, dir } = makeGapsDb();
    try {
      const raw = db.getRawDb();
      raw.prepare(
        "INSERT INTO project_dependencies (package_name, version, project_path, language) VALUES ('tokio', NULL, 'd:/proj', 'rust')",
      ).run();
      // An advisory titled for tokio, long fixed below the installed version.
      raw.prepare(
        `INSERT INTO source_items (id, title, url, source_type, content, created_at, published_at, relevance_score)
         VALUES (1, '[RUSTSEC-2021-0072] tokio: Task dropped in wrong thread', 'https://x', 'osv', 'tokio advisory', datetime('now', '-2 days'), datetime('now', '-30 days'), 0.9)`,
      ).run();
      raw.prepare(
        `INSERT INTO osv_advisories (package_name, affected_ranges, withdrawn_at)
         VALUES ('tokio', '[{"events":[{"introduced":"0"},{"fixed":"1.8.1"}]}]', NULL)`,
      ).run();

      const liveIntel = {
        isInitialized: () => true,
        getResolvedDeps: () => [
          { name: "tokio", version: "1.50.0", ecosystem: "crates.io" } as ResolvedDependency,
        ],
      };
      const result = executeKnowledgeGaps(db, { min_severity: "low" }, liveIntel);
      const tokioGap = result.gaps?.find((g) => g.dependency === "tokio");
      // The advisory names the dep, but the installed 1.50.0 is past the fix —
      // the gap must not be graded on security tiers.
      if (tokioGap) {
        expect(["low", "medium"]).toContain(tokioGap.gap_severity);
        expect(tokioGap.version).toBe("1.50.0");
      }
      // With the default medium floor, the same call must not produce a
      // critical tokio gap.
      const defaultResult = executeKnowledgeGaps(db, {}, liveIntel);
      const critical = defaultResult.gaps?.find(
        (g) => g.dependency === "tokio" && g.gap_severity === "critical",
      );
      expect(critical).toBeUndefined();
    } finally {
      db.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it("requires an ecosystem cue before an English-word dep counts a mention", () => {
    const { db, dir } = makeGapsDb();
    try {
      const raw = db.getRawDb();
      raw.prepare(
        "INSERT INTO project_dependencies (package_name, version, project_path, language) VALUES ('tower', '0.5.3', 'd:/proj', 'rust')",
      ).run();
      // The word, not the crate — must contribute no evidence.
      raw.prepare(
        `INSERT INTO source_items (id, title, url, source_type, content, created_at, relevance_score)
         VALUES (1, 'Show HN: Highrise — one-tap tower stacker, every height is a new record', 'https://x', 'hackernews', 'a fun little game about a tower', datetime('now', '-1 day'), 0.5)`,
      ).run();
      // The crate — must survive the guard.
      raw.prepare(
        `INSERT INTO source_items (id, title, url, source_type, content, created_at, relevance_score)
         VALUES (2, 'tower 0.5.4 released with new middleware combinators', 'https://y', 'hackernews', 'the rust tower crate', datetime('now', '-1 day'), 0.5)`,
      ).run();

      const result = executeKnowledgeGaps(db, { min_severity: "low" }, null);
      const towerGap = result.gaps?.find((g) => g.dependency === "tower");
      expect(towerGap).toBeDefined();
      const titles = towerGap!.missed_items.map((i) => i.title || "");
      expect(titles.some((t) => t.includes("stacker"))).toBe(false);
      expect(titles.some((t) => t.includes("0.5.4 released"))).toBe(true);
    } finally {
      db.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it("excludes advisories published in the distant past from missed items", () => {
    const { db, dir } = makeGapsDb();
    try {
      const raw = db.getRawDb();
      raw.prepare(
        "INSERT INTO project_dependencies (package_name, version, project_path, language) VALUES ('hono', '4.13.2', 'd:/proj', 'javascript')",
      ).run();
      // Backfilled last week, published five years ago: not "missed" reading.
      raw.prepare(
        `INSERT INTO source_items (id, title, url, source_type, content, created_at, published_at, relevance_score)
         VALUES (1, '[GHSA-old] hono: ancient advisory', 'https://x', 'osv', 'hono', datetime('now', '-2 days'), '2020-09-04 15:00:00', 0.9)`,
      ).run();

      const result = executeKnowledgeGaps(db, { min_severity: "low" }, null);
      const honoGap = result.gaps?.find((g) => g.dependency === "hono");
      expect(honoGap).toBeUndefined();
    } finally {
      db.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});
