#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
 *
 * Protocol: MCP TypeScript SDK v2, served via `serveStdio` — 2025-era hosts
 * (classic `initialize` handshake) and 2026-07-28 hosts (stateless
 * `server/discover`) are both supported on the same stdio endpoint.
 */
import { serveStdio } from "@modelcontextprotocol/server/stdio";
import { Server } from "@modelcontextprotocol/server";
import { getDb, closeDb, DB_PATH, SESSIONS_DIR } from "./db.js";
import { getToolDefinitions, dispatchTool } from "./tools/index.js";
import type { ToolContext } from "./types.js";

// Initialize database
const db = getDb();

// Build shared context for tool handlers
const toolContext: ToolContext = { db, sessionsDir: SESSIONS_DIR };

/**
 * Build a Server instance with the tool handlers registered. `serveStdio`
 * takes a factory and pins one instance per connection; handlers close over
 * the module-level database singleton.
 */
function buildServer(): Server {
  const server = new Server(
    { name: "mcp-memory-server", version: "2.0.0" },
    { capabilities: { tools: {} } }
  );

  // Handle tool listing
  server.setRequestHandler("tools/list", async () => ({
    tools: getToolDefinitions(),
  }));

  // Handle tool calls
  server.setRequestHandler("tools/call", async (request) => {
    const { name, arguments: args } = request.params;

    try {
      const result = dispatchTool(
        name,
        (args as Record<string, unknown>) || {},
        toolContext
      );

      if (!result) {
        return {
          content: [{ type: "text" as const, text: `Unknown tool: ${name}` }],
          isError: true,
        };
      }

      return result;
    } catch (error) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Error: ${error instanceof Error ? error.message : String(error)}`,
          },
        ],
        isError: true,
      };
    }
  });

  return server;
}

// Start server — `serveStdio` owns the era decision per connection: a
// 2025-era `initialize` opening is served exactly as before; a 2026-07-28
// `server/discover` opening gets the stateless modern protocol.
function main() {
  serveStdio(buildServer, {
    onerror: (error) => {
      console.error(`[Memory] stdio serving error: ${error.message}`);
    },
  });

  const toolCount = getToolDefinitions().length;
  console.error(
    `MCP Memory Server v2.0 started -- ${toolCount} tools, stdio transport`
  );
  console.error(`  Database: ${DB_PATH}`);
  console.error(`  Sessions: ${SESSIONS_DIR}`);
}

main();

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
