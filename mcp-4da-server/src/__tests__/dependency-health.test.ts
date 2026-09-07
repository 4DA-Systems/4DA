// SPDX-License-Identifier: Apache-2.0
/**
 * dependency_health must count "vulnerable" the way vulnerability_scan does.
 *
 * Live 2026-09-07: over one scan, vulnerability_scan reported 5 vulnerable
 * packages (with the rest filed under platform_inactive_vulnerabilities and
 * maintenance_notices) while dependency_health said 11 — it counted every
 * advisory row. Two tools, one scan, two numbers.
 */
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { executeDependencyHealth } from "../tools/dependency-health.js";
import type { FourDADatabase } from "../db.js";
import type { VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

let root: string;
let webDir: string;
let priorOffline: string | undefined;

const noDb = null as unknown as FourDADatabase;

function makeEntry(over: Partial<VulnerabilityEntry>): VulnerabilityEntry {
  return {
    package: "react",
    currentVersion: "19.2.6",
    ecosystem: "npm",
    isDev: false,
    isDirect: true,
    devScopeKnown: true,
    vulnId: "GHSA-test-0001",
    aliases: [],
    severity: "high",
    cvssScore: 7.5,
    summary: "test advisory",
    fixedVersion: "19.2.7",
    published: "2026-01-01T00:00:00Z",
    references: [],
    target: null,
    platformActive: true,
    sourceDirs: [],
    ...over,
  };
}

function makeScan(entries: VulnerabilityEntry[]): VulnerabilityScanResult {
  const bySeverity = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
  for (const e of entries) bySeverity[e.severity]++;
  return {
    scannedAt: new Date().toISOString(),
    projectPath: webDir,
    ecosystemsScanned: ["npm"],
    totalScanned: 10,
    totalVulnerable: new Set(entries.map((e) => e.package)).size,
    platformInactiveVulnerable: 0,
    bySeverity,
    vulnerabilities: entries,
    cleanCount: 10 - entries.length,
    scanDurationMs: 5,
    cached: true,
    offline: true,
  };
}

function makeIntel(scan: VulnerabilityScanResult): LiveIntelligence {
  const li = new LiveIntelligence(new Database(":memory:"));
  li.initFromDependencyGroups([{ dir: webDir, language: "javascript", deps: ["react"], devDeps: [] }]);
  (li as unknown as { lastVulnScan: VulnerabilityScanResult }).lastVulnScan = scan;
  return li;
}

beforeAll(() => {
  priorOffline = process.env.FOURDA_OFFLINE;
  process.env.FOURDA_OFFLINE = "true";
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-dep-health-"));
  webDir = path.join(root, "web");
  fs.mkdirSync(webDir);
  fs.writeFileSync(
    path.join(webDir, "package-lock.json"),
    JSON.stringify({ packages: { "node_modules/react": { version: "19.2.6" } } }),
  );
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
  fs.rmSync(root, { recursive: true, force: true });
});

describe("dependency_health — vulnerable means the actionable set", () => {
  const scan = makeScan([
    makeEntry({}),
    makeEntry({ package: "nix", currentVersion: "0.29.0", ecosystem: "crates.io", vulnId: "GHSA-inactive", severity: "critical", platformActive: false, target: "cfg(unix)" }),
    makeEntry({ package: "paste", currentVersion: "1.0.15", ecosystem: "crates.io", vulnId: "RUSTSEC-2024-0436", severity: "unknown", summary: "paste - no longer maintained", fixedVersion: null }),
  ]);

  it("counts distinct packages that are built on this host and not maintenance notices", async () => {
    const result = await executeDependencyHealth(noDb, {}, makeIntel(scan));

    expect(result.vulnerableCount).toBe(1);
    expect(result.advisoryCount).toBe(3);
    expect(result.summary).toContain("1 vulnerable");
    expect(result.summary).toContain("2 advisories not counted");
  });

  it("breaks severity down over the same set", async () => {
    const result = await executeDependencyHealth(noDb, {}, makeIntel(scan));

    expect(result.vulnerabilitySummary).toEqual({ critical: 0, high: 1, medium: 0, low: 0 });
  });

  it("reports all healthy when every advisory is inactive or a maintenance notice", async () => {
    const quiet = makeScan([
      makeEntry({ package: "nix", vulnId: "GHSA-inactive", severity: "critical", platformActive: false, target: "cfg(unix)" }),
      makeEntry({ package: "paste", vulnId: "RUSTSEC-2024-0436", severity: "unknown", summary: "paste - no longer maintained", fixedVersion: null }),
    ]);
    const result = await executeDependencyHealth(noDb, {}, makeIntel(quiet));

    expect(result.vulnerableCount).toBe(0);
    expect(result.advisoryCount).toBe(2);
    expect(result.summary).toContain("all healthy");
    expect(result.healthScore).toBe(100);
  });
});
