// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tools: remember_learning, recall_learnings
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleRememberLearning(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { topic, content, context } = args as {
    topic: string;
    content: string;
    context?: string;
  };

  ctx.db
    .prepare(
      `INSERT INTO learnings (topic, content, context) VALUES (?, ?, ?)`
    )
    .run(topic, content, context || null);

  try {
    ctx.db
      .prepare(
        `INSERT INTO memory_fts (source, key, content) VALUES (?, ?, ?)`
      )
      .run("learning", topic, `${content} ${context || ""}`);
  } catch {
    // Ignore duplicate FTS entries
  }

  return {
    content: [
      { type: "text", text: `Learning about '${topic}' stored.` },
    ],
  };
}

function handleRecallLearnings(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { search, limit = 10 } = args as {
    search: string;
    limit?: number;
  };
  const pattern = `%${search}%`;

  const results = ctx.db
    .prepare(
      `SELECT * FROM learnings
       WHERE topic LIKE ? OR content LIKE ?
       ORDER BY created_at DESC
       LIMIT ?`
    )
    .all(pattern, pattern, limit);

  return {
    content: [
      {
        type: "text",
        text:
          results.length > 0
            ? JSON.stringify(results, null, 2)
            : "No learnings found.",
      },
    ],
  };
}

export const learningTools: ToolEntry[] = [
  {
    definition: {
      name: "remember_learning",
      description:
        "Store something learned during development (gotchas, patterns, discoveries).",
      inputSchema: {
        type: "object",
        properties: {
          topic: {
            type: "string",
            description:
              "Topic category (e.g., 'tauri', 'sqlite-vss', 'react')",
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
    handler: handleRememberLearning,
  },
  {
    definition: {
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
    handler: handleRecallLearnings,
  },
];
