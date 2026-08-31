// SPDX-License-Identifier: Apache-2.0
/**
 * Contract tests for 4DA MCP Server tool handlers.
 *
 * These tests create an in-memory SQLite database with the 4DA schema,
 * exercise the tool execute functions, and verify they return the expected
 * structures and handle edge cases (empty DB, invalid params) gracefully.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeGetRelevantContent } from "../tools/get-relevant-content.js";
import { executeGetContext } from "../tools/get-context.js";
import { executeRecordFeedback } from "../tools/record-feedback.js";
import { executeKnowledgeGaps } from "../tools/knowledge-gaps.js";
import { executeGetActionableSignals } from "../tools/get-actionable-signals.js";
import { executeAgentMemory } from "../tools/agent-memory.js";
import { executeDecisionMemory } from "../tools/decision-memory.js";
import { executeCheckDecisionAlignment } from "../tools/decision-enforcement.js";
import { executeWhatShouldIKnow } from "../tools/what-should-i-know.js";

// =============================================================================
// Schema helper — creates all tables that 4DA tools expect to exist
// =============================================================================

/**
 * Minimal 4DA schema needed by the MCP tool layer.
 *
 * Matches the production schema from src-tauri/src/db.rs and
 * src-tauri/src/context_engine.rs. We omit columns and tables
 * that the MCP tools never read.
 */
const SCHEMA_SQL = `
  -- Core content table
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
  CREATE INDEX IF NOT EXISTS idx_source_type ON source_items(source_type);
  CREATE INDEX IF NOT EXISTS idx_source_type_created ON source_items(source_type, created_at);

  -- User identity (singleton row)
  CREATE TABLE IF NOT EXISTS user_identity (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    role TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
  );

  -- Tech stack
  CREATE TABLE IF NOT EXISTS tech_stack (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    technology TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT (datetime('now'))
  );

  -- Domains of interest
  CREATE TABLE IF NOT EXISTS domains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT (datetime('now'))
  );

  -- Explicit interests
  CREATE TABLE IF NOT EXISTS explicit_interests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    weight REAL DEFAULT 1.0,
    embedding BLOB,
    source TEXT DEFAULT 'explicit',
    created_at TEXT DEFAULT (datetime('now'))
  );

  -- Exclusions
  CREATE TABLE IF NOT EXISTS exclusions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT (datetime('now'))
  );

  -- ACE detected tech
  CREATE TABLE IF NOT EXISTS detected_tech (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL,
    confidence REAL DEFAULT 0.5,
    source TEXT NOT NULL,
    evidence TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
  );

  -- ACE active topics
  CREATE TABLE IF NOT EXISTS active_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    weight REAL DEFAULT 0.5,
    confidence REAL DEFAULT 0.5,
    embedding BLOB,
    source TEXT NOT NULL,
    last_seen TEXT DEFAULT (datetime('now')),
    decay_applied INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now'))
  );

  -- Learned topic affinities
  CREATE TABLE IF NOT EXISTS topic_affinities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    embedding BLOB,
    positive_signals INTEGER DEFAULT 0,
    negative_signals INTEGER DEFAULT 0,
    total_exposures INTEGER DEFAULT 0,
    affinity_score REAL DEFAULT 0.0,
    confidence REAL DEFAULT 0.0,
    last_interaction TEXT DEFAULT (datetime('now')),
    decay_applied INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
  );

  -- Anti-topics
  CREATE TABLE IF NOT EXISTS anti_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    rejection_count INTEGER DEFAULT 0,
    confidence REAL DEFAULT 0.0,
    auto_detected INTEGER DEFAULT 1,
    user_confirmed INTEGER DEFAULT 0,
    first_rejection TEXT DEFAULT (datetime('now')),
    last_rejection TEXT DEFAULT (datetime('now')),
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
  );

  -- Interactions (feedback)
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
  CREATE INDEX IF NOT EXISTS idx_interactions_item ON interactions(source_item_id);
  CREATE INDEX IF NOT EXISTS idx_interactions_action ON interactions(action);

  -- Project dependencies (for knowledge-gaps tool)
  CREATE TABLE IF NOT EXISTS project_dependencies (
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

  -- Temporal events (for signal chains, export context, etc.)
  CREATE TABLE IF NOT EXISTS temporal_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    subject TEXT NOT NULL,
    data JSON NOT NULL,
    embedding BLOB,
    source_item_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT
  );

  -- Item relationships
  CREATE TABLE IF NOT EXISTS item_relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_item_id INTEGER NOT NULL,
    related_item_id INTEGER NOT NULL,
    relationship_type TEXT NOT NULL,
    strength REAL DEFAULT 1.0,
    metadata JSON,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  -- Source health
  CREATE TABLE IF NOT EXISTS source_health (
    source_type TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'unknown',
    last_success TEXT,
    last_error TEXT,
    error_count INTEGER NOT NULL DEFAULT 0,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    items_fetched INTEGER NOT NULL DEFAULT 0,
    response_time_ms INTEGER NOT NULL DEFAULT 0,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  -- Developer decisions
  CREATE TABLE IF NOT EXISTS developer_decisions (
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
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (superseded_by) REFERENCES developer_decisions(id)
  );

  -- Agent memory
  CREATE TABLE IF NOT EXISTS agent_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    agent_type TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    subject TEXT NOT NULL,
    content TEXT NOT NULL,
    context_tags TEXT DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    promoted_to_decision_id INTEGER,
    FOREIGN KEY (promoted_to_decision_id) REFERENCES developer_decisions(id)
  );

  -- Briefings
  CREATE TABLE IF NOT EXISTS briefings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    model TEXT,
    item_count INTEGER NOT NULL DEFAULT 0,
    tokens_used INTEGER,
    latency_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
`;

// =============================================================================
// Helper: create a FourDADatabase backed by an in-memory SQLite database
// =============================================================================

