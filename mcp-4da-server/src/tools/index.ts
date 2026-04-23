// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tool exports for 4DA MCP Server — 14 tools across 5 categories
 *
 * Security (3)     — vulnerability scanning, dependency health, upgrade planning
 * Intelligence (6) — briefing, ecosystem news, context, content feed, signals, knowledge gaps, feedback
 * Decisions (2)    — decision memory, alignment checking
 * Agent (1)        — cross-session persistent memory
 * Identity (1)     — developer DNA profile
 */

// Security
export {
  vulnerabilityScanTool,
  executeVulnerabilityScan,
} from "./vulnerability-scan.js";

export {
  dependencyHealthTool,
  executeDependencyHealth,
} from "./dependency-health.js";

export {
  upgradePlannerTool,
  executeUpgradePlanner,
} from "./upgrade-planner.js";

// Intelligence
export {
  whatShouldIKnowTool,
  executeWhatShouldIKnow,
} from "./what-should-i-know.js";

export {
  ecosystemPulseTool,
  executeEcosystemPulse,
} from "./ecosystem-pulse.js";

export { getContextTool, executeGetContext } from "./get-context.js";

export {
  getRelevantContentTool,
  executeGetRelevantContent,
} from "./get-relevant-content.js";

export {
  getActionableSignalsTool,
  executeGetActionableSignals,
} from "./get-actionable-signals.js";

export {
  knowledgeGapsTool,
  executeKnowledgeGaps,
} from "./knowledge-gaps.js";

export {
  recordFeedbackTool,
  executeRecordFeedback,
} from "./record-feedback.js";

// Decisions
export {
  decisionMemoryTool,
  executeDecisionMemory,
} from "./decision-memory.js";

export {
  checkDecisionAlignmentTool,
  executeCheckDecisionAlignment,
} from "./decision-enforcement.js";

// Agent
export {
  agentMemoryTool,
  executeAgentMemory,
} from "./agent-memory.js";

// Identity
export {
  developerDnaTool,
  executeDeveloperDna,
} from "./developer-dna.js";
