// SPDX-License-Identifier: Apache-2.0
/**
 * Phase 0 honesty-contract tests for upgrade_planner.
 *
 * Guards the three dishonesties fixed in the MCP hardening pass:
 * 1. Transitive CVEs were counted in scan totals but never recommended —
 *    now surfaced as `waiting_on_upstream` steps with honest labels.
 * 2. `projectPath` reported `process.cwd()` regardless of where dependencies
 *    were actually resolved from — now reports the real resolution root.
 * 3. A missing vulnerability scan silently produced a CVE-blind ranking —
 *    now disclosed in the summary and via `vulnerabilityDataAvailable`.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { executeUpgradePlanner } from "../tools/upgrade-planner.js";
import type { FourDADatabase } from "../db.js";
import type { VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

let root: string;
let webDir: string;
let rustDir: string;
let priorOffline: string | undefined;

const noDb = null as unknown as FourDADatabase;

function makeEntry(overrides: Partial<VulnerabilityEntry>): VulnerabilityEntry {
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
    fixedVersion: null,
    published: "2026-01-01T00:00:00Z",
    references: [],
    target: null,
    platformActive: true,
    ...overrides,
  };
}

function makeScan(entries: VulnerabilityEntry[], projectPath: string): VulnerabilityScanResult {
  return {
    scannedAt: new Date().toISOString(),
    projectPath,
    ecosystemsScanned: ["npm"],
    totalScanned: 10,
    totalVulnerable: entries.length,
    platformInactiveVulnerable: 0,
    bySeverity: { critical: 0, high: entries.length, medium: 0, low: 0, unknown: 0 },
    vulnerabilities: entries,
    cleanCount: 10 - entries.length,
    scanDurationMs: 5,
    cached: true,
    offline: true,
  };
}

/** Offline LiveIntelligence over a real lockfile fixture, with an injected scan. */
function makeIntel(scan: VulnerabilityScanResult | null): LiveIntelligence {
  const li = new LiveIntelligence(new Database(":memory:"));
  li.initFromDependencyGroups([
    { dir: webDir, language: "javascript", deps: ["react"], devDeps: [] },
    { dir: rustDir, language: "rust", deps: ["tokio"], devDeps: [] },
  ]);
  if (scan) (li as unknown as { lastVulnScan: VulnerabilityScanResult }).lastVulnScan = scan;
  return li;
}

beforeAll(() => {
  // Offline mode must be set before LiveIntelligence construction: registry
  // fetches return inert stubs, so tests are deterministic and network-free.
  priorOffline = process.env.FOURDA_OFFLINE;
  process.env.FOURDA_OFFLINE = "true";

  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-upgrade-plan-"));
  webDir = path.join(root, "web");
  fs.mkdirSync(webDir);
  fs.writeFileSync(
    path.join(webDir, "package-lock.json"),
    JSON.stringify({
      packages: {
        "node_modules/react": { version: "19.2.6" },
        "node_modules/zustand": { version: "5.0.14" },
      },
    }),
  );
  rustDir = path.join(root, "src-tauri");
  fs.mkdirSync(rustDir);
  fs.writeFileSync(
    path.join(rustDir, "Cargo.lock"),
    ['[[package]]', 'name = "tokio"', 'version = "1.50.0"', ""].join("\n"),
  );
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
  fs.rmSync(root, { recursive: true, force: true });
});

describe("upgrade_planner — transitive CVEs become waiting_on_upstream steps", () => {
  it("recommends a transitive vulnerable package with honest labels", async () => {
    const scan = makeScan(
      [
        makeEntry({}),
        makeEntry({
          package: "zustand",
          currentVersion: "5.0.14",
          isDirect: false,
          devScopeKnown: false,
          vulnId: "GHSA-test-0002",
          fixedVersion: "5.0.15",
        }),
      ],
      webDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    const transitive = result.recommendations.find((r) => r.package === "zustand");
    expect(transitive).toBeDefined();
    expect(transitive!.scope).toBe("transitive");
    expect(transitive!.action).toBe("waiting_on_upstream");
    expect(transitive!.targetVersion).toBe("5.0.15");
    // Computed from current -> target (it read "unknown" before 2026-10-02).
    expect(transitive!.upgradeType).toBe("patch");
    expect(transitive!.breaking).toBe(false);
    expect(transitive!.reasons.some((r) => r.includes("Transitive dependency"))).toBe(true);
    expect(result.waitingOnUpstream).toBe(1);
  });

  it("ranks fixable-now (direct) above waiting-on-upstream at equal risk", async () => {
    const scan = makeScan(
      [
        makeEntry({ fixedVersion: "19.2.7" }),
        makeEntry({ package: "zustand", currentVersion: "5.0.14", isDirect: false, vulnId: "GHSA-test-0002", fixedVersion: "5.0.15" }),
      ],
      webDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    const directIdx = result.recommendations.findIndex((r) => r.package === "react");
    const transitiveIdx = result.recommendations.findIndex((r) => r.package === "zustand");
    expect(directIdx).toBeGreaterThanOrEqual(0);
    expect(transitiveIdx).toBeGreaterThan(directIdx);
    expect(result.recommendations[directIdx].scope).toBe("direct");
    expect(result.recommendations[directIdx].action).toBe("upgrade_direct");
    // The smallest version that fixes it, not the newest release.
    expect(result.recommendations[directIdx].targetVersion).toBe("19.2.7");
  });

  it("says no_fix_available, never upgrade_direct with no target, when no fixed release exists", async () => {
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(makeScan([makeEntry({ fixedVersion: null })], webDir)));
    const react = result.recommendations.find((r) => r.package === "react")!;
    expect(react.action).toBe("no_fix_available");
    expect(result.summary).toMatch(/no fixed release/);
  });

  it("does not count unmaintained-package notices as vulnerabilities", async () => {
    const scan = makeScan(
      [makeEntry({ package: "paste", currentVersion: "1.0.15", isDirect: false, vulnId: "RUSTSEC-2024-0436", summary: "paste - no longer maintained" })],
      webDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));
    expect(result.recommendations.find((r) => r.package === "paste")).toBeUndefined();
    expect(result.summary).toMatch(/1 unmaintained-package notice/);
  });

  it("excludes known-dev transitive vulns by default, keeps unknown-scope ones", async () => {
    const scan = makeScan(
      [
        makeEntry({ package: "dev-only-pkg", isDirect: false, isDev: true, devScopeKnown: true, vulnId: "GHSA-dev-1" }),
        makeEntry({ package: "unknown-scope-pkg", isDirect: false, isDev: true, devScopeKnown: false, vulnId: "GHSA-dev-2" }),
      ],
      webDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    expect(result.recommendations.find((r) => r.package === "dev-only-pkg")).toBeUndefined();
    expect(result.recommendations.find((r) => r.package === "unknown-scope-pkg")).toBeDefined();
  });

  it("labels platform-inactive transitive vulns without suppressing them", async () => {
    const scan = makeScan(
      [makeEntry({ package: "win-only", isDirect: false, vulnId: "GHSA-plat-1", platformActive: false, target: "cfg(unix)" })],
      webDir,
    );
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(scan));

    const rec = result.recommendations.find((r) => r.package === "win-only");
    expect(rec).toBeDefined();
    expect(rec!.platformActive).toBe(false);
    expect(rec!.reasons.some((r) => r.includes("not active on this platform"))).toBe(true);
  });
});

