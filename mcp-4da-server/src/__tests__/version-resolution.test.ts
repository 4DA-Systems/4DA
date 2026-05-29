// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
import { resolveVersions } from "../live/version-resolver.js";
import { LiveIntelligence } from "../live/index.js";

let root: string;
let rustDir: string;
let npmDir: string;

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
});

describe("LiveIntelligence.initFromDependencyGroups", () => {
  it("resolves each group from its own directory and merges ecosystems", () => {
    const li = new LiveIntelligence(new Database(":memory:"));
    li.initFromDependencyGroups([
      { dir: rustDir, language: "rust", deps: ["tokio", "serde"], devDeps: [] },
      { dir: npmDir, language: "javascript", deps: ["react", "zustand"], devDeps: [] },
    ]);

    const withVersion = li.getResolvedDeps().filter((d) => d.version !== null);
    const ecosystems = new Set(withVersion.map((d) => d.ecosystem));
    expect(ecosystems.has("crates.io")).toBe(true);
    expect(ecosystems.has("npm")).toBe(true);
    expect(withVersion).toHaveLength(4);
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
