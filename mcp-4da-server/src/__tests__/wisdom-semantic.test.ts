// SPDX-License-Identifier: Apache-2.0
/**
 * Tests for OPTIONAL hybrid wisdom retrieval in what_should_i_know.
 *
 * The provider is configured via env (Ollama) and the network is stubbed at
 * `fetch`, so the real embedText + semanticScores + per-table blend run
 * deterministically. The test DB has developer_decisions + agent_memory WITHOUT
 * embedding columns, exercising lazy ensureColumn for both tables.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeWhatShouldIKnow } from "../tools/what-should-i-know.js";
import { executeDecisionMemory } from "../tools/decision-memory.js";
import { executeAgentMemory } from "../tools/agent-memory.js";

function categoryVector(text: string): number[] {
  const t = (text || "").toLowerCase();
  if (/mongo|document|nosql|collection/.test(t)) return [1, 0, 0, 0];
  if (/postgres|\bsql\b|relational/.test(t)) return [0, 1, 0, 0];
  if (/auth|login|session|jwt|oauth/.test(t)) return [0, 0, 1, 0];
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

const SCHEMA = `
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
  CREATE TABLE agent_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    agent_type TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    subject TEXT NOT NULL,
    content TEXT NOT NULL,
    context_tags TEXT DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    promoted_to_decision_id INTEGER
  );
`;

function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.exec(SCHEMA);
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

describe("what_should_i_know hybrid wisdom", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    process.env.FOURDA_EMBED_PROVIDER = "ollama";
    process.env.FOURDA_EMBED_MODEL = "test-embed";
    delete process.env.FOURDA_OFFLINE;
    installEmbeddingStub();
    db = createTestDatabase();
    // A decision that chose Postgres over "mongo".
    executeDecisionMemory(db, {
      action: "record",
      subject: "Datastore choice",
      decision: "Use PostgreSQL for relational data",
      rationale: "Relational integrity matters here.",
      alternatives_rejected: ["mongo"],
      context_tags: ["storage"],
    });
    // An unrelated memory (auth topic).
    executeAgentMemory(db, {
      action: "store",
      session_id: "s1",
      agent_type: "claude_code",
      memory_type: "warning",
      subject: "Auth approach",
      content: "JWT rotation on login",
      context_tags: ["auth"],
    });
  });

  afterEach(() => {
    db.close();
    vi.unstubAllGlobals();
    delete process.env.FOURDA_EMBED_PROVIDER;
    delete process.env.FOURDA_EMBED_MODEL;
  });

  it("surfaces a paraphrased prior decision in the briefing (hybrid mode)", async () => {
    // "document database" shares NO words with the Postgres/mongo decision, but
    // is the same topic — hybrid wisdom should surface it; lexical would not.
    const res = (await executeWhatShouldIKnow(db, {
      task: "plan a document database migration",
    })) as {
      wisdom_recall_mode: string;
      relevant_wisdom: Array<{ subject: string; type: string }>;
    };

    expect(res.wisdom_recall_mode).toBe("hybrid");
    const subjects = res.relevant_wisdom.map((w) => w.subject);
    expect(subjects).toContain("Datastore choice");
    // The unrelated auth memory should NOT surface for a datastore task.
    expect(subjects).not.toContain("Auth approach");
  });

  it("stays lexical (sync) when no embedding provider is configured", () => {
    process.env.FOURDA_OFFLINE = "true";

    const res = executeWhatShouldIKnow(db, {
      task: "plan a document database migration",
    }) as { wisdom_recall_mode: string };

    expect(res).not.toBeInstanceOf(Promise);
    expect(res.wisdom_recall_mode).toBe("ranked_lexical");
  });
});
