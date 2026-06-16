// SPDX-License-Identifier: Apache-2.0
/**
 * agent_memory tool
 *
 * Cross-agent persistent memory. What Claude Code learns, Cursor can access.
 * Enables AI agents to store and recall memories across sessions and tools.
 */

import type { FourDADatabase } from "../db.js";
import { rankRowsByRecall, type RecallField } from "./recall.js";
import {
  getEmbeddingConfig,
  semanticScores,
  type EmbeddingConfig,
} from "../embeddings.js";

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
// Size Limits
// ============================================================================

/** Maximum content size per entry: 10KB */
const MAX_CONTENT_BYTES = 10 * 1024;
/** Maximum entries per agent */
const MAX_ENTRIES_PER_AGENT = 1000;

/** Weighted text fields for lexical recall ranking (shared by lexical + hybrid paths). */
const MEMORY_RECALL_FIELDS: RecallField<AgentMemoryRow>[] = [
  { name: "subject", weight: 4, value: (row) => row.subject },
  { name: "tags", weight: 3, value: (row) => row.context_tags },
  { name: "content", weight: 2, value: (row) => row.content },
  { name: "type", weight: 1, value: (row) => row.memory_type },
];

/** Blend weight for semantic cosine vs normalized lexical score in hybrid recall. */
const SEMANTIC_BLEND = 0.65;

/**
 * The text embedded for a memory. Exported so every surface that embeds a memory
 * row (this tool AND the what_should_i_know briefing) uses IDENTICAL text —
 * otherwise they would write conflicting vectors into the shared
 * agent_memory.embedding column.
 */
export function memoryEmbedText(row: { subject: string; content: string }): string {
  return `${row.subject}\n${row.content}`;
}

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
    context_tags: parseStringList(row.context_tags),
    created_at: row.created_at,
    expires_at: row.expires_at,
    promoted_to_decision_id: row.promoted_to_decision_id,
  };
}

function parseStringList(json: string | null): string[] {
  try {
    const parsed = JSON.parse(json || "[]");
    return Array.isArray(parsed) ? parsed.map(String) : [];
  } catch {
    return [];
  }
}

// ============================================================================
// Execute
// ============================================================================

