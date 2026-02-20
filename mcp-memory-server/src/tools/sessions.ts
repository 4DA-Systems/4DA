/**
 * Tools: list_sessions, search_sessions, get_session_messages, index_sessions
 */

import { existsSync, readdirSync } from "fs";
import { join } from "path";
import { indexSession } from "../sessions.js";
import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleListSessions(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { limit = 20, index_new = true } = args as {
    limit?: number;
    index_new?: boolean;
  };

  // Check for new sessions to index
  if (index_new && existsSync(ctx.sessionsDir)) {
    const files = readdirSync(ctx.sessionsDir).filter((f) =>
      f.endsWith(".jsonl")
    );
    const allIndexed = ctx.db
      .prepare(`SELECT * FROM session_index ORDER BY session_date DESC`)
      .all() as Array<{ session_file: string }>;
    const indexed = new Set(allIndexed.map((r) => r.session_file));

    for (const file of files) {
      if (!indexed.has(file)) {
        indexSession(ctx.db, ctx.sessionsDir, file);
      }
    }
  }

  const sessions = ctx.db
    .prepare(`SELECT * FROM session_index ORDER BY session_date DESC`)
    .all();
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

function handleSearchSessions(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { query, limit = 10 } = args as { query: string; limit?: number };

  try {
    const results = ctx.db
      .prepare(
        `SELECT DISTINCT session_file, snippet(session_fts, 1, '>>>', '<<<', '...', 64) as snippet
         FROM session_fts
         WHERE session_fts MATCH ?
         LIMIT ?`
      )
      .all(query, limit);

    return {
      content: [
        {
          type: "text",
          text:
            (results as unknown[]).length > 0
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

function handleGetSessionMessages(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { session_file, offset = 0, limit = 50 } = args as {
    session_file: string;
    offset?: number;
    limit?: number;
  };

  const getMessages = ctx.db.prepare(
    `SELECT * FROM session_messages
     WHERE session_file = ?
     ORDER BY message_index
     LIMIT ? OFFSET ?`
  );

  const messages = getMessages.all(session_file, limit, offset);

  if ((messages as unknown[]).length === 0) {
    // Try to index first if not found
    if (existsSync(join(ctx.sessionsDir, session_file))) {
      indexSession(ctx.db, ctx.sessionsDir, session_file);
      const retryMessages = getMessages.all(session_file, limit, offset);
      if ((retryMessages as unknown[]).length > 0) {
        return {
          content: [
            { type: "text", text: JSON.stringify(retryMessages, null, 2) },
          ],
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

function handleIndexSessions(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { force = false } = args as { force?: boolean };

  if (!existsSync(ctx.sessionsDir)) {
    return {
      content: [
        {
          type: "text",
          text: `Sessions directory not found: ${ctx.sessionsDir}`,
        },
      ],
    };
  }

  const files = readdirSync(ctx.sessionsDir).filter((f) =>
    f.endsWith(".jsonl")
  );
  const indexed = force
    ? new Set<string>()
    : new Set(
        (
          ctx.db
            .prepare(
              `SELECT * FROM session_index ORDER BY session_date DESC`
            )
            .all() as Array<{ session_file: string }>
        ).map((r) => r.session_file)
      );

  let newlyIndexed = 0;
  let totalMessages = 0;

  for (const file of files) {
    if (!indexed.has(file)) {
      const result = indexSession(ctx.db, ctx.sessionsDir, file);
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

export const sessionTools: ToolEntry[] = [
  {
    definition: {
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
            description:
              "Also index any new unindexed sessions (default: true)",
          },
        },
      },
    },
    handler: handleListSessions,
  },
  {
    definition: {
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
    handler: handleSearchSessions,
  },
  {
    definition: {
      name: "get_session_messages",
      description:
        "Get messages from a specific past session. Use to read exact conversation history.",
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
            description:
              "Number of messages to retrieve (default: 50)",
          },
        },
        required: ["session_file"],
      },
    },
    handler: handleGetSessionMessages,
  },
  {
    definition: {
      name: "index_sessions",
      description:
        "Index all session transcripts for searching. Run this periodically to index new sessions.",
      inputSchema: {
        type: "object",
        properties: {
          force: {
            type: "boolean",
            description:
              "Re-index all sessions even if already indexed (default: false)",
          },
        },
      },
    },
    handler: handleIndexSessions,
  },
];
