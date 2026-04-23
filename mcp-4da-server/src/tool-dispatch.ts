// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
  return {
    content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
  };
}

/** Check if a tool exists in the dispatch map */
export function hasDispatchTool(name: string): boolean {
  return name in DISPATCH_MAP;
}

/** Get count of registered tool executors */
export function getDispatchToolCount(): number {
  return Object.keys(DISPATCH_MAP).length;
}
