// SPDX-License-Identifier: Apache-2.0
/**
 * Tool Dispatch Registry
 *
 * Map-based dispatch for the 14 active tools.
 * Adding a new tool = add to this map + schema-registry + barrel export.
 */

import type { FourDADatabase } from "./db.js";

import {
  executeGetRelevantContent,
  executeGetContext,
  executeRecordFeedback,
  executeGetActionableSignals,
  executeKnowledgeGaps,
  executeDecisionMemory,
  executeCheckDecisionAlignment,
  executeAgentMemory,
  executeDeveloperDna,
  executeWhatShouldIKnow,
  executeVulnerabilityScan,
  executeEcosystemPulse,
  executeDependencyHealth,
  executeUpgradePlanner,
} from "./tools/index.js";

import { getLiveIntelligence } from "./live-singleton.js";

/**
 * Executor signature — all tool execute functions follow this shape.
 * Awaiting a sync return resolves immediately, so we can await uniformly.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ToolExecutor = (db: FourDADatabase, params: any) => unknown | Promise<unknown>;

/**
 * Dispatch map — tool name → execute function
 */
const DISPATCH_MAP: Record<string, ToolExecutor> = {
  // Dependency Security
  vulnerability_scan: (db, params) => executeVulnerabilityScan(db, params, getLiveIntelligence()),
  dependency_health: (db, params) => executeDependencyHealth(db, params, getLiveIntelligence()),
  upgrade_planner: (db, params) => executeUpgradePlanner(db, params, getLiveIntelligence()),

  // Intelligence
  what_should_i_know: executeWhatShouldIKnow,
  ecosystem_pulse: (db, params) => executeEcosystemPulse(db, params, getLiveIntelligence()),
  get_context: executeGetContext,
  get_relevant_content: executeGetRelevantContent,
  get_actionable_signals: executeGetActionableSignals,
  knowledge_gaps: executeKnowledgeGaps,
  record_feedback: executeRecordFeedback,

  // Decisions
  decision_memory: executeDecisionMemory,
  check_decision_alignment: executeCheckDecisionAlignment,

  // Agent
  agent_memory: executeAgentMemory,

  // Identity
  developer_dna: executeDeveloperDna,
};

/**
 * DB-backed tools whose payload reflects the curated feed and can therefore go stale. The MCP
 * server reads `4da.db` but cannot fetch or score, so these get a `data_freshness` annotation that
 * tells the caller whether the data is fresh. Live tools (vulnerability_scan, dependency_health,
 * ecosystem_pulse) fetch on demand and are excluded; static tools (get_context, decisions, memory)
 * are not time-sensitive feed data and are excluded too.
 */
const FRESHNESS_TOOLS = new Set([
  "get_relevant_content",
  "get_actionable_signals",
  "what_should_i_know",
]);

/**
 * Dispatch a tool call by name. Awaits uniformly (no-op for sync executors).
 * Returns MCP-formatted response with JSON-serialized result.
 */
export async function dispatchTool(
  name: string,
  db: FourDADatabase,
  args: Record<string, unknown> | undefined,
): Promise<{ content: Array<{ type: string; text: string }> }> {
  const executor = DISPATCH_MAP[name];
  if (!executor) {
    throw new Error(`Unknown tool: ${name}`);
  }

  const result = await executor(db, (args || {}) as Record<string, unknown>);
  const payload = FRESHNESS_TOOLS.has(name) ? attachFreshness(db, result) : result;
  return {
    content: [{ type: "text", text: JSON.stringify(payload, null, 2) }],
  };
}

/**
 * Attach a `data_freshness` block so consumers can distinguish fresh feed data from stale. An array
 * result (get_relevant_content) is wrapped as `{ data_freshness, item_count, items }`; an object
 * result gains a `data_freshness` field. Best-effort: if the freshness read fails, the raw result
 * is returned unchanged rather than failing the tool call.
 */
function attachFreshness(db: FourDADatabase, result: unknown): unknown {
  let data_freshness;
  try {
    data_freshness = db.getFreshness();
  } catch {
    return result;
  }
  if (Array.isArray(result)) {
    return { data_freshness, item_count: result.length, items: result };
  }
  if (result && typeof result === "object") {
    return { data_freshness, ...(result as Record<string, unknown>) };
  }
  return { data_freshness, result };
}

/** Check if a tool exists in the dispatch map */
export function hasDispatchTool(name: string): boolean {
  return name in DISPATCH_MAP;
}

/** Get count of registered tool executors */
export function getDispatchToolCount(): number {
  return Object.keys(DISPATCH_MAP).length;
}
