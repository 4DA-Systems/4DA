/**
 * Database module for 4DA MCP Server
 *
 * Read-only access to the 4DA SQLite database.
 * Only the record_feedback function performs writes.
 */

import path from "path";
import * as fs from "fs";
import * as os from "os";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

// Type-only import (erased at compile time) — keeps Database.Database type usable.
// Runtime import is dynamic below, so native binding failures get a clear error message.
import type BetterSqlite3 from "better-sqlite3";

let Database: typeof BetterSqlite3;
try {
  Database = (await import("better-sqlite3")).default;
} catch (err) {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(
    "\n  [4DA] Failed to load better-sqlite3 native bindings.\n\n" +
    "  This usually means your system is missing C++ build tools.\n" +
    "  Install the appropriate tools for your platform:\n\n" +
    "    macOS:   xcode-select --install\n" +
    "    Ubuntu:  sudo apt install build-essential python3\n" +
    "    Windows: npm install -g windows-build-tools\n\n" +
    `  Error: ${msg}\n\n` +
    "  After installing build tools, run: npm rebuild better-sqlite3\n"
  );
  process.exit(1);
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
import type {
  SourceItem,
  RelevantItem,
  Interest,
  DetectedTech,
  ActiveTopic,
  TopicAffinity,
  AntiTopic,
  UserContext,
  ScoreBreakdown,
  MatchingContext,
  RelevanceExplanation,
  FeedbackAction,
  FeedbackResult,
} from "./types.js";

import type { ProjectScanResult } from "./project-scanner.js";

/**
 * Resolve the database path by checking multiple locations in priority order:
 * 1. FOURDA_DB_PATH env var
 * 2. data/4da.db relative to cwd (development)
 * 3. data/4da.db relative to project root (mcp-4da-server is inside project root)
 * 4. Platform-specific Tauri app data dirs (production)
 * 5. Final fallback: data/4da.db relative to cwd
 */
function getDefaultDbPath(): string {
  // 1. Environment variable (highest priority)
  if (process.env.FOURDA_DB_PATH) {
    return process.env.FOURDA_DB_PATH;
  }

  // 2. Relative to cwd (development)
  const cwdPath = path.resolve(process.cwd(), "data", "4da.db");
  if (fs.existsSync(cwdPath)) {
    return cwdPath;
  }

  // 3. Relative to project root (mcp-4da-server is inside project root)
  const projectRootPath = path.resolve(__dirname, "..", "..", "data", "4da.db");
  if (fs.existsSync(projectRootPath)) {
    return projectRootPath;
  }

  // 4. Platform-specific Tauri app data dirs (production)
  const platform = process.platform;
  let appDataPath: string;
  if (platform === "win32") {
    appDataPath = path.join(
      process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"),
      "com.4da.app",
      "data",
      "4da.db"
    );
  } else if (platform === "darwin") {
    appDataPath = path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "com.4da.app",
      "data",
      "4da.db"
    );
  } else {
    appDataPath = path.join(
      os.homedir(),
      ".local",
      "share",
      "com.4da.app",
      "data",
      "4da.db"
    );
  }
  if (fs.existsSync(appDataPath)) {
    return appDataPath;
  }

  // 5. Final fallback
  return path.resolve(process.cwd(), "data", "4da.db");
}

// =============================================================================
// Database Validation
// =============================================================================

export interface DatabaseValidationResult {
  valid: boolean;
  error?: string;
  tables?: string[];
  /** True when no existing DB was found — standalone mode will create one */
  standalone?: boolean;
}

/**
 * 4DA Database accessor
 */
export class FourDADatabase {
  private db: BetterSqlite3.Database;
  private _isStandalone: boolean = false;

