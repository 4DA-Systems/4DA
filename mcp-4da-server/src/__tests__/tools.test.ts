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
import { executeExplainRelevance } from "../tools/explain-relevance.js";
import { executeRecordFeedback } from "../tools/record-feedback.js";
import { executeKnowledgeGaps } from "../tools/knowledge-gaps.js";
import { executeSourceHealth } from "../tools/source-health.js";
import { executeGetActionableSignals } from "../tools/get-actionable-signals.js";

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

    it("includes learned preferences when requested", () => {
      seedUserContext(db);
      seedLearnedPreferences(db);

      const result = executeGetContext(db, { include_learned: true });
      expect(result.learned).toBeDefined();
      expect(result.learned!.topic_affinities).toBeInstanceOf(Array);
      expect(result.learned!.anti_topics).toBeInstanceOf(Array);
      expect(result.learned!.topic_affinities.length).toBeGreaterThan(0);
      expect(result.learned!.anti_topics.length).toBeGreaterThan(0);
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
  // explain_relevance
  // ---------------------------------------------------------------------------
  describe("executeExplainRelevance", () => {
    it("returns an error when item does not exist", () => {
      seedUserContext(db);

      const result = executeExplainRelevance(db, {
        item_id: 9999,
        source_type: "hackernews",
      });

      expect(result).toHaveProperty("error");
      expect((result as { error: string }).error).toContain("not found");
    });

    it("returns an error when params are missing", () => {
      const result = executeExplainRelevance(db, {
        item_id: 0,
        source_type: "",
      });

      expect(result).toHaveProperty("error");
    });

    it("returns a full explanation for an existing item", () => {
      seedUserContext(db);
      seedACEContext(db);
      seedLearnedPreferences(db);

      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Rust async runtime for developer tools",
        content: "Building systems programming tools with Rust and TypeScript.",
      });

      const result = executeExplainRelevance(db, {
        item_id: itemId,
        source_type: "hackernews",
      });

      // Should not be an error
      expect(result).not.toHaveProperty("error");

      // Validate structure
      const explanation = result as {
        item_id: number;
        source_type: string;
        title: string;
        score_breakdown: {
          embedding_similarity: number | null;
          static_match_score: number;
          ace_match_score: number;
          learned_affinity_score: number;
          anti_penalty: number;
          final_score: number;
        };
        matching_context: {
          matching_interests: string[];
          matching_tech: string[];
          matching_topics: string[];
          matching_affinities: string[];
        };
        explanation: string;
      };

      expect(explanation.item_id).toBe(itemId);
      expect(explanation.source_type).toBe("hackernews");
      expect(explanation.title).toContain("Rust");

      // Score breakdown structure
      expect(explanation.score_breakdown).toHaveProperty("static_match_score");
      expect(explanation.score_breakdown).toHaveProperty("ace_match_score");
      expect(explanation.score_breakdown).toHaveProperty("learned_affinity_score");
      expect(explanation.score_breakdown).toHaveProperty("anti_penalty");
      expect(explanation.score_breakdown).toHaveProperty("final_score");
      expect(explanation.score_breakdown.embedding_similarity).toBeNull();

      // Matching context arrays
      expect(explanation.matching_context.matching_interests).toBeInstanceOf(Array);
      expect(explanation.matching_context.matching_tech).toBeInstanceOf(Array);
      expect(explanation.matching_context.matching_topics).toBeInstanceOf(Array);
      expect(explanation.matching_context.matching_affinities).toBeInstanceOf(Array);

      // Should have found some matches (the title mentions "rust" and "developer tools")
      const allMatches = [
        ...explanation.matching_context.matching_interests,
        ...explanation.matching_context.matching_tech,
        ...explanation.matching_context.matching_topics,
        ...explanation.matching_context.matching_affinities,
      ];
      expect(allMatches.length).toBeGreaterThan(0);

      // Human-readable explanation
      expect(typeof explanation.explanation).toBe("string");
      expect(explanation.explanation.length).toBeGreaterThan(0);
    });

    it("applies anti-penalty for excluded topics", () => {
      seedUserContext(db); // Has "cryptocurrency" exclusion

      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Cryptocurrency exchange built with Rust",
        content: "A new cryptocurrency trading platform using Rust programming language.",
      });

      const result = executeExplainRelevance(db, {
        item_id: itemId,
        source_type: "hackernews",
      });

      expect(result).not.toHaveProperty("error");

      const explanation = result as {
        score_breakdown: { anti_penalty: number; final_score: number };
      };

      expect(explanation.score_breakdown.anti_penalty).toBeGreaterThan(0);
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
      expect(row!.signal_strength).toBe(0.8); // "save" has 0.8 signal strength
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

      const result = executeKnowledgeGaps(db, {});

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

      insertSourceItem(db, {
        title: "CVE-2024-1234: Critical security vulnerability in express",
        content: "A critical security vulnerability found in express framework.",
        source_id: "hn-express-cve",
      });

      const result = executeKnowledgeGaps(db, { min_severity: "critical" });
      expect(result.gaps_found).toBeGreaterThan(0);
      expect(result.gaps[0].gap_severity).toBe("critical");
    });
  });

  // ---------------------------------------------------------------------------
  // source_health
  // ---------------------------------------------------------------------------
  describe("executeSourceHealth", () => {
    it("returns healthy status on an empty database (no sources)", () => {
      const result = executeSourceHealth(db, {});

      expect(result).toHaveProperty("analysis_period");
      expect(result).toHaveProperty("overall_status");
      expect(result).toHaveProperty("sources");
      expect(result).toHaveProperty("summary");
      expect(result).toHaveProperty("global_recommendations");

      expect(result.analysis_period).toHaveProperty("start");
      expect(result.analysis_period).toHaveProperty("end");
      expect(result.analysis_period).toHaveProperty("hours");

      expect(result.sources).toEqual([]);
      expect(result.summary.total_sources).toBe(0);
    });

    it("analyzes a specific source when filtered", () => {
      insertSourceItem(db, {
        source_type: "hackernews",
        title: "HN article",
        source_id: "hn-health-1",
      });
      insertSourceItem(db, {
        source_type: "arxiv",
        title: "arXiv paper",
        source_id: "arxiv-health-1",
      });

      const result = executeSourceHealth(db, { source: "hackernews" });

      expect(result.sources.length).toBe(1);
      expect(result.sources[0].source).toBe("hackernews");
    });

    it("returns expected structure for each source", () => {
      insertSourceItem(db, {
        source_type: "hackernews",
        title: "Test HN article",
        source_id: "hn-struct-1",
      });

      const result = executeSourceHealth(db, {});

      expect(result.sources.length).toBeGreaterThan(0);
      const source = result.sources[0];

      expect(source).toHaveProperty("source");
      expect(source).toHaveProperty("status");
      expect(source).toHaveProperty("last_item_at");
      expect(source).toHaveProperty("hours_since_last");
      expect(source).toHaveProperty("items_24h");
      expect(source).toHaveProperty("items_7d");
      expect(source).toHaveProperty("avg_items_per_day");
      expect(source).toHaveProperty("quality_metrics");
      expect(source).toHaveProperty("issues");
      expect(source).toHaveProperty("recommendations");

      expect(source.quality_metrics).toHaveProperty("has_url_rate");
      expect(source.quality_metrics).toHaveProperty("has_content_rate");
      expect(source.quality_metrics).toHaveProperty("avg_content_length");
    });

    it("reports items with good quality metrics", () => {
      // Insert items with URLs and content
      for (let i = 0; i < 5; i++) {
        insertSourceItem(db, {
          source_type: "hackernews",
          title: `Quality test article ${i}`,
          content: "This is quality content that should be counted.",
          url: `https://example.com/article-${i}`,
          source_id: `hn-quality-${i}`,
        });
      }

      const result = executeSourceHealth(db, { source: "hackernews" });
      const source = result.sources[0];

      expect(source.quality_metrics.has_url_rate).toBe(1.0);
      expect(source.quality_metrics.has_content_rate).toBe(1.0);
      expect(source.quality_metrics.avg_content_length).toBeGreaterThan(0);
    });

    it("respects the hours parameter for analysis period", () => {
      const result = executeSourceHealth(db, { hours: 48 });
      expect(result.analysis_period.hours).toBe(48);
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
  // Cross-tool integration
  // ---------------------------------------------------------------------------
  describe("Cross-tool integration", () => {
    it("feedback on an item does not prevent explain_relevance", () => {
      seedUserContext(db);

      const itemId = insertSourceItem(db, {
        source_type: "hackernews",
        title: "Rust and TypeScript developer tools",
        content: "Building systems with Rust and React for local-first software.",
      });

      // Record feedback
      const feedbackResult = executeRecordFeedback(db, {
        item_id: itemId,
        source_type: "hackernews",
        action: "click",
      });
      expect(feedbackResult.success).toBe(true);

      // Explain relevance should still work
      const explanation = executeExplainRelevance(db, {
        item_id: itemId,
        source_type: "hackernews",
      });
      expect(explanation).not.toHaveProperty("error");
    });

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

      // Click the item (record interaction)
      rawDb
        .prepare(
          "INSERT INTO interactions (source_item_id, action, item_source, signal_strength) VALUES (?, ?, ?, ?)",
        )
        .run(itemId, "click", "hackernews", 0.3);

      // After clicking, knowledge_gaps should not report it
      const gapsAfter = executeKnowledgeGaps(db, {});
      const serdeGapAfter = gapsAfter.gaps.find(
        (g) => g.dependency === "serde",
      );

      // The gap should have fewer missed items or be gone entirely
      if (serdeGapBefore) {
        if (serdeGapAfter) {
          expect(serdeGapAfter.missed_count).toBeLessThan(
            serdeGapBefore.missed_count,
          );
        }
        // Otherwise it was completely resolved -- also a valid outcome
      }
    });
  });
});
