// SPDX-License-Identifier: Apache-2.0
/**
 * A scan result is stored only if no later scan request has stored one.
 *
 * Found 2026-10-03 (pre-publish verification, 2 of 6 runs): the init warmup
 * scan (runtime scope) finished after an agent's `include_dev` scan and
 * replaced it, so upgrade_planner planned the dev dependency node-fetch 2.6.0
 * with no advisories and targeted the ESM-only 3.3.2 instead of 2.6.7.
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import Database from "better-sqlite3";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { LiveIntelligence } from "../live/index.js";
import type { ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

let dir: string;
let priorOffline: string | undefined;

beforeAll(() => {
  priorOffline = process.env.FOURDA_OFFLINE;
  delete process.env.FOURDA_OFFLINE;
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "scan-order-"));
  fs.writeFileSync(path.join(dir, "package.json"), JSON.stringify({ dependencies: { lodash: "4.17.15" }, devDependencies: { "node-fetch": "2.6.0" } }));
  fs.writeFileSync(
    path.join(dir, "package-lock.json"),
    JSON.stringify({
      lockfileVersion: 3,
      packages: {
        "": { dependencies: { lodash: "4.17.15" }, devDependencies: { "node-fetch": "2.6.0" } },
        "node_modules/lodash": { version: "4.17.15" },
        "node_modules/node-fetch": { version: "2.6.0", dev: true },
      },
    }),
  );
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
  fs.rmSync(dir, { recursive: true, force: true });
});

const entry = (dep: ResolvedDependency): VulnerabilityEntry => ({
  package: dep.name,
  currentVersion: dep.version ?? "",
  ecosystem: dep.ecosystem,
  isDev: dep.isDev,
  isDirect: dep.isDirect,
  devScopeKnown: true,
  vulnId: `GHSA-${dep.name}`,
  aliases: [],
  severity: "high",
  cvssScore: 7.5,
  summary: `${dep.name} advisory`,
  fixedVersion: dep.name === "node-fetch" ? "2.6.7" : "4.17.21",
  published: "2022-01-01T00:00:00Z",
  references: [],
  target: null,
  platformActive: true,
  sourceDirs: [dir],
});

/** A scanner whose answer for the first request arrives last. */
function slowFirstScanner() {
  let calls = 0;
  return {
    scan: async (deps: ResolvedDependency[], projectPath: string): Promise<VulnerabilityScanResult> => {
      const delay = ++calls === 1 ? 120 : 10;
      await new Promise((r) => setTimeout(r, delay));
      return {
        scannedAt: new Date().toISOString(),
        projectPath,
        ecosystemsScanned: ["npm"],
        totalScanned: deps.length,
        totalVulnerable: deps.length,
        platformInactiveVulnerable: 0,
        bySeverity: { critical: 0, high: deps.length, medium: 0, low: 0, unknown: 0 },
        vulnerabilities: deps.map(entry),
        cleanCount: 0,
        scanDurationMs: delay,
        cached: false,
        offline: false,
      };
    },
  };
}

describe("scan request order", () => {
  it("a slow earlier scan never replaces a newer one", async () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["lodash"], devDeps: ["node-fetch"] }]);
    (li as unknown as { osvScanner: unknown }).osvScanner = slowFirstScanner();

    const warmup = li.scanVulnerabilities(dir); // runtime scope, answers last
    const agent = li.scanVulnerabilities(dir, { includeDev: true });
    await Promise.all([warmup, agent]);

    const stored = li.getVulnerabilities();
    expect(stored?.vulnerabilities.map((v) => v.package).sort()).toEqual(["lodash", "node-fetch"]);
    expect(li.lastScanIncludesDev()).toBe(true);
  });

  it("a caller that needs dev scope gets a dev-inclusive scan, not the runtime warmup", async () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["lodash"], devDeps: ["node-fetch"] }]);
    (li as unknown as { osvScanner: unknown }).osvScanner = slowFirstScanner();

    const runtime = await li.ensureVulnerabilities(dir, 5000);
    expect(runtime?.vulnerabilities.map((v) => v.package)).toEqual(["lodash"]);
    const withDev = await li.ensureVulnerabilities(dir, 5000, { includeDev: true });
    expect(withDev?.vulnerabilities.map((v) => v.package).sort()).toEqual(["lodash", "node-fetch"]);
  });
});
