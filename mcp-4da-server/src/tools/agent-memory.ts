/**
 * agent_memory tool
 *
 * Cross-agent persistent memory. What Claude Code learns, Cursor can access.
 * Enables AI agents to store and recall memories across sessions and tools.
 */

import type { FourDADatabase } from "../db.js";

// ============================================================================
// Types
// ============================================================================

export interface AgentMemoryParams {
  action: "store" | "recall" | "recall_by_tags" | "get_recent";
  // store
  session_id?: string;
  agent_type?: string;
  memory_type?: string; // discovery, decision, context, warning, preference
  subject?: string;
  content?: string;
  context_tags?: string[];
  expires_at?: string;
  // recall
  query?: string;
  filter_agent?: string;
  limit?: number;
  // recall_by_tags
  tags?: string[];
  // get_recent
  since?: string; // ISO datetime
}

interface AgentMemoryRow {
  id: number;
  session_id: string;
  agent_type: string;
  memory_type: string;
  subject: string;
  content: string;
  context_tags: string;
  created_at: string;
  expires_at: string | null;
  promoted_to_decision_id: number | null;
}

// ============================================================================
// Tool Definition
// ============================================================================

export const agentMemoryTool = {
  name: "agent_memory",
  description:
    "Cross-agent persistent memory. Actions: store (save a memory), recall (search by subject), recall_by_tags (search by tags), get_recent (memories since timestamp). What one agent learns, all agents can access.",
  inputSchema: {
    type: "object" as const,
    properties: {
      action: {
        type: "string",
        enum: ["store", "recall", "recall_by_tags", "get_recent"],
        description: "Action to perform",
      },
      session_id: {
        type: "string",
        description: "Session identifier (for store)",
      },
      agent_type: {
        type: "string",
        description:
          "Agent identifier, e.g. claude_code, cursor, windsurf (for store)",
      },
      memory_type: {
        type: "string",
        enum: ["discovery", "decision", "context", "warning", "preference"],
        description: "Type of memory (for store). Default: context",
      },
      subject: {
        type: "string",
        description: "Short subject line for the memory (for store)",
      },
      content: {
        type: "string",
        description: "Full memory content (for store)",
      },
      context_tags: {
        type: "array",
        items: { type: "string" },
        description: "Tags for categorization (for store)",
      },
      expires_at: {
        type: "string",
        description: "ISO datetime when this memory expires (for store, optional)",
      },
      query: {
        type: "string",
        description: "Search term to match against subject and tags (for recall)",
      },
      filter_agent: {
        type: "string",
        description: "Filter results to a specific agent type (for recall, get_recent)",
      },
      limit: {
        type: "number",
        description: "Max results to return (default 20)",
      },
      tags: {
        type: "array",
        items: { type: "string" },
        description: "Tags to search for (for recall_by_tags)",
      },
      since: {
        type: "string",
        description: "ISO datetime to get memories after (for get_recent)",
      },
    },
    required: ["action"],
  },
};

// ============================================================================
// Helpers
// ============================================================================

function parseMemoryRow(row: AgentMemoryRow) {
  return {
    id: row.id,
    session_id: row.session_id,
    agent_type: row.agent_type,
    memory_type: row.memory_type,
    subject: row.subject,
    content: row.content,
    context_tags: JSON.parse(row.context_tags || "[]") as string[],
    created_at: row.created_at,
    expires_at: row.expires_at,
    promoted_to_decision_id: row.promoted_to_decision_id,
  };
}

// ============================================================================
// Execute
// ============================================================================

