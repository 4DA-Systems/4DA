// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tools: delete_decision, delete_state, delete_learning, delete_code_location
 *
 * Cleanup tools to remove stale entries that accumulate over time.
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleDeleteDecision(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key } = args as { key: string };
  const result = ctx.db
    .prepare(`DELETE FROM decisions WHERE key = ?`)
    .run(key);

  // Also clean up FTS
  try {
    ctx.db
      .prepare(`DELETE FROM memory_fts WHERE source = 'decision' AND key = ?`)
      .run(key);
  } catch {
    // FTS entry may not exist
  }

  return {
    content: [
      {
        type: "text",
        text:
          result.changes > 0
            ? `Decision '${key}' deleted (${result.changes} row).`
            : `No decision found with key '${key}'.`,
      },
    ],
  };
}

function handleDeleteState(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key } = args as { key: string };
  const result = ctx.db.prepare(`DELETE FROM state WHERE key = ?`).run(key);

  return {
    content: [
      {
        type: "text",
        text:
          result.changes > 0
            ? `State '${key}' deleted (${result.changes} row).`
            : `No state found with key '${key}'.`,
      },
    ],
  };
}

function handleDeleteLearning(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { id } = args as { id: number };
  const result = ctx.db
    .prepare(`DELETE FROM learnings WHERE id = ?`)
    .run(id);

  return {
    content: [
      {
        type: "text",
        text:
          result.changes > 0
            ? `Learning #${id} deleted (${result.changes} row).`
            : `No learning found with id ${id}.`,
      },
    ],
  };
}

function handleDeleteCodeLocation(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { name: locName } = args as { name: string };
  const result = ctx.db
    .prepare(`DELETE FROM code_locations WHERE name = ?`)
    .run(locName);

  return {
    content: [
      {
        type: "text",
        text:
          result.changes > 0
            ? `Code location '${locName}' deleted (${result.changes} row${result.changes > 1 ? "s" : ""}).`
            : `No code location found with name '${locName}'.`,
      },
    ],
  };
}

export const deleteTools: ToolEntry[] = [
  {
    definition: {
      name: "delete_decision",
      description: "Delete a stored decision by key. Use to clean up stale or superseded decisions.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description: "The decision key to delete",
          },
        },
        required: ["key"],
      },
    },
    handler: handleDeleteDecision,
  },
  {
    definition: {
      name: "delete_state",
      description: "Delete a stored state entry by key. Use to clean up stale state.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description: "The state key to delete",
          },
        },
        required: ["key"],
      },
    },
    handler: handleDeleteState,
  },
  {
    definition: {
      name: "delete_learning",
      description: "Delete a stored learning by its numeric id.",
      inputSchema: {
        type: "object",
        properties: {
          id: {
            type: "number",
            description: "The learning id to delete",
          },
        },
        required: ["id"],
      },
    },
    handler: handleDeleteLearning,
  },
  {
    definition: {
      name: "delete_code_location",
      description: "Delete a stored code location by name.",
      inputSchema: {
        type: "object",
        properties: {
          name: {
            type: "string",
            description: "The code location name to delete",
          },
        },
        required: ["name"],
      },
    },
    handler: handleDeleteCodeLocation,
  },
];
