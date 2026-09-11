// SPDX-License-Identifier: Apache-2.0
/**
 * Install drift and re-resolution on lockfile change (5.1.0).
 *
 * Measured 2026-09-10 on the founder machine:
 * 1. mcp-4da-server/pnpm-lock.yaml pinned hono 4.13.5 while
 *    mcp-4da-server/node_modules/hono held 4.13.1 for 25 days, vulnerable to
 *    CVE-2026-84363/-84364/-84365. Every 4DA surface read the lockfile, so
 *    every surface reported hono fixed.
 * 2. A server process started before the lockfile bump kept reporting hono
 *    4.13.3 (the lockfile at its start) with `_meta.cached: false`: versions
 *    were resolved once at init and never again, and `cached` only ever
 *    described the OSV lookup.
 */
import { describe, it, expect, beforeAll, afterAll, afterEach, vi } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { restampDepContext } from "../live/osv-scanner.js";
import { installFixFor, readInstalledVersion } from "../live/install-state.js";
import { executeVulnerabilityScan } from "../tools/vulnerability-scan.js";
import { executeDependencyHealth } from "../tools/dependency-health.js";
import { executeUpgradePlanner } from "../tools/upgrade-planner.js";
import type { FourDADatabase } from "../db.js";
import type { ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

const noDb = null as unknown as FourDADatabase;

let root: string;
let priorOffline: string | undefined;
let seq = 0;

const pnpmLock = (version: string) =>
  [
    "lockfileVersion: '9.0'",
    "",
    "importers:",
    "",
    "  .:",
    "    dependencies:",
    "      hono:",
    "        specifier: ^4.12.34",
    `        version: ${version}`,
    "",
    "packages:",
    "",
    `  hono@${version}:`,
    "    resolution: {integrity: sha512-fixture}",
    "",
  ].join("\n");

function writeInstalled(dir: string, name: string, version: string): void {
  const pkgDir = path.join(dir, "node_modules", ...name.split("/"));
  fs.mkdirSync(pkgDir, { recursive: true });
  fs.writeFileSync(path.join(pkgDir, "package.json"), JSON.stringify({ name, version }));
}

/** A pnpm project pinning hono `lockVersion`; `installed` null = never installed. */
function project(lockVersion: string, installed: string | null): string {
  const dir = path.join(root, `app-${++seq}`);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "package.json"),
    JSON.stringify({ name: `app-${seq}`, dependencies: { hono: "^4.12.34" } }),
  );
  fs.writeFileSync(path.join(dir, "pnpm-lock.yaml"), pnpmLock(lockVersion));
  if (installed) {
    writeInstalled(dir, "hono", installed);
    fs.writeFileSync(path.join(dir, "node_modules", ".modules.yaml"), "layoutVersion: 5\n");
  }
  return dir;
}

function template(over: Partial<VulnerabilityEntry>): VulnerabilityEntry {
  return {
    package: "", currentVersion: "", ecosystem: "npm", isDev: false, isDirect: true,
    devScopeKnown: true, vulnId: "GHSA-test", aliases: [], severity: "high", cvssScore: 7.5,
    summary: "hono advisory", fixedVersion: "4.13.4", published: "2026-08-01T00:00:00Z",
    references: [], target: null, platformActive: true, sourceDirs: [], ...over,
  };
}

type Advisories = Record<string, Array<Partial<VulnerabilityEntry>>>;

