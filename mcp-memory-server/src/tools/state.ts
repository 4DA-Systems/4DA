/**
 * Tools: update_state, get_state
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleUpdateState(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key, value } = args as { key: string; value: string };

  ctx.db
    .prepare(
      `INSERT INTO state (key, value)
       VALUES (?, ?)
       ON CONFLICT(key) DO UPDATE SET
         value = excluded.value,
         updated_at = CURRENT_TIMESTAMP`
    )
    .run(key, value);

  return {
    content: [{ type: "text", text: `State '${key}' updated.` }],
  };
}

function handleGetState(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { key } = args as { key?: string };

  let results;
  if (key) {
    const result = ctx.db
      .prepare(`SELECT * FROM state WHERE key = ?`)
      .get(key);
    results = result ? [result] : [];
  } else {
    results = ctx.db.prepare(`SELECT * FROM state ORDER BY key`).all();
  }

  return {
    content: [
      {
        type: "text",
        text:
          (results as unknown[]).length > 0
            ? JSON.stringify(results, null, 2)
            : "No state found.",
      },
    ],
  };
}

export const stateTools: ToolEntry[] = [
  {
    definition: {
      name: "update_state",
      description:
        "Update current project state. Use to track what we're working on, what's blocked, etc.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description:
              "State key (e.g., 'current_task', 'blocked_on', 'last_file_modified')",
          },
          value: {
            type: "string",
            description: "State value",
          },
        },
        required: ["key", "value"],
      },
    },
    handler: handleUpdateState,
  },
  {
    definition: {
      name: "get_state",
      description:
        "Get current project state. Use to understand what we were working on.",
      inputSchema: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description:
              "Specific state key (optional - omit to get all state)",
          },
        },
      },
    },
    handler: handleGetState,
  },
];
