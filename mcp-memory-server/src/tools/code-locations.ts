/**
 * Tools: remember_code_location, recall_code_locations
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleRememberCodeLocation(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { name: locName, file_path, line_number, purpose } = args as {
    name: string;
    file_path: string;
    line_number?: number;
    purpose?: string;
  };

  ctx.db
    .prepare(
      `INSERT INTO code_locations (name, file_path, line_number, purpose)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(name, file_path) DO UPDATE SET
         line_number = excluded.line_number,
         purpose = excluded.purpose,
         updated_at = CURRENT_TIMESTAMP`
    )
    .run(locName, file_path, line_number || null, purpose || null);

  return {
    content: [
      { type: "text", text: `Code location '${locName}' stored.` },
    ],
  };
}

function handleRecallCodeLocations(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { search, limit = 20 } = args as { search: string; limit?: number };
  const pattern = `%${search}%`;

  const results = ctx.db
    .prepare(
      `SELECT * FROM code_locations
       WHERE name LIKE ? OR purpose LIKE ? OR file_path LIKE ?
       ORDER BY updated_at DESC
       LIMIT ?`
    )
    .all(pattern, pattern, pattern, limit);

  return {
    content: [
      {
        type: "text",
        text:
          results.length > 0
            ? JSON.stringify(results, null, 2)
            : "No code locations found.",
      },
    ],
  };
}

export const codeLocationTools: ToolEntry[] = [
  {
    definition: {
      name: "remember_code_location",
      description: "Store important code location for quick reference.",
      inputSchema: {
        type: "object",
        properties: {
          name: {
            type: "string",
            description:
              "Descriptive name (e.g., 'main-indexer', 'feed-component')",
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
    handler: handleRememberCodeLocation,
  },
  {
    definition: {
      name: "recall_code_locations",
      description: "Find remembered code locations.",
      inputSchema: {
        type: "object",
        properties: {
          search: {
            type: "string",
            description: "Search by name, path, or purpose",
          },
          limit: {
            type: "number",
            description: "Maximum results to return (default: 20)",
          },
        },
        required: ["search"],
      },
    },
    handler: handleRecallCodeLocations,
  },
];