/** A scan result exactly as OSV would shape it for these deps: rows re-stamped with each dep's context. */
function scanOf(deps: ResolvedDependency[], projectPath: string, advisories: Advisories): VulnerabilityScanResult {
  const vulnerabilities: VulnerabilityEntry[] = [];
  for (const dep of deps) {
    for (const adv of advisories[`${dep.name}@${dep.version}`] ?? []) {
      vulnerabilities.push(restampDepContext(template(adv), dep));
    }
  }
  const bySeverity = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
  for (const v of vulnerabilities) bySeverity[v.severity]++;
  const vulnerable = new Set(vulnerabilities.map((v) => `${v.package}@${v.currentVersion}`)).size;
  return {
    scannedAt: new Date().toISOString(), projectPath, ecosystemsScanned: ["npm"],
    totalScanned: deps.length, totalVulnerable: vulnerable, platformInactiveVulnerable: 0,
    bySeverity, vulnerabilities, cleanCount: deps.length - vulnerable, scanDurationMs: 1,
    cached: false, offline: false,
  };
}

/** Network ON, OSV stubbed: `scanned` records every dependency list OSV was asked about. */
function liveWithOsv(advisories: Advisories, gate?: Promise<void>) {
  const prior = process.env.FOURDA_OFFLINE;
  delete process.env.FOURDA_OFFLINE;
  const li = new LiveIntelligence(new Database(":memory:"));
  if (prior !== undefined) process.env.FOURDA_OFFLINE = prior;
  const scanned: ResolvedDependency[][] = [];
  (li as unknown as { osvScanner: { scan: (d: ResolvedDependency[], p: string) => Promise<VulnerabilityScanResult> } }).osvScanner = {
    scan: async (deps, projectPath) => {
      scanned.push(deps);
      if (gate) await gate;
      return scanOf(deps, projectPath, advisories);
    },
  };
  return { li, scanned };
}

const HONO_4131: Advisories = {
  "hono@4.13.1": [
    { vulnId: "GHSA-hono-0001", aliases: ["CVE-2026-84363"], summary: "hono: path traversal in serveStatic" },
    { vulnId: "GHSA-hono-0002", aliases: ["CVE-2026-84364"], summary: "hono: CSRF middleware bypass" },
    { vulnId: "GHSA-hono-0003", aliases: ["CVE-2026-84365"], severity: "medium", summary: "hono: cookie parsing" },
  ],
};

function touchFuture(file: string, secondsAhead = 120): void {
  const t = new Date(Date.now() + secondsAhead * 1000);
  fs.utimesSync(file, t, t);
}

