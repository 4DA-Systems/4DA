// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for the merged-but-not-running guards (2026-08-30).
 *
 * The live MCP server ran a dist/ built two days before its src/: three
 * already-fixed defects presented as live bugs. And the schema-refused
 * scheduled engine froze the feed for two days with log-only symptoms.
 * These guards make both failure classes name themselves.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { checkBuildStaleness, _resetBuildStalenessCache } from "../build-staleness.js";
import { FourDADatabase } from "../db.js";

let root: string;

beforeEach(() => {
  _resetBuildStalenessCache();
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-staleness-"));
});

afterEach(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

function writeWithMtime(file: string, content: string, mtimeMs: number): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
  fs.utimesSync(file, new Date(mtimeMs), new Date(mtimeMs));
}

describe("checkBuildStaleness", () => {
  it("flags a dist older than src and names the remedy", () => {
    const now = Date.now();
    writeWithMtime(path.join(root, "src", "index.ts"), "// new", now);
    writeWithMtime(path.join(root, "dist", "index.js"), "// old", now - 60 * 60_000);

    const result = checkBuildStaleness(path.join(root, "dist"), path.join(root, "src"));
    expect(result).not.toBeNull();
    expect(result!.stale).toBe(true);
    expect(result!.note).toContain("pnpm --dir mcp-4da-server run build");
  });

  it("passes a dist at least as new as src", () => {
    const now = Date.now();
    writeWithMtime(path.join(root, "src", "index.ts"), "// src", now - 60_000);
    writeWithMtime(path.join(root, "dist", "index.js"), "// built", now);

    const result = checkBuildStaleness(path.join(root, "dist"), path.join(root, "src"));
    expect(result).not.toBeNull();
    expect(result!.stale).toBe(false);
    expect(result!.note).toBeNull();
  });

  it("tolerates clock skew inside the 2s window", () => {
    const now = Date.now();
    writeWithMtime(path.join(root, "src", "index.ts"), "// src", now + 1_000);
    writeWithMtime(path.join(root, "dist", "index.js"), "// built", now);

    const result = checkBuildStaleness(path.join(root, "dist"), path.join(root, "src"));
    expect(result!.stale).toBe(false);
  });

  it("returns null when src/ is absent (published package shape)", () => {
    writeWithMtime(path.join(root, "dist", "index.js"), "// built", Date.now());
    const result = checkBuildStaleness(path.join(root, "dist"), path.join(root, "src"));
    expect(result).toBeNull();
  });

  it("scans nested source files, not just the top level", () => {
    const now = Date.now();
    writeWithMtime(path.join(root, "src", "index.ts"), "// old src", now - 60 * 60_000);
    writeWithMtime(path.join(root, "src", "tools", "deep.ts"), "// new src", now);
    writeWithMtime(path.join(root, "dist", "index.js"), "// built", now - 30 * 60_000);

    const result = checkBuildStaleness(path.join(root, "dist"), path.join(root, "src"));
    expect(result!.stale).toBe(true);
  });
});

describe("engine-block marker in data_freshness", () => {
  function makeDb(dir: string): FourDADatabase {
    const dbPath = path.join(dir, "4da.db");
    const raw = new Database(dbPath);
    raw.exec("CREATE TABLE source_items (id INTEGER PRIMARY KEY, created_at TEXT)");
    raw.close();
    return new FourDADatabase(dbPath);
  }

  it("surfaces the marker's cause and timestamp when present", () => {
    const db = makeDb(root);
    try {
      fs.writeFileSync(
        path.join(root, ".engine-blocked"),
        JSON.stringify({
          at: "2026-08-30T02:00:01Z",
          error: "Database schema version 113 is newer than this version of 4DA supports (max 111)",
        }),
      );
      const freshness = db.getFreshness();
      expect(freshness.engine_blocked_at).toBe("2026-08-30T02:00:01Z");
      expect(freshness.engine_blocked_error).toContain("max 111");
      expect(freshness.note).toContain("ENGINE BLOCKED");
      expect(freshness.note).toContain("rebuilt/updated 4DA binary");
    } finally {
      db.close();
    }
  });

  it("adds nothing when no marker exists", () => {
    const db = makeDb(root);
    try {
      const freshness = db.getFreshness();
      expect(freshness.engine_blocked_at).toBeUndefined();
      expect(freshness.note).not.toContain("ENGINE BLOCKED");
    } finally {
      db.close();
    }
  });

  it("ignores a garbage marker rather than failing the freshness read", () => {
    const db = makeDb(root);
    try {
      fs.writeFileSync(path.join(root, ".engine-blocked"), "not json at all");
      const freshness = db.getFreshness();
      expect(freshness.engine_blocked_at).toBeUndefined();
    } finally {
      db.close();
    }
  });
});
