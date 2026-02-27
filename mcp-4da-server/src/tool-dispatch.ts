/**
 * Tool Dispatch Registry
 *
 * Map-based dispatch replacing the monolithic switch statement in index.ts.
 * All 30 execute functions are imported once and mapped by tool name.
 *
 * Adding a new tool = add to this map + schema-registry + barrel export.
 */

import type { FourDADatabase } from "./db.js";

import {
  executeGetRelevantContent,
  executeGetContext,
  executeExplainRelevance,
  executeRecordFeedback,
  executeScoreAutopsy,
  executeTrendAnalysis,
  executeDailyBriefing,
  executeContextAnalysis,
  executeSourceHealth,
  executeTopicConnections,
  executeConfigValidator,
  executeLLMStatus,
  executeGetActionableSignals,
  executeExportContextPacket,
  executeKnowledgeGaps,
  executeSignalChains,
  executeSemanticShifts,
  executeReverseMentions,
  executeAttentionReport,
  executeProjectHealth,
  executeDecisionMemory,
  executeTechRadar,
  executeCheckDecisionAlignment,
  executeAgentMemory,
  executeAgentSessionBrief,
  executeDelegationScore,
  executeDeveloperDna,
  executeAutophagyStatus,
  executeDecisionWindows,
  executeCompoundAdvantage,
} from "./tools/index.js";

/**
 * Executor signature — all tool execute functions follow this shape.
 * Awaiting a sync return resolves immediately, so we can await uniformly.
 *
 * Uses `any` for params because each tool defines its own strict param type —
 * type safety is enforced inside each tool file, not at the dispatch boundary.
 * (The original switch statement used `as unknown as XParams` on every call.)
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ToolExecutor = (db: FourDADatabase, params: any) => unknown | Promise<unknown>;

/**
 * Dispatch map — tool name → execute function
 */
const DISPATCH_MAP: Record<string, ToolExecutor> = {
  // Core
  get_relevant_content: executeGetRelevantContent,
  get_context: executeGetContext,
  explain_relevance: executeExplainRelevance,
  record_feedback: executeRecordFeedback,

  // Intelligence
  score_autopsy: executeScoreAutopsy,
  trend_analysis: executeTrendAnalysis,
  daily_briefing: executeDailyBriefing,
  context_analysis: executeContextAnalysis,
  topic_connections: executeTopicConnections,
  get_actionable_signals: executeGetActionableSignals,
  signal_chains: executeSignalChains,
  semantic_shifts: executeSemanticShifts,
  attention_report: executeAttentionReport,

  // Diagnostic
  source_health: executeSourceHealth,
  config_validator: executeConfigValidator,
  llm_status: executeLLMStatus,

  // Knowledge & Health
  export_context_packet: executeExportContextPacket,
  knowledge_gaps: executeKnowledgeGaps,
  reverse_mentions: executeReverseMentions,
  project_health: executeProjectHealth,

  // Decision Intelligence
  decision_memory: executeDecisionMemory,
  tech_radar: executeTechRadar,
  check_decision_alignment: executeCheckDecisionAlignment,

  // Agent Autonomy
  agent_memory: executeAgentMemory,
  agent_session_brief: executeAgentSessionBrief,
  delegation_score: executeDelegationScore,

  // Developer DNA
  developer_dna: executeDeveloperDna,

  // Intelligence Metabolism
  autophagy_status: executeAutophagyStatus,
  decision_windows: executeDecisionWindows,
  compound_advantage: executeCompoundAdvantage,
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