describe("upgrade_planner — projectPath honesty", () => {
  it("reports the dependency resolution root, not process.cwd()", async () => {
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(makeScan([makeEntry({})], webDir)));
    const normalized = result.projectPath.replace(/\\/g, "/");
    expect(normalized).toBe(root.replace(/\\/g, "/"));
    expect(result.projectPath).not.toBe(process.cwd());
  });

  it("getProjectRoot returns the single group dir when only one group exists", () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([{ dir: webDir, language: "javascript", deps: ["react"], devDeps: [] }]);
    expect(li.getProjectRoot()?.replace(/\\/g, "/")).toBe(webDir.replace(/\\/g, "/"));
  });
});

describe("upgrade_planner — CVE-blind disclosure and provenance", () => {
  it("discloses a CVE-blind plan when no scan can run (offline)", async () => {
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(null));
    expect(result.vulnerabilityDataAvailable).toBe(false);
    expect(result.summary).toContain("offline mode (FOURDA_OFFLINE), so this plan is CVE-blind");
  });

  // 2026-10-03 agent eval: a fresh session's first plan answered "CVE data not
  // loaded — run vulnerability_scan first", and `package: "diff"` (a vulnerable
  // transitive) came back "not a dependency of this project".
  describe("online, with no scan run yet", () => {
    function onlineIntel(scanCalls: Array<{ includeDev?: boolean }>): LiveIntelligence {
      const prior = process.env.FOURDA_OFFLINE;
      delete process.env.FOURDA_OFFLINE;
      const li = new LiveIntelligence(new Database(":memory:"));
      process.env.FOURDA_OFFLINE = prior;
      li.initFromDependencyGroups([{ dir: webDir, language: "javascript", deps: ["react"], devDeps: [] }]);
      const zustand = makeEntry({ package: "zustand", currentVersion: "5.0.14", isDirect: false, vulnId: "GHSA-zust-0001", fixedVersion: "5.0.15" });
      (li as unknown as { osvScanner: unknown }).osvScanner = {
        scan: async () => ({ ...makeScan([zustand], webDir), offline: false, cached: false }),
      };
      li.fetchRegistryHealth = async () => [];
      const scan = li.scanVulnerabilities.bind(li);
      li.scanVulnerabilities = (p, o) => (scanCalls.push(o ?? {}), scan(p, o));
      return li;
    }

    it("runs the scan itself and plans the vulnerable transitive asked for by name", async () => {
      const calls: Array<{ includeDev?: boolean }> = [];
      const result = (await executeUpgradePlanner(noDb, { package: "Zustand", include_dev: true }, onlineIntel(calls))) as {
        vulnerabilityDataAvailable: boolean; recommendations: Array<{ package: string; targetVersion: string; scope: string; action: string }>; summary: string;
      };
      expect(calls).toEqual([{ includeDev: true }]);
      expect(result.vulnerabilityDataAvailable).toBe(true);
      expect(result.recommendations).toMatchObject([{ package: "zustand", targetVersion: "5.0.15", scope: "transitive", action: "waiting_on_upstream" }]);
      expect(result.summary).not.toContain("not a dependency");
    });

    it("says what the lockfile holds when there is nothing to plan, instead of 'not a dependency'", async () => {
      const result = await executeUpgradePlanner(noDb, { package: "react" }, onlineIntel([]));
      expect(result.summary).toContain("react 19.2.6 (direct): nothing to upgrade");
      const missing = await executeUpgradePlanner(noDb, { package: "left-pad" }, onlineIntel([]));
      expect(missing.summary).toContain(`"left-pad" is not in this project's lockfiles`);
    });
  });

  it("always labels itself a standalone heuristic", async () => {
    const result = await executeUpgradePlanner(noDb, {}, makeIntel(null));
    expect(result.provenance.mode).toBe("standalone_heuristic");
    expect(result.provenance.note).toContain("Not the 4DA desktop app");
  });
});
