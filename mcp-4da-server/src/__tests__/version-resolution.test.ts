// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for per-directory dependency version resolution.
 *
 * Guards the 4DA database-mode bug where every dependency was resolved against
 * a single global cwd. Rust crates live under src-tauri/ (no Cargo.lock at the
 * repo root), so all of them resolved to a null version and were silently
 * dropped from the OSV vulnerability scan — the scan reported npm-only.
 *
 * The fix resolves each dependency from its OWN manifest directory. These tests
 * assert that contract directly with throwaway lock-file fixtures.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { LiveCache } from "../live/cache.js";
import { resolveAuditVersions, resolveVersions } from "../live/version-resolver.js";
import { LiveIntelligence } from "../live/index.js";
import { extractCvssScore, OsvScanner } from "../live/osv-scanner.js";
import { RateLimiter } from "../live/rate-limiter.js";
import type { ResolvedDependency } from "../live/types.js";

let root: string;
let rustDir: string;
let npmDir: string;
let pnpmDir: string;

beforeAll(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-resolve-"));

  // A Rust crate directory with its own Cargo.lock (mirrors src-tauri/).
  rustDir = path.join(root, "src-tauri");
  fs.mkdirSync(rustDir);
  fs.writeFileSync(
    path.join(rustDir, "Cargo.lock"),
    [
      "[[package]]",
      'name = "tokio"',
      'version = "1.50.0"',
      "",
      "[[package]]",
      'name = "serde"',
      'version = "1.0.200"',
      "",
      "[[package]]",
      'name = "bytes"',
      'version = "1.10.1"',
      "",
    ].join("\n"),
  );

  // An npm package directory at a different location (mirrors the repo root).
  npmDir = path.join(root, "web");
  fs.mkdirSync(npmDir);
  fs.writeFileSync(
    path.join(npmDir, "package-lock.json"),
    JSON.stringify({
      packages: {
        "node_modules/react": { version: "19.2.6" },
        "node_modules/zustand": { version: "5.0.14" },
      },
    }),
  );

  pnpmDir = path.join(root, "pnpm");
  fs.mkdirSync(pnpmDir);
  fs.writeFileSync(
    path.join(pnpmDir, "pnpm-lock.yaml"),
    [
      "lockfileVersion: '9.0'",
      "importers:",
      "  .:",
      "    dependencies:",
      "      react:",
      "        specifier: ^19.0.0",
      "        version: 19.2.6",
      "packages:",
      "  '@scope/transitive@2.3.4':",
      "    resolution: {integrity: fake}",
      "  react@19.2.6:",
      "    resolution: {integrity: fake}",
      "",
    ].join("\n"),
  );
});

afterAll(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

describe("resolveVersions — per-directory", () => {
  it("resolves Rust crates from a subdirectory's Cargo.lock", () => {
    const resolved = resolveVersions(rustDir, ["tokio", "serde"], [], "rust");
    expect(resolved.every((d) => d.ecosystem === "crates.io")).toBe(true);
    expect(resolved.find((d) => d.name === "tokio")?.version).toBe("1.50.0");
    expect(resolved.find((d) => d.name === "serde")?.version).toBe("1.0.200");
    expect(resolved.every((d) => d.isDirect)).toBe(true);
  });

  it("does NOT resolve Rust crates from a directory without Cargo.lock (the bug)", () => {
    // Resolving the same crates from the repo root — where no Cargo.lock exists —
    // yields null versions, which the OSV scanner then drops.
    const resolved = resolveVersions(root, ["tokio", "serde"], [], "rust");
    expect(resolved.every((d) => d.version === null)).toBe(true);
  });

  it("maps 'javascript' language to the npm ecosystem", () => {
    const resolved = resolveVersions(npmDir, ["react"], [], "javascript");
    expect(resolved[0].ecosystem).toBe("npm");
    expect(resolved[0].version).toBe("19.2.6");
  });

  it("enumerates transitive lockfile packages only for the audit resolver", () => {
    const direct = resolveVersions(npmDir, ["react"], [], "javascript");
    const audit = resolveAuditVersions(npmDir, ["react"], [], "javascript");

    expect(direct.map((d) => d.name)).toEqual(["react"]);
    expect(audit.find((d) => d.name === "react")?.isDirect).toBe(true);
    expect(audit.find((d) => d.name === "zustand")?.isDirect).toBe(false);
    expect(audit.find((d) => d.name === "zustand")?.devScopeKnown).toBe(false);
  });

  it("enumerates scoped transitive packages from pnpm v9 lockfiles", () => {
    const audit = resolveAuditVersions(pnpmDir, ["react"], [], "javascript");
    expect(audit.find((d) => d.name === "@scope/transitive")?.version).toBe("2.3.4");
    expect(audit.find((d) => d.name === "@scope/transitive")?.isDirect).toBe(false);
  });
});

describe("LiveIntelligence.initFromDependencyGroups", () => {
  it("resolves each group from its own directory and merges ecosystems", () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([
      { dir: rustDir, language: "rust", deps: ["tokio", "serde"], devDeps: [] },
      { dir: npmDir, language: "javascript", deps: ["react"], devDeps: [] },
    ]);

    const withVersion = li.getResolvedDeps().filter((d) => d.version !== null);
    const ecosystems = new Set(withVersion.map((d) => d.ecosystem));
    expect(ecosystems.has("crates.io")).toBe(true);
    expect(ecosystems.has("npm")).toBe(true);
    expect(withVersion).toHaveLength(3);
    expect(li.getAuditDeps().some((d) => d.name === "bytes" && !d.isDirect)).toBe(true);
    expect(li.getAuditDeps().some((d) => d.name === "zustand" && !d.isDirect)).toBe(true);
  });

  it("deduplicates a crate shared across two workspace groups", () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([
      { dir: rustDir, language: "rust", deps: ["tokio"], devDeps: [] },
      { dir: rustDir, language: "rust", deps: ["tokio"], devDeps: [] },
    ]);
    expect(li.getResolvedDeps().filter((d) => d.name === "tokio")).toHaveLength(1);
  });
});

describe("OsvScanner full-tree batching", () => {
  it("does not silently truncate scans above one OSV batch", async () => {
    const database = new Database(":memory:");
    const cache = new LiveCache(database);
    const scanner = new OsvScanner(cache, new RateLimiter({ osv: { maxPerMinute: 10 } }));
    const deps: ResolvedDependency[] = Array.from({ length: 250 }, (_, index) => ({
      name: `package-${index}`,
      version: "1.0.0",
      ecosystem: "npm",
      isDev: false,
      isDirect: false,
      devScopeKnown: false,
    }));
    for (const dep of deps) {
      cache.set(`osv:${dep.ecosystem}:${dep.name}:${dep.version}`, [], "osv", 3600);
    }

    const result = await scanner.scan(deps, root);
    expect(result.totalScanned).toBe(250);
    expect(result.cleanCount).toBe(250);
  });

  it("does not mistake the CVSS vector version for the vulnerability score", () => {
    expect(extractCvssScore([{ type: "CVSS_V3", score: "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H" }]))
      .toBeNull();
    expect(extractCvssScore([{ type: "CVSS_V3", score: "9.8" }])).toBe(9.8);
  });
});
