// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Schema Registry for Dynamic Context Discovery
 *
 * Reduces tool listing from ~4500 tokens to ~500 tokens by:
 * - Returning only one-liner summaries in list_tools
 * - Storing full schemas as MCP Resources (lazy-loaded)
 *
 * Also provides category/tag metadata for tool discovery and filtering.
 *
 * Full schemas available at: 4da://schema/{tool_name}
 * Category manifest at: 4da://categories
 */

/** Tool categories — maps to the 8 functional groupings in the MCP server */
export type ToolCategory =
  | "core"
  | "intelligence"
  | "diagnostic"
  | "knowledge"
  | "decisions"
  | "agent"
  | "identity"
  | "metabolism";

/** Shape of each entry in the tool registry */
export interface ToolRegistryEntry {
  summary: string;
  schemaFile: string;
  category: ToolCategory;
  tags: string[];
  standalone: boolean;
}

/**
 * Slim tool registry — one-liner descriptions + category/tag metadata.
 * Full schemas are stored in schemas/*.json and exposed as MCP Resources.
 */
export const TOOL_REGISTRY: Record<string, ToolRegistryEntry> = {
  // Core Tools
  get_relevant_content: {
    summary: "Query filtered content by relevance, source, time",
    schemaFile: "get-relevant-content.json",
    category: "core",
    tags: ["content", "feed", "relevance", "filter", "query"],
    standalone: false,
  },
  get_context: {
    summary: "Get user's interests, tech stack, learned affinities",
    schemaFile: "get-context.json",
    category: "core",
    tags: ["context", "interests", "tech-stack", "profile"],
    standalone: true,
  },
  explain_relevance: {
    summary: "Understand why an item scored the way it did",
    schemaFile: "explain-relevance.json",
    category: "core",
    tags: ["relevance", "scoring", "explanation", "debug"],
    standalone: false,
  },
  record_feedback: {
    summary: "Teach 4DA what you like/dislike (click, save, dismiss)",
    schemaFile: "record-feedback.json",
    category: "core",
    tags: ["feedback", "preference", "learning", "interaction"],
    standalone: false,
  },

  // Intelligence Tools
  score_autopsy: {
    summary: "Deep forensic analysis of relevance scores (AI-powered)",
    schemaFile: "score-autopsy.json",
    category: "intelligence",
    tags: ["scoring", "forensic", "analysis", "ai", "debug"],
    standalone: false,
  },
  trend_analysis: {
    summary: "Statistical patterns, anomalies, and predictions (AI-powered)",
    schemaFile: "trend-analysis.json",
    category: "intelligence",
    tags: ["trends", "patterns", "predictions", "ai", "statistics"],
    standalone: false,
  },
  daily_briefing: {
    summary: "Executive summary of discoveries (AI-powered)",
    schemaFile: "daily-briefing.json",
    category: "intelligence",
    tags: ["briefing", "summary", "daily", "ai", "digest"],
    standalone: false,
  },
  context_analysis: {
    summary: "Optimize your context for better relevance (AI-powered)",
    schemaFile: "context-analysis.json",
    category: "intelligence",
    tags: ["context", "optimization", "relevance", "ai"],
    standalone: false,
  },
  topic_connections: {
    summary: "Build knowledge graphs from content (AI-powered)",
    schemaFile: "topic-connections.json",
    category: "intelligence",
    tags: ["topics", "knowledge-graph", "connections", "ai"],
    standalone: false,
  },
  get_actionable_signals: {
    summary: "Classify content into actionable signals with priority levels",
    schemaFile: "get-actionable-signals.json",
    category: "intelligence",
    tags: ["signals", "priority", "actionable", "classification"],
    standalone: false,
  },
  signal_chains: {
    summary: "Get causal signal chains connecting related events over time",
    schemaFile: "signal-chains.json",
    category: "intelligence",
    tags: ["signals", "causal", "chains", "temporal"],
    standalone: false,
  },
  semantic_shifts: {
    summary: "Detect narrative shifts in topics you follow",
    schemaFile: "semantic-shifts.json",
    category: "intelligence",
    tags: ["narrative", "shifts", "topics", "detection"],
    standalone: false,
  },
  attention_report: {
    summary: "Analyze attention allocation vs codebase needs",
    schemaFile: "attention-report.json",
    category: "intelligence",
    tags: ["attention", "allocation", "codebase", "analysis"],
    standalone: false,
  },

  // Diagnostic Tools
  source_health: {
    summary: "Diagnose source fetching and data quality issues",
    schemaFile: "source-health.json",
    category: "diagnostic",
    tags: ["sources", "health", "fetching", "quality"],
    standalone: false,
  },
  config_validator: {
    summary: "Validate configuration and detect issues",
    schemaFile: "config-validator.json",
    category: "diagnostic",
    tags: ["config", "validation", "settings", "health"],
    standalone: false,
  },
  llm_status: {
    summary: "Check LLM/Ollama configuration and availability",
    schemaFile: "llm-status.json",
    category: "diagnostic",
    tags: ["llm", "ollama", "status", "availability"],
    standalone: false,
  },

  // Knowledge & Health Tools
  export_context_packet: {
    summary: "Generate portable context packet for session handoff",
    schemaFile: "export-context.json",
    category: "knowledge",
    tags: ["export", "context", "handoff", "portable"],
    standalone: false,
  },
  knowledge_gaps: {
    summary: "Detect knowledge gaps in your project dependencies",
    schemaFile: "knowledge-gaps.json",
    category: "knowledge",
    tags: ["gaps", "dependencies", "knowledge", "detection"],
    standalone: false,
  },
  reverse_mentions: {
    summary: "Find where your projects are mentioned in sources",
    schemaFile: "reverse-mentions.json",
    category: "knowledge",
    tags: ["mentions", "projects", "sources", "discovery"],
    standalone: false,
  },
  project_health: {
    summary: "Project health radar for dependency freshness and security",
    schemaFile: "project-health.json",
    category: "knowledge",
    tags: ["project", "health", "dependencies", "security"],
    standalone: false,
  },

  // Decision Intelligence Tools
  decision_memory: {
    summary: "Manage developer decisions (record, list, check, update, supersede)",
    schemaFile: "decision-memory.json",
    category: "decisions",
    tags: ["decisions", "memory", "record", "manage"],
    standalone: true,
  },
  tech_radar: {
    summary: "Generate tech radar from decisions and content signals",
    schemaFile: "tech-radar.json",
    category: "decisions",
    tags: ["tech-radar", "decisions", "signals", "adopt"],
    standalone: false,
  },
  check_decision_alignment: {
    summary: "Check if a technology or pattern aligns with active decisions before suggesting changes",
    schemaFile: "decision-enforcement.json",
    category: "decisions",
    tags: ["alignment", "decisions", "enforcement", "check"],
    standalone: true,
  },

  // Agent Autonomy Tools
  agent_memory: {
    summary: "Cross-agent persistent memory — store and recall across sessions and tools",
    schemaFile: "agent-memory.json",
    category: "agent",
    tags: ["agent", "memory", "persistent", "cross-session"],
    standalone: true,
  },
  agent_session_brief: {
    summary: "Tailored session startup context for AI agents — decisions, changes, memories",
    schemaFile: "agent-session-brief.json",
    category: "agent",
    tags: ["agent", "session", "brief", "startup"],
    standalone: false,
  },
  delegation_score: {
    summary: "AI-delegatability assessment — should the agent proceed or ask the human?",
    schemaFile: "delegation-score.json",
    category: "agent",
    tags: ["agent", "delegation", "autonomy", "assessment"],
    standalone: false,
  },

  // Developer DNA
  developer_dna: {
    summary: "Export your Developer DNA — tech identity, dependencies, engagement, blind spots",
    schemaFile: "developer-dna.json",
    category: "identity",
    tags: ["identity", "dna", "profile", "tech-stack", "export"],
    standalone: false,
  },

  // Intelligence Metabolism Tools
  autophagy_status: {
    summary: "Get intelligence metabolism status — autophagy cycles, calibration accuracy, anti-patterns",
    schemaFile: "autophagy-status.json",
    category: "metabolism",
    tags: ["autophagy", "metabolism", "calibration", "health"],
    standalone: false,
  },
  decision_windows: {
    summary: "View and manage decision windows — time-bounded opportunities requiring attention",
    schemaFile: "decision-windows.json",
    category: "metabolism",
    tags: ["decisions", "windows", "time-bounded", "opportunities"],
    standalone: false,
  },
  compound_advantage: {
    summary: "Get compound advantage score — measures intelligence leverage for decisions",
    schemaFile: "compound-advantage.json",
    category: "metabolism",
    tags: ["compound", "advantage", "leverage", "score"],
    standalone: false,
  },

  // Agent Feedback Tools
  record_agent_feedback: {
    summary: "Record whether agent-recommended content was used, rejected, or partially used",
    schemaFile: "record-agent-feedback.json",
    category: "agent",
    tags: ["agent", "feedback", "learning", "pasifa", "scoring"],
    standalone: false,
  },
  get_agent_feedback_stats: {
    summary: "Get statistics on agent recommendation usage — source usefulness, top items, trends",
    schemaFile: "agent-feedback-stats.json",
    category: "agent",
    tags: ["agent", "feedback", "statistics", "analytics", "pasifa"],
    standalone: false,
  },

  // Synthesis Tools
  what_should_i_know: {
    summary: "Pre-task intelligence briefing — advisories, decisions, signals, delegation assessment",
    schemaFile: "what-should-i-know.json",
    category: "agent",
    tags: ["agent", "briefing", "synthesis", "pre-task", "delegation", "advisories"],
    standalone: true,
  },

  // Trust & Preemption Tools
  trust_summary: {
    summary: "Intelligence quality metrics — precision, action rate, false positives, preemption wins",
    schemaFile: "trust-summary.json",
    category: "intelligence",
    tags: ["trust", "precision", "quality", "metrics", "reliability"],
    standalone: false,
  },
  preemption_feed: {
    summary: "Forward-looking alerts — signal chains, dependency risks, knowledge gaps",
    schemaFile: "preemption-feed.json",
    category: "intelligence",
    tags: ["preemption", "alerts", "risks", "dependencies", "forward-looking"],
    standalone: false,
  },

  // Live Intelligence Tools
  vulnerability_scan: {
    summary: "Scan dependencies for known CVEs via OSV.dev — zero config, multi-ecosystem",
    schemaFile: "vulnerability-scan.json",
    category: "knowledge",
    tags: ["security", "vulnerabilities", "cve", "dependencies", "osv", "scanning"],
    standalone: true,
  },
  ecosystem_pulse: {
    summary: "Live ecosystem news relevant to your tech stack from Hacker News",
    schemaFile: "ecosystem-pulse.json",
    category: "intelligence",
    tags: ["ecosystem", "news", "hacker-news", "trends", "live"],
    standalone: true,
  },
  dependency_health: {
    summary: "Dependency health — version freshness, deprecation, CVEs across npm/Rust/Python/Go",
    schemaFile: "dependency-health.json",
    category: "knowledge",
    tags: ["dependencies", "health", "outdated", "deprecated", "registry", "versions"],
    standalone: true,
  },
  upgrade_planner: {
    summary: "Ranked upgrade recommendations — prioritized by CVE severity, deprecation, version distance",
    schemaFile: "upgrade-planner.json",
    category: "knowledge",
    tags: ["upgrade", "dependencies", "recommendations", "versions", "planning"],
    standalone: true,
  },
};