export function executeAgentMemory(
  db: FourDADatabase,
  params: AgentMemoryParams,
): object | Promise<object> {
  const rawDb = db.getRawDb();

  switch (params.action) {
    // ========================================================================
    // STORE
    // ========================================================================
    case "store": {
      if (!params.subject || !params.content) {
        return { error: "subject and content are required for store action" };
      }

      // --- Size limit: max 10KB per entry ---
      const contentBytes = new TextEncoder().encode(params.content).length;
      if (contentBytes > MAX_CONTENT_BYTES) {
        return {
          error: `Content too large: ${contentBytes} bytes exceeds ${MAX_CONTENT_BYTES} byte limit (10KB)`,
        };
      }

      // --- Storage quota: max 1000 entries per agent ---
      const agentType = params.agent_type || "unknown";
      try {
        const countRow = rawDb
          .prepare(
            `SELECT COUNT(*) as cnt FROM agent_memory WHERE agent_type = ?`,
          )
          .get(agentType) as { cnt: number } | undefined;
        const currentCount = countRow?.cnt ?? 0;
        if (currentCount >= MAX_ENTRIES_PER_AGENT) {
          return {
            error: `Storage quota exceeded: agent "${agentType}" has ${currentCount} entries (max ${MAX_ENTRIES_PER_AGENT}). Delete old entries before storing new ones.`,
          };
        }
      } catch (quotaErr) {
        return {
          error: `Failed to check storage quota: ${quotaErr instanceof Error ? quotaErr.message : String(quotaErr)}`,
        };
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
        let sql = `SELECT id, session_id, agent_type, memory_type, subject, content,
                          context_tags, created_at, expires_at, promoted_to_decision_id
                   FROM agent_memory
                   WHERE (expires_at IS NULL OR expires_at > datetime('now'))`;
        const sqlParams: (string | number)[] = [];

        if (params.filter_agent) {
          sql += ` AND agent_type = ?`;
          sqlParams.push(params.filter_agent);
        }

        sql += ` ORDER BY created_at DESC LIMIT ?`;
        sqlParams.push(MAX_ENTRIES_PER_AGENT);

        const rows = rawDb.prepare(sql).all(...sqlParams) as AgentMemoryRow[];
        const limit = params.limit || 20;

        // Optional semantic recall — only when an embedding provider is configured.
        // Returns a Promise (dispatch awaits uniformly); every other path stays
        // synchronous, so callers without a provider behave exactly as before.
        const embedConfig = getEmbeddingConfig();
        if (embedConfig) {
          return enhanceRecallWithSemantics(db, params.query, rows, limit, embedConfig);
        }

        const ranked = rankRowsByRecall(rows, params.query, MEMORY_RECALL_FIELDS, limit);
        return lexicalRecallResult(ranked, rows.length);
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
        // Prefilter to entries whose tags literally contain ANY requested tag...
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
          .all(...tagParams, MAX_ENTRIES_PER_AGENT) as AgentMemoryRow[];

        // ...then rank by relevance so the strongest tag/subject/content matches
        // lead, returning the same shape as `recall` (matched_fields, recall_score).
        const ranked = rankRowsByRecall(
          rows,
          params.tags.join(" "),
          [
            { name: "tags", weight: 4, value: (row) => row.context_tags },
            { name: "subject", weight: 2, value: (row) => row.subject },
            { name: "content", weight: 1, value: (row) => row.content },
          ],
          params.limit || 20,
        );

        return {
          memories: ranked.map((item) => ({
            ...parseMemoryRow(item.row),
            matched_fields: item.matched_fields,
            recall_score: item.score,
          })),
          count: ranked.length,
          candidate_count: rows.length,
          recall_mode: "ranked_lexical",
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

// ============================================================================
// Recall result shaping + optional semantic enhancement
// ============================================================================

interface RankedMemory {
  row: AgentMemoryRow;
  score: number;
  matched_fields: string[];
}

/** Shape a lexical-only recall response (the default, provider-free path). */
function lexicalRecallResult(
  ranked: RankedMemory[],
  candidateCount: number,
): object {
  return {
    memories: ranked.map((item) => ({
      ...parseMemoryRow(item.row),
      matched_fields: item.matched_fields,
      recall_score: item.score,
    })),
    count: ranked.length,
    candidate_count: candidateCount,
    recall_mode: "ranked_lexical",
  };
}

/**
 * Hybrid recall: blends alias-aware lexical scoring with cosine similarity over
 * provider-backed embeddings. Embeddings are cached in the agent_memory table and
 * backfilled lazily (capped per call). ANY failure to embed the query degrades to
 * pure lexical recall, and recall_mode reports exactly which path produced the result.
 */
async function enhanceRecallWithSemantics(
  db: FourDADatabase,
  query: string,
  rows: AgentMemoryRow[],
  limit: number,
  config: EmbeddingConfig,
): Promise<object> {
  // Lexical baseline over ALL candidates — used for blending and as the fallback.
  const lexical = rankRowsByRecall(rows, query, MEMORY_RECALL_FIELDS, rows.length);
  const lexById = new Map<number, RankedMemory>();
  for (const item of lexical) lexById.set(item.row.id, item);
  const maxLex = lexical.length ? lexical[0].score : 0;

  const sem = await semanticScores(
    db,
    "agent_memory",
    query,
    rows.map((row) => ({ id: row.id, text: memoryEmbedText(row) })),
    config,
  );

  // Query could not be embedded -> the provider is effectively unavailable.
  if (!sem.queryEmbedded) {
    return {
      ...lexicalRecallResult(lexical.slice(0, limit), rows.length),
      semantic_note: "embedding provider unreachable — used lexical recall",
    };
  }

  const blended = rows
    .map((row) => {
      const semantic = Math.max(0, sem.semanticById.get(row.id) ?? 0);
      const lex = lexById.get(row.id);
      const lexNorm = maxLex > 0 && lex ? lex.score / maxLex : 0;
      const score = SEMANTIC_BLEND * semantic + (1 - SEMANTIC_BLEND) * lexNorm;
      return { row, semantic, matched: lex?.matched_fields ?? [], score };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit);

  return {
    memories: blended.map((x) => ({
      ...parseMemoryRow(x.row),
      matched_fields: x.matched,
      recall_score: Math.round(x.score * 1000) / 1000,
      semantic_score: Math.round(x.semantic * 1000) / 1000,
    })),
    count: blended.length,
    candidate_count: rows.length,
    embedded_candidates: sem.embeddedCount,
    newly_embedded: sem.newlyEmbedded,
    embedding_model: sem.model,
    recall_mode: sem.embeddedCount > 0 ? "hybrid" : "ranked_lexical",
  };
}
