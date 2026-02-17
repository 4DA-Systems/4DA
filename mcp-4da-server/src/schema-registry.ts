/**
 * Schema Registry for Dynamic Context Discovery
 *
 * Reduces tool listing from ~4500 tokens to ~500 tokens by:
 * - Returning only one-liner summaries in list_tools
 * - Storing full schemas as MCP Resources (lazy-loaded)
 *
 * Full schemas available at: 4da://schema/{tool_name}
 */

/**
 * Slim tool registry - one-liner descriptions only
 * Full schemas are stored in schemas/*.json and exposed as MCP Resources
 */
export const TOOL_REGISTRY: Record<string, { summary: string; schemaFile: string }> = {
  // Core Tools
  get_relevant_content: {
    summary: "Query filtered content by relevance, source, time",
    schemaFile: "get-relevant-content.json",
  },
  get_context: {
    summary: "Get user's interests, tech stack, learned affinities",
    schemaFile: "get-context.json",
  },
  explain_relevance: {
    summary: "Understand why an item scored the way it did",
    schemaFile: "explain-relevance.json",
  },
  record_feedback: {
    summary: "Teach 4DA what you like/dislike (click, save, dismiss)",
    schemaFile: "record-feedback.json",
  },

  // Superpower Tools
  score_autopsy: {
    summary: "Deep forensic analysis of relevance scores (AI-powered)",
    schemaFile: "score-autopsy.json",
  },
  trend_analysis: {
    summary: "Statistical patterns, anomalies, and predictions (AI-powered)",
    schemaFile: "trend-analysis.json",
  },
  daily_briefing: {
    summary: "Executive summary of discoveries (AI-powered)",
    schemaFile: "daily-briefing.json",
  },
  context_analysis: {
    summary: "Optimize your context for better relevance (AI-powered)",
    schemaFile: "context-analysis.json",
  },
  source_health: {
    summary: "Diagnose source fetching and data quality issues",
    schemaFile: "source-health.json",
  },
  topic_connections: {
    summary: "Build knowledge graphs from content (AI-powered)",
    schemaFile: "topic-connections.json",
  },
  config_validator: {
    summary: "Validate configuration and detect issues",
    schemaFile: "config-validator.json",
  },
  llm_status: {
    summary: "Check LLM/Ollama configuration and availability",
    schemaFile: "llm-status.json",
  },
  get_actionable_signals: {
    summary: "Classify content into actionable signals with priority levels",
    schemaFile: "get-actionable-signals.json",
  },

  // Innovation Feature Tools
  export_context_packet: {
    summary: "Generate portable context packet for session handoff",
    schemaFile: "export-context.json",
  },
  knowledge_gaps: {
    summary: "Detect knowledge gaps in your project dependencies",
    schemaFile: "knowledge-gaps.json",
  },
  signal_chains: {
    summary: "Get causal signal chains connecting related events over time",
    schemaFile: "signal-chains.json",
  },
  semantic_shifts: {
    summary: "Detect narrative shifts in topics you follow",
    schemaFile: "semantic-shifts.json",
  },
  reverse_mentions: {
    summary: "Find where your projects are mentioned in sources",
    schemaFile: "reverse-mentions.json",
  },
  attention_report: {
    summary: "Analyze attention allocation vs codebase needs",
    schemaFile: "attention-report.json",
  },
  project_health: {
    summary: "Project health radar for dependency freshness and security",
    schemaFile: "project-health.json",
  },

  // Decision Intelligence Tools
  decision_memory: {
    summary: "Manage developer decisions (record, list, check, update, supersede)",
    schemaFile: "decision-memory.json",
  },
  tech_radar: {
    summary: "Generate tech radar from decisions and content signals",
    schemaFile: "tech-radar.json",
  },
  check_decision_alignment: {
    summary: "Check if a technology or pattern aligns with active decisions before suggesting changes",
    schemaFile: "decision-enforcement.json",
  },

  // Agent Autonomy Tools
  agent_memory: {
    summary: "Cross-agent persistent memory — store and recall across sessions and tools",
    schemaFile: "agent-memory.json",
  },
  agent_session_brief: {
    summary: "Tailored session startup context for AI agents — decisions, changes, memories",
    schemaFile: "agent-session-brief.json",
  },
  delegation_score: {
    summary: "AI-delegatability assessment — should the agent proceed or ask the human?",
    schemaFile: "delegation-score.json",
  },
};

/**
 * Get slim tool list for list_tools response
 * Returns minimal schema (just type: object) - full schema via resources
 */
export function getSlimToolList(): Array<{
  name: string;
  description: string;
  inputSchema: { type: "object" };
}> {
  return Object.entries(TOOL_REGISTRY).map(([name, info]) => ({
    name,
    description: info.summary,
    inputSchema: { type: "object" as const },
  }));
}

/**
 * Get list of schema resources for ListResources
 */
export function getSchemaResources(): Array<{
  uri: string;
  name: string;
  description: string;
  mimeType: string;
}> {
  return Object.entries(TOOL_REGISTRY).map(([name, info]) => ({
    uri: `4da://schema/${name}`,
    name: `${name} schema`,
    description: `Full JSON Schema for the ${name} tool`,
    mimeType: "application/json",
  }));
}

/**
 * Check if a tool exists
 */
export function hasToolSchema(toolName: string): boolean {
  return toolName in TOOL_REGISTRY;
}

/**
 * Get schema filename for a tool
 */
export function getSchemaFilename(toolName: string): string | null {
  return TOOL_REGISTRY[toolName]?.schemaFile || null;
}
