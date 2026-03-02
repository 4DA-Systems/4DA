/**
 * Tool registry -- aggregates all tool definitions and handlers.
 */

import type { Tool } from "@modelcontextprotocol/sdk/types.js";
import type { ToolContext, ToolResponse, ToolEntry } from "../types.js";

import { decisionTools } from "./decisions.js";
import { stateTools } from "./state.js";
import { learningTools } from "./learnings.js";
import { codeLocationTools } from "./code-locations.js";
import { searchTools } from "./search.js";
import { sessionTools } from "./sessions.js";
import { metricTools } from "./metrics.js";
import { deleteTools } from "./delete-tools.js";

/** All registered tools in order. */
const allTools: ToolEntry[] = [
  ...decisionTools,
  ...stateTools,
  ...learningTools,
  ...codeLocationTools,
  ...searchTools,
  ...sessionTools,
  ...metricTools,
  ...deleteTools,
];

/** Handler lookup by tool name. */
const handlerMap = new Map<
  string,
  (args: Record<string, unknown>, ctx: ToolContext) => ToolResponse
>();

for (const entry of allTools) {
  handlerMap.set(entry.definition.name, entry.handler);
}

/** All tool definitions for the ListTools response. */
export function getToolDefinitions(): Tool[] {
  return allTools.map((t) => t.definition);
}

/** Dispatch a tool call by name. Returns null if tool not found. */
export function dispatchTool(
  name: string,
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse | null {
  const handler = handlerMap.get(name);
  if (!handler) return null;
  return handler(args, ctx);
}
