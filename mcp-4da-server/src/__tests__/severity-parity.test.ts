// SPDX-License-Identifier: Apache-2.0
/**
 * One severity rule with the desktop app (AD-046).
 *
 * Measured 2026-09-10: vulnerability_scan graded `sandbox@3.1.2` (a transitive
 * of paddle-webhook, `is_direct: false`, `dev_scope_known: false`) CRITICAL —
 * `by_severity.critical: 1` — while the app graded the same advisory High.
 * The shared rule: (a) transitive-only clamps critical to high; then (b)
 * dev-only drops one level. Unknown dev scope gets no discount.
 */
import { describe, it, expect, beforeAll, afterAll, afterEach, vi } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { restampDepContext } from "../live/osv-scanner.js";
import { canonicalStoragePath } from "../live/dev-scope.js";
import {
  presentedSeverity,
  scopeAdjustedSeverity,
  type DependencyScope,
  type SeverityTier,
} from "../live/severity-scope.js";
import { executeVulnerabilityScan } from "../tools/vulnerability-scan.js";
import type { FourDADatabase } from "../db.js";
import type { ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

const noDb = null as unknown as FourDADatabase;

// ---------------------------------------------------------------------------
// The rule
// ---------------------------------------------------------------------------

describe("scopeAdjustedSeverity — the rule shared with osv::identity::scope_adjusted_urgency", () => {
  const DIRECT_RUNTIME: DependencyScope = { isDirect: true, isDev: false, devScopeKnown: true };
  const DIRECT_DEV: DependencyScope = { isDirect: true, isDev: true, devScopeKnown: true };
  const TRANSITIVE_RUNTIME: DependencyScope = { isDirect: false, isDev: false, devScopeKnown: true };
  const TRANSITIVE_DEV: DependencyScope = { isDirect: false, isDev: true, devScopeKnown: true };
  const TRANSITIVE_UNKNOWN: DependencyScope = { isDirect: false, isDev: false, devScopeKnown: false };
  // A dev flag without a determination behind it is not dev.
  const TRANSITIVE_UNKNOWN_DEV_FLAG: DependencyScope = { isDirect: false, isDev: true, devScopeKnown: false };
  const DIRECT_UNKNOWN_DEV_FLAG: DependencyScope = { isDirect: true, isDev: true, devScopeKnown: false };

  it("matches the spec table", () => {
    expect(scopeAdjustedSeverity("critical", DIRECT_RUNTIME)).toBe("critical");
    expect(scopeAdjustedSeverity("critical", TRANSITIVE_RUNTIME)).toBe("high");
    expect(scopeAdjustedSeverity("critical", DIRECT_DEV)).toBe("high");
    expect(scopeAdjustedSeverity("critical", TRANSITIVE_DEV)).toBe("medium");
    expect(scopeAdjustedSeverity("high", DIRECT_DEV)).toBe("medium");
    expect(scopeAdjustedSeverity("high", TRANSITIVE_DEV)).toBe("medium");
    expect(scopeAdjustedSeverity("high", TRANSITIVE_RUNTIME)).toBe("high");
  });

  it("covers every tier in every scope", () => {
    const scopes: Array<[string, DependencyScope]> = [
      ["direct runtime", DIRECT_RUNTIME],
      ["direct dev", DIRECT_DEV],
      ["transitive runtime", TRANSITIVE_RUNTIME],
      ["transitive dev", TRANSITIVE_DEV],
      ["transitive unknown", TRANSITIVE_UNKNOWN],
      ["transitive unknown (dev flag)", TRANSITIVE_UNKNOWN_DEV_FLAG],
      ["direct unknown (dev flag)", DIRECT_UNKNOWN_DEV_FLAG],
    ];
    // Literal expectations, one column per scope above, in that order.
    const expected: Record<SeverityTier, SeverityTier[]> = {
      critical: ["critical", "high", "high", "medium", "high", "high", "critical"],
      high: ["high", "medium", "high", "medium", "high", "high", "high"],
      medium: ["medium", "low", "medium", "low", "medium", "medium", "medium"],
      low: ["low", "low", "low", "low", "low", "low", "low"],
      unknown: ["unknown", "unknown", "unknown", "unknown", "unknown", "unknown", "unknown"],
    };
    for (const [tier, row] of Object.entries(expected) as Array<[SeverityTier, SeverityTier[]]>) {
      scopes.forEach(([label, scope], i) => {
        expect(scopeAdjustedSeverity(tier, scope), `${tier} / ${label}`).toBe(row[i]);
      });
    }
  });

  it("treats a missing scope field on a scan row as no discount", () => {
    const row = { severity: "critical", isDirect: undefined, isDev: true, devScopeKnown: undefined } as unknown as VulnerabilityEntry;
    expect(presentedSeverity(row)).toBe("critical");
  });
});

// ---------------------------------------------------------------------------
// The sandbox shape, end to end through vulnerability_scan
// ---------------------------------------------------------------------------

let root: string;
let paddleDir: string;

const PADDLE_LOCK = [
  "lockfileVersion: '9.0'",
  "",
  "importers:",
  "",
  "  .:",
  "    dependencies:",
  "      '@paddle/paddle-node-sdk':",
  "        specifier: ^2.0.0",
  "        version: 2.3.0",
  "",
  "packages:",
  "",
  "  '@paddle/paddle-node-sdk@2.3.0':",
  "    resolution: {integrity: sha512-fixture}",
  "",
  "  sandbox@3.1.2:",
  "    resolution: {integrity: sha512-fixture}",
  "",
].join("\n");

const SANDBOX: Partial<VulnerabilityEntry> = {
  vulnId: "GHSA-sbx-0001",
  summary: "Sandbox Breakout",
  severity: "critical",
  cvssScore: 9.8,
  fixedVersion: "3.1.3",
};

function scanOf(deps: ResolvedDependency[], projectPath: string): VulnerabilityScanResult {
  const vulnerabilities: VulnerabilityEntry[] = deps
    .filter((d) => d.name === "sandbox" && d.version === "3.1.2")
    .map((dep) =>
      restampDepContext(
        {
          package: "", currentVersion: "", ecosystem: "npm", isDev: false, isDirect: true, devScopeKnown: true,
          vulnId: "", aliases: [], severity: "unknown", cvssScore: null, summary: "", fixedVersion: null,
          published: "2026-09-01T00:00:00Z", references: [], target: null, platformActive: true, sourceDirs: [],
          ...SANDBOX,
        } as VulnerabilityEntry,
        dep,
      ),
    );
  const bySeverity = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
  for (const v of vulnerabilities) bySeverity[v.severity]++;
  return {
    scannedAt: new Date().toISOString(), projectPath, ecosystemsScanned: ["npm"], totalScanned: deps.length,
    totalVulnerable: vulnerabilities.length ? 1 : 0, platformInactiveVulnerable: 0, bySeverity, vulnerabilities,
    cleanCount: deps.length - (vulnerabilities.length ? 1 : 0), scanDurationMs: 1, cached: false, offline: false,
  };
}

/** The app's `dependency_instances` table, as its Phase 92 migration creates it. */
function inventoryDb(row?: { isDev: number; scope?: string }): Database.Database {
  const db = new Database(":memory:");
  db.exec(`CREATE TABLE dependency_instances (
    id INTEGER PRIMARY KEY, project_path TEXT NOT NULL, ecosystem TEXT NOT NULL,
    package_name TEXT NOT NULL, version TEXT NOT NULL, is_direct INTEGER NOT NULL DEFAULT 0,
    is_dev INTEGER NOT NULL DEFAULT 0, scope TEXT NOT NULL DEFAULT 'unknown',
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_path, ecosystem, package_name, version))`);
  if (row) {
    db.prepare(
      "INSERT INTO dependency_instances (project_path, ecosystem, package_name, version, is_direct, is_dev, scope) VALUES (?, 'npm', 'sandbox', '3.1.2', 0, ?, ?)",
    ).run(canonicalStoragePath(paddleDir), row.isDev, row.scope ?? "unknown");
  }
  return db;
}

async function scanPaddle(db: Database.Database): Promise<Record<string, any>> {
  const prior = process.env.FOURDA_OFFLINE;
  delete process.env.FOURDA_OFFLINE;
  const li = new LiveIntelligence(db);
  if (prior !== undefined) process.env.FOURDA_OFFLINE = prior;
  (li as unknown as { osvScanner: { scan: typeof scanOf } }).osvScanner = { scan: async (d, p) => scanOf(d, p) } as never;
  li.initFromDependencyGroups([{ dir: paddleDir, language: "javascript", deps: ["@paddle/paddle-node-sdk"], devDeps: [] }]);
  vi.spyOn(process, "cwd").mockReturnValue(paddleDir);
  return (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;
}

beforeAll(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-severity-parity-"));
  fs.mkdirSync(path.join(root, ".git"));
  paddleDir = path.join(root, "paddle-webhook");
  fs.mkdirSync(paddleDir);
  fs.writeFileSync(
    path.join(paddleDir, "package.json"),
    JSON.stringify({ dependencies: { "@paddle/paddle-node-sdk": "^2.0.0" } }),
  );
  fs.writeFileSync(path.join(paddleDir, "pnpm-lock.yaml"), PADDLE_LOCK);
});

afterAll(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("vulnerability_scan — sandbox@3.1.2 graded as the app grades it", () => {
  it("presents a transitive of unknown scope as high, keeping the advisory's critical beside it", async () => {
    const out = await scanPaddle(new Database(":memory:"));
    const sandbox = out.vulnerabilities.find((v: { package: string }) => v.package === "sandbox");

    expect(sandbox.is_direct).toBe(false);
    expect(sandbox.dev_scope_known).toBe(false);
    expect(sandbox.severity).toBe("high");
    expect(sandbox.advisory_severity).toBe("critical");
    expect(sandbox.severity_note).toContain("transitive-only");
    expect(out.by_severity).toMatchObject({ critical: 0, high: 1 });
    expect(out.advisory_by_severity).toMatchObject({ critical: 1, high: 0 });
    expect(out._meta.severity_rule).toContain("caps critical at high");
  });

  it("drops to medium when the app's dependency_instances says is_dev = 1", async () => {
    const out = await scanPaddle(inventoryDb({ isDev: 1 }));
    const sandbox = out.vulnerabilities.find((v: { package: string }) => v.package === "sandbox");

    // Graded down, not dropped: the default scan keeps known-dev transitives.
    expect(sandbox).toBeDefined();
    expect(sandbox.dev_scope_known).toBe(true);
    expect(sandbox.is_dev).toBe(true);
    expect(sandbox.severity).toBe("medium");
    expect(sandbox.advisory_severity).toBe("critical");
    expect(out.by_severity).toMatchObject({ critical: 0, high: 0, medium: 1 });
    expect(out.advisory_by_severity).toMatchObject({ critical: 1 });
  });

  it("does not read the app's placeholder row (is_dev = 0, scope 'unknown') as a determination", async () => {
    const out = await scanPaddle(inventoryDb({ isDev: 0, scope: "unknown" }));
    const sandbox = out.vulnerabilities.find((v: { package: string }) => v.package === "sandbox");
    expect(sandbox.dev_scope_known).toBe(false);
    expect(sandbox.severity).toBe("high");
  });

  it("a determined runtime row is known scope: the transitive clamp alone applies", async () => {
    const out = await scanPaddle(inventoryDb({ isDev: 0, scope: "runtime" }));
    const sandbox = out.vulnerabilities.find((v: { package: string }) => v.package === "sandbox");
    expect(sandbox.dev_scope_known).toBe(true);
    expect(sandbox.is_dev).toBe(false);
    expect(sandbox.severity).toBe("high");
  });

  it("filters on the presented grade: severity_filter critical no longer lists it", async () => {
    const prior = process.env.FOURDA_OFFLINE;
    delete process.env.FOURDA_OFFLINE;
    const li = new LiveIntelligence(new Database(":memory:"));
    if (prior !== undefined) process.env.FOURDA_OFFLINE = prior;
    (li as unknown as { osvScanner: { scan: typeof scanOf } }).osvScanner = { scan: async (d, p) => scanOf(d, p) } as never;
    li.initFromDependencyGroups([{ dir: paddleDir, language: "javascript", deps: ["@paddle/paddle-node-sdk"], devDeps: [] }]);
    vi.spyOn(process, "cwd").mockReturnValue(paddleDir);

    const critical = (await executeVulnerabilityScan(noDb, { severity_filter: "critical" }, li)) as Record<string, any>;
    expect(critical.vulnerabilities).toHaveLength(0);
    const high = (await executeVulnerabilityScan(noDb, { severity_filter: "high" }, li)) as Record<string, any>;
    expect(high.vulnerabilities).toHaveLength(1);
  });
});
