// SPDX-License-Identifier: Apache-2.0
/**
 * Tests for OPTIONAL semantic (hybrid) recall over developer_decisions, used by
 * check_decision_alignment and decision_memory's check_alignment.
 *
 * The provider is configured via env (Ollama) and the network is stubbed at
 * `fetch`, so the real embedText + semanticScores + blend run deterministically.
 * The test DB starts WITHOUT embedding columns, exercising lazy ensureColumn.
 *
 * Key invariant under test: semantic retrieval SURFACES paraphrased decisions and
 * reports them as `possible_conflicts`, but it never fabricates a HARD conflict —
 * hard conflicts remain lexical/alias-grounded.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeDecisionMemory } from "../tools/decision-memory.js";
import { executeCheckDecisionAlignment } from "../tools/decision-enforcement.js";

// Topic one-hot vectors: same-topic texts cosine-1, different topics cosine-0.
function categoryVector(text: string): number[] {
  const t = (text || "").toLowerCase();
  if (/mongo|document|nosql|bson|collection/.test(t)) return [1, 0, 0, 0];
  if (/postgres|sql|relational|acid/.test(t)) return [0, 1, 0, 0];
  if (/redis|cache|kv|memcache/.test(t)) return [0, 0, 1, 0];
  return [0, 0, 0, 1];
}

function installEmbeddingStub() {
  const fetchMock = vi.fn(async (_url: unknown, init?: { body?: string }) => {
    const body = init?.body ? JSON.parse(init.body) : {};
    const text: string = body.prompt ?? body.input ?? "";
    return { ok: true, json: async () => ({ embedding: categoryVector(text) }) } as unknown as Response;
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

const DECISIONS_SCHEMA = `
  CREATE TABLE developer_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_type TEXT NOT NULL,
    subject TEXT NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT,
    alternatives_rejected TEXT DEFAULT '[]',
    context_tags TEXT DEFAULT '[]',
    confidence REAL NOT NULL DEFAULT 0.8,
    status TEXT NOT NULL DEFAULT 'active',
    superseded_by INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
`;

function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.exec(DECISIONS_SCHEMA);
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

function record(
  db: FourDADatabase,
  subject: string,
  decision: string,
  alternatives: string[],
  tags: string[],
): void {
  executeDecisionMemory(db, {
    action: "record",
    subject,
    decision,
    rationale: "test rationale",
    alternatives_rejected: alternatives,
    context_tags: tags,
  });
}

describe("developer_decisions semantic recall", () => {
  let db: FourDADatabase;
  let fetchMock: ReturnType<typeof installEmbeddingStub>;

  beforeEach(() => {
    process.env.FOURDA_EMBED_PROVIDER = "ollama";
    process.env.FOURDA_EMBED_MODEL = "test-embed";
    delete process.env.FOURDA_OFFLINE;
    fetchMock = installEmbeddingStub();
    db = createTestDatabase();
    // A decision that chose Postgres and rejected "mongo".
    record(
      db,
      "Primary datastore",
      "Use PostgreSQL for relational data",
      ["mongo"],
      ["storage"],
    );
    // An unrelated decision.
    record(db, "Cache layer", "Use Redis for caching", ["memcache"], ["cache"]);
  });

  afterEach(() => {
    db.close();
    vi.unstubAllGlobals();
    delete process.env.FOURDA_EMBED_PROVIDER;
    delete process.env.FOURDA_EMBED_MODEL;
  });

  it("surfaces a paraphrased rejection as a possible_conflict (not a hard conflict)", async () => {
    // "document database" was never literally rejected, but it is the same topic
    // as the rejected "mongo" — semantic retrieval should flag it for review.
    const res = (await executeCheckDecisionAlignment(db, {
      technology: "document database",
    })) as {
      aligned: boolean;
      recall_mode: string;
      conflicts: unknown[];
      possible_conflicts: Array<{ subject: string; similarity: number }>;
    };

    expect(res.recall_mode).toBe("hybrid");
    // No LITERAL rejection of "document database" -> not a hard conflict.
    expect(res.conflicts).toHaveLength(0);
    expect(res.aligned).toBe(true);
    // But the paraphrase IS surfaced as a possible conflict.
    expect(res.possible_conflicts.length).toBeGreaterThanOrEqual(1);
    expect(res.possible_conflicts[0].subject).toBe("Primary datastore");
    expect(res.possible_conflicts[0].similarity).toBeGreaterThan(0.6);
  });

  it("still reports a HARD, grounded conflict on a literal match", async () => {
    // "mongo" was literally rejected -> grounded hard conflict (lexical, not semantic).
    const res = (await executeCheckDecisionAlignment(db, {
      technology: "mongo",
    })) as {
      aligned: boolean;
      conflicts: unknown[];
      possible_conflicts: unknown[];
      recall_mode: string;
    };

    expect(res.aligned).toBe(false);
    expect(res.conflicts).toHaveLength(1);
    // A grounded hard conflict is NOT also double-counted as a possible conflict.
    expect(res.possible_conflicts).toHaveLength(0);
  });

  it("decision_memory.check_alignment also runs hybrid retrieval", async () => {
    const res = (await executeDecisionMemory(db, {
      action: "check_alignment",
      technology: "document store",
    })) as {
      aligned: boolean;
      recall_mode: string;
      possible_conflicts: unknown[];
    };

    expect(res.recall_mode).toBe("hybrid");
    expect(res.possible_conflicts.length).toBeGreaterThanOrEqual(1);
  });

  it("stays lexical (sync) for alignment when no provider is configured", () => {
    process.env.FOURDA_OFFLINE = "true";

    const res = executeCheckDecisionAlignment(db, { technology: "postgresql" }) as {
      recall_mode: string;
      possible_conflicts: unknown[];
    };

    expect(res).not.toBeInstanceOf(Promise);
    expect(res.recall_mode).toBe("ranked_lexical");
    expect(res.possible_conflicts).toHaveLength(0); // no semantic scoring offline
  });
});
