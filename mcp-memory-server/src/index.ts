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
} from "@modelcontextprotocol/sdk/types.js";

import { getDb, closeDb, DB_PATH, SESSIONS_DIR } from "./db.js";
import { getToolDefinitions, dispatchTool } from "./tools/index.js";
import type { ToolContext } from "./types.js";

// Initialize database
const db = getDb();

// Build shared context for tool handlers
const toolContext: ToolContext = { db, sessionsDir: SESSIONS_DIR };

// Create server
const server = new Server(
  { name: "mcp-memory-server", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

// Handle tool listing
server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: getToolDefinitions(),
}));

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    const result = dispatchTool(
      name,
      (args as Record<string, unknown>) || {},
      toolContext
    );

    if (!result) {
      return {
        content: [{ type: "text", text: `Unknown tool: ${name}` }],
        isError: true,
      };
    }

    return result;
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

  const toolCount = getToolDefinitions().length;
  console.error(
    `MCP Memory Server v1.0 started -- ${toolCount} tools, stdio transport`
  );
  console.error(`  Database: ${DB_PATH}`);
  console.error(`  Sessions: ${SESSIONS_DIR}`);
}

main().catch(console.error);

// Handle graceful shutdown
process.on("SIGINT", () => {
  console.error("[Memory] Received SIGINT -- shutting down gracefully");
  closeDb();
  process.exit(0);
});

process.on("SIGTERM", () => {
  console.error("[Memory] Received SIGTERM -- shutting down gracefully");
  closeDb();
  process.exit(0);
});