/**
 * Creates a FourDADatabase instance backed by a fresh in-memory database
 * with the full 4DA schema applied.
 *
 * The trick: FourDADatabase stores the raw Database in a private field
 * called `db`. We construct a raw in-memory Database, apply the schema,
 * then inject it into a FourDADatabase via Object.create + property set
 * to bypass the constructor that tries to open a file path.
 */
function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.pragma("journal_mode = WAL");
  rawDb.exec(SCHEMA_SQL);

  // Build a FourDADatabase without invoking the file-opening constructor.
  // The class stores its connection in a private field named `db`.
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

/**
 * Inserts a source item into the test database and returns its ID.
 */
function insertSourceItem(
  db: FourDADatabase,
  overrides: Partial<{
    source_type: string;
    source_id: string;
    url: string | null;
    title: string;
    content: string;
    content_hash: string;
    created_at: string;
  }> = {},
): number {
  const rawDb = db.getRawDb();
  const now = new Date().toISOString().replace("T", " ").slice(0, 19);
  const stmt = rawDb.prepare(`
    INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, created_at, last_seen)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
  `);
  const result = stmt.run(
    overrides.source_type ?? "hackernews",
    overrides.source_id ?? `hn-${Date.now()}-${Math.random()}`,
    overrides.url ?? "https://example.com/article",
    overrides.title ?? "Test Article",
    overrides.content ?? "This is test content about Rust and TypeScript.",
    overrides.content_hash ?? `hash-${Date.now()}-${Math.random()}`,
    overrides.created_at ?? now,
    now,
  );
  return result.lastInsertRowid as number;
}

/**
 * Seeds the database with user context data (identity, tech stack, interests, etc.)
 */
function seedUserContext(db: FourDADatabase): void {
  const rawDb = db.getRawDb();

  rawDb.prepare("INSERT INTO user_identity (id, role) VALUES (1, 'Senior Developer')").run();

  const insertTech = rawDb.prepare("INSERT INTO tech_stack (technology) VALUES (?)");
  insertTech.run("rust");
  insertTech.run("typescript");
  insertTech.run("react");
  insertTech.run("sqlite");

  const insertDomain = rawDb.prepare("INSERT INTO domains (domain) VALUES (?)");
  insertDomain.run("developer tools");
  insertDomain.run("privacy");

  const insertInterest = rawDb.prepare(
    "INSERT INTO explicit_interests (topic, weight, source) VALUES (?, ?, 'explicit')",
  );
  insertInterest.run("systems programming", 1.0);
  insertInterest.run("web assembly", 0.8);
  insertInterest.run("local-first software", 0.9);

  const insertExclusion = rawDb.prepare("INSERT INTO exclusions (topic) VALUES (?)");
  insertExclusion.run("cryptocurrency");
  insertExclusion.run("nft");
}

/**
 * Seeds ACE-detected context.
 */
function seedACEContext(db: FourDADatabase): void {
  const rawDb = db.getRawDb();

  const insertTech = rawDb.prepare(
    "INSERT INTO detected_tech (name, category, confidence, source) VALUES (?, ?, ?, ?)",
  );
  insertTech.run("tauri", "framework", 0.9, "manifest");
  insertTech.run("vite", "build-tool", 0.8, "config_file");
  insertTech.run("better-sqlite3", "library", 0.7, "manifest");

  const insertTopic = rawDb.prepare(
    "INSERT INTO active_topics (topic, weight, confidence, source) VALUES (?, ?, ?, ?)",
  );
  insertTopic.run("mcp protocol", 0.8, 0.7, "file_content");
  insertTopic.run("embedding search", 0.6, 0.5, "git_commit");
}

/**
 * Seeds learned preferences (affinities and anti-topics).
 */
function seedLearnedPreferences(db: FourDADatabase): void {
  const rawDb = db.getRawDb();

  const insertAffinity = rawDb.prepare(
    "INSERT INTO topic_affinities (topic, positive_signals, negative_signals, total_exposures, affinity_score, confidence) VALUES (?, ?, ?, ?, ?, ?)",
  );
  insertAffinity.run("rust async", 5, 0, 8, 0.7, 0.6);
  insertAffinity.run("sqlite performance", 3, 1, 6, 0.4, 0.5);

  const insertAnti = rawDb.prepare(
    "INSERT INTO anti_topics (topic, rejection_count, confidence, auto_detected, user_confirmed) VALUES (?, ?, ?, ?, ?)",
  );
  insertAnti.run("blockchain", 4, 0.8, 1, 0);
}

// =============================================================================
// Tests
// =============================================================================

