// SPDX-License-Identifier: Apache-2.0
/**
 * Regressions from the 2026-10-02 agent eval:
 *
 * 1. `by_severity` read "medium 3, low 1, unknown 9" beside three listed rows:
 *    it counted every raw OSV row (alias duplicates, maintenance notices,
 *    other-platform advisories) instead of the findings it listed.
 * 2. rsa 0.9.10, built for relay and not for src-tauri on Windows, read
 *    "not built for x86_64-pc-windows-msvc" beside `platform_active: true`:
 *    the merged entry kept the inactive copy's target label.
 */
import { describe, expect, it } from "vitest";
import { dedupeDependencies } from "../live/dependency-set.js";
import type { ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";
import { formatScanResult } from "../tools/vulnerability-scan-format.js";

function vuln(over: Partial<VulnerabilityEntry>): VulnerabilityEntry {
  return {
    package: "pkg",
    currentVersion: "1.0.0",
    ecosystem: "crates.io",
    isDev: false,
    isDirect: true,
    devScopeKnown: true,
    vulnId: "GHSA-0000-0000-0000",
    aliases: [],
    severity: "medium",
    cvssScore: 5,
    summary: "A bug",
    fixedVersion: "1.0.1",
    published: "2026-01-01T00:00:00Z",
    references: [],
    target: null,
    platformActive: true,
    sourceDirs: ["/repo"],
    ...over,
  };
}

function scan(vulnerabilities: VulnerabilityEntry[]): VulnerabilityScanResult {
  return {
    scannedAt: "2026-10-02T00:00:00Z",
    projectPath: "/repo",
    ecosystemsScanned: ["crates.io"],
    totalScanned: 10,
    totalVulnerable: vulnerabilities.length,
    platformInactiveVulnerable: 1,
    bySeverity: { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 },
    vulnerabilities,
    cleanCount: 5,
    scanDurationMs: 1,
    cached: false,
    offline: false,
  };
}

describe("vulnerability_scan severity counts", () => {
  it("counts exactly the listed findings", () => {
    const out = formatScanResult(
      scan([
        vuln({ package: "a", vulnId: "GHSA-a", aliases: ["RUSTSEC-a"], severity: "high" }),
        // Same finding under its other id: collapsed, counted once.
        vuln({ package: "a", vulnId: "RUSTSEC-a", aliases: ["GHSA-a"], severity: "high" }),
        vuln({ package: "b", vulnId: "GHSA-b", severity: "medium" }),
        // Maintenance notice and an other-platform advisory: listed elsewhere, not counted.
        vuln({ package: "c", vulnId: "RUSTSEC-c", severity: "unknown", summary: "c is unmaintained" }),
        vuln({ package: "d", vulnId: "GHSA-d", severity: "low", platformActive: false, target: "cfg(unix)" }),
      ]),
      undefined,
      { resolution: null, reResolvedThisCall: false, drift: [], includeDev: true },
    );
    expect(out.vulnerabilities.map((v) => v.package)).toEqual(["a", "b"]);
    expect(out.by_severity).toEqual({ critical: 0, high: 1, medium: 1, low: 0, unknown: 0 });
    expect(out.advisory_by_severity).toEqual({ critical: 0, high: 1, medium: 1, low: 0, unknown: 0 });
    expect(out.maintenance_notices).toHaveLength(1);
    expect(out.platform_inactive_vulnerabilities).toHaveLength(1);
  });
});

describe("dedupeDependencies target label", () => {
  const dep = (over: Partial<ResolvedDependency>): ResolvedDependency =>
    ({
      name: "rsa",
      version: "0.9.10",
      ecosystem: "crates.io",
      isDirect: false,
      isDev: false,
      devScopeKnown: true,
      platformActive: true,
      target: null,
      sourceDirs: [],
      ...over,
    }) as ResolvedDependency;

  it("takes the active copy's label when the inactive copy came first", () => {
    const [merged] = dedupeDependencies([
      dep({ platformActive: false, target: "cfg(not(windows))", sourceDirs: ["/repo/src-tauri"] }),
      dep({ platformActive: true, target: null, sourceDirs: ["/repo/relay"] }),
    ]);
    expect(merged.platformActive).toBe(true);
    expect(merged.target).toBeNull();
    expect(merged.sourceDirs).toEqual(["/repo/src-tauri", "/repo/relay"]);
  });

  it("keeps the active label when an inactive copy follows", () => {
    const [merged] = dedupeDependencies([
      dep({ platformActive: true, target: null }),
      dep({ platformActive: false, target: "cfg(not(windows))" }),
    ]);
    expect(merged.platformActive).toBe(true);
    expect(merged.target).toBeNull();
  });

  it("fills a missing label from a copy in the same state", () => {
    const [merged] = dedupeDependencies([
      dep({ platformActive: false, target: null }),
      dep({ platformActive: false, target: "cfg(unix)" }),
    ]);
    expect(merged.target).toBe("cfg(unix)");
  });
});
