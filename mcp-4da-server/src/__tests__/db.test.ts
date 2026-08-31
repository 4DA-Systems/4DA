// SPDX-License-Identifier: Apache-2.0
/**
 * Tests for 4DA MCP Server database resilience layer.
 *
 * Covers:
 * - Database validation with missing file
 * - Database validation with valid file
 * - Query retry on SQLITE_BUSY / SQLITE_LOCKED
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import Database from "better-sqlite3";
import { DEPENDENCY_GROUP_QUERY, FourDADatabase } from "../db.js";

// =============================================================================
// Helper: create an in-memory FourDADatabase (same pattern as tools.test.ts)
// =============================================================================

function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.pragma("journal_mode = WAL");

  // Minimal schema
  rawDb.exec(`
    CREATE TABLE IF NOT EXISTS user_identity (
      id INTEGER PRIMARY KEY CHECK (id = 1),
      role TEXT
    );
    CREATE TABLE IF NOT EXISTS tech_stack (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      technology TEXT NOT NULL UNIQUE
    );
    CREATE TABLE IF NOT EXISTS domains (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      domain TEXT NOT NULL UNIQUE
    );
    CREATE TABLE IF NOT EXISTS explicit_interests (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL UNIQUE,
      weight REAL DEFAULT 1.0,
      embedding BLOB,
      source TEXT DEFAULT 'explicit'
    );
    CREATE TABLE IF NOT EXISTS exclusions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL UNIQUE
    );
    CREATE TABLE IF NOT EXISTS source_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      source_type TEXT NOT NULL,
      source_id TEXT NOT NULL,
      url TEXT,
      title TEXT NOT NULL,
      content TEXT NOT NULL DEFAULT '',
      content_hash TEXT NOT NULL,
      created_at TEXT NOT NULL DEFAULT (datetime('now')),
      last_seen TEXT NOT NULL DEFAULT (datetime('now')),
      UNIQUE(source_type, source_id)
    );
    CREATE TABLE IF NOT EXISTS detected_tech (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL UNIQUE,
      category TEXT NOT NULL,
      confidence REAL DEFAULT 0.5,
      source TEXT NOT NULL,
      evidence TEXT
    );
    CREATE TABLE IF NOT EXISTS active_topics (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL UNIQUE,
      weight REAL DEFAULT 0.5,
      confidence REAL DEFAULT 0.5,
      embedding BLOB,
      source TEXT NOT NULL,
      last_seen TEXT DEFAULT (datetime('now')),
      decay_applied INTEGER DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS topic_affinities (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL UNIQUE,
      embedding BLOB,
      positive_signals INTEGER DEFAULT 0,
      negative_signals INTEGER DEFAULT 0,
      total_exposures INTEGER DEFAULT 0,
      affinity_score REAL DEFAULT 0.0,
      confidence REAL DEFAULT 0.0
    );
    CREATE TABLE IF NOT EXISTS anti_topics (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL UNIQUE,
      rejection_count INTEGER DEFAULT 0,
      confidence REAL DEFAULT 0.0,
      auto_detected INTEGER DEFAULT 1,
      user_confirmed INTEGER DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS interactions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      source_item_id INTEGER,
      item_id INTEGER,
      action TEXT,
      action_type TEXT,
      action_data TEXT,
      item_topics TEXT,
      item_source TEXT,
      signal_strength REAL DEFAULT 0.5,
      timestamp TEXT DEFAULT (datetime('now'))
    );
  `);

  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

// =============================================================================
// Tests: validateDatabase
// =============================================================================

describe("FourDADatabase.validateDatabase", () => {
  it("returns standalone flag with message for a missing file", () => {
    const result = FourDADatabase.validateDatabase(
      "/tmp/nonexistent-4da-test-db-" + Date.now() + ".db",
    );

    expect(result.valid).toBe(false);
    expect(result.standalone).toBe(true);
    expect(result.error).toBeDefined();
    expect(result.error).toContain("Standalone mode");
    expect(result.tables).toBeUndefined();
  });

  it("returns valid with table list for a valid in-memory database written to disk", async () => {
    const { tmpdir } = await import("os");
    const { writeFileSync, unlinkSync } = await import("fs");
    const { join } = await import("path");

    // Create a real SQLite file via better-sqlite3
    const tmpPath = join(tmpdir(), `4da-test-validate-${Date.now()}.db`);
    const tempDb = new Database(tmpPath);
    tempDb.exec("CREATE TABLE test_table (id INTEGER PRIMARY KEY)");
    tempDb.close();

    try {
      const result = FourDADatabase.validateDatabase(tmpPath);

      expect(result.valid).toBe(true);
      expect(result.error).toBeUndefined();
      expect(result.tables).toBeDefined();
      expect(result.tables).toBeInstanceOf(Array);
      expect(result.tables).toContain("test_table");
    } finally {
      // Clean up
      try {
        unlinkSync(tmpPath);
      } catch {
        // Ignore cleanup errors
      }
    }
  });

  it("returns invalid for a corrupt database file", async () => {
    const { tmpdir } = await import("os");
    const { writeFileSync, unlinkSync } = await import("fs");
    const { join } = await import("path");

    // Write garbage bytes to simulate a corrupt file
    const tmpPath = join(tmpdir(), `4da-test-corrupt-${Date.now()}.db`);
    writeFileSync(tmpPath, "this is not a valid sqlite database file at all!");

    try {
      const result = FourDADatabase.validateDatabase(tmpPath);

      expect(result.valid).toBe(false);
      expect(result.error).toBeDefined();
      // The error message will mention failure to open
      expect(result.error!.length).toBeGreaterThan(0);
    } finally {
      try {
        unlinkSync(tmpPath);
      } catch {
        // Ignore cleanup errors
      }
    }
  });
});

// =============================================================================
// Tests: queryWithRetry
// =============================================================================

describe("FourDADatabase.queryWithRetry", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  it("returns the result of a successful query without retrying", () => {
    const result = db.queryWithRetry(() => {
      return db
        .getRawDb()
        .prepare("SELECT 42 as answer")
        .get() as { answer: number };
    });

    expect(result.answer).toBe(42);
  });

  it("retries on SQLITE_BUSY and succeeds on second attempt", () => {
    let attempts = 0;

    const result = db.queryWithRetry(() => {
      attempts++;
      if (attempts === 1) {
        const err = new Error("database is locked") as Error & { code: string };
        err.code = "SQLITE_BUSY";
        throw err;
      }
      return "success";
    }, 1);

    expect(result).toBe("success");
    expect(attempts).toBe(2);
  });

  it("retries on SQLITE_LOCKED and succeeds on second attempt", () => {
    let attempts = 0;

    const result = db.queryWithRetry(() => {
      attempts++;
      if (attempts === 1) {
        const err = new Error("database table is locked") as Error & { code: string };
        err.code = "SQLITE_LOCKED";
        throw err;
      }
      return "success";
    }, 1);

    expect(result).toBe("success");
    expect(attempts).toBe(2);
  });

  it("throws after exhausting retries on persistent SQLITE_BUSY", () => {
    let attempts = 0;

    expect(() => {
      db.queryWithRetry(() => {
        attempts++;
        const err = new Error("database is locked") as Error & { code: string };
        err.code = "SQLITE_BUSY";
        throw err;
      }, 1);
    }).toThrow("database is locked");

    // Should have attempted: 1 initial + 1 retry = 2
    expect(attempts).toBe(2);
  });

  it("does not retry on non-BUSY/LOCKED errors", () => {
    let attempts = 0;

    expect(() => {
      db.queryWithRetry(() => {
        attempts++;
        throw new Error("some other error");
      }, 3);
    }).toThrow("some other error");

    // Should fail immediately without retrying
    expect(attempts).toBe(1);
  });

  it("works with zero retries (no retry)", () => {
    let attempts = 0;

    expect(() => {
      db.queryWithRetry(() => {
        attempts++;
        const err = new Error("database is locked") as Error & { code: string };
        err.code = "SQLITE_BUSY";
        throw err;
      }, 0);
    }).toThrow("database is locked");

    expect(attempts).toBe(1);
  });
});

// =============================================================================
// Standalone-schema compatibility — the is_direct regression
//
// 5.0.2 shipped a minimal standalone schema WITHOUT `is_direct`, while the
// full-DB init branch queries it (DEPENDENCY_GROUP_QUERY). Session 1 of a
// standalone install worked (fresh DB, scan path); session 2+ hit the full-DB
// branch, threw `no such column: is_direct`, and silently disabled
// vulnerability_scan / dependency_health / upgrade_planner. These tests run
// the EXACT production query against both standalone shapes so the two modes
// can never drift apart unnoticed again.
// =============================================================================

describe("standalone schema compatibility (is_direct regression)", () => {
  // These two create a real SQLite file on disk and run the open-path migration
  // over it. That is genuinely slow on a cold hosted Windows runner — measured
  // at 11.7s and 12.6s in the `Fresh clone (windows-latest)` job against
  // vitest's 5s default, while the same tests finish in well under a second
  // locally. They failed that job on timeout alone, with 217 of 219 tests
  // passing and no assertion ever failing: a red build reporting runner speed
  // rather than a regression.
  //
  // The budget is deliberately generous. A timeout should catch a hang, not
  // adjudicate how fast a disk is.
  const COLD_RUNNER_TIMEOUT_MS = 60_000;

  async function withTmpDb(
    name: string,
    fn: (tmpPath: string) => void | Promise<void>,
  ): Promise<void> {
    const { tmpdir } = await import("os");
    const { unlinkSync } = await import("fs");
    const { join } = await import("path");
    const tmpPath = join(
      tmpdir(),
      `4da-test-${name}-${Date.now()}-${Math.floor(Math.random() * 1e6)}.db`,
    );
    try {
      await fn(tmpPath);
    } finally {
      for (const suffix of ["", "-wal", "-shm"]) {
        try {
          unlinkSync(tmpPath + suffix);
        } catch {
          // Ignore cleanup errors
        }
      }
    }
  }

  it("fresh standalone schema satisfies the production dependency-group query", async () => {
    await withTmpDb("standalone-fresh", (tmpPath) => {
      const db = new FourDADatabase(tmpPath);
      try {
        expect(db.isStandalone).toBe(true);
        const raw = db.getRawDb();
        // Scanner-style insert: no is_direct column named — DEFAULT must apply.
        raw
          .prepare(
            "INSERT OR IGNORE INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language) VALUES (?, ?, ?, ?, ?, ?)",
          )
          .run("/tmp/proj", "packagejson", "react", "19.0.0", 0, "javascript");
        const rows = raw.prepare(DEPENDENCY_GROUP_QUERY).all() as Array<{
          package_name: string;
          is_direct: number;
        }>;
        expect(rows).toHaveLength(1);
        expect(rows[0].is_direct).toBe(1);
      } finally {
        db.close();
      }
    });
  }, COLD_RUNNER_TIMEOUT_MS);

  it("legacy 5.0.2 standalone DB (no is_direct) is upgraded on open and the query succeeds", async () => {
    await withTmpDb("standalone-legacy", (tmpPath) => {
      // Session 1 under 5.0.2: minimal schema WITHOUT is_direct.
      const legacy = new Database(tmpPath);
      legacy.exec(`
        CREATE TABLE project_dependencies (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          project_path TEXT NOT NULL,
          manifest_type TEXT NOT NULL,
          package_name TEXT NOT NULL,
          version TEXT,
          is_dev INTEGER DEFAULT 0,
          language TEXT NOT NULL,
          last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
          UNIQUE(project_path, package_name)
        );
      `);
      legacy
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .run("/tmp/proj", "cargotoml", "tokio", "1.50.0", 0, "rust");
      legacy.close();

      // Session 2: the server reopens the existing DB (full-DB branch).
      const db = new FourDADatabase(tmpPath);
      try {
        const rows = db.getRawDb().prepare(DEPENDENCY_GROUP_QUERY).all() as Array<{
          package_name: string;
          is_direct: number;
        }>;
        expect(rows).toHaveLength(1);
        // Backfilled as direct — manifest-scanned deps are direct by definition.
        expect(rows[0].is_direct).toBe(1);
      } finally {
        db.close();
      }
    });
  }, COLD_RUNNER_TIMEOUT_MS);
});