describe("4DA MCP Tool Handlers", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  // ---------------------------------------------------------------------------
  // get_relevant_content
  // ---------------------------------------------------------------------------
  describe("executeGetRelevantContent", () => {
    it("returns an empty array on an empty database", () => {
      // Need user_identity row for getUserContext
      db.getRawDb().prepare("INSERT INTO user_identity (id, role) VALUES (1, NULL)").run();

      const result = executeGetRelevantContent(db, {});
      expect(result).toBeInstanceOf(Array);
      expect(result).toHaveLength(0);
    });

    it("returns items matching user interests", () => {
      seedUserContext(db);
      seedACEContext(db);

      // Insert an item that matches "rust" interest/tech
      insertSourceItem(db, {
        title: "New Rust 2025 Edition Features",
        content: "The Rust programming language announces exciting new features for systems programming.",
      });

      // Insert an item that should be excluded (cryptocurrency)
      insertSourceItem(db, {
        title: "Bitcoin reaches new high",
        content: "Cryptocurrency markets surge as bitcoin and nft trading increases.",
      });

      const result = executeGetRelevantContent(db, {
        min_score: 0.01, // Very low threshold to capture matches
        since_hours: 1,
      });

      expect(result).toBeInstanceOf(Array);
      // The "rust" article should score above the threshold
      // The "cryptocurrency" article should be filtered by exclusion
      for (const item of result) {
        expect(item.title).not.toContain("Bitcoin");
      }
    });

    it("returns items with the expected structure", () => {
      seedUserContext(db);

      insertSourceItem(db, {
        title: "Rust async patterns for systems programming",
        content: "Deep dive into async programming with Rust for developer tools.",
        url: "https://example.com/rust-async",
      });

      const result = executeGetRelevantContent(db, {
        min_score: 0.01,
        since_hours: 1,
        limit: 10,
      });

      if (result.length > 0) {
        const item = result[0];
        expect(item).toHaveProperty("id");
        expect(item).toHaveProperty("source_type");
        expect(item).toHaveProperty("source_id");
        expect(item).toHaveProperty("url");
        expect(item).toHaveProperty("title");
        expect(item).toHaveProperty("content");
        expect(item).toHaveProperty("relevance_score");
        expect(item).toHaveProperty("created_at");
        expect(item).toHaveProperty("discovered_ago");
        expect(typeof item.relevance_score).toBe("number");
        expect(item.relevance_score).toBeGreaterThanOrEqual(0);
        expect(item.relevance_score).toBeLessThanOrEqual(1);
        expect(item).toHaveProperty("evidence_class");
        expect([
          "osv_verified",
          "dependency_grounded",
          "semantic_only",
          "keyword_heuristic",
        ]).toContain(item.evidence_class);
      }
    });

    it("derives evidence_class from grounding on the Rust-scored path", () => {
      seedUserContext(db);
      const raw = db.getRawDb();
      // Promote the test DB to the "Rust-scored" shape so the grounded path runs.
      raw.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      raw.exec("ALTER TABLE source_items ADD COLUMN content_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_priority TEXT");
      raw.exec(
        "CREATE TABLE source_item_dependencies (id INTEGER PRIMARY KEY AUTOINCREMENT, source_item_id INTEGER, package_name TEXT)",
      );

      const grounded = insertSourceItem(db, { title: "React 19 release notes", content: "react" });
      const security = insertSourceItem(db, { title: "CVE in axios", content: "axios advisory" });
      const semantic = insertSourceItem(db, { title: "A Rust blog post", content: "rust musings" });
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.9 WHERE id IN (?, ?, ?)")
        .run(grounded, security, semantic);
      raw.prepare("UPDATE source_items SET content_type = 'security_advisory' WHERE id = ?").run(security);
      const insDep = raw.prepare(
        "INSERT INTO source_item_dependencies (source_item_id, package_name) VALUES (?, ?)",
      );
      insDep.run(grounded, "react");
      insDep.run(security, "axios"); // semantic intentionally gets no dependency row

      const result = executeGetRelevantContent(db, { min_score: 0.5, since_hours: 1, limit: 50 });
      const byId = new Map(result.map((r) => [r.id, r.evidence_class]));
      expect(byId.get(grounded)).toBe("dependency_grounded");
      expect(byId.get(security)).toBe("osv_verified"); // grounded + security advisory
      expect(byId.get(semantic)).toBe("semantic_only"); // no dependency edge
    });

    it("deep 30-day fallback excludes stale-pipeline-version scores", () => {
      // REGRESSION GUARD (scoring-drain elimination, Phase 1b completion): the
      // 720h/zero-floor fallback reaches items whose stored scores come from an
      // OLDER scoring pipeline after a version bump. Those must not be ranked —
      // they are numbers the current brain doesn't stand behind. The guard keys
      // on MAX(scored_pipeline_version) as the live version, so it needs no
      // hardcoded constant.
      seedUserContext(db);
      const raw = db.getRawDb();
      raw.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      raw.exec("ALTER TABLE source_items ADD COLUMN content_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_priority TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN scored_pipeline_version INTEGER DEFAULT 0");

      // Both items live ONLY in the deep window (20 days old) so the 24h and
      // 168h tiers come back empty and the 720h fallback fires.
      const twentyDaysAgo = new Date(Date.now() - 20 * 24 * 60 * 60 * 1000)
        .toISOString()
        .replace("T", " ")
        .slice(0, 19);
      const current = insertSourceItem(db, {
        title: "current-brain item",
        content: "rust",
        created_at: twentyDaysAgo,
      });
      const stale = insertSourceItem(db, {
        title: "stale-epoch item",
        content: "rust",
        created_at: twentyDaysAgo,
      });
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.7, scored_pipeline_version = 17 WHERE id = ?")
        .run(current);
      // Stale item even scores HIGHER — exactly the case that must not win.
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.94, scored_pipeline_version = 16 WHERE id = ?")
        .run(stale);

      const result = executeGetRelevantContent(db, { min_score: 0.5, since_hours: 24, limit: 50 });
      const ids = result.map((r) => r.id);
      expect(ids).toContain(current);
      expect(ids).not.toContain(stale);
    });

    it("never returns items the pipeline explicitly rejected (feed_relevant = 0)", () => {
      // REGRESSION GUARD (v18 look-alike gate): the v18 categorical gate makes an
      // ungrounded registry release non-relevant while deliberately KEEPING its
      // capped score (0.37 / 0.42) for ranking and display. A score-only filter
      // therefore STILL surfaces it — live 2026-07-26, 30 crates_io items inside
      // the 30-day window carried feed_relevant = 0 and were returned anyway.
      // Mirrors the desktop content graph's Phase-95 corpus parity: return what
      // the current brain stands behind, not what merely clears a threshold.
      seedUserContext(db);
      const raw = db.getRawDb();
      raw.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      raw.exec("ALTER TABLE source_items ADD COLUMN content_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_priority TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN feed_relevant INTEGER");

      const curated = insertSourceItem(db, { title: "curated item", content: "rust" });
      const unjudged = insertSourceItem(db, { title: "not yet judged", content: "rust" });
      const rejected = insertSourceItem(db, {
        title: "crates.io: vvva_js v2.0.1",
        content: "rust",
      });
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.5, feed_relevant = 1 WHERE id = ?")
        .run(curated);
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.5, feed_relevant = NULL WHERE id = ?")
        .run(unjudged);
      // The rejected look-alike scores ABOVE the 0.35 default floor — the leak.
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.42, feed_relevant = 0 WHERE id = ?")
        .run(rejected);

      const result = executeGetRelevantContent(db, { min_score: 0.35, since_hours: 24, limit: 50 });
      const ids = result.map((r) => r.id);
      expect(ids).toContain(curated);
      // Cold-start doctrine: unjudged is "not yet curated", NOT "rejected".
      expect(ids).toContain(unjudged);
      expect(ids).not.toContain(rejected);
    });

    it("orders by rank_score when present, falling back to evidence (schema 110)", () => {
      // Evidence/rank separation (desktop audit items 12+26): relevance_score
      // is the batch-independent EVIDENCE score; rank_score is the analysis
      // cycle's batch-relative display rank. Ranked reads order by
      // COALESCE(rank_score, relevance_score) DESC — mirroring the Rust
      // RANKED_ORDER_EXPR (src-tauri/src/db/scoring_queries.rs) — while the
      // min_score membership filter stays on evidence.
      seedUserContext(db);
      const raw = db.getRawDb();
      raw.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      raw.exec("ALTER TABLE source_items ADD COLUMN content_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_priority TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN rank_score REAL");

      // Evidence order would be evidenceHigh > ranked > unranked; the batch
      // layer ranked `ranked` to the top. Ranked order must win, and the
      // never-ranked items must still appear, ordered by their evidence.
      // Titles are deliberately dissimilar: the response path collapses
      // near-duplicate titles, which must not eat these fixtures.
      const evidenceHigh = insertSourceItem(db, { title: "Tokio scheduler internals deep dive", content: "rust" });
      const ranked = insertSourceItem(db, { title: "SQLite WAL checkpoint tuning guide", content: "rust" });
      const unranked = insertSourceItem(db, { title: "Kubernetes operator reconciliation patterns", content: "rust" });
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.80, rank_score = NULL WHERE id = ?")
        .run(evidenceHigh);
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.60, rank_score = 0.92 WHERE id = ?")
        .run(ranked);
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.50, rank_score = NULL WHERE id = ?")
        .run(unranked);

      const result = executeGetRelevantContent(db, { min_score: 0.35, since_hours: 24, limit: 50 });
      const ids = result.map((r) => r.id);
      expect(ids).toEqual([ranked, evidenceHigh, unranked]);

      // Membership stays on EVIDENCE: a huge rank cannot buy membership for
      // an item whose evidence is below the floor.
      const noiseWithRank = insertSourceItem(db, { title: "noise with a stale rank", content: "rust" });
      raw
        .prepare("UPDATE source_items SET relevance_score = 0.10, rank_score = 0.99 WHERE id = ?")
        .run(noiseWithRank);
      const second = executeGetRelevantContent(db, { min_score: 0.35, since_hours: 24, limit: 50 });
      expect(second.map((r) => r.id)).not.toContain(noiseWithRank);
    });

    it("deep-fallback actionable signals never trust stale-version stored signals", () => {
      // get_actionable_signals trusts persisted signal_type/signal_priority at
      // confidence 0.90 — on the deep fallback those columns MUST come from the
      // current pipeline only.
      seedUserContext(db);
      const raw = db.getRawDb();
      raw.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      raw.exec("ALTER TABLE source_items ADD COLUMN content_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_type TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN signal_priority TEXT");
      raw.exec("ALTER TABLE source_items ADD COLUMN scored_pipeline_version INTEGER DEFAULT 0");

      const twentyDaysAgo = new Date(Date.now() - 20 * 24 * 60 * 60 * 1000)
        .toISOString()
        .replace("T", " ")
        .slice(0, 19);
      const stale = insertSourceItem(db, {
        title: "stale security alert",
        content: "cve",
        created_at: twentyDaysAgo,
      });
      const current = insertSourceItem(db, {
        title: "current release item",
        content: "release",
        created_at: twentyDaysAgo,
      });
      raw
        .prepare(
          "UPDATE source_items SET relevance_score = 0.9, scored_pipeline_version = 16, signal_type = 'security_alert', signal_priority = 'critical' WHERE id = ?",
        )
        .run(stale);
      raw
        .prepare(
          "UPDATE source_items SET relevance_score = 0.6, scored_pipeline_version = 17, signal_type = 'release', signal_priority = 'medium' WHERE id = ?",
        )
        .run(current);

      const { signals } = executeGetActionableSignals(db, { since_hours: 24, limit: 50 });
      const ids = signals.map((s) => s.id);
      expect(ids).toContain(current);
      expect(ids).not.toContain(stale);
    });

    it("labels standalone keyword-scored items as keyword_heuristic", () => {
      seedUserContext(db);
      insertSourceItem(db, { title: "Rust async patterns", content: "async rust for developer tools" });
      const result = executeGetRelevantContent(db, { min_score: 0.01, since_hours: 1 });
      for (const item of result) {
        expect(item.evidence_class).toBe("keyword_heuristic");
      }
    });

    it("respects the limit parameter", () => {
      seedUserContext(db);

      // Insert multiple items
      for (let i = 0; i < 10; i++) {
        insertSourceItem(db, {
          title: `Rust and TypeScript integration part ${i}`,
          content: `Article about systems programming with developer tools using react and sqlite.`,
          source_id: `hn-limit-test-${i}`,
        });
      }

      const result = executeGetRelevantContent(db, {
        min_score: 0.01,
        since_hours: 1,
        limit: 3,
      });

      expect(result.length).toBeLessThanOrEqual(3);
    });

    it("respects the source_type filter", () => {
      seedUserContext(db);

      insertSourceItem(db, {
        source_type: "hackernews",
        title: "Rust systems programming news",
        content: "Developer tools built with rust and typescript.",
        source_id: "hn-filter-1",
      });

      insertSourceItem(db, {
        source_type: "arxiv",
        title: "Rust formal verification paper",
        content: "Systems programming with formal methods.",
        source_id: "arxiv-filter-1",
      });

      const result = executeGetRelevantContent(db, {
        min_score: 0.01,
        since_hours: 1,
        source_type: "hackernews",
      });

      for (const item of result) {
        expect(item.source_type).toBe("hackernews");
      }
    });

    it("clamps min_score to valid range", () => {
      seedUserContext(db);

      // Should not throw even with out-of-range values
      const result1 = executeGetRelevantContent(db, { min_score: -5 });
      expect(result1).toBeInstanceOf(Array);

      const result2 = executeGetRelevantContent(db, { min_score: 99 });
      expect(result2).toBeInstanceOf(Array);
    });

    it("clamps limit to valid range", () => {
      seedUserContext(db);

      // Limit 0 should become 1
      const result = executeGetRelevantContent(db, { limit: 0 });
      expect(result).toBeInstanceOf(Array);

      // Limit 999 should be clamped to 100
      const result2 = executeGetRelevantContent(db, { limit: 999 });
      expect(result2).toBeInstanceOf(Array);
    });
  });

  // ---------------------------------------------------------------------------
  // get_context
  // ---------------------------------------------------------------------------
  describe("executeGetContext", () => {
    it("returns default context on an empty database", () => {
      const result = executeGetContext(db, {});
      expect(result).toHaveProperty("role");
      expect(result).toHaveProperty("tech_stack");
      expect(result).toHaveProperty("domains");
      expect(result).toHaveProperty("interests");
      expect(result).toHaveProperty("exclusions");
      expect(result.role).toBeNull();
      expect(result.tech_stack).toEqual([]);
      expect(result.domains).toEqual([]);
      expect(result.interests).toEqual([]);
      expect(result.exclusions).toEqual([]);
    });

    it("includes ACE context when requested", () => {
      seedUserContext(db);
      seedACEContext(db);

      const result = executeGetContext(db, { include_ace: true });
      expect(result.ace).toBeDefined();
      expect(result.ace!.detected_tech).toBeInstanceOf(Array);
      expect(result.ace!.active_topics).toBeInstanceOf(Array);
      expect(result.ace!.detected_tech.length).toBeGreaterThan(0);
      expect(result.ace!.active_topics.length).toBeGreaterThan(0);
    });

    it("excludes ACE context when not requested", () => {
      seedUserContext(db);
      seedACEContext(db);

      const result = executeGetContext(db, { include_ace: false });
      expect(result.ace).toBeUndefined();
    });

    it("learned preferences are permanently empty (implicit capture removed in v20b/schema 105)", () => {
      seedUserContext(db);
      // Even seeded legacy rows are quarantined data the product no longer
      // honors — the shape stays for API stability, the content stays empty.
      seedLearnedPreferences(db);

      const result = executeGetContext(db, { include_learned: true });
      expect(result.learned).toBeDefined();
      expect(result.learned!.topic_affinities).toEqual([]);
      expect(result.learned!.anti_topics).toEqual([]);
    });

    it("excludes learned preferences when not requested", () => {
      seedUserContext(db);
      seedLearnedPreferences(db);

      const result = executeGetContext(db, { include_learned: false });
      expect(result.learned).toBeUndefined();
    });

    it("returns populated identity fields", () => {
      seedUserContext(db);

      const result = executeGetContext(db, { include_ace: false, include_learned: false });
      expect(result.role).toBe("Senior Developer");
      expect(result.tech_stack).toContain("rust");
      expect(result.tech_stack).toContain("typescript");
      expect(result.tech_stack).toContain("react");
      expect(result.domains).toContain("developer tools");
      expect(result.interests.length).toBe(3);
      expect(result.exclusions).toContain("cryptocurrency");
      expect(result.exclusions).toContain("nft");
    });

    it("defaults to including both ACE and learned when params are empty", () => {
      seedUserContext(db);
      seedACEContext(db);
      seedLearnedPreferences(db);

      const result = executeGetContext(db, {});
      expect(result.ace).toBeDefined();
      expect(result.learned).toBeDefined();
    });

    it("interest items have correct structure", () => {
      seedUserContext(db);

      const result = executeGetContext(db, { include_ace: false, include_learned: false });
      for (const interest of result.interests) {
        expect(interest).toHaveProperty("id");
        expect(interest).toHaveProperty("topic");
        expect(interest).toHaveProperty("weight");
        expect(interest).toHaveProperty("source");
        expect(typeof interest.id).toBe("number");
        expect(typeof interest.topic).toBe("string");
        expect(typeof interest.weight).toBe("number");
      }
    });
  });

  // ---------------------------------------------------------------------------
  // record_feedback
  // ---------------------------------------------------------------------------
  describe("executeRecordFeedback", () => {
    it("returns error when required params are missing", () => {
      // Missing item_id
      const result = executeRecordFeedback(db, {
        item_id: 0, // falsy
        source_type: "hackernews",
        action: "click",
      });

      expect(result.success).toBe(false);
      expect(result.message).toContain("required");
    });

    it("returns error for non-existent item", () => {
      const result = executeRecordFeedback(db, {
        item_id: 99999,
        source_type: "hackernews",
        action: "click",
      });

      expect(result.success).toBe(false);
      expect(result.message).toContain("not found");
    });

    it("returns error for invalid action", () => {
      const result = executeRecordFeedback(db, {
        item_id: 1,
        source_type: "hackernews",
        action: "invalid_action" as "click",
      });

      expect(result.success).toBe(false);
      expect(result.message).toContain("Invalid action");
    });

    it("successfully records click feedback", () => {
      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Test article for feedback",
      });

      const result = executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "hackernews",
        action: "click",
      });

      expect(result.success).toBe(true);
      expect(result.message).toContain("click");
      expect(result.interaction_id).toBeDefined();
      expect(typeof result.interaction_id).toBe("number");
    });

    it("successfully records save feedback", () => {
      const itemId = insertSourceItem(db, {
        source_type: "arxiv",
        title: "Test paper for save",
      });

      const result = executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "arxiv",
        action: "save",
      });

      expect(result.success).toBe(true);
      expect(result.message).toContain("save");
    });

    it("successfully records dismiss feedback", () => {
      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Test article to dismiss",
      });

      const result = executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "hackernews",
        action: "dismiss",
      });

      expect(result.success).toBe(true);
    });

    it("successfully records mark_irrelevant feedback", () => {
      const itemId = insertSourceItem(db, {
        source_type: "reddit",
        title: "Irrelevant article",
      });

      const result = executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "reddit",
        action: "mark_irrelevant",
      });

      expect(result.success).toBe(true);
    });

    it("persists feedback to the interactions table", () => {
      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Persistence test",
      });

      executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "hackernews",
        action: "save",
      });

      // Verify the interaction was persisted
      const rawDb = db.getRawDb();
      const row = rawDb
        .prepare("SELECT * FROM interactions WHERE item_id = ?")
        .get(itemId) as { action_type: string; signal_strength: number } | undefined;

      expect(row).toBeDefined();
      expect(row!.action_type).toBe("save");
      // v19: unified onto the canonical ACE strength scale — Save is 1.0
      // (the old MCP-only 0.8 was one of three incompatible scales that
      // poisoned every consumer aggregating signal_strength).
      expect(row!.signal_strength).toBe(1.0);
    });
  });

  // ---------------------------------------------------------------------------
  // knowledge_gaps
  // ---------------------------------------------------------------------------
  describe("executeKnowledgeGaps", () => {
    it("returns an informative message when no dependencies are tracked", () => {
      const result = executeKnowledgeGaps(db, {});

      expect(result).toHaveProperty("gaps");
      expect(result).toHaveProperty("summary");
      expect(result.gaps).toEqual([]);
      expect(result.summary).toContain("No project dependencies");
    });

    it("returns gaps structure with tracked dependencies", () => {
      const rawDb = db.getRawDb();

      // Add a dependency
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "react", "18.2.0", "javascript");

      const result = executeKnowledgeGaps(db, {});

      expect(result).toHaveProperty("gaps");
      expect(result).toHaveProperty("total_dependencies");
      expect(result).toHaveProperty("gaps_found");
      expect(result).toHaveProperty("summary");
      expect(result.total_dependencies).toBe(1);
    });

    it("detects gaps when source items mention tracked dependencies", () => {
      const rawDb = db.getRawDb();

      // Add dependency
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "react", "18.2.0", "javascript");

      // Add a source item mentioning react (not interacted with)
      insertSourceItem(db, {
        title: "React 19 breaking changes and migration guide",
        content: "React 19 introduces significant changes to the API that require migration.",
        source_id: "hn-react-gap-1",
      });

      // A single unread mention grades as "low" (honest quantity grading) —
      // opt in to low to see it.
      const result = executeKnowledgeGaps(db, { min_severity: "low" });

      expect(result.gaps_found).toBeGreaterThan(0);
      expect(result.gaps[0].dependency).toBe("react");
      expect(result.gaps[0].missed_items.length).toBeGreaterThan(0);
      expect(result.gaps[0]).toHaveProperty("gap_severity");
      expect(result.gaps[0]).toHaveProperty("missed_count");
      expect(result.gaps[0]).toHaveProperty("version");
      expect(result.gaps[0]).toHaveProperty("project_path");
      expect(result.gaps[0]).toHaveProperty("language");
    });

    it("filters gaps by severity level", () => {
      const rawDb = db.getRawDb();

      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "lodash", "4.17.21", "javascript");

      // Add a source item (medium severity since just 1 item)
      insertSourceItem(db, {
        title: "Lodash performance improvements in v5",
        content: "Lodash announces major performance improvements.",
        source_id: "hn-lodash-1",
      });

      // Filter for critical only -- should exclude the medium gap
      const result = executeKnowledgeGaps(db, { min_severity: "critical" });
      expect(result.gaps_found).toBe(0);
    });

    it("classifies security-related gaps as critical", () => {
      const rawDb = db.getRawDb();

      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "express", "4.18.0", "javascript");

      // critical requires an actual ADVISORY (cve/osv source or
      // security_advisory content_type) whose TITLE names the dependency.
      insertSourceItem(db, {
        title: "CVE-2024-1234: Critical security vulnerability in express",
        content: "A critical security vulnerability found in express framework.",
        source_id: "cve-express-1",
        source_type: "cve",
      });

      const result = executeKnowledgeGaps(db, { min_severity: "critical" });
      expect(result.gaps_found).toBeGreaterThan(0);
      expect(result.gaps[0].gap_severity).toBe("critical");
    });

    // -------------------------------------------------------------------------
    // Grounding regressions — each guards an observed false-positive class from
    // the 2026-08-21 live audit (substring "invite"→vite, a Gwyneth Paltrow
    // article filed under hono, a 2014 StackOverflow post as "missed intel",
    // and blanket "critical" severity from co-mentioned advisories).
    // -------------------------------------------------------------------------

    it("does not match a package name inside another word (invite is not vite)", () => {
      const rawDb = db.getRawDb();
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "vite", "7.0.0", "javascript");

      insertSourceItem(db, {
        title: "Rocket Reversi multiplayer is live — invite a friend with a room link",
        content: "Sign in, open Multiplayer, then invite a friend. Invited players join instantly.",
        source_id: "hn-reversi-1",
      });

      const result = executeKnowledgeGaps(db, { min_severity: "low" });
      expect(result.gaps_found).toBe(0);
    });

    it("an advisory that merely co-mentions the dep in its body is not critical", () => {
      const rawDb = db.getRawDb();
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "hono", "4.13.2", "javascript");

      insertSourceItem(db, {
        title: "CVE-2026-61663: django CMS missing authorization in render_object_structure",
        content: "The affected stack also bundles hono in unrelated tooling examples.",
        source_id: "cve-django-1",
        source_type: "cve",
      });

      const result = executeKnowledgeGaps(db, { min_severity: "low" });
      expect(result.gaps_found).toBe(1);
      expect(result.gaps[0].gap_severity).toBe("low");
    });

    it("ignores mentions older than the 30-day window", () => {
      const rawDb = db.getRawDb();
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "react", "18.2.0", "javascript");

      insertSourceItem(db, {
        title: "How do I convert an image to Base64 in react?",
        content: "Old react question.",
        source_id: "so-old-react",
        created_at: "2014-01-01 00:00:00",
      });

      const result = executeKnowledgeGaps(db, { min_severity: "low" });
      expect(result.gaps_found).toBe(0);
    });

    it("applies the relevance floor when the scoring column exists", () => {
      const rawDb = db.getRawDb();
      rawDb.exec("ALTER TABLE source_items ADD COLUMN relevance_score REAL");
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "package.json", "react", "18.2.0", "javascript");

      const noiseId = insertSourceItem(db, {
        title: "react mentioned in passing in an off-topic listicle",
        source_id: "hn-react-noise",
      });
      const signalId = insertSourceItem(db, {
        title: "React 19 breaking changes and migration guide",
        source_id: "hn-react-signal",
      });
      rawDb.prepare("UPDATE source_items SET relevance_score = 0.05 WHERE id = ?").run(noiseId);
      rawDb.prepare("UPDATE source_items SET relevance_score = 0.5 WHERE id = ?").run(signalId);

      const result = executeKnowledgeGaps(db, { min_severity: "low" });
      expect(result.gaps_found).toBe(1);
      expect(result.gaps[0].missed_count).toBe(1);
      expect(result.gaps[0].missed_items[0].title).toContain("React 19");
    });
  });

  // ---------------------------------------------------------------------------
  // get_actionable_signals
  // ---------------------------------------------------------------------------
  describe("executeGetActionableSignals", () => {
    it("returns an empty signals array on an empty database", () => {
      seedUserContext(db);

      const result = executeGetActionableSignals(db, {});

      expect(result).toHaveProperty("signals");
      expect(result).toHaveProperty("total");
      expect(result).toHaveProperty("summary");
      expect(result.signals).toEqual([]);
      expect(result.total).toBe(0);
    });

    it("classifies a security alert correctly", () => {
      seedUserContext(db);
      seedACEContext(db);

      insertSourceItem(db, {
        title: "CVE-2025-1234: Critical vulnerability in popular npm package",
        content: "A zero-day exploit has been found affecting the supply chain. Patch immediately.",
        source_id: "hn-cve-1",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
      });

      // May or may not be found depending on relevance scoring
      // But if found, should be classified as security_alert
      const securitySignals = result.signals.filter(
        (s) => s.signal_type === "security_alert",
      );
      if (securitySignals.length > 0) {
        expect(securitySignals[0].signal_type).toBe("security_alert");
        expect(securitySignals[0].triggers.length).toBeGreaterThan(0);
      }
    });

    it("classifies a breaking change correctly", () => {
      seedUserContext(db);
      seedACEContext(db);

      insertSourceItem(db, {
        title: "React 20 breaking change: deprecated lifecycle methods removed",
        content: "Migration guide for the major release dropping support for legacy APIs.",
        source_id: "hn-breaking-1",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
      });

      const breakingSignals = result.signals.filter(
        (s) => s.signal_type === "breaking_change",
      );
      if (breakingSignals.length > 0) {
        expect(breakingSignals[0].signal_type).toBe("breaking_change");
      }
    });

    it("classifies a tool discovery correctly", () => {
      seedUserContext(db);
      seedACEContext(db);

      insertSourceItem(db, {
        title: "Show HN: We built a new open source alternative to Webpack",
        content: "Announcing our new lightweight build tool. Just released and blazing fast.",
        source_id: "hn-tool-1",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
      });

      const toolSignals = result.signals.filter(
        (s) => s.signal_type === "tool_discovery",
      );
      if (toolSignals.length > 0) {
        expect(toolSignals[0].signal_type).toBe("tool_discovery");
      }
    });

    it("returns signals with the expected structure", () => {
      seedUserContext(db);
      seedACEContext(db);

      insertSourceItem(db, {
        title: "CVE-2025-9999: vulnerability in Rust crate tauri",
        content: "Security vulnerability found in tauri framework. Update immediately.",
        source_id: "hn-signal-struct",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
      });

      if (result.signals.length > 0) {
        const signal = result.signals[0];
        expect(signal).toHaveProperty("id");
        expect(signal).toHaveProperty("title");
        expect(signal).toHaveProperty("url");
        expect(signal).toHaveProperty("source_type");
        expect(signal).toHaveProperty("relevance_score");
        expect(signal).toHaveProperty("signal_type");
        expect(signal).toHaveProperty("signal_priority");
        expect(signal).toHaveProperty("action");
        expect(signal).toHaveProperty("triggers");
        expect(signal).toHaveProperty("confidence");
        expect(signal).toHaveProperty("discovered_ago");

        expect(["critical", "high", "medium", "low"]).toContain(signal.signal_priority);
        expect(signal.triggers).toBeInstanceOf(Array);
      }
    });

    it("respects the priority_filter parameter", () => {
      seedUserContext(db);

      // Insert several items
      insertSourceItem(db, {
        title: "Tutorial: how to learn Rust step by step",
        content: "A beginner guide to systems programming with Rust.",
        source_id: "hn-priority-filter-1",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
        priority_filter: "critical",
      });

      for (const signal of result.signals) {
        expect(signal.signal_priority).toBe("critical");
      }
    });

    it("respects the signal_type filter parameter", () => {
      seedUserContext(db);

      insertSourceItem(db, {
        title: "CVE-2025-5678: Security vulnerability in express",
        content: "Critical security exploit found in web framework.",
        source_id: "hn-type-filter-1",
      });
      insertSourceItem(db, {
        title: "Tutorial: Deep dive into Rust async patterns",
        content: "Comprehensive guide to async programming best practices.",
        source_id: "hn-type-filter-2",
      });

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
        signal_type: "learning",
      });

      for (const signal of result.signals) {
        expect(signal.signal_type).toBe("learning");
      }
    });

    it("respects the limit parameter", () => {
      seedUserContext(db);

      for (let i = 0; i < 10; i++) {
        insertSourceItem(db, {
          title: `CVE-2025-${1000 + i}: Vulnerability in package-${i}`,
          content: `Security exploit and zero-day vulnerability found.`,
          source_id: `hn-limit-signal-${i}`,
        });
      }

      const result = executeGetActionableSignals(db, {
        since_hours: 1,
        limit: 2,
      });

      expect(result.signals.length).toBeLessThanOrEqual(2);
    });
  });

  // ---------------------------------------------------------------------------
  // FourDADatabase: getRawDb and close
  // ---------------------------------------------------------------------------
  describe("FourDADatabase lifecycle", () => {
    it("getRawDb returns a usable database instance", () => {
      const rawDb = db.getRawDb();
      expect(rawDb).toBeDefined();

      // Should be able to execute queries
      const result = rawDb
        .prepare("SELECT COUNT(*) as cnt FROM source_items")
        .get() as { cnt: number };
      expect(result.cnt).toBe(0);
    });

    it("close does not throw", () => {
      expect(() => db.close()).not.toThrow();
      // Re-create for afterEach cleanup
      db = createTestDatabase();
    });
  });

  // ---------------------------------------------------------------------------
  // Agent and decision recall
  // ---------------------------------------------------------------------------
  describe("agent and decision recall", () => {
    it("recalls agent memories from content, not just subject or tags", () => {
      executeAgentMemory(db, {
        action: "store",
        session_id: "s1",
        agent_type: "codex",
        memory_type: "warning",
        subject: "token storage",
        content: "Authentication tokens must stay in the OS keychain, never localStorage.",
        context_tags: ["credentials"],
      });

      const result = executeAgentMemory(db, {
        action: "recall",
        query: "auth",
      }) as {
        count: number;
        memories: Array<{ subject: string; matched_fields: string[] }>;
      };

      expect(result.count).toBe(1);
      expect(result.memories[0].subject).toBe("token storage");
      expect(result.memories[0].matched_fields).toContain("content");
    });

    it("detects decision conflicts through aliases in rejected alternatives", async () => {
      executeDecisionMemory(db, {
        action: "record",
        subject: "local database",
        decision: "Use SQLite for local-first storage",
        rationale: "The app must work offline and keep raw data local.",
        alternatives_rejected: ["postgres"],
        context_tags: ["storage"],
      });

      const result = await executeCheckDecisionAlignment(db, {
        technology: "postgresql",
      });

      expect(result.aligned).toBe(false);
      expect(result.conflicts).toHaveLength(1);
      expect(result.relevant_decisions[0].relationship).toBe("conflict");
    });

    it("returns relevant wisdom in pre-task briefings", async () => {
      executeDecisionMemory(db, {
        action: "record",
        subject: "MCP HTTP transport",
        decision: "Keep HTTP transport localhost-only until signed auth is enforced.",
        rationale: "Network exposure without signature verification is unsafe.",
        context_tags: ["mcp", "auth"],
      });
      executeAgentMemory(db, {
        action: "store",
        session_id: "s2",
        agent_type: "codex",
        memory_type: "warning",
        subject: "HTTP auth gap",
        content: "The MCP server must not be exposed remotely without signed auth.",
        context_tags: ["mcp"],
      });

      const result = await executeWhatShouldIKnow(db, {
        task: "Expose MCP HTTP transport for another coding agent",
      });

      expect(result.relevant_wisdom.length).toBeGreaterThanOrEqual(2);
      expect(result.relevant_wisdom.map((w) => w.subject)).toContain("MCP HTTP transport");
      expect(result.relevant_wisdom.map((w) => w.subject)).toContain("HTTP auth gap");
    });
  });

  // ---------------------------------------------------------------------------
  // Cross-tool integration
  // ---------------------------------------------------------------------------
  describe("Cross-tool integration", () => {
    it("knowledge_gaps excludes items that have been clicked or saved", () => {
      const rawDb = db.getRawDb();

      // Add dependency
      rawDb
        .prepare(
          "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language) VALUES (?, ?, ?, ?, ?)",
        )
        .run("/home/user/project", "Cargo.toml", "serde", "1.0.0", "rust");

      // Add source item about serde
      const itemId = insertSourceItem(db, {
        title: "Serde 2.0 release with breaking changes",
        content: "Serde announces major version with new serialization API.",
        source_id: "hn-serde-clicked",
      });

      // Before clicking, there should be a gap
      const gapsBefore = executeKnowledgeGaps(db, {});
      const serdeGapBefore = gapsBefore.gaps.find(
        (g) => g.dependency === "serde",
      );

      // Click the item (record interaction). The app records engagement in the
      // canonical interactions.item_id / .action_type columns — knowledge_gaps must
      // read THOSE to suppress engaged items (the older source_item_id/action
      // columns are unused; seeding them would silently test nothing).
      rawDb
        .prepare(
          "INSERT INTO interactions (item_id, action_type, item_source, signal_strength) VALUES (?, ?, ?, ?)",
        )
        .run(itemId, "click", "hackernews", 0.3);

      // After clicking, knowledge_gaps should not report it
      const gapsAfter = executeKnowledgeGaps(db, {});
      const serdeGapAfter = gapsAfter.gaps.find(
        (g) => g.dependency === "serde",
      );

      // The clicked item must be suppressed: either the serde gap is gone, or it
      // no longer lists the clicked item (strictly fewer missed items than before).
      if (serdeGapBefore) {
        if (serdeGapAfter) {
          expect(serdeGapAfter.missed_count).toBeLessThan(
            serdeGapBefore.missed_count,
          );
          expect(serdeGapAfter.missed_items.some((i) => i.id === itemId)).toBe(false);
        }
        // Otherwise it was completely resolved -- also a valid outcome.
      }
    });
  });
});
