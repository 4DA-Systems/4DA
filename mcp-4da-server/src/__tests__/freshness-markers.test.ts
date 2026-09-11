// SPDX-License-Identifier: Apache-2.0
/**
 * The database-recovery voice in data_freshness (5.1.0).
 *
 * The headless refresh engine writes `data/.db-recovered` beside `4da.db`
 * when it restored the database from a backup or quarantined it and started a
 * fresh empty one: `{"at", "kind", "detail"}`. A fresh database looks fresh by
 * every other freshness field, so without this every DB-backed answer would
 * present an empty or partial corpus as the real one. The desktop app shows
 * the marker once and deletes it; the MCP server only reads it.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { dbRecoveryNote, readDbRecoveredMarker, readEngineBlockMarker } from "../freshness-markers.js";

let root: string;

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-recovered-"));
});

afterEach(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

function makeDb(dir: string): FourDADatabase {
  const dbPath = path.join(dir, "4da.db");
  const raw = new Database(dbPath);
  raw.exec("CREATE TABLE source_items (id INTEGER PRIMARY KEY, created_at TEXT)");
  raw.close();
  return new FourDADatabase(dbPath);
}

function writeMarker(content: unknown): string {
  const file = path.join(root, ".db-recovered");
  fs.writeFileSync(file, typeof content === "string" ? content : JSON.stringify(content));
  return file;
}

describe("data_freshness — the .db-recovered marker", () => {
  it("round-trips a restored-from-backup marker into fields and a note, and leaves the marker in place", () => {
    const db = makeDb(root);
    try {
      const marker = writeMarker({
        at: "2026-09-10T20:15:03Z",
        kind: "restored_from_backup",
        detail: "D:/4DA/data/4da.db.corrupt",
      });
      const freshness = db.getFreshness();
      expect(freshness.db_recovered_at).toBe("2026-09-10T20:15:03Z");
      expect(freshness.db_recovery_kind).toBe("restored_from_backup");
      expect(freshness.db_recovery_detail).toBe("D:/4DA/data/4da.db.corrupt");
      expect(freshness.note).toContain(
        "The database was restored from a backup by the background refresh at 2026-09-10T20:15:03Z — results may be incomplete or empty; the preserved file is D:/4DA/data/4da.db.corrupt.",
      );
      // Read-only: the desktop app owns the marker's lifecycle.
      expect(fs.existsSync(marker)).toBe(true);
    } finally {
      db.close();
    }
  });

  it("says a quarantine replaced the database with a fresh empty one", () => {
    const db = makeDb(root);
    try {
      writeMarker({ at: "2026-09-10T20:15:03Z", kind: "quarantined_no_backup", detail: "data/4da.db.corrupt" });
      const freshness = db.getFreshness();
      expect(freshness.db_recovery_kind).toBe("quarantined_no_backup");
      expect(freshness.note).toContain("replaced with a fresh empty database by the background refresh at 2026-09-10T20:15:03Z");
      expect(freshness.note).toContain("the preserved file is data/4da.db.corrupt");
    } finally {
      db.close();
    }
  });

  it("names a failed recovery and its reason", () => {
    const note = dbRecoveryNote({ at: "2026-09-10T20:15:03Z", kind: "recovery_failed", detail: "backup unreadable" });
    expect(note).toContain("failed to recover the database at 2026-09-10T20:15:03Z");
    expect(note).toContain("backup unreadable");
  });

  it("still surfaces a kind this server does not know yet", () => {
    const db = makeDb(root);
    try {
      writeMarker({ at: "2026-09-10T20:15:03Z", kind: "rebuilt_from_wal", detail: "wal replayed" });
      const freshness = db.getFreshness();
      expect(freshness.db_recovery_kind).toBe("rebuilt_from_wal");
      expect(freshness.note).toContain("recorded a database recovery (rebuilt_from_wal)");
    } finally {
      db.close();
    }
  });

  it("adds nothing when the marker is absent", () => {
    const db = makeDb(root);
    try {
      const freshness = db.getFreshness();
      expect(freshness.db_recovered_at).toBeUndefined();
      expect(freshness.db_recovery_kind).toBeUndefined();
      expect(freshness.db_recovery_detail).toBeUndefined();
      expect(freshness.note).not.toContain("background refresh at");
    } finally {
      db.close();
    }
  });

  it("ignores a garbage or malformed marker rather than failing the freshness read", () => {
    const db = makeDb(root);
    try {
      for (const garbage of [
        "not json at all",
        "[1, 2, 3]",
        JSON.stringify({ at: 12, kind: "restored_from_backup", detail: "x" }),
        JSON.stringify({ at: "2026-09-10T20:15:03Z", detail: "x" }),
        JSON.stringify({ at: "", kind: "restored_from_backup", detail: "x" }),
        JSON.stringify({ at: "2026-09-10T20:15:03Z", kind: "restored_from_backup" }),
      ]) {
        writeMarker(garbage);
        const freshness = db.getFreshness();
        expect(freshness.db_recovered_at, garbage).toBeUndefined();
        expect(freshness.db_recovery_kind, garbage).toBeUndefined();
      }
    } finally {
      db.close();
    }
  });

  it("reads no marker for an in-memory database (no directory to read it from)", () => {
    expect(readDbRecoveredMarker(":memory:")).toBeNull();
    expect(readEngineBlockMarker(":memory:")).toBeNull();
  });

  it("the engine-block marker and the recovery marker surface together", () => {
    const db = makeDb(root);
    try {
      fs.writeFileSync(
        path.join(root, ".engine-blocked"),
        JSON.stringify({ at: "2026-09-10T02:00:01Z", error: "schema 123 is newer than this binary supports (max 122)" }),
      );
      writeMarker({ at: "2026-09-10T20:15:03Z", kind: "restored_from_backup", detail: "x.corrupt" });
      const freshness = db.getFreshness();
      expect(freshness.engine_blocked_at).toBe("2026-09-10T02:00:01Z");
      expect(freshness.db_recovered_at).toBe("2026-09-10T20:15:03Z");
      expect(freshness.note).toContain("ENGINE BLOCKED");
      expect(freshness.note).toContain("restored from a backup");
    } finally {
      db.close();
    }
  });
});
