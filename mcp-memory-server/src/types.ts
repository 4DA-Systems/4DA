// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Shared types for MCP Memory Server tool modules.
 */
import type { Tool } from "@modelcontextprotocol/server";

import type Database from "better-sqlite3";

/** Context passed to every tool handler */
export interface ToolContext {
  db: Database.Database;
  sessionsDir: string;
}

/**
 * Standard MCP tool response (index signature required by MCP SDK).
 * `type` is the `"text"` literal — SDK v2 types handler returns from the spec,
 * so a widened `string` no longer assigns to `CallToolResult.content`.
 */
export interface ToolResponse {
  [key: string]: unknown;
  content: Array<{ type: "text"; text: string }>;
  isError?: boolean;
}

/** Each tool module exports an array of these */
export interface ToolEntry {
  definition: Tool;
  handler: (args: Record<string, unknown>, ctx: ToolContext) => ToolResponse;
}
