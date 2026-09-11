// SPDX-License-Identifier: Apache-2.0
/**
 * pnpm and yarn lockfile readers, tested against the shapes the tools write.
 *
 * The earlier pnpm fixture had no blank lines between entries, which is not
 * how pnpm writes a lockfile, so a reader whose pattern crossed line breaks
 * passed it while every scoped transitive package in a real lockfile reached
 * OSV as `'@scope/name` (measured 2026-09-11: 525 names in this repository).
 * The last suite reads this repository's own lockfile for that reason.
 */

import { describe, it, expect } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { parsePnpmKey, readPnpmLock, readYarnLock } from "../live/js-lockfile-readers.js";
import { resolveVersionSource } from "../live/lockfile-parsers.js";

function read(content: string, reader = readPnpmLock): Map<string, string> {
  const versions = new Map<string, string>();
  reader(content, versions);
  return versions;
}

/** Keys that are not a bare package name: a quote, a space, a colon, a peer suffix, a version. */
function badKeys(versions: Map<string, string>): string[] {
  return [...versions.keys()].filter((k) => !/^(?:@[^\s/@'"():]+\/)?[^\s/@'"():]+$/.test(k));
}

// pnpm 9 output, blank lines included: the shape that broke the old reader.
const PNPM_V9 = [
  "lockfileVersion: '9.0'",
  "",
  "settings:",
  "  autoInstallPeers: true",
  "",
  "importers:",
  "",
  "  .:",
  "    dependencies:",
  "      '@tanstack/react-virtual':",
  "        specifier: ^3.13.12",
  "        version: 3.13.12(react-dom@19.2.3(react@19.2.3))(react@19.2.3)",
  "    devDependencies:",
  "      eslint:",
  "        specifier: ^9.39.4",
  "        version: 9.39.4(jiti@2.7.0)",
  "",
  "packages:",
  "",
  "  '@humanfs/core@0.19.1':",
  "    resolution: {integrity: sha512-fake}",
  "    engines: {node: '>=18.18.0'}",
  "",
  "  '@humanfs/node@0.16.7':",
  "    resolution: {integrity: sha512-fake}",
  "    engines: {node: '>=18.18.0'}",
  "",
  "  '@tanstack/react-virtual@3.13.12':",
  "    resolution: {integrity: sha512-fake}",
  "    peerDependencies:",
  "      react: ^16.8.0 || ^17.0.0 || ^18.0.0 || ^19.0.0",
  "",
  "  eslint@9.39.4:",
  "    resolution: {integrity: sha512-fake}",
  "    hasBin: true",
  "",
  "  git-dep@https://codeload.github.com/o/r/tar.gz/0123abc:",
  "    resolution: {tarball: https://codeload.github.com/o/r/tar.gz/0123abc}",
  "",
  "snapshots:",
  "",
  "  '@humanfs/core@0.19.1': {}",
  "",
  "  '@humanfs/node@0.16.7':",
  "    dependencies:",
  "      '@humanfs/core': 0.19.1",
  "      '@humanwhocodes/retry': 0.4.3",
  "",
  "  '@tanstack/react-virtual@3.13.12(react-dom@19.2.3(react@19.2.3))(react@19.2.3)':",
  "    dependencies:",
  "      react: 19.2.3",
  "",
  "  eslint@9.39.4(jiti@2.7.0):",
  "    dependencies:",
  "      '@humanfs/node': 0.16.7",
  "",
].join("\n");

describe("readPnpmLock: pnpm 9, as pnpm writes it", () => {
  const versions = read(PNPM_V9);

  it("names a scoped transitive package without its quote", () => {
    expect(versions.get("@humanfs/node")).toBe("0.16.7");
    expect(versions.get("@humanfs/core")).toBe("0.19.1");
    expect(versions.has("'@humanfs/node")).toBe(false);
  });

  it("reads direct pins from the importer, with peer suffixes removed", () => {
    expect(versions.get("@tanstack/react-virtual")).toBe("3.13.12");
    expect(versions.get("eslint")).toBe("9.39.4");
  });

  it("drops keys that do not pin a registry version and invents no names", () => {
    expect(versions.has("git-dep")).toBe(false);
    expect(badKeys(versions)).toEqual([]);
    expect(versions.size).toBe(4);
  });

  it("gives the same answer with or without blank lines, and with CRLF", () => {
    const dense = PNPM_V9.split("\n").filter((l) => l.trim() !== "").join("\n");
    expect(read(dense)).toEqual(versions);
    expect(read(PNPM_V9.replace(/\n/g, "\r\n"))).toEqual(versions);
  });

  it("is what resolveVersionSource reads from a pnpm directory", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "4da-pnpm9-"));
    try {
      fs.writeFileSync(path.join(dir, "pnpm-lock.yaml"), PNPM_V9);
      const source = resolveVersionSource(dir, "npm");
      expect(source.kind).toBe("lockfile");
      expect(source.versions).toEqual(versions);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("readPnpmLock: older lockfile versions", () => {
  it("reads a v6 single-project file", () => {
    const versions = read(
      [
        "lockfileVersion: '6.0'",
        "",
        "dependencies:",
        "  react:",
        "    specifier: ^18.2.0",
        "    version: 18.2.0",
        "",
        "devDependencies:",
        "  '@types/react':",
        "    specifier: ^18.2.0",
        "    version: 18.2.14",
        "",
        "packages:",
        "",
        "  /@babel/core@7.22.5:",
        "    resolution: {integrity: sha512-fake}",
        "",
        "  /react-dom@18.2.0(react@18.2.0):",
        "    resolution: {integrity: sha512-fake}",
        "    dev: false",
        "",
      ].join("\n"),
    );
    expect(Object.fromEntries(versions)).toEqual({
      react: "18.2.0",
      "@types/react": "18.2.14",
      "@babel/core": "7.22.5",
      "react-dom": "18.2.0",
    });
  });

  it("reads a v5 file, peers after an underscore", () => {
    const versions = read(
      [
        "lockfileVersion: 5.4",
        "",
        "specifiers:",
        "  react: ^17.0.2",
        "",
        "dependencies:",
        "  react: 17.0.2",
        "",
        "packages:",
        "",
        "  /@babel/core/7.18.6:",
        "    resolution: {integrity: sha512-fake}",
        "",
        "  /@testing-library/react/12.1.5_react-dom@17.0.2+react@17.0.2:",
        "    resolution: {integrity: sha512-fake}",
        "",
        "  /react-dom/17.0.2_react@17.0.2:",
        "    resolution: {integrity: sha512-fake}",
        "",
      ].join("\n"),
    );
    expect(Object.fromEntries(versions)).toEqual({
      react: "17.0.2",
      "@babel/core": "7.18.6",
      "@testing-library/react": "12.1.5",
      "react-dom": "17.0.2",
    });
  });

  it.each([
    ["'@humanfs/node@0.16.7'", ["@humanfs/node", "0.16.7"]],
    ["eslint@9.39.4(jiti@2.7.0)", ["eslint", "9.39.4"]],
    ["/@babel/core@7.22.5", ["@babel/core", "7.22.5"]],
    ["/react-dom/17.0.2_react@17.0.2", ["react-dom", "17.0.2"]],
    ["typescript@6.0.0-beta", ["typescript", "6.0.0-beta"]],
    ["git-dep@https://codeload.github.com/o/r/tar.gz/0123abc", null],
    ["local@file:../local", null],
  ])("parsePnpmKey(%s)", (key, expected) => {
    expect(parsePnpmKey(key)).toEqual(expected);
  });
});

describe("readYarnLock", () => {
  it("reads scoped packages from a v1 lockfile", () => {
    const versions = read(
      [
        "# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.",
        "# yarn lockfile v1",
        "",
        "",
        '"@babel/core@^7.0.0", "@babel/core@^7.12.3":',
        '  version "7.22.5"',
        '  resolved "https://registry.yarnpkg.com/@babel/core/-/core-7.22.5.tgz"',
        "",
        "lodash@^4.17.21:",
        '  version "4.17.21"',
        "",
      ].join("\n"),
      readYarnLock,
    );
    expect(Object.fromEntries(versions)).toEqual({ "@babel/core": "7.22.5", lodash: "4.17.21" });
  });

  it("reads berry, skipping its metadata and workspace entries", () => {
    const versions = read(
      [
        "__metadata:",
        "  version: 8",
        "  cacheKey: 10c0",
        "",
        '"@babel/core@npm:^7.0.0":',
        "  version: 7.22.5",
        '  resolution: "@babel/core@npm:7.22.5"',
        "",
        '"app@workspace:.":',
        "  version: 0.0.0-use.local",
        '  resolution: "app@workspace:."',
        "",
      ].join("\n"),
      readYarnLock,
    );
    expect(Object.fromEntries(versions)).toEqual({ "@babel/core": "7.22.5" });
  });
});

const repoLock = fileURLToPath(new URL("../../../pnpm-lock.yaml", import.meta.url));

describe.skipIf(!fs.existsSync(repoLock))("readPnpmLock: this repository's own lockfile", () => {
  it("names every scoped package key, and only bare names", () => {
    const content = fs.readFileSync(repoLock, "utf-8");
    const versions = read(content);
    const scoped = new Set([...content.matchAll(/^ {2}'(@[^@']+\/[^@']+)@\d[^']*':\r?$/gm)].map((m) => m[1]));
    expect(scoped.size).toBeGreaterThan(0);
    const missing = [...scoped].filter((name) => !versions.has(name));
    expect(missing).toEqual([]);
    expect(badKeys(versions)).toEqual([]);
  });
});
