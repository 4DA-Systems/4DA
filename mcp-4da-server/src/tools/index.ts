// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tool exports for 4DA MCP Server — 35 tools across 8 categories
 *
 * Core (4)                  — content feed, context, relevance, feedback
 * Intelligence (9)          — briefings, signals, autopsy, trends, topics, chains, shifts, attention
 * Diagnostic (3)            — source health, config validation, LLM status
 * Knowledge & Health (4)    — knowledge gaps, project health, reverse mentions, context export
 * Decision Intelligence (3) — decision memory, tech radar, alignment checks
 * Agent Autonomy (6)        — persistent memory, session briefs, delegation scoring, agent feedback, pre-task briefing
 * Developer DNA (1)         — tech identity profile
 * Intelligence Metabolism (3)— autophagy status, decision windows, compound advantage
 * Trust & Preemption (2)    — trust summary, preemption feed
 */

// Core Tools
export {
  getRelevantContentTool,
  executeGetRelevantContent,
} from "./get-relevant-content.js";

export { getContextTool, executeGetContext } from "./get-context.js";

export {
  explainRelevanceTool,
  executeExplainRelevance,
} from "./explain-relevance.js";

export {
  recordFeedbackTool,
  executeRecordFeedback,
} from "./record-feedback.js";

// Superpower Tools
export {
  scoreAutopsyTool,
  executeScoreAutopsy,
} from "./score-autopsy.js";

export {
  trendAnalysisTool,
  executeTrendAnalysis,
} from "./trend-analysis.js";

export {
  dailyBriefingTool,
  executeDailyBriefing,
} from "./daily-briefing.js";

export {
  contextAnalysisTool,
  executeContextAnalysis,
} from "./context-analysis.js";

export {
  sourceHealthTool,
  executeSourceHealth,
} from "./source-health.js";

export {
  topicConnectionsTool,
  executeTopicConnections,
} from "./topic-connections.js";

export {
  configValidatorTool,
  executeConfigValidator,
} from "./config-validator.js";

export {
  llmStatusTool,
  executeLLMStatus,
} from "./llm-status.js";

// Signal Classifier
export {
  getActionableSignalsTool,
  executeGetActionableSignals,
} from "./get-actionable-signals.js";

// Innovation Feature Tools
export {
  exportContextPacketTool,
  executeExportContextPacket,
} from "./export-context.js";

export {
  knowledgeGapsTool,
  executeKnowledgeGaps,
} from "./knowledge-gaps.js";

export {
  signalChainsTool,
  executeSignalChains,
} from "./signal-chains.js";

export {
  semanticShiftsTool,
  executeSemanticShifts,
} from "./semantic-shifts.js";

export {
  reverseMentionsTool,
  executeReverseMentions,
} from "./reverse-mentions.js";

export {
  attentionReportTool,
  executeAttentionReport,
} from "./attention-report.js";

export {
  projectHealthTool,
  executeProjectHealth,
} from "./project-health.js";

// Decision Intelligence Tools
export {
  decisionMemoryTool,
  executeDecisionMemory,
} from "./decision-memory.js";

export {
  techRadarTool,
  executeTechRadar,
} from "./tech-radar.js";

export {
  checkDecisionAlignmentTool,
  executeCheckDecisionAlignment,
} from "./decision-enforcement.js";

// Agent Autonomy Tools
export {
  agentMemoryTool,
  executeAgentMemory,
} from "./agent-memory.js";

export {
  agentSessionBriefTool,
  executeAgentSessionBrief,
} from "./agent-session-brief.js";

export {
  delegationScoreTool,
  executeDelegationScore,
} from "./delegation-score.js";

// Developer DNA
export {
  developerDnaTool,
  executeDeveloperDna,
} from "./developer-dna.js";

// Intelligence Metabolism Tools
export {
  autophagyStatusTool,
  executeAutophagyStatus,
} from "./autophagy-status.js";

export {
  decisionWindowsTool,
  executeDecisionWindows,
} from "./decision-windows.js";

export {
  compoundAdvantageTool,
  executeCompoundAdvantage,
} from "./compound-advantage.js";

// Agent Feedback Tools
export {
  recordAgentFeedbackTool,
  executeRecordAgentFeedback,
} from "./record-agent-feedback.js";

export {
  agentFeedbackStatsTool,
  executeGetAgentFeedbackStats,
} from "./agent-feedback-stats.js";

// Synthesis Tools
export {
  whatShouldIKnowTool,
  executeWhatShouldIKnow,
} from "./what-should-i-know.js";

// Trust & Preemption Tools
export {
  trustSummaryTool,
  executeTrustSummary,
} from "./trust-summary.js";

export {
  preemptionFeedTool,
  executePreemptionFeed,
} from "./preemption-feed.js";

// Live Intelligence Tools
export {
  vulnerabilityScanTool,
  executeVulnerabilityScan,
} from "./vulnerability-scan.js";