/**
 * Get slim tool list for list_tools response
 * Returns minimal schema (just type: object) — full schema via resources
 */
export function getSlimToolList(standaloneOnly?: boolean): Array<{
  name: string;
  description: string;
  inputSchema: { type: "object" };
}> {
  return Object.entries(TOOL_REGISTRY)
    .filter(([, info]) => standaloneOnly == null || info.standalone === standaloneOnly)
    .map(([name, info]) => ({
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
  return Object.entries(TOOL_REGISTRY).map(([name]) => ({
    uri: `4da://schema/${name}`,
    name: `${name} schema`,
    description: `Full JSON Schema for the ${name} tool`,
    mimeType: "application/json",
  }));
}

/** Check if a tool exists */
export function hasToolSchema(toolName: string): boolean {
  return toolName in TOOL_REGISTRY;
}

/** Get schema filename for a tool */
export function getSchemaFilename(toolName: string): string | null {
  return TOOL_REGISTRY[toolName]?.schemaFile || null;
}

/** Get tool names grouped by category */
export function getToolsByCategory(): Record<ToolCategory, string[]> {
  const result: Record<string, string[]> = {};
  for (const [name, entry] of Object.entries(TOOL_REGISTRY)) {
    if (!result[entry.category]) {
      result[entry.category] = [];
    }
    result[entry.category].push(name);
  }
  return result as Record<ToolCategory, string[]>;
}

/** Structured category manifest for the 4da://categories resource */
export function getCategoryManifest(): {
  version: string;
  total_tools: number;
  categories: Record<ToolCategory, { tools: string[]; count: number }>;
} {
  const grouped = getToolsByCategory();
  const categories = {} as Record<ToolCategory, { tools: string[]; count: number }>;

  for (const [cat, tools] of Object.entries(grouped)) {
    categories[cat as ToolCategory] = { tools, count: tools.length };
  }

  return {
    version: "1.0.0",
    total_tools: Object.keys(TOOL_REGISTRY).length,
    categories,
  };
}

/** Find tools matching any of the given tags */
export function getToolsByTags(tags: string[]): string[] {
  const tagSet = new Set(tags.map((t) => t.toLowerCase()));
  return Object.entries(TOOL_REGISTRY)
    .filter(([, entry]) => entry.tags.some((t) => tagSet.has(t.toLowerCase())))
    .map(([name]) => name);
}
