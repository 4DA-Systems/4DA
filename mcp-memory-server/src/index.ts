#!/usr/bin/env node
/**
 * MCP Memory Server
 *
 * Provides persistent project memory for Claude Code sessions.
 * Survives context rot by storing decisions, state, and learnings
 * in a SQLite database that can be queried semantically.
 *
 * Also provides access to archived session transcripts for
 * referencing past conversations.
 *
 * Tools:
 * - remember_decision: Store an architectural/design decision
 * - recall_decisions: Query stored decisions
 * - update_state: Update current project state
 * - get_state: Get current project state
 * - remember_learning: Store something learned during development
 * - recall_learnings: Query stored learnings
 * - search_memory: Full-text search across all memory
 * - list_sessions: List all archived sessions
 * - search_sessions: Search through past session transcripts
 * - get_session_messages: Get messages from a specific session
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  Tool,
} from "@modelcontextprotocol/sdk/types.js";
import Database from "better-sqlite3";
import { existsSync, mkdirSync, readdirSync, readFileSync } from "fs";
import { dirname, join, basename } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Paths
const DB_PATH = process.env.MEMORY_DB_PATH || join(__dirname, "..", "memory.db");
const SESSIONS_DIR = process.env.SESSIONS_DIR || "/mnt/d/4DA/.claude/sessions/transcripts";

// Ensure directories exist
const dbDir = dirname(DB_PATH);
if (!existsSync(dbDir)) {
  mkdirSync(dbDir, { recursive: true });
}

const db = new Database(DB_PATH);

// Initialize schema
db.exec(`
  CREATE TABLE IF NOT EXISTS decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT UNIQUE NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT,
    alternatives TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT UNIQUE NOT NULL,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS learnings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    content TEXT NOT NULL,
    context TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS code_locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_number INTEGER,
    purpose TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(name, file_path)
  );

  CREATE TABLE IF NOT EXISTS session_index (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_file TEXT UNIQUE NOT NULL,
    session_date TEXT,
    message_count INTEGER,
    indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS session_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_file TEXT NOT NULL,
    message_index INTEGER NOT NULL,
    role TEXT,
    content TEXT,
    UNIQUE(session_file, message_index)
  );

  CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    source,
    key,
    content,
    tokenize='porter'
  );

  CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
    session_file,
    content,
    tokenize='porter'
  );

  -- CADE Quality Metrics table
  CREATE TABLE IF NOT EXISTS quality_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    metric_type TEXT NOT NULL,
    value REAL NOT NULL,
    context TEXT,
    task_id TEXT,
    session_id TEXT
  );

  CREATE INDEX IF NOT EXISTS idx_metrics_type ON quality_metrics(metric_type);
  CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON quality_metrics(timestamp);
`);

// Prepared statements
const insertDecision = db.prepare(`
  INSERT INTO decisions (key, decision, rationale, alternatives)
  VALUES (?, ?, ?, ?)
  ON CONFLICT(key) DO UPDATE SET
    decision = excluded.decision,
    rationale = excluded.rationale,
    alternatives = excluded.alternatives,
    updated_at = CURRENT_TIMESTAMP
`);

const getDecision = db.prepare(`
  SELECT * FROM decisions WHERE key = ?
`);

const getAllDecisions = db.prepare(`
  SELECT * FROM decisions ORDER BY updated_at DESC
`);

const searchDecisions = db.prepare(`
  SELECT * FROM decisions
  WHERE decision LIKE ? OR rationale LIKE ? OR key LIKE ?
  ORDER BY updated_at DESC
`);

const upsertState = db.prepare(`
  INSERT INTO state (key, value)
  VALUES (?, ?)
  ON CONFLICT(key) DO UPDATE SET
    value = excluded.value,
    updated_at = CURRENT_TIMESTAMP
`);

const getState = db.prepare(`
  SELECT * FROM state WHERE key = ?
`);

const getAllState = db.prepare(`
  SELECT * FROM state ORDER BY key
`);

const insertLearning = db.prepare(`
  INSERT INTO learnings (topic, content, context)
  VALUES (?, ?, ?)
`);

const searchLearnings = db.prepare(`
  SELECT * FROM learnings
  WHERE topic LIKE ? OR content LIKE ?
  ORDER BY created_at DESC
  LIMIT ?
`);

const upsertCodeLocation = db.prepare(`
  INSERT INTO code_locations (name, file_path, line_number, purpose)
  VALUES (?, ?, ?, ?)
  ON CONFLICT(name, file_path) DO UPDATE SET
    line_number = excluded.line_number,
    purpose = excluded.purpose,
    updated_at = CURRENT_TIMESTAMP
`);

const searchCodeLocations = db.prepare(`
  SELECT * FROM code_locations
  WHERE name LIKE ? OR purpose LIKE ? OR file_path LIKE ?
  ORDER BY updated_at DESC
`);

const insertFts = db.prepare(`
  INSERT INTO memory_fts (source, key, content)
  VALUES (?, ?, ?)
`);

const searchFts = db.prepare(`
  SELECT * FROM memory_fts WHERE memory_fts MATCH ? LIMIT ?
`);

const insertSessionIndex = db.prepare(`
  INSERT OR REPLACE INTO session_index (session_file, session_date, message_count)
  VALUES (?, ?, ?)
`);

// CADE Quality Metrics prepared statements
const insertMetric = db.prepare(`
  INSERT INTO quality_metrics (metric_type, value, context, task_id, session_id)
  VALUES (?, ?, ?, ?, ?)
`);

const getMetrics = db.prepare(`
  SELECT * FROM quality_metrics
  WHERE metric_type LIKE ?
  ORDER BY timestamp DESC
  LIMIT ?
`);

const getMetricsSince = db.prepare(`
  SELECT * FROM quality_metrics
  WHERE timestamp >= ?
  ORDER BY timestamp DESC
`);

const getMetricsAggregate = db.prepare(`
  SELECT
    metric_type,
    COUNT(*) as count,
    AVG(value) as avg_value,
    MIN(value) as min_value,
    MAX(value) as max_value,
    SUM(value) as sum_value
  FROM quality_metrics
  WHERE timestamp >= ?
  GROUP BY metric_type
`);

const insertSessionMessage = db.prepare(`
  INSERT OR REPLACE INTO session_messages (session_file, message_index, role, content)
  VALUES (?, ?, ?, ?)
`);

const insertSessionFts = db.prepare(`
  INSERT INTO session_fts (session_file, content)
  VALUES (?, ?)
`);

const searchSessionFts = db.prepare(`
  SELECT DISTINCT session_file, snippet(session_fts, 1, '>>>', '<<<', '...', 64) as snippet
  FROM session_fts
  WHERE session_fts MATCH ?
  LIMIT ?
`);

const getSessionMessages = db.prepare(`
  SELECT * FROM session_messages
  WHERE session_file = ?
  ORDER BY message_index
  LIMIT ? OFFSET ?
`);

const getAllSessionIndex = db.prepare(`
  SELECT * FROM session_index ORDER BY session_date DESC
`);

// Helper: Parse JSONL transcript file
interface TranscriptMessage {
  type: string;
  role?: string;
  content?: string | Array<{ type: string; text?: string }>;
  message?: {
    role?: string;
    content?: string | Array<{ type: string; text?: string }>;
  };
}

function parseTranscript(filePath: string): Array<{ role: string; content: string }> {
  const messages: Array<{ role: string; content: string }> = [];

  try {
    const content = readFileSync(filePath, "utf-8");
    const lines = content.split("\n").filter((line) => line.trim());

    for (const line of lines) {
      try {
        const entry = JSON.parse(line) as TranscriptMessage;

        // Handle different transcript formats
        let role = "";
        let text = "";

        if (entry.type === "human" || entry.role === "user") {
          role = "user";
          if (typeof entry.content === "string") {
            text = entry.content;
          } else if (Array.isArray(entry.content)) {
            text = entry.content
              .filter((c) => c.type === "text")
              .map((c) => c.text || "")
              .join("\n");
          }
        } else if (entry.type === "assistant" || entry.role === "assistant") {
          role = "assistant";
          if (typeof entry.content === "string") {
            text = entry.content;
          } else if (Array.isArray(entry.content)) {
            text = entry.content
              .filter((c) => c.type === "text")
              .map((c) => c.text || "")
              .join("\n");
          }
        } else if (entry.message) {
          role = entry.message.role || "unknown";
          if (typeof entry.message.content === "string") {
            text = entry.message.content;
          } else if (Array.isArray(entry.message.content)) {
            text = entry.message.content
              .filter((c) => c.type === "text")
              .map((c) => c.text || "")
              .join("\n");
          }
        }

        if (role && text) {
          messages.push({ role, content: text });
        }
      } catch {
        // Skip malformed lines
      }
    }
  } catch {
    // File read error
  }

  return messages;
}

// Helper: Index a session transcript
function indexSession(sessionFile: string): { indexed: boolean; messageCount: number } {
  const filePath = join(SESSIONS_DIR, sessionFile);

  if (!existsSync(filePath)) {
    return { indexed: false, messageCount: 0 };
  }

  const messages = parseTranscript(filePath);

  // Extract date from filename (session_YYYYMMDD_HHMMSS_id.jsonl)
  const dateMatch = sessionFile.match(/(\d{8})_(\d{6})/);
  const sessionDate = dateMatch
    ? `${dateMatch[1].slice(0, 4)}-${dateMatch[1].slice(4, 6)}-${dateMatch[1].slice(6, 8)} ${dateMatch[2].slice(0, 2)}:${dateMatch[2].slice(2, 4)}:${dateMatch[2].slice(4, 6)}`
    : "unknown";

  // Store in index
  insertSessionIndex.run(sessionFile, sessionDate, messages.length);

  // Store messages and index for FTS
  const insertMany = db.transaction((msgs: Array<{ role: string; content: string }>) => {
    for (let i = 0; i < msgs.length; i++) {
      insertSessionMessage.run(sessionFile, i, msgs[i].role, msgs[i].content);

      // Only index user and assistant messages for FTS
      if (msgs[i].role === "user" || msgs[i].role === "assistant") {
        try {
          insertSessionFts.run(sessionFile, msgs[i].content);
        } catch {
          // Ignore FTS errors (duplicate entries)
        }
      }
    }
  });

  insertMany(messages);

  return { indexed: true, messageCount: messages.length };
}

// Tool definitions
const tools: Tool[] = [
  {
    name: "remember_decision",
    description:
      "Store an architectural or design decision. Use this when a significant choice is made that should survive context compaction.",
    inputSchema: {
      type: "object",
      properties: {
        key: {
          type: "string",
          description: "Unique identifier for the decision (e.g., 'auth-strategy', 'db-choice')",
        },
        decision: {
          type: "string",
          description: "The decision that was made",
        },
        rationale: {
          type: "string",
          description: "Why this decision was made",
        },
        alternatives: {
          type: "string",
          description: "What alternatives were considered and rejected",
        },
      },
      required: ["key", "decision"],
    },
  },
  {
    name: "recall_decisions",
    description: "Retrieve stored decisions. Use when you need to remember what was decided.",
    inputSchema: {
      type: "object",
      properties: {
        key: {
          type: "string",
          description: "Specific decision key to retrieve (optional - omit to get all)",
        },
        search: {
          type: "string",
          description: "Search term to filter decisions (optional)",
        },
      },
    },
  },
  {
    name: "update_state",
    description:
      "Update current project state. Use to track what we're working on, what's blocked, etc.",
    inputSchema: {
      type: "object",
      properties: {
        key: {
          type: "string",
          description: "State key (e.g., 'current_task', 'blocked_on', 'last_file_modified')",
        },
        value: {
          type: "string",
          description: "State value",
        },
      },
      required: ["key", "value"],
    },
  },
  {
    name: "get_state",
    description: "Get current project state. Use to understand what we were working on.",
    inputSchema: {
      type: "object",
      properties: {
        key: {
          type: "string",
          description: "Specific state key (optional - omit to get all state)",
        },
      },
    },
  },
  {
    name: "remember_learning",
    description: "Store something learned during development (gotchas, patterns, discoveries).",
    inputSchema: {
      type: "object",
      properties: {
        topic: {
          type: "string",
          description: "Topic category (e.g., 'tauri', 'sqlite-vss', 'react')",
        },
        content: {
          type: "string",
          description: "What was learned",
        },
        context: {
          type: "string",
          description: "Context in which this was learned",
        },
      },
      required: ["topic", "content"],
    },
  },
  {
    name: "recall_learnings",
    description: "Retrieve stored learnings by topic or search.",
    inputSchema: {
      type: "object",
      properties: {
        search: {
          type: "string",
          description: "Search term for topic or content",
        },
        limit: {
          type: "number",
          description: "Maximum results to return (default: 10)",
        },
      },
      required: ["search"],
    },
  },
  {
    name: "remember_code_location",
    description: "Store important code location for quick reference.",
    inputSchema: {
      type: "object",
      properties: {
        name: {
          type: "string",
          description: "Descriptive name (e.g., 'main-indexer', 'feed-component')",
        },
        file_path: {
          type: "string",
          description: "Path to the file",
        },
        line_number: {
          type: "number",
          description: "Line number (optional)",
        },
        purpose: {
          type: "string",
          description: "What this code does",
        },
      },
      required: ["name", "file_path"],
    },
  },
  {
    name: "recall_code_locations",
    description: "Find remembered code locations.",
    inputSchema: {
      type: "object",
      properties: {
        search: {
          type: "string",
          description: "Search by name, path, or purpose",
        },
      },
      required: ["search"],
    },
  },
  {
    name: "search_memory",
    description: "Full-text search across all memory (decisions, learnings, etc.)",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "Search query",
        },
        limit: {
          type: "number",
          description: "Maximum results (default: 20)",
        },
      },
      required: ["query"],
    },
  },
  {
    name: "list_sessions",
    description:
      "List all archived session transcripts. Use to see what past sessions are available.",
    inputSchema: {
      type: "object",
      properties: {
        limit: {
          type: "number",
          description: "Maximum sessions to return (default: 20)",
        },
        index_new: {
          type: "boolean",
          description: "Also index any new unindexed sessions (default: true)",
        },
      },
    },
  },
  {
    name: "search_sessions",
    description:
      "Search through past session transcripts. Use to find conversations about specific topics.",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "Search query to find in past sessions",
        },
        limit: {
          type: "number",
          description: "Maximum results (default: 10)",
        },
      },
      required: ["query"],
    },
  },
  {
    name: "get_session_messages",
    description: "Get messages from a specific past session. Use to read exact conversation history.",
    inputSchema: {
      type: "object",
      properties: {
        session_file: {
          type: "string",
          description: "Session filename from list_sessions",
        },
        offset: {
          type: "number",
          description: "Start from message index (default: 0)",
        },
        limit: {
          type: "number",
          description: "Number of messages to retrieve (default: 50)",
        },
      },
      required: ["session_file"],
    },
  },
  {
    name: "index_sessions",
    description:
      "Index all session transcripts for searching. Run this periodically to index new sessions.",
    inputSchema: {
      type: "object",
      properties: {
        force: {
          type: "boolean",
          description: "Re-index all sessions even if already indexed (default: false)",
        },
      },
    },
  },
  // CADE Quality Metrics tools
  {
    name: "record_metric",
    description:
      "Record a quality metric for CADE tracking. Use to track rework, iterations, gate passes, etc.",
    inputSchema: {
      type: "object",
      properties: {
        metric_type: {
          type: "string",
          description:
            "Type of metric: 'rework', 'iteration_count', 'gate_pass', 'gate_fail', 'confidence', 'task_complete'",
        },
        value: {
          type: "number",
          description: "Numeric value for the metric",
        },
        context: {
          type: "string",
          description: "Context about what generated this metric",
        },
        task_id: {
          type: "string",
          description: "Optional task identifier",
        },
        session_id: {
          type: "string",
          description: "Optional session identifier",
        },
      },
      required: ["metric_type", "value"],
    },
  },
  {
    name: "get_metrics",
    description: "Retrieve quality metrics with optional filtering.",
    inputSchema: {
      type: "object",
      properties: {
        metric_type: {
          type: "string",
          description: "Filter by metric type (optional, supports wildcards)",
        },
        since: {
          type: "string",
          description: "ISO date string to get metrics since (e.g., '2024-01-01')",
        },
        limit: {
          type: "number",
          description: "Maximum results to return (default: 100)",
        },
      },
    },
  },
  {
    name: "get_quality_report",
    description:
      "Generate a summary quality report showing aggregated metrics and trends.",
    inputSchema: {
      type: "object",
      properties: {
        since: {
          type: "string",
          description: "ISO date string for report period start (default: 7 days ago)",
        },
      },
    },
  },
];

// Create server
const server = new Server(
  {
    name: "mcp-memory-server",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// Handle tool listing
server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools,
}));

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case "remember_decision": {
        const { key, decision, rationale, alternatives } = args as {
          key: string;
          decision: string;
          rationale?: string;
          alternatives?: string;
        };

        insertDecision.run(key, decision, rationale || null, alternatives || null);

        // Also add to FTS index
        try {
          insertFts.run("decision", key, `${decision} ${rationale || ""} ${alternatives || ""}`);
        } catch {
          // Ignore duplicate FTS entries
        }

        return {
          content: [{ type: "text", text: `Decision '${key}' stored successfully.` }],
        };
      }

      case "recall_decisions": {
        const { key, search } = args as { key?: string; search?: string };

        let results;
        if (key) {
          const result = getDecision.get(key);
          results = result ? [result] : [];
        } else if (search) {
          const pattern = `%${search}%`;
          results = searchDecisions.all(pattern, pattern, pattern);
        } else {
          results = getAllDecisions.all();
        }

        return {
          content: [
            {
              type: "text",
              text: results.length > 0 ? JSON.stringify(results, null, 2) : "No decisions found.",
            },
          ],
        };
      }

      case "update_state": {
        const { key, value } = args as { key: string; value: string };
        upsertState.run(key, value);
        return {
          content: [{ type: "text", text: `State '${key}' updated.` }],
        };
      }

      case "get_state": {
        const { key } = args as { key?: string };

        let results;
        if (key) {
          const result = getState.get(key);
          results = result ? [result] : [];
        } else {
          results = getAllState.all();
        }

        return {
          content: [
            {
              type: "text",
              text: results.length > 0 ? JSON.stringify(results, null, 2) : "No state found.",
            },
          ],
        };
      }

      case "remember_learning": {
        const { topic, content, context } = args as {
          topic: string;
          content: string;
          context?: string;
        };

        insertLearning.run(topic, content, context || null);
        try {
          insertFts.run("learning", topic, `${content} ${context || ""}`);
        } catch {
          // Ignore duplicate FTS entries
        }

        return {
          content: [{ type: "text", text: `Learning about '${topic}' stored.` }],
        };
      }

      case "recall_learnings": {
        const { search, limit = 10 } = args as { search: string; limit?: number };
        const pattern = `%${search}%`;
        const results = searchLearnings.all(pattern, pattern, limit);

        return {
          content: [
            {
              type: "text",
              text: results.length > 0 ? JSON.stringify(results, null, 2) : "No learnings found.",
            },
          ],
        };
      }

      case "remember_code_location": {
        const { name: locName, file_path, line_number, purpose } = args as {
          name: string;
          file_path: string;
          line_number?: number;
          purpose?: string;
        };

        upsertCodeLocation.run(locName, file_path, line_number || null, purpose || null);

        return {
          content: [{ type: "text", text: `Code location '${locName}' stored.` }],
        };
      }

      case "recall_code_locations": {
        const { search } = args as { search: string };
        const pattern = `%${search}%`;
        const results = searchCodeLocations.all(pattern, pattern, pattern);

        return {
          content: [
            {
              type: "text",
              text:
                results.length > 0 ? JSON.stringify(results, null, 2) : "No code locations found.",
            },
          ],
        };
      }

      case "search_memory": {
        const { query, limit = 20 } = args as { query: string; limit?: number };

        try {
          const results = searchFts.all(query, limit);
          return {
            content: [
              {
                type: "text",
                text: results.length > 0 ? JSON.stringify(results, null, 2) : "No results found.",
              },
            ],
          };
        } catch {
          return {
            content: [
              {
                type: "text",
                text: "Full-text search query failed. Try a simpler search term.",
              },
            ],
          };
        }
      }

      case "list_sessions": {
        const { limit = 20, index_new = true } = args as {
          limit?: number;
          index_new?: boolean;
        };

        // Check for new sessions to index
        if (index_new && existsSync(SESSIONS_DIR)) {
          const files = readdirSync(SESSIONS_DIR).filter((f) => f.endsWith(".jsonl"));
          const indexed = new Set(
            (getAllSessionIndex.all() as Array<{ session_file: string }>).map((r) => r.session_file)
          );

          for (const file of files) {
            if (!indexed.has(file)) {
              indexSession(file);
            }
          }
        }

        const sessions = getAllSessionIndex.all();
        const limited = (sessions as Array<Record<string, unknown>>).slice(0, limit);

        return {
          content: [
            {
              type: "text",
              text:
                limited.length > 0
                  ? JSON.stringify(limited, null, 2)
                  : "No sessions found. Sessions are archived when you exit Claude Code.",
            },
          ],
        };
      }

      case "search_sessions": {
        const { query, limit = 10 } = args as { query: string; limit?: number };

        try {
          const results = searchSessionFts.all(query, limit);
          return {
            content: [
              {
                type: "text",
                text:
                  results.length > 0
                    ? JSON.stringify(results, null, 2)
                    : "No matching sessions found. Try different search terms.",
              },
            ],
          };
        } catch {
          return {
            content: [
              {
                type: "text",
                text: "Session search failed. Try a simpler search term.",
              },
            ],
          };
        }
      }

      case "get_session_messages": {
        const { session_file, offset = 0, limit = 50 } = args as {
          session_file: string;
          offset?: number;
          limit?: number;
        };

        const messages = getSessionMessages.all(session_file, limit, offset);

        if ((messages as Array<unknown>).length === 0) {
          // Try to index first if not found
          if (existsSync(join(SESSIONS_DIR, session_file))) {
            indexSession(session_file);
            const retryMessages = getSessionMessages.all(session_file, limit, offset);
            if ((retryMessages as Array<unknown>).length > 0) {
              return {
                content: [{ type: "text", text: JSON.stringify(retryMessages, null, 2) }],
              };
            }
          }
          return {
            content: [
              {
                type: "text",
                text: `Session '${session_file}' not found or has no messages.`,
              },
            ],
          };
        }

        return {
          content: [{ type: "text", text: JSON.stringify(messages, null, 2) }],
        };
      }

      case "index_sessions": {
        const { force = false } = args as { force?: boolean };

        if (!existsSync(SESSIONS_DIR)) {
          return {
            content: [
              {
                type: "text",
                text: `Sessions directory not found: ${SESSIONS_DIR}`,
              },
            ],
          };
        }

        const files = readdirSync(SESSIONS_DIR).filter((f) => f.endsWith(".jsonl"));
        const indexed = force
          ? new Set<string>()
          : new Set(
              (getAllSessionIndex.all() as Array<{ session_file: string }>).map(
                (r) => r.session_file
              )
            );

        let newlyIndexed = 0;
        let totalMessages = 0;

        for (const file of files) {
          if (!indexed.has(file)) {
            const result = indexSession(file);
            if (result.indexed) {
              newlyIndexed++;
              totalMessages += result.messageCount;
            }
          }
        }

        return {
          content: [
            {
              type: "text",
              text: `Indexed ${newlyIndexed} new sessions with ${totalMessages} total messages. Total sessions: ${files.length}`,
            },
          ],
        };
      }

      // CADE Quality Metrics handlers
      case "record_metric": {
        const { metric_type, value, context, task_id, session_id } = args as {
          metric_type: string;
          value: number;
          context?: string;
          task_id?: string;
          session_id?: string;
        };

        insertMetric.run(
          metric_type,
          value,
          context || null,
          task_id || null,
          session_id || null
        );

        return {
          content: [
            {
              type: "text",
              text: `Metric '${metric_type}' recorded with value ${value}.`,
            },
          ],
        };
      }

      case "get_metrics": {
        const { metric_type, since, limit = 100 } = args as {
          metric_type?: string;
          since?: string;
          limit?: number;
        };

        let results;
        if (since) {
          results = getMetricsSince.all(since);
          if (metric_type) {
            results = (results as Array<{ metric_type: string }>).filter(
              (r) => r.metric_type.includes(metric_type)
            );
          }
          results = (results as Array<unknown>).slice(0, limit);
        } else {
          const pattern = metric_type ? `%${metric_type}%` : "%";
          results = getMetrics.all(pattern, limit);
        }

        return {
          content: [
            {
              type: "text",
              text:
                (results as Array<unknown>).length > 0
                  ? JSON.stringify(results, null, 2)
                  : "No metrics found.",
            },
          ],
        };
      }

      case "get_quality_report": {
        const { since } = args as { since?: string };

        // Default to 7 days ago
        const sinceDate =
          since || new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString();

        const aggregates = getMetricsAggregate.all(sinceDate) as Array<{
          metric_type: string;
          count: number;
          avg_value: number;
          min_value: number;
          max_value: number;
          sum_value: number;
        }>;

        // Calculate derived metrics
        const report: Record<string, unknown> = {
          period_start: sinceDate,
          period_end: new Date().toISOString(),
          metrics: {},
        };

        for (const agg of aggregates) {
          (report.metrics as Record<string, unknown>)[agg.metric_type] = {
            count: agg.count,
            average: Math.round(agg.avg_value * 100) / 100,
            min: agg.min_value,
            max: agg.max_value,
            total: agg.sum_value,
          };
        }

        // Add summary interpretations
        const metrics = report.metrics as Record<string, { count: number; average: number }>;
        const summary: string[] = [];

        if (metrics.rework) {
          const reworkRate = metrics.rework.average;
          summary.push(
            `Rework rate: ${(reworkRate * 100).toFixed(1)}% (target: <20%)`
          );
        }

        if (metrics.gate_pass && metrics.gate_fail) {
          const passRate =
            metrics.gate_pass.count /
            (metrics.gate_pass.count + metrics.gate_fail.count);
          summary.push(
            `Gate pass rate: ${(passRate * 100).toFixed(1)}% (target: >70%)`
          );
        }

        if (metrics.iteration_count) {
          summary.push(
            `Mean iterations: ${metrics.iteration_count.average.toFixed(1)} (target: <5)`
          );
        }

        report.summary = summary;

        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(report, null, 2),
            },
          ],
        };
      }

      default:
        return {
          content: [{ type: "text", text: `Unknown tool: ${name}` }],
          isError: true,
        };
    }
  } catch (error) {
    return {
      content: [
        {
          type: "text",
          text: `Error: ${error instanceof Error ? error.message : String(error)}`,
        },
      ],
      isError: true,
    };
  }
});

// Start server
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("MCP Memory Server running on stdio");
}

main().catch(console.error);
