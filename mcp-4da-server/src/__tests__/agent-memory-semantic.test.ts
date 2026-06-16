// SPDX-License-Identifier: Apache-2.0
/**
 * Tests for OPTIONAL semantic (hybrid) recall in agent_memory.
 *
 * The embedding provider is configured via env (Ollama) and the network is
 * stubbed at `fetch`, so the REAL embedText + semanticScores + blend logic run
 * with deterministic vectors. The test DB intentionally starts WITHOUT the
 * embedding columns, so the lazy ensureColumn migration is covered too.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeAgentMemory } from "../tools/agent-memory.js";

// Category one-hot vectors: same-topic texts are cosine-1, different topics cosine-0.
function categoryVector(text: string): number[] {
  const t = (text || "").toLowerCase();
  if (/auth|login|session|jwt|oauth/.test(t)) return [1, 0, 0, 0];
  if (/deploy|\bci\b|build|pipeline|ship/.test(t)) return [0, 1, 0, 0];
  if (/postgres|database|\bsql\b|vacuum|index/.test(t)) return [0, 0, 1, 0];
  return [0, 0, 0, 1];
}

/** Stub Ollama's /api/embeddings so the real embedText pipeline runs offline. */
function installEmbeddingStub() {
  const fetchMock = vi.fn(async (_url: unknown, init?: { body?: string }) => {
    const body = init?.body ? JSON.parse(init.body) : {};
    const text: string = body.prompt ?? body.input ?? "";
    return { ok: true, json: async () => ({ embedding: categoryVector(text) }) } as unknown as Response;
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

const AGENT_MEMORY_SCHEMA = `
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
  rawDb.exec(AGENT_MEMORY_SCHEMA);
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

function store(db: FourDADatabase, subject: string, content: string, tags: string[]): void {
  executeAgentMemory(db, {
    action: "store",
    agent_type: "claude_code",
    session_id: "s1",
    subject,
    content,
    context_tags: tags,
  });
}

describe("agent_memory semantic recall", () => {
  let db: FourDADatabase;
  let fetchMock: ReturnType<typeof installEmbeddingStub>;

  beforeEach(() => {
    process.env.FOURDA_EMBED_PROVIDER = "ollama";
    process.env.FOURDA_EMBED_MODEL = "test-embed";
    delete process.env.FOURDA_OFFLINE;
    fetchMock = installEmbeddingStub();
    db = createTestDatabase();
    store(db, "Session token handling", "rotate jwt on login", ["auth"]);
    store(db, "CI deploy pipeline", "build and ship the app", ["devops"]);
    store(db, "Postgres index tuning", "vacuum and analyze the table", ["database"]);
  });

  afterEach(() => {
    db.close();
    vi.unstubAllGlobals();
    delete process.env.FOURDA_EMBED_PROVIDER;
    delete process.env.FOURDA_EMBED_MODEL;
  });

  it("recalls a semantically-related memory with no lexical overlap (hybrid mode)", async () => {
    // "authentication flow" shares NO words with "Session token handling /
    // rotate jwt on login", so a purely lexical recall would return nothing.
    const res = (await executeAgentMemory(db, {
      action: "recall",
      query: "authentication flow",
    })) as {
      recall_mode: string;
      count: number;
      newly_embedded: number;
      embedded_candidates: number;
      memories: Array<{ subject: string; semantic_score: number }>;
    };

    expect(res.recall_mode).toBe("hybrid");
    expect(res.count).toBe(1);
    expect(res.memories[0].subject).toBe("Session token handling");
    expect(res.memories[0].semantic_score).toBeGreaterThan(0.9);
    expect(res.newly_embedded).toBe(3);
    expect(res.embedded_candidates).toBe(3);
  });

  it("caches embeddings: a second recall embeds nothing new", async () => {
    await executeAgentMemory(db, { action: "recall", query: "authentication flow" });
    fetchMock.mockClear();

    const res = (await executeAgentMemory(db, {
      action: "recall",
      query: "user login security",
    })) as { recall_mode: string; newly_embedded: number; embedded_candidates: number };

    expect(res.newly_embedded).toBe(0); // all rows already cached
    expect(res.embedded_candidates).toBe(3);
    expect(res.recall_mode).toBe("hybrid");
    // Warm cache: only the query is embedded — one fetch, not four.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("falls back to lexical recall when the query cannot be embedded", async () => {
    fetchMock.mockImplementationOnce(
      async () => ({ ok: false, json: async () => ({}) }) as unknown as Response,
    );

    const res = (await executeAgentMemory(db, {
      action: "recall",
      query: "session token",
    })) as { recall_mode: string; semantic_note?: string; count: number };

    expect(res.recall_mode).toBe("ranked_lexical");
    expect(res.semantic_note).toContain("lexical");
    expect(res.count).toBeGreaterThan(0); // 'session token' still matches lexically
  });

  it("stays fully lexical (sync) when no embedding provider is configured", () => {
    process.env.FOURDA_OFFLINE = "true"; // forces getEmbeddingConfig() -> null

    const res = executeAgentMemory(db, { action: "recall", query: "session" }) as {
      recall_mode: string;
    };

    expect(res).not.toBeInstanceOf(Promise);
    expect(res.recall_mode).toBe("ranked_lexical");
  });
});