beforeAll(() => {
  priorOffline = process.env.FOURDA_OFFLINE;
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-install-drift-"));
  // The repository boundary: hoisted-workspace lookups never climb above it.
  fs.mkdirSync(path.join(root, ".git"));
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
  fs.rmSync(root, { recursive: true, force: true });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("install state — what node_modules holds", () => {
  it("reads the installed version, including scoped names and hoisted workspaces", () => {
    const dir = project("4.13.5", "4.13.1");
    expect(readInstalledVersion(dir, "hono")).toBe("4.13.1");

    writeInstalled(dir, "@scope/pkg", "2.0.0");
    expect(readInstalledVersion(dir, "@scope/pkg")).toBe("2.0.0");

    // A workspace member whose dependency was hoisted to the workspace root.
    const ws = path.join(root, `ws-${++seq}`);
    const member = path.join(ws, "packages", "member");
    fs.mkdirSync(path.join(member, "node_modules", ".bin"), { recursive: true });
    writeInstalled(ws, "left-pad", "1.3.0");
    expect(readInstalledVersion(member, "left-pad")).toBe("1.3.0");

    expect(readInstalledVersion(dir, "not-installed")).toBeNull();
    fs.mkdirSync(path.join(dir, "node_modules", "broken"), { recursive: true });
    fs.writeFileSync(path.join(dir, "node_modules", "broken", "package.json"), "{ not json");
    expect(readInstalledVersion(dir, "broken")).toBeNull();
  });

  it("names the reinstall command for the lockfile that pinned the version", () => {
    expect(installFixFor("/p/pnpm-lock.yaml")).toBe("pnpm install");
    expect(installFixFor("/p/package-lock.json")).toBe("npm ci");
    expect(installFixFor("/p/yarn.lock")).toBe("yarn install");
    expect(installFixFor("/p/Cargo.lock")).toBeNull();
  });
});

describe("resolution — the lockfile version and the installed version side by side", () => {
  it("resolves hono 4.13.5 from the lockfile and 4.13.1 from node_modules, and queues the installed copy for OSV", () => {
    const dir = project("4.13.5", "4.13.1");
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);

    const hono = li.getResolvedDeps().find((d) => d.name === "hono");
    expect(hono?.version).toBe("4.13.5");
    expect(hono?.installedVersion).toBe("4.13.1");

    expect(li.getInstallDrift()).toEqual([
      { package: "hono", dir, lockfileVersion: "4.13.5", installedVersion: "4.13.1", fix: "pnpm install", isDev: false },
    ]);
    const audit = li.getAuditDeps().filter((d) => d.name === "hono");
    expect(audit.map((d) => d.version).sort()).toEqual(["4.13.1", "4.13.5"]);
    expect(audit.find((d) => d.version === "4.13.1")?.installDriftOf).toBe("4.13.5");
    expect(audit.find((d) => d.version === "4.13.5")?.installDriftOf).toBeUndefined();
  });

  it("reports no drift when the directory has no node_modules", () => {
    const dir = project("4.13.5", null);
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);

    expect(li.getInstallDrift()).toEqual([]);
    expect(li.getResolvedDeps()[0].installedVersion).toBeUndefined();
    expect(li.getAuditDeps().filter((d) => d.name === "hono")).toHaveLength(1);
  });

  it("never compares node_modules against a manifest's version floor", () => {
    const dir = path.join(root, `manifest-only-${++seq}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, "package.json"), JSON.stringify({ dependencies: { hono: "^4.12.34" } }));
    writeInstalled(dir, "hono", "4.13.1");
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);

    // "^4.12.34" read as 4.12.34 is a floor, not a pin: 4.13.1 is not drift.
    expect(li.getResolvedDeps()[0].version).toBe("4.12.34");
    expect(li.getInstallDrift()).toEqual([]);
  });
});

describe("vulnerability_scan — install drift and resolution provenance in the output", () => {
  it("lists the drift with its reinstall command and OSV's answer for the installed copy", async () => {
    const dir = project("4.13.5", "4.13.1");
    const { li, scanned } = liveWithOsv(HONO_4131);
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    vi.spyOn(process, "cwd").mockReturnValue(dir);

    const out = (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;

    // OSV was asked about the installed version, not only the lockfile's.
    expect(scanned[0].map((d) => `${d.name}@${d.version}`).sort()).toEqual(["hono@4.13.1", "hono@4.13.5"]);
    expect(out.install_drift).toHaveLength(1);
    expect(out.install_drift[0]).toMatchObject({
      package: "hono",
      dir: ".",
      lockfile_version: "4.13.5",
      installed_version: "4.13.1",
      vulnerable_installed: true,
      lockfile_version_vulnerable: false,
      fix: "pnpm install",
    });
    expect(out.install_drift[0].note).toContain("The lockfile is patched but node_modules is not");
  });

  it("says in words that the lockfile is patched but node_modules is not, and recommends the reinstall", async () => {
    const dir = project("4.13.5", "4.13.1");
    const { li } = liveWithOsv(HONO_4131);
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    vi.spyOn(process, "cwd").mockReturnValue(dir);

    const out = (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;

    expect(out.vulnerabilities).toHaveLength(3);
    for (const v of out.vulnerabilities) {
      expect(v.current_version).toBe("4.13.1");
      expect(v.installed_version).toBe("4.13.1");
      expect(v.lockfile_version).toBe("4.13.5");
      expect(v.install_note).toContain("the lockfile is patched (pins 4.13.5)");
      expect(v.install_note).toContain("run `pnpm install`");
    }
    // A reinstall, not "Upgrade hono 4.13.1 -> 4.13.4": the lockfile is already past that.
    expect(out.recommendations).toHaveLength(1);
    expect(out.recommendations[0]).toContain("Reinstall hono");
    expect(out.recommendations[0]).toContain("already pins 4.13.5, which is not affected");
    expect(out.recommendations[0]).toContain("`pnpm install`");
    expect(out.recommendations.some((r: string) => r.startsWith("Upgrade hono"))).toBe(false);
  });

  it("reports where and when versions were resolved, and that `cached` is the OSV lookup", async () => {
    const dir = project("4.13.5", null);
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    vi.spyOn(process, "cwd").mockReturnValue(dir);

    const out = (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;
    const meta = out._meta;
    expect(meta.osv_cached).toBe(meta.cached);
    expect(meta.resolution.re_resolved_this_call).toBe(false);
    expect(Date.parse(meta.resolution.resolved_at)).not.toBeNaN();
    expect(meta.resolution.lockfiles).toEqual([
      {
        path: "pnpm-lock.yaml",
        kind: "lockfile",
        mtime: new Date(fs.statSync(path.join(dir, "pnpm-lock.yaml")).mtimeMs).toISOString(),
      },
    ]);
    expect(meta.resolution.note).toContain("not dependency resolution");
    expect(out.install_drift).toEqual([]);
  });
});

describe("re-resolution when a lockfile changes", () => {
  it("re-resolves when the lockfile's mtime changes and drops the stored scan", async () => {
    const dir = project("4.13.5", null);
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    await li.scanVulnerabilities(dir);
    expect(li.getVulnerabilities()).not.toBeNull();

    expect(li.refreshIfLockfilesChanged()).toBe(false);
    touchFuture(path.join(dir, "pnpm-lock.yaml"));
    expect(li.refreshIfLockfilesChanged()).toBe(true);
    expect(li.getVulnerabilities()).toBeNull();
    expect(li.refreshIfLockfilesChanged()).toBe(false);
  });

  it("picks up the rewritten lockfile's versions (the 4.13.3 → 4.13.5 pull)", () => {
    const dir = project("4.13.3", null);
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    expect(li.getResolvedDeps()[0].version).toBe("4.13.3");

    fs.writeFileSync(path.join(dir, "pnpm-lock.yaml"), pnpmLock("4.13.5"));
    touchFuture(path.join(dir, "pnpm-lock.yaml"));
    expect(li.refreshIfLockfilesChanged()).toBe(true);
    expect(li.getResolvedDeps()[0].version).toBe("4.13.5");
    expect(li.getAuditDeps().map((d) => d.version)).toEqual(["4.13.5"]);
  });

  it("notices a lockfile that appears where the resolver had fallen back to the manifest", () => {
    const dir = path.join(root, `late-lock-${++seq}`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, "package.json"), JSON.stringify({ dependencies: { hono: "^4.12.34" } }));
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    expect(li.getResolvedDeps()[0].version).toBe("4.12.34");

    fs.writeFileSync(path.join(dir, "pnpm-lock.yaml"), pnpmLock("4.13.5"));
    expect(li.refreshIfLockfilesChanged()).toBe(true);
    expect(li.getResolvedDeps()[0].version).toBe("4.13.5");
  });

  it("notices a reinstall that fixes the drift without touching the lockfile", () => {
    const dir = project("4.13.5", "4.13.1");
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    expect(li.getInstallDrift()).toHaveLength(1);

    writeInstalled(dir, "hono", "4.13.5");
    fs.writeFileSync(path.join(dir, "node_modules", ".modules.yaml"), "layoutVersion: 5\nreinstalled: true\n");
    expect(li.refreshIfLockfilesChanged()).toBe(true);
    expect(li.getInstallDrift()).toEqual([]);
    expect(li.getResolvedDeps()[0].installedVersion).toBe("4.13.5");
  });

  it("never stores a scan that a re-resolution overtook, and ensureVulnerabilities will not serve it", async () => {
    const dir = project("4.13.5", null);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const { li } = liveWithOsv({}, gate);
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);

    li.startVulnerabilityWarmup(dir);
    const waiting = li.ensureVulnerabilities(dir, 5_000);
    touchFuture(path.join(dir, "pnpm-lock.yaml"));
    expect(li.refreshIfLockfilesChanged()).toBe(true); // another tool call noticed first
    release();

    // The warmup described the old dependency set: not stored, not served.
    await expect(waiting).resolves.toBeNull();
    expect(li.getVulnerabilities()).toBeNull();
  });

  it("vulnerability_scan says it re-resolved on the call that noticed the change", async () => {
    const dir = project("4.13.3", null);
    const { li } = liveWithOsv({});
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    vi.spyOn(process, "cwd").mockReturnValue(dir);

    fs.writeFileSync(path.join(dir, "pnpm-lock.yaml"), pnpmLock("4.13.5"));
    touchFuture(path.join(dir, "pnpm-lock.yaml"));
    const first = (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;
    expect(first._meta.resolution.re_resolved_this_call).toBe(true);
    const second = (await executeVulnerabilityScan(noDb, {}, li)) as Record<string, any>;
    expect(second._meta.resolution.re_resolved_this_call).toBe(false);
  });
});

describe("dependency_health and upgrade_planner show the installed version", () => {
  function offlineIntelWithScan(dir: string): LiveIntelligence {
    process.env.FOURDA_OFFLINE = "true";
    const li = new LiveIntelligence(new Database(":memory:"));
    if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
    else process.env.FOURDA_OFFLINE = priorOffline;
    li.initFromDependencyGroups([{ dir, language: "javascript", deps: ["hono"], devDeps: [] }]);
    const scan = scanOf(li.getAuditDeps(), dir, HONO_4131);
    (li as unknown as { lastVulnScan: VulnerabilityScanResult }).lastVulnScan = scan;
    return li;
  }

  it("dependency_health puts installedVersion beside the lockfile's currentVersion", async () => {
    const dir = project("4.13.5", "4.13.1");
    const result = await executeDependencyHealth(noDb, {}, offlineIntelWithScan(dir));

    const hono = result.dependencies.find((d) => d.name === "hono");
    expect(hono?.currentVersion).toBe("4.13.5");
    expect(hono?.installedVersion).toBe("4.13.1");
    expect(result.vulnerableCount).toBe(1);
    expect(result.summary).toContain("installed at a different version than the lockfile pins");
    expect(result.summary).not.toContain("all healthy");
  });

  it("upgrade_planner turns a vulnerable installed copy of a patched lockfile into a reinstall step", async () => {
    const dir = project("4.13.5", "4.13.1");
    const result = await executeUpgradePlanner(noDb, {}, offlineIntelWithScan(dir));

    const hono = result.recommendations.find((r) => r.package === "hono");
    expect(hono).toBeDefined();
    expect(hono!.currentVersion).toBe("4.13.5");
    expect(hono!.installedVersion).toBe("4.13.1");
    expect(hono!.installFix).toBe("pnpm install");
    expect(hono!.action).toBe("reinstall");
    expect(hono!.targetVersion).toBe("4.13.5");
    expect(hono!.risk).toBe("high");
    expect(hono!.reasons.some((r) => r.includes("node_modules has 4.13.1"))).toBe(true);
    expect(hono!.reasons.some((r) => r.includes("The installed 4.13.1 has 3 known CVEs"))).toBe(true);
    expect(result.summary).toContain("need only a reinstall");
  });

  it("a row with no drift carries no installedVersion", async () => {
    const dir = project("4.13.5", "4.13.5");
    const result = await executeDependencyHealth(noDb, {}, offlineIntelWithScan(dir));
    expect(result.dependencies.find((d) => d.name === "hono")?.installedVersion).toBeUndefined();
  });
});