  constructor(dbPath?: string) {
    const resolvedPath = dbPath || getDefaultDbPath();

    // Resolve path - if relative, resolve from cwd
    const absolutePath = path.isAbsolute(resolvedPath)
      ? resolvedPath
      : path.resolve(process.cwd(), resolvedPath);

    const isNew = !fs.existsSync(absolutePath);

    // Ensure parent directory exists (standalone mode may need to create it)
    if (isNew) {
      const dir = path.dirname(absolutePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
    }

    try {
      this.db = new Database(absolutePath, { readonly: false }); // Need write for feedback
      this.db.pragma("journal_mode = WAL");
    } catch (error) {
      throw new Error(
        `Failed to open 4DA database at ${absolutePath}: ${error instanceof Error ? error.message : String(error)}`
      );
    }

    // Standalone mode: create schema for a brand-new database
    if (isNew) {
      this.createMinimalSchema();
      this._isStandalone = true;
    }
  }

  /**
   * Whether this database was freshly created in standalone mode
   * (no pre-existing 4DA desktop app database found).
   */
  get isStandalone(): boolean {
    return this._isStandalone;
  }

  /**
   * Validate that a database file exists, is readable, and passes integrity checks.
   *
   * Use this before accepting tool calls to ensure the database is in a good state.
   *
   * @param dbPath - Path to the database file. If omitted, uses the default resolution.
   * @returns Validation result with table list on success, or error details on failure.
   */
  static validateDatabase(dbPath?: string): DatabaseValidationResult {
    const resolvedPath = dbPath || getDefaultDbPath();
    const absolutePath = path.isAbsolute(resolvedPath)
      ? resolvedPath
      : path.resolve(process.cwd(), resolvedPath);

    // Check if file exists — if not, standalone mode will create it
    if (!fs.existsSync(absolutePath)) {
      return {
        valid: false,
        standalone: true,
        error: `No existing database at ${absolutePath}. Standalone mode will create one on startup.`,
      };
    }

    // Try to open and run integrity check
    let testDb: BetterSqlite3.Database | null = null;
    try {
      testDb = new Database(absolutePath, { readonly: true });

      // Run integrity check
      const integrityResult = testDb.pragma("integrity_check") as { integrity_check: string }[];
      const integrityStatus = integrityResult[0]?.integrity_check;

      if (integrityStatus !== "ok") {
        return {
          valid: false,
          error: `Database integrity check failed: ${integrityStatus}. `
            + "The database file may be corrupt. Try deleting data/4da.db and restarting 4DA.",
        };
      }

      // List tables
      const tables = testDb
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .all() as { name: string }[];

      return {
        valid: true,
        tables: tables.map((t) => t.name),
      };
    } catch (error) {
      return {
        valid: false,
        error: `Failed to open database at ${absolutePath}: `
          + (error instanceof Error ? error.message : String(error)),
      };
    } finally {
      try {
        testDb?.close();
      } catch {
        // Ignore close errors during validation
      }
    }
  }

  /**
   * Get the raw better-sqlite3 database instance for custom queries
   */
  getRawDb(): BetterSqlite3.Database {
    return this.db;
  }

  /**
   * Close the database connection
   */
  close(): void {
    this.db.close();
  }

  // ===========================================================================
  // Standalone Mode — Schema & Population
  // ===========================================================================

  /**
   * Create the minimal schema required for MCP tools to function.
   * Mirrors the desktop app's tables but only the subset that MCP tools query.
   * Called once when creating a brand-new standalone database.
   */
  private createMinimalSchema(): void {
    this.db.exec(`
      -- Schema version tracking
      CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY
      );
      INSERT INTO schema_version (version) VALUES (1);

      -- Source items (content feed) — queried by get_relevant_content, source_health, daily_briefing, etc.
      CREATE TABLE IF NOT EXISTS source_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_type TEXT NOT NULL,
        source_id TEXT NOT NULL,
        url TEXT,
        title TEXT NOT NULL,
        content TEXT NOT NULL DEFAULT '',
        content_hash TEXT NOT NULL DEFAULT '',
        embedding BLOB NOT NULL DEFAULT x'00',
        signal_type TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        last_seen TEXT NOT NULL DEFAULT (datetime('now')),
        UNIQUE(source_type, source_id)
      );
      CREATE INDEX IF NOT EXISTS idx_source_type ON source_items(source_type);
      CREATE INDEX IF NOT EXISTS idx_source_type_created ON source_items(source_type, created_at);

      -- User identity — queried by get_context
      CREATE TABLE IF NOT EXISTS user_identity (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        role TEXT,
        created_at TEXT DEFAULT (datetime('now')),
        updated_at TEXT DEFAULT (datetime('now'))
      );

      -- Tech stack (user-declared) — queried by get_context, tech_radar, developer_dna
      CREATE TABLE IF NOT EXISTS tech_stack (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        technology TEXT NOT NULL UNIQUE,
        created_at TEXT DEFAULT (datetime('now'))
      );

      -- Domains of interest — queried by get_context
      CREATE TABLE IF NOT EXISTS domains (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        domain TEXT NOT NULL UNIQUE,
        created_at TEXT DEFAULT (datetime('now'))
      );

      -- Explicit interests — queried by get_context, developer_dna
      CREATE TABLE IF NOT EXISTS explicit_interests (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        topic TEXT NOT NULL UNIQUE,
        weight REAL DEFAULT 1.0,
        embedding BLOB,
        source TEXT DEFAULT 'explicit',
        created_at TEXT DEFAULT (datetime('now'))
      );
      CREATE INDEX IF NOT EXISTS idx_interests_topic ON explicit_interests(topic);

      -- Exclusions — queried by get_context
      CREATE TABLE IF NOT EXISTS exclusions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        topic TEXT NOT NULL UNIQUE,
        created_at TEXT DEFAULT (datetime('now'))
      );

      -- ACE detected tech — queried by get_context, tech_radar, developer_dna
      CREATE TABLE IF NOT EXISTS detected_tech (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        category TEXT NOT NULL,
        confidence REAL DEFAULT 0.5,
        source TEXT NOT NULL DEFAULT 'project_scan',
        evidence TEXT,
        created_at TEXT DEFAULT (datetime('now')),
        updated_at TEXT DEFAULT (datetime('now'))
      );
      CREATE INDEX IF NOT EXISTS idx_detected_tech_name ON detected_tech(name);
      CREATE INDEX IF NOT EXISTS idx_detected_tech_confidence ON detected_tech(confidence);

      -- Active topics — queried by get_context
      CREATE TABLE IF NOT EXISTS active_topics (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        topic TEXT NOT NULL UNIQUE,
        weight REAL DEFAULT 0.5,
        confidence REAL DEFAULT 0.5,
        embedding BLOB,
        source TEXT NOT NULL DEFAULT 'project_scan',
        last_seen TEXT DEFAULT (datetime('now')),
        decay_applied INTEGER DEFAULT 0,
        created_at TEXT DEFAULT (datetime('now'))
      );
      CREATE INDEX IF NOT EXISTS idx_active_topics_topic ON active_topics(topic);

      -- Topic affinities (learned) — queried by get_context, developer_dna, attention_report
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

      -- Anti-topics (learned exclusions) — queried by get_context
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

      -- Interactions — queried by record_feedback, developer_dna, knowledge_gaps
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
      CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
      CREATE INDEX IF NOT EXISTS idx_interactions_item ON interactions(source_item_id);

      -- Project dependencies — queried by project_health, tech_radar, developer_dna, knowledge_gaps
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
      CREATE INDEX IF NOT EXISTS idx_deps_package ON project_dependencies(package_name);
      CREATE INDEX IF NOT EXISTS idx_deps_project ON project_dependencies(project_path);

      -- Developer decisions — queried by tech_radar, decision_memory
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
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
      );
      CREATE INDEX IF NOT EXISTS idx_decisions_type ON developer_decisions(decision_type);
      CREATE INDEX IF NOT EXISTS idx_decisions_subject ON developer_decisions(subject);

      -- Temporal events — queried by signal_chains, semantic_shifts, trend_analysis
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
      CREATE INDEX IF NOT EXISTS idx_temporal_type_time ON temporal_events(event_type, created_at);

      -- Item relationships — queried by reverse_mentions, topic_connections
      CREATE TABLE IF NOT EXISTS item_relationships (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_item_id INTEGER NOT NULL,
        related_item_id INTEGER NOT NULL,
        relationship_type TEXT NOT NULL,
        strength REAL DEFAULT 1.0,
        metadata JSON,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        UNIQUE(source_item_id, related_item_id, relationship_type)
      );

      -- Agent memory — queried by agent_memory, agent_session_brief
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
        promoted_to_decision_id INTEGER
      );
      CREATE INDEX IF NOT EXISTS idx_agent_memory_type ON agent_memory(memory_type);
      CREATE INDEX IF NOT EXISTS idx_agent_memory_session ON agent_memory(session_id);

      -- Source health — queried by source_health diagnostic
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

      -- Sources registry
      CREATE TABLE IF NOT EXISTS sources (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_type TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        config TEXT,
        last_fetch TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      );
    `);
  }

  /**
   * Populate the standalone database from a project scan result.
   * Inserts detected tech, dependencies, and topics so MCP tools return useful data.
   */
  populateFromScan(scan: ProjectScanResult): void {
    const insertTech = this.db.prepare(
      "INSERT OR IGNORE INTO detected_tech (name, category, confidence, source) VALUES (?, ?, ?, ?)",
    );
    const insertTopic = this.db.prepare(
      "INSERT OR IGNORE INTO active_topics (topic, weight, confidence, source) VALUES (?, ?, ?, ?)",
    );
    const insertDep = this.db.prepare(
      "INSERT OR IGNORE INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language) VALUES (?, ?, ?, ?, ?, ?)",
    );
    const insertInterest = this.db.prepare(
      "INSERT OR IGNORE INTO explicit_interests (topic, weight, source) VALUES (?, ?, ?)",
    );
    const insertTechStack = this.db.prepare(
      "INSERT OR IGNORE INTO tech_stack (technology) VALUES (?)",
    );

    // Run all inserts in a single transaction for speed
    const populate = this.db.transaction(() => {
      // Languages -> detected_tech + tech_stack + active_topics
      for (const lang of scan.languages) {
        insertTech.run(lang, "language", 0.95, "project_scan");
        insertTechStack.run(lang);
        insertTopic.run(lang, 0.8, 0.9, "project_scan");
      }

      // Frameworks -> detected_tech + tech_stack + active_topics
      for (const fw of scan.frameworks) {
        insertTech.run(fw, "framework", 0.85, "project_scan");
        insertTechStack.run(fw);
        insertTopic.run(fw, 0.7, 0.85, "project_scan");
      }

      // Determine primary ecosystem for dependency labeling.
      // For multi-language projects, we use the primary ecosystem;
      // this is a simplification — the desktop app's ACE engine handles
      // per-manifest tracking more precisely.
      const ecosystem = scan.languages.includes("typescript") || scan.languages.includes("javascript")
        ? "npm"
        : scan.languages.includes("rust")
          ? "rust"
          : scan.languages.includes("python")
            ? "python"
            : scan.languages.includes("go")
              ? "go"
              : "unknown";

      const manifestType = ecosystem === "rust"
        ? "Cargo.toml"
        : ecosystem === "python"
          ? "pyproject.toml"
          : ecosystem === "go"
            ? "go.mod"
            : "package.json";

      // Production dependencies
      for (const dep of scan.dependencies) {
        insertDep.run(scan.projectPath, manifestType, dep, null, 0, ecosystem);
      }

      // Dev dependencies
      for (const dep of scan.devDependencies) {
        insertDep.run(scan.projectPath, manifestType, dep, null, 1, ecosystem);
      }

      // Topics -> active_topics + explicit_interests (so get_context returns them)
      for (const topic of scan.topics) {
        insertTopic.run(topic, 0.7, 0.8, "project_scan");
        insertInterest.run(topic, 0.8, "project_scan");
      }
    });

    populate();
  }

  /**
   * Execute a database operation with retry logic for SQLITE_BUSY / SQLITE_LOCKED.
   *
   * better-sqlite3 is synchronous, so contention with the Tauri backend (which
   * also writes to the same WAL database) can occasionally surface as BUSY/LOCKED.
   * A single retry after a short pause is sufficient in practice.
   *
   * @param fn - The synchronous database operation to execute.
   * @param maxRetries - Maximum number of retries (default: 1).
   */
  queryWithRetry<T>(fn: () => T, maxRetries: number = 1): T {
    try {
      return fn();
    } catch (error: unknown) {
      const code = (error as { code?: string }).code;
      if (maxRetries > 0 && (code === "SQLITE_BUSY" || code === "SQLITE_LOCKED")) {
        // Busy-wait for 100ms then retry
        const start = Date.now();
        while (Date.now() - start < 100) {
          /* busy wait */
        }
        return this.queryWithRetry(fn, maxRetries - 1);
      }
      throw error;
    }
  }

  // ===========================================================================
  // Source Items
  // ===========================================================================

  /**
   * Get relevant content items with computed relevance scores
   */
  getRelevantContent(
    minScore: number = 0.35,
    sourceType?: string,
    limit: number = 20,
    sinceHours: number = 24
  ): RelevantItem[] {
    const sinceDate = new Date(Date.now() - sinceHours * 60 * 60 * 1000)
      .toISOString()
      .replace("T", " ")
      .slice(0, 19);

    // Get user context for scoring
    const context = this.getUserContext(true, true);

    // Build query - get recent items
    let query = `
      SELECT id, source_type, source_id, url, title, content, content_hash, created_at, last_seen
      FROM source_items
      WHERE datetime(created_at) >= datetime(?)
    `;
    const params: (string | number)[] = [sinceDate];

    if (sourceType) {
      query += ` AND source_type = ?`;
      params.push(sourceType);
    }

    query += ` ORDER BY created_at DESC LIMIT ?`;
    params.push(limit * 5); // Get more to filter

    const stmt = this.db.prepare(query);
    const items = stmt.all(...params) as SourceItem[];

    // Compute relevance scores and filter
    const scoredItems: RelevantItem[] = [];
    const now = Date.now();

    for (const item of items) {
      const score = this.computeRelevanceScore(item, context);

      if (score >= minScore) {
        const createdAt = new Date(item.created_at.replace(" ", "T") + "Z");
        const hoursAgo = Math.round((now - createdAt.getTime()) / (1000 * 60 * 60));
        const discoveredAgo =
          hoursAgo < 1
            ? "< 1 hour ago"
            : hoursAgo < 24
              ? `${hoursAgo} hours ago`
              : `${Math.round(hoursAgo / 24)} days ago`;

        scoredItems.push({
          id: item.id,
          source_type: item.source_type,
          source_id: item.source_id,
          url: item.url,
          title: item.title,
          content: item.content.substring(0, 500), // Truncate for readability
          relevance_score: Math.round(score * 100) / 100,
          created_at: item.created_at,
          discovered_ago: discoveredAgo,
        });
      }
    }

    // Sort by relevance and limit
    return scoredItems
      .sort((a, b) => b.relevance_score - a.relevance_score)
      .slice(0, limit);
  }

  /**
   * Get a single source item by ID and type
   */
  getSourceItem(itemId: number, sourceType: string): SourceItem | null {
    const stmt = this.db.prepare(`
      SELECT id, source_type, source_id, url, title, content, content_hash, created_at, last_seen
      FROM source_items
      WHERE id = ? AND source_type = ?
    `);
    return (stmt.get(itemId, sourceType) as SourceItem) || null;
  }

  // ===========================================================================
  // User Context
  // ===========================================================================

  /**
   * Get the user's context (what 4DA knows about them)
   */
  getUserContext(includeAce: boolean = true, includeLearned: boolean = true): UserContext {
    // Static identity
    const roleStmt = this.db.prepare("SELECT role FROM user_identity WHERE id = 1");
    const roleRow = roleStmt.get() as { role: string | null } | undefined;

    const techStmt = this.db.prepare("SELECT technology FROM tech_stack ORDER BY technology");
    const tech = techStmt.all() as { technology: string }[];

    const domainsStmt = this.db.prepare("SELECT domain FROM domains ORDER BY domain");
    const domains = domainsStmt.all() as { domain: string }[];

    const interestsStmt = this.db.prepare(`
      SELECT id, topic, weight, source FROM explicit_interests ORDER BY weight DESC
    `);
    const interests = interestsStmt.all() as Interest[];

    const exclusionsStmt = this.db.prepare("SELECT topic FROM exclusions ORDER BY topic");
    const exclusions = exclusionsStmt.all() as { topic: string }[];

    const context: UserContext = {
      role: roleRow?.role || null,
      tech_stack: tech.map((t) => t.technology),
      domains: domains.map((d) => d.domain),
      interests: interests,
      exclusions: exclusions.map((e) => e.topic),
    };

    // ACE-detected context
    if (includeAce) {
      try {
        const detectedTechStmt = this.db.prepare(`
          SELECT name, category, confidence, source
          FROM detected_tech
          WHERE confidence > 0.3
          ORDER BY confidence DESC
          LIMIT 50
        `);
        const detectedTech = detectedTechStmt.all() as DetectedTech[];

        const activeTopicsStmt = this.db.prepare(`
          SELECT topic, weight, confidence, source, last_seen
          FROM active_topics
          WHERE confidence > 0.3
          ORDER BY weight DESC
          LIMIT 50
        `);
        const activeTopics = activeTopicsStmt.all() as ActiveTopic[];

        context.ace = {
          detected_tech: detectedTech,
          active_topics: activeTopics,
        };
      } catch {
        // ACE tables might not exist
        context.ace = {
          detected_tech: [],
          active_topics: [],
        };
      }
    }

    // Learned preferences
    if (includeLearned) {
      try {
        const affinitiesStmt = this.db.prepare(`
          SELECT topic, affinity_score, confidence, positive_signals, negative_signals, total_exposures
          FROM topic_affinities
          WHERE confidence > 0.1
          ORDER BY affinity_score DESC
          LIMIT 50
        `);
        const affinities = affinitiesStmt.all() as TopicAffinity[];

        const antiTopicsStmt = this.db.prepare(`
          SELECT topic, rejection_count, confidence, auto_detected, user_confirmed
          FROM anti_topics
          WHERE confidence > 0.3
          ORDER BY confidence DESC
          LIMIT 50
        `);
        const antiTopics = antiTopicsStmt.all() as AntiTopic[];

        context.learned = {
          topic_affinities: affinities,
          anti_topics: antiTopics.map((at) => ({
            ...at,
            auto_detected: Boolean(at.auto_detected),
            user_confirmed: Boolean(at.user_confirmed),
          })),
        };
      } catch {
        // Learned tables might not exist
        context.learned = {
          topic_affinities: [],
          anti_topics: [],
        };
      }
    }

    return context;
  }

  // ===========================================================================
  // Relevance Explanation
  // ===========================================================================

  /**
   * Explain why an item was considered relevant
   */
  explainRelevance(itemId: number, sourceType: string): RelevanceExplanation | null {
    const item = this.getSourceItem(itemId, sourceType);
    if (!item) {
      return null;
    }

    const context = this.getUserContext(true, true);
    const itemTopics = this.extractTopics(item.title + " " + item.content);

    // Compute matching elements
    const matchingInterests = context.interests
      .filter((i) => itemTopics.some((t) => this.topicMatches(t, i.topic)))
      .map((i) => i.topic);

    const matchingTech = context.tech_stack.filter((tech) =>
      itemTopics.some((t) => this.topicMatches(t, tech))
    );

    const matchingTopics =
      context.ace?.active_topics
        .filter((at) => itemTopics.some((t) => this.topicMatches(t, at.topic)))
        .map((at) => at.topic) || [];

    const matchingAffinities =
      context.learned?.topic_affinities
        .filter(
          (ta) =>
            ta.affinity_score > 0 && itemTopics.some((t) => this.topicMatches(t, ta.topic))
        )
        .map((ta) => ta.topic) || [];

    // Compute score breakdown
    const staticScore =
      matchingInterests.length * 0.3 +
      matchingTech.length * 0.2 +
      (context.domains.some((d) => itemTopics.some((t) => this.topicMatches(t, d)))
        ? 0.15
        : 0);

    const aceScore = context.ace
      ? matchingTopics.length * 0.1 +
        context.ace.detected_tech.filter((dt) =>
          itemTopics.some((t) => this.topicMatches(t, dt.name))
        ).length *
          0.05
      : 0;

    const learnedScore = context.learned
      ? context.learned.topic_affinities
          .filter(
            (ta) =>
              ta.affinity_score > 0 && itemTopics.some((t) => this.topicMatches(t, ta.topic))
          )
          .reduce((sum, ta) => sum + ta.affinity_score * 0.1 * ta.confidence, 0)
      : 0;

    // Anti-penalty from exclusions and anti-topics
    let antiPenalty = 0;
    for (const exclusion of context.exclusions) {
      if (itemTopics.some((t) => this.topicMatches(t, exclusion))) {
        antiPenalty = 1.0; // Hard exclusion
        break;
      }
    }
    if (antiPenalty === 0 && context.learned) {
      for (const antiTopic of context.learned.anti_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, antiTopic.topic))) {
          antiPenalty = Math.max(antiPenalty, antiTopic.confidence * 0.5);
        }
      }
    }

    const finalScore = Math.max(0, Math.min(1, staticScore + aceScore + learnedScore - antiPenalty));

    const scoreBreakdown: ScoreBreakdown = {
      embedding_similarity: null, // Not computing embedding similarity in MCP server
      static_match_score: Math.round(staticScore * 100) / 100,
      ace_match_score: Math.round(aceScore * 100) / 100,
      learned_affinity_score: Math.round(learnedScore * 100) / 100,
      anti_penalty: Math.round(antiPenalty * 100) / 100,
      final_score: Math.round(finalScore * 100) / 100,
    };

    const matchingContext: MatchingContext = {
      matching_interests: matchingInterests,
      matching_tech: matchingTech,
      matching_topics: matchingTopics,
      matching_affinities: matchingAffinities,
    };

    // Generate human-readable explanation
    const explanationParts: string[] = [];

    if (matchingInterests.length > 0) {
      explanationParts.push(
        `Matches your interests: ${matchingInterests.slice(0, 3).join(", ")}`
      );
    }
    if (matchingTech.length > 0) {
      explanationParts.push(`Related to your tech stack: ${matchingTech.slice(0, 3).join(", ")}`);
    }
    if (matchingTopics.length > 0) {
      explanationParts.push(
        `Relevant to recent work: ${matchingTopics.slice(0, 3).join(", ")}`
      );
    }
    if (matchingAffinities.length > 0) {
      explanationParts.push(
        `You've shown interest in: ${matchingAffinities.slice(0, 3).join(", ")}`
      );
    }
    if (antiPenalty > 0) {
      explanationParts.push(`Score reduced due to excluded topics`);
    }

    const explanation =
      explanationParts.length > 0
        ? explanationParts.join(". ") + "."
        : "No specific matches found - may be based on general relevance.";

    return {
      item_id: itemId,
      source_type: sourceType,
      title: item.title,
      score_breakdown: scoreBreakdown,
      matching_context: matchingContext,
      explanation,
    };
  }

  // ===========================================================================
  // Feedback
  // ===========================================================================

  /**
   * Record user feedback on an item
   */
  recordFeedback(
    itemId: number,
    sourceType: string,
    action: FeedbackAction
  ): FeedbackResult {
    // Verify item exists
    const item = this.getSourceItem(itemId, sourceType);
    if (!item) {
      return {
        success: false,
        message: `Item ${itemId} of type ${sourceType} not found`,
      };
    }

    // Map action to signal strength
    const signalStrength: Record<FeedbackAction, number> = {
      click: 0.3,
      save: 0.8,
      dismiss: -0.2,
      mark_irrelevant: -0.5,
    };

    try {
      const stmt = this.db.prepare(`
        INSERT INTO interactions (item_id, action_type, item_source, signal_strength, timestamp)
        VALUES (?, ?, ?, ?, datetime('now'))
      `);

      const result = stmt.run(itemId, action, sourceType, signalStrength[action]);

      return {
        success: true,
        message: `Recorded ${action} feedback for item ${itemId}`,
        interaction_id: result.lastInsertRowid as number,
      };
    } catch (error) {
      return {
        success: false,
        message: `Failed to record feedback: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }

  // ===========================================================================
  // Helper Methods
  // ===========================================================================

  /**
   * Compute relevance score for an item based on user context
   */
  private computeRelevanceScore(item: SourceItem, context: UserContext): number {
    const itemTopics = this.extractTopics(item.title + " " + item.content);

    // Check hard exclusions first
    for (const exclusion of context.exclusions) {
      if (itemTopics.some((t) => this.topicMatches(t, exclusion))) {
        return 0;
      }
    }

    let score = 0;

    // Static identity matching
    for (const interest of context.interests) {
      if (itemTopics.some((t) => this.topicMatches(t, interest.topic))) {
        score += 0.3 * interest.weight;
      }
    }

    for (const tech of context.tech_stack) {
      if (itemTopics.some((t) => this.topicMatches(t, tech))) {
        score += 0.2;
      }
    }

    for (const domain of context.domains) {
      if (itemTopics.some((t) => this.topicMatches(t, domain))) {
        score += 0.15;
      }
    }

    // ACE-detected context
    if (context.ace) {
      for (const topic of context.ace.active_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, topic.topic))) {
          score += 0.1 * topic.weight * topic.confidence;
        }
      }

      for (const tech of context.ace.detected_tech) {
        if (itemTopics.some((t) => this.topicMatches(t, tech.name))) {
          score += 0.05 * tech.confidence;
        }
      }
    }

    // Learned affinities
    if (context.learned) {
      for (const affinity of context.learned.topic_affinities) {
        if (
          affinity.affinity_score > 0 &&
          itemTopics.some((t) => this.topicMatches(t, affinity.topic))
        ) {
          score += affinity.affinity_score * 0.1 * affinity.confidence;
        }
      }

      // Anti-topics penalty
      for (const antiTopic of context.learned.anti_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, antiTopic.topic))) {
          score -= antiTopic.confidence * 0.3;
        }
      }
    }

    return Math.max(0, Math.min(1, score));
  }

  /**
   * Extract topics from text (simple keyword extraction)
   */
  private extractTopics(text: string): string[] {
    // Simple topic extraction - split on word boundaries, filter meaningful words
    return text
      .toLowerCase()
      .split(/[\s\-_.,;:!?'"()\[\]{}]+/)
      .filter((word) => word.length > 2)
      .filter((word) => !this.isStopWord(word));
  }

  /**
   * Check if a topic matches a term (case-insensitive, partial match)
   */
  private topicMatches(topic: string, term: string): boolean {
    const normalizedTopic = topic.toLowerCase();
    const normalizedTerm = term.toLowerCase();
    return (
      normalizedTopic.includes(normalizedTerm) || normalizedTerm.includes(normalizedTopic)
    );
  }

  /**
   * Check if a word is a stop word
   */
  private isStopWord(word: string): boolean {
    const stopWords = new Set([
      "the",
      "a",
      "an",
      "and",
      "or",
      "but",
      "in",
      "on",
      "at",
      "to",
      "for",
      "of",
      "with",
      "by",
      "from",
      "as",
      "is",
      "was",
      "are",
      "were",
      "been",
      "be",
      "have",
      "has",
      "had",
      "do",
      "does",
      "did",
      "will",
      "would",
      "could",
      "should",
      "may",
      "might",
      "can",
      "this",
      "that",
      "these",
      "those",
      "it",
      "its",
      "they",
      "them",
      "their",
      "we",
      "us",
      "our",
      "you",
      "your",
      "he",
      "she",
      "him",
      "her",
      "his",
      "hers",
      "who",
      "what",
      "when",
      "where",
      "why",
      "how",
      "all",
      "each",
      "every",
      "both",
      "few",
      "more",
      "most",
      "other",
      "some",
      "such",
      "no",
      "not",
      "only",
      "same",
      "so",
      "than",
      "too",
      "very",
      "just",
      "also",
      "now",
      "here",
      "there",
      "then",
      "new",
      "first",
      "one",
      "two",
    ]);
    return stopWords.has(word);
  }
}

/**
 * Create a database instance
 */
export function createDatabase(dbPath?: string): FourDADatabase {
  return new FourDADatabase(dbPath);
}
