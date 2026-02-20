/**
 * Tool: search_memory
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleSearchMemory(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { query, limit = 20 } = args as { query: string; limit?: number };

  try {
    const results = ctx.db
      .prepare(`SELECT * FROM memory_fts WHERE memory_fts MATCH ? LIMIT ?`)
      .all(query, limit);

    return {
      content: [
        {
          type: "text",
          text:
            results.length > 0
              ? JSON.stringify(results, null, 2)
              : "No results found.",
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

export const searchTools: ToolEntry[] = [
  {
    definition: {
      name: "search_memory",
      description:
        "Full-text search across all memory (decisions, learnings, etc.)",
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
    handler: handleSearchMemory,
  },
];
