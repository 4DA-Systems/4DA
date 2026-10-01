// SPDX-License-Identifier: Apache-2.0
/**
 * The relevance judge decides what feed tools show (2026-10-02).
 *
 * Measured 2026-10-01 on the live corpus: of 154 items the pipeline stamped
 * security_alert in 14 days, the judge accepted 10, rejected 62 and never saw
 * 82; 151 of 950 feed-relevant items carried a latest verdict below 0.5. The
 * MCP tools read none of it, so judge-rejected OpenAI company news reached
 * agents as "Critical: Security issue affects openai".
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeGetActionableSignals } from "../tools/get-actionable-signals.js";
import { executeGetRelevantContent } from "../tools/get-relevant-content.js";

const SCHEMA = `
  CREATE TABLE source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT NOT NULL, source_id TEXT NOT NULL, url TEXT,
    title TEXT NOT NULL, content TEXT NOT NULL DEFAULT '', content_hash TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')), last_seen TEXT NOT NULL DEFAULT (datetime('now')),
    relevance_score REAL, content_type TEXT, signal_type TEXT, signal_priority TEXT, feed_relevant INTEGER,
    UNIQUE(source_type, source_id)
  );
  CREATE TABLE llm_judgments (
    id INTEGER PRIMARY KEY AUTOINCREMENT, source_item_id INTEGER NOT NULL, relevance_score REAL NOT NULL,
    explanation TEXT, actions TEXT, confidence REAL, model TEXT, prompt_version TEXT, judged_at TEXT
  );
  CREATE TABLE user_identity (id INTEGER PRIMARY KEY CHECK (id = 1), role TEXT);
  CREATE TABLE tech_stack (id INTEGER PRIMARY KEY AUTOINCREMENT, technology TEXT NOT NULL UNIQUE);
  CREATE TABLE detected_tech (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, category TEXT NOT NULL, confidence REAL DEFAULT 0.5, source TEXT NOT NULL);
  CREATE TABLE active_topics (id INTEGER PRIMARY KEY AUTOINCREMENT, topic TEXT NOT NULL UNIQUE, weight REAL DEFAULT 0.5, confidence REAL DEFAULT 0.5, source TEXT NOT NULL, last_seen TEXT DEFAULT (datetime('now')));
`;

let db: FourDADatabase;
let seq = 0;

beforeEach(() => {
  const raw = new Database(":memory:");
  raw.exec(SCHEMA);
  db = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (db as any).db = raw;
});
afterEach(() => db.close());

function item(over: { title: string; source_type: string; signal_type?: string; signal_priority?: string; verdicts?: number[] }): number {
  seq++;
  const raw = db.getRawDb();
  const id = raw
    .prepare(
      `INSERT INTO source_items (source_type, source_id, url, title, relevance_score, signal_type, signal_priority, feed_relevant)
       VALUES (?, ?, ?, ?, 0.9, ?, ?, 1)`,
    )
    .run(over.source_type, `s-${seq}`, `https://example.com/${seq}`, over.title, over.signal_type ?? null, over.signal_priority ?? null)
    .lastInsertRowid as number;
  for (const v of over.verdicts ?? []) {
    raw.prepare("INSERT INTO llm_judgments (source_item_id, relevance_score) VALUES (?, ?)").run(id, v);
  }
  return id;
}

describe("get_actionable_signals: the judge gates prose, never invents a class", () => {
  it("drops a judge-rejected security headline, keeps an accepted one capped at medium, keeps advisory records", () => {
    const rejected = item({ title: "OpenAI still doesn't seem to have a handle on all of its rogue AI activity", source_type: "hackernews", signal_type: "security_alert", signal_priority: "alert", verdicts: [0.7, 0.1] });
    const accepted = item({ title: "rustls advisory explained", source_type: "hackernews", signal_type: "security_alert", signal_priority: "critical", verdicts: [0.8] });
    const advisory = item({ title: "[CVE-2026-1] hono: header injection", source_type: "cve", signal_type: "security_alert", signal_priority: "critical" });
    const unjudgedProse = item({ title: "A dataset of vulnerable code", source_type: "arxiv", signal_type: "security_alert", signal_priority: "alert" });
    const unclassified = item({ title: "cve patch vulnerability rust", source_type: "hackernews", verdicts: [0.9] });

    const { signals } = executeGetActionableSignals(db, { limit: 50, since_hours: 48 }, null);
    const byId = new Map(signals.map((s) => [s.id, s]));

    expect(byId.has(rejected)).toBe(false);
    expect(byId.has(unjudgedProse)).toBe(false);
    // No keyword classifier: an unstamped item is not a signal, whatever its words.
    expect(byId.has(unclassified)).toBe(false);
    expect(byId.get(accepted)?.signal_priority).toBe("medium");
    expect(byId.get(accepted)?.confidence).toBe(0.8);
    expect(byId.get(advisory)?.signal_priority).toBe("critical");
    expect(byId.get(advisory)?.action).toContain("vulnerability_scan");
  });
});

describe("get_relevant_content: judge-rejected items are not relevant", () => {
  it("filters items whose latest verdict is below 0.5 and reports the verdict", () => {
    const rejected = item({ title: "Restaurant website builder tutorial", source_type: "devto", verdicts: [0.2] });
    const accepted = item({ title: "Tauri 2.9 released", source_type: "hackernews", verdicts: [0.3, 0.9] });
    const unjudged = item({ title: "rusqlite 0.37 notes", source_type: "lobsters" });

    const items = executeGetRelevantContent(db, { limit: 50, since_hours: 24 });
    const byId = new Map(items.map((i) => [i.id, i]));
    expect(byId.has(rejected)).toBe(false);
    expect(byId.get(accepted)?.judge_relevance).toBe(0.9);
    expect(byId.get(unjudged)?.judge_relevance).toBeNull();
  });
});