export function executeAgentMemory(
  db: FourDADatabase,
  params: AgentMemoryParams,
): object {
  const rawDb = db.getRawDb();

  switch (params.action) {
    // ========================================================================
    // STORE
    // ========================================================================
    case "store": {
      if (!params.subject || !params.content) {
        return { error: "subject and content are required for store action" };
      }

      try {
        const stmt = rawDb.prepare(
          `INSERT INTO agent_memory
             (session_id, agent_type, memory_type, subject, content, context_tags, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
        );

        const result = stmt.run(
          params.session_id || "unknown",
          params.agent_type || "unknown",
          params.memory_type || "context",
          params.subject,
          params.content,
          JSON.stringify(params.context_tags || []),
          params.expires_at || null,
        );

        return {
          success: true,
          id: result.lastInsertRowid,
          message: `Memory stored: ${params.subject}`,
        };
      } catch (error) {
        return {
          error: `Failed to store memory: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    }

    // ========================================================================
    // RECALL
    // ========================================================================
    case "recall": {
      if (!params.query) {
        return { error: "query is required for recall action" };
      }

      try {
        const pattern = `%${params.query.toLowerCase()}%`;
        let sql = `SELECT id, session_id, agent_type, memory_type, subject, content,
                          context_tags, created_at, expires_at, promoted_to_decision_id
                   FROM agent_memory
                   WHERE (LOWER(subject) LIKE ? OR LOWER(context_tags) LIKE ?)
                   AND (expires_at IS NULL OR expires_at > datetime('now'))`;
        const sqlParams: (string | number)[] = [pattern, pattern];

        if (params.filter_agent) {
          sql += ` AND agent_type = ?`;
          sqlParams.push(params.filter_agent);
        }

        sql += ` ORDER BY created_at DESC LIMIT ?`;
        sqlParams.push(params.limit || 20);

        const rows = rawDb.prepare(sql).all(...sqlParams) as AgentMemoryRow[];

        return {
          memories: rows.map(parseMemoryRow),
          count: rows.length,
        };
      } catch (error) {
        return {
          memories: [],
          count: 0,
          error: `Failed to recall memories: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    }

    // ========================================================================
    // RECALL BY TAGS
    // ========================================================================
    case "recall_by_tags": {
      if (!params.tags || params.tags.length === 0) {
        return { error: "tags array is required for recall_by_tags action" };
      }

      try {
        // Build OR conditions for each tag
        const conditions = params.tags.map(
          () => `LOWER(context_tags) LIKE ?`,
        );
        const tagParams = params.tags.map((t) => `%${t.toLowerCase()}%`);

        const sql = `SELECT id, session_id, agent_type, memory_type, subject, content,
                            context_tags, created_at, expires_at, promoted_to_decision_id
                     FROM agent_memory
                     WHERE (${conditions.join(" OR ")})
                     AND (expires_at IS NULL OR expires_at > datetime('now'))
                     ORDER BY created_at DESC LIMIT ?`;

        const rows = rawDb
          .prepare(sql)
          .all(...tagParams, params.limit || 20) as AgentMemoryRow[];

        return {
          memories: rows.map(parseMemoryRow),
          count: rows.length,
        };
      } catch (error) {
        return {
          memories: [],
          count: 0,
          error: `Failed to recall by tags: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    }

    // ========================================================================
    // GET RECENT
    // ========================================================================
    case "get_recent": {
      const since =
        params.since ||
        new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString();

      try {
        let sql = `SELECT id, session_id, agent_type, memory_type, subject, content,
                          context_tags, created_at, expires_at, promoted_to_decision_id
                   FROM agent_memory
                   WHERE created_at > ?`;
        const sqlParams: (string | number)[] = [since];

        if (params.filter_agent) {
          sql += ` AND agent_type = ?`;
          sqlParams.push(params.filter_agent);
        }

        sql += ` ORDER BY created_at DESC LIMIT ?`;
        sqlParams.push(params.limit || 50);

        const rows = rawDb.prepare(sql).all(...sqlParams) as AgentMemoryRow[];

        return {
          memories: rows.map(parseMemoryRow),
          count: rows.length,
          since,
        };
      } catch (error) {
        return {
          memories: [],
          count: 0,
          since,
          error: `Failed to get recent memories: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    }

    default:
      return { error: `Unknown action: ${params.action}` };
  }
}
