/**
 * Tool exports for 4DA MCP Server
 *
 * Core Tools:
 * - get_relevant_content: Query filtered content by relevance
 * - get_context: Get user's interests, tech stack, learned affinities
 * - explain_relevance: Understand why an item scored the way it did
 * - record_feedback: Teach 4DA what you like/dislike
 *
 * Superpower Tools:
 * - score_autopsy: Deep forensic analysis of relevance scores
 * - trend_analysis: Statistical patterns and anomaly detection
 * - daily_briefing: Executive summaries of discoveries
 * - context_analysis: Optimize your context for better relevance
 * - source_health: Diagnose source pipeline issues
 * - topic_connections: Build knowledge graphs from content
 * - config_validator: Validate configuration and detect issues
 * - get_actionable_signals: Classify items into actionable signal types with priority
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
