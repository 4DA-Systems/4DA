// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tools: remember_decision, recall_decisions
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleRememberDecision(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key, decision, rationale, alternatives } = args as {
    key: string;
    decision: string;
    rationale?: string;
    alternatives?: string;
  };

  const insertDecision = ctx.db.prepare(`
    INSERT INTO decisions (key, decision, rationale, alternatives)
    VALUES (?, ?, ?, ?)
    ON CONFLICT(key) DO UPDATE SET
      decision = excluded.decision,
      rationale = excluded.rationale,
      alternatives = excluded.alternatives,
      updated_at = CURRENT_TIMESTAMP
  `);

  const insertFts = ctx.db.prepare(`
    INSERT INTO memory_fts (source, key, content)
    VALUES (?, ?, ?)
  `);

  insertDecision.run(key, decision, rationale || null, alternatives || null);

  try {
    insertFts.run("decision", key, `${decision} ${rationale || ""} ${alternatives || ""}`);
  } catch {
    // Ignore duplicate FTS entries
  }

  return {
    content: [{ type: "text", text: `Decision '${key}' stored successfully.` }],
  };
}

/** Compact representation: key + truncated decision + date */
function compactDecision(row: Record<string, unknown>): Record<string, unknown> {
  const decision = String(row.decision || "");
  return {
    key: row.key,
    decision: decision.length > 120 ? decision.slice(0, 117) + "..." : decision,
    updated_at: row.updated_at,
  };
}

function handleRecallDecisions(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key, search, limit = 10, compact } = args as {
    key?: string;
    search?: string;
    limit?: number;
    compact?: boolean;
  };

  // Default compact to true when listing multiple (no key specified)
  const useCompact = compact !== undefined ? compact : !key;

  let results: unknown[];
  if (key) {
    const result = ctx.db
      .prepare(`SELECT * FROM decisions WHERE key = ?`)
      .get(key);
    results = result ? [result] : [];
  } else if (search) {
    const pattern = `%${search}%`;
    results = ctx.db
      .prepare(
        `SELECT * FROM decisions
         WHERE decision LIKE ? OR rationale LIKE ? OR key LIKE ?
         ORDER BY updated_at DESC
         LIMIT ?`
      )
      .all(pattern, pattern, pattern, limit);
  } else {
    results = ctx.db
      .prepare(`SELECT * FROM decisions ORDER BY updated_at DESC LIMIT ?`)
      .all(limit);
  }

  const output = useCompact
    ? (results as Record<string, unknown>[]).map(compactDecision)
    : results;

  return {
    content: [
      {
        type: "text",
        text:
          output.length > 0
            ? JSON.stringify(output, null, 2)
            : "No decisions found.",
      },
    ],
  };
}

export const decisionTools: ToolEntry[] = [
  {
    definition: {
      name: "remember_decision",
      description:
        "Store an architectural or design decision. Use this when a significant choice is made that should survive context compaction.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description:
              "Unique identifier for the decision (e.g., 'auth-strategy', 'db-choice')",
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
    handler: handleRememberDecision,
  },
  {
    definition: {
      name: "recall_decisions",
      description:
        "Retrieve stored decisions. Use when you need to remember what was decided.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description:
              "Specific decision key to retrieve (optional - omit to get all)",
          },
          search: {
            type: "string",
            description: "Search term to filter decisions (optional)",
          },
          limit: {
            type: "number",
            description:
              "Maximum decisions to return (default: 10). Only applies when listing multiple.",
          },
          compact: {
            type: "boolean",
            description:
              "Compact mode: returns key + truncated decision + date only. Default true when listing, false for single-key lookup.",
          },
        },
      },
    },
    handler: handleRecallDecisions,
  },
];
