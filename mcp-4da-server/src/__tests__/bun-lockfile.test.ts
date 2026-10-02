// SPDX-License-Identifier: Apache-2.0
/**
 * bun.lock: the FastAPI full-stack template's frontend is locked by bun.lock
 * only, and 52 advisories went unscanned before this reader (2026-10-03
 * pre-publish benchmark). Shape follows that real file.
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { readBunLock, stripTrailingCommas } from "../live/bun-lockfile.js";
import { resolveVersionSource } from "../live/lockfile-parsers.js";
import { InstanceSet } from "../live/lockfile-types.js";

const LOCK = `{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "fastapi-cloud",
    },
    "frontend": {
      "name": "frontend",
      "dependencies": {
        "axios": "1.18.0",
        "next": "16.2.6",
      },
      "devDependencies": {
        "@tanstack/router-plugin": "^1.168.23",
      },
    },
  },
  "packages": {
    "frontend": ["frontend@workspace:frontend"],
    "axios": ["axios@1.18.0", "", { "dependencies": { "follow-redirects": "^1.16.0" } }, "sha512-a"],
    "follow-redirects": ["follow-redirects@1.16.0", "", {}, "sha512-b"],
    "next": ["next@16.2.6", "", { "dependencies": { "postcss": "8.4.31" } }, "sha512-c"],
    "postcss": ["postcss@8.5.6", "", {}, "sha512-d"],
    "next/postcss": ["postcss@8.4.31", "", {}, "sha512-e"],
    "@tanstack/router-plugin": ["@tanstack/router-plugin@1.168.23", "", { "dependencies": { "@babel/core": "^7.28.5" } }, "sha512-f"],
    "@babel/core": ["@babel/core@7.28.6", "", {}, "sha512-g"],
    "@tanstack/router-plugin/@babel/core": ["@babel/core@7.28.5", "", {}, "sha512-h"],
    "weird": ["weird@1.0.0", "", { "description": "a string with ,} inside" }, "sha512-i"],
  },
}
`;

describe("readBunLock", () => {
  it("reads every registry copy, hoisted primary, and skips workspace entries", () => {
    const versions = new Map<string, string>();
    const instances = new InstanceSet();
    readBunLock(LOCK, versions, instances);
    expect(versions.get("axios")).toBe("1.18.0");
    expect(versions.get("postcss")).toBe("8.5.6"); // the hoisted copy, not next's nested one
    expect(versions.get("@babel/core")).toBe("7.28.6");
    expect(versions.has("frontend")).toBe(false);
    const all = instances.toArray().map((i) => `${i.name}@${i.version}`).sort();
    expect(all).toEqual([
      "@babel/core@7.28.5",
      "@babel/core@7.28.6",
      "@tanstack/router-plugin@1.168.23",
      "axios@1.18.0",
      "follow-redirects@1.16.0",
      "next@16.2.6",
      "postcss@8.4.31",
      "postcss@8.5.6",
      "weird@1.0.0",
    ]);
  });

  it("knows dev scope from the workspaces: runtime via dependencies, dev only via devDependencies", () => {
    const instances = new InstanceSet();
    readBunLock(LOCK, new Map(), instances);
    const dev = Object.fromEntries(instances.toArray().map((i) => [`${i.name}@${i.version}`, i.dev]));
    expect(dev["axios@1.18.0"]).toBe(false);
    expect(dev["follow-redirects@1.16.0"]).toBe(false);
    expect(dev["postcss@8.4.31"]).toBe(false); // next's nested copy, reached from a runtime dependency
    expect(dev["@tanstack/router-plugin@1.168.23"]).toBe(true);
    expect(dev["@babel/core@7.28.5"]).toBe(true); // the plugin's nested copy
    expect(dev["postcss@8.5.6"]).toBeUndefined(); // reached by nothing declared
  });

  it("drops trailing commas but never touches string contents", () => {
    expect(JSON.parse(stripTrailingCommas('{"a": [1, 2,], "b": "x,}",}'))).toEqual({ a: [1, 2], b: "x,}" });
  });
});

describe("resolveVersionSource with only bun.lock", () => {
  let dir: string;
  beforeAll(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), "bun-lock-"));
    fs.writeFileSync(path.join(dir, "package.json"), JSON.stringify({ dependencies: { axios: "1.18.0" } }));
    fs.writeFileSync(path.join(dir, "bun.lock"), LOCK);
  });
  afterAll(() => fs.rmSync(dir, { recursive: true, force: true }));

  it("resolves from bun.lock as a lockfile, not from package.json ranges", () => {
    const source = resolveVersionSource(dir, "npm");
    expect(source.kind).toBe("lockfile");
    expect(path.basename(source.source ?? "")).toBe("bun.lock");
    expect(source.instances.some((i) => i.name === "postcss" && i.version === "8.4.31")).toBe(true);
  });
});
