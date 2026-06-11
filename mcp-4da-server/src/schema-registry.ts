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

/** Tool categories — maps to the functional groupings in the MCP server */
export type ToolCategory =
  | "security"
  | "intelligence"
  | "decisions"
  | "agent"
  | "identity";

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
 *
 * 14 tools total: 9 standalone + 5 full-mode.
 */
export const TOOL_REGISTRY: Record<string, ToolRegistryEntry> = {
  // --- Dependency Security (standalone) ---
  vulnerability_scan: {
    summary: "Scan project dependencies for known CVEs via OSV.dev (npm/Rust/Python/Go), zero config. Call when the user asks about security, vulnerabilities, or CVEs, asks whether their dependencies are safe, or before you recommend adding a dependency.",
    schemaFile: "vulnerability-scan.json",
    category: "security",
    tags: ["security", "vulnerabilities", "cve", "dependencies", "osv"],
    standalone: true,
  },
  dependency_health: {
    summary: "Health of project dependencies: version freshness, deprecation status, and CVEs across npm/Rust/Python/Go. Call when the user asks whether their dependencies are outdated, stale, deprecated, or need updating.",
    schemaFile: "dependency-health.json",
    category: "security",
    tags: ["dependencies", "health", "outdated", "deprecated", "versions"],
    standalone: true,
  },
  upgrade_planner: {
    summary: "Ranked, prioritized upgrade plan (by CVE severity, deprecation, version distance) separating safe quick wins from breaking changes. Call when the user asks what to upgrade or how to plan upgrades, or after dependency_health surfaces problems.",
    schemaFile: "upgrade-planner.json",
    category: "security",
    tags: ["upgrade", "dependencies", "recommendations", "versions"],
    standalone: true,
  },

  // --- Intelligence (mixed) ---
  what_should_i_know: {
    summary: "Pre-task intelligence briefing: advisories, active decision windows, signals, and ecosystem updates for a given task. Call BEFORE starting any non-trivial task to get caught up on what matters first.",
    schemaFile: "what-should-i-know.json",
    category: "intelligence",
    tags: ["briefing", "advisories", "pre-task", "signals"],
    standalone: true,
  },
  ecosystem_pulse: {
    summary: "Live Hacker News discussions filtered to the user's detected tech stack. Call when the user asks what is new, trending, or happening in their ecosystem or tech community.",
    schemaFile: "ecosystem-pulse.json",
    category: "intelligence",
    tags: ["ecosystem", "news", "hacker-news", "live"],
    standalone: true,
  },
  get_context: {
    summary: "What 4DA knows about the user: declared role, tech stack, interests, ACE-detected activity, and learned affinities. Call FIRST when you need to understand what the user works on or cares about before answering or recommending.",
    schemaFile: "get-context.json",
    category: "intelligence",
    tags: ["context", "interests", "tech-stack", "profile"],
    standalone: true,
  },
  get_relevant_content: {
    summary: "The user's personalized feed: articles, advisories, and releases scored by relevance to their stack and interests. Call when the user asks what they should read, what is relevant to them, or for content on a specific topic.",
    schemaFile: "get-relevant-content.json",
    category: "intelligence",
    tags: ["content", "feed", "relevance", "filter"],
    standalone: false,
  },
  get_actionable_signals: {
    summary: "Classify content into prioritized signal types (security_alert, breaking_change, tool_discovery, tech_trend, learning, competitive_intel). Call when the user wants to know what is actionable or urgent, not just relevant.",
    schemaFile: "get-actionable-signals.json",
    category: "intelligence",
    tags: ["signals", "priority", "actionable", "classification"],
    standalone: false,
  },
  knowledge_gaps: {
    summary: "Blind spots: dependencies the user relies on but never reads about, where a breaking change or CVE could catch them off guard. Call when the user asks what they are missing, what they are not tracking, or where their blind spots are.",
    schemaFile: "knowledge-gaps.json",
    category: "intelligence",
    tags: ["gaps", "dependencies", "knowledge", "blind-spots"],
    standalone: false,
  },
  record_feedback: {
    summary: "Teach 4DA by recording click/save/dismiss/mark_irrelevant on a content item to sharpen future scoring. Call AFTER the user reacts to a surfaced item (opens, saves, or dismisses it).",
    schemaFile: "record-feedback.json",
    category: "intelligence",
    tags: ["feedback", "learning", "save", "dismiss"],
    standalone: false,
  },

  // --- Decisions (standalone) ---
  decision_memory: {
    summary: "Record, list, update, or supersede the developer's architectural and technology decisions across sessions. Call when the user makes, changes, or asks about a settled decision, choice, or convention.",
    schemaFile: "decision-memory.json",
    category: "decisions",
    tags: ["decisions", "memory", "record", "architecture"],
    standalone: true,
  },
  check_decision_alignment: {
    summary: "Check whether a technology or pattern aligns with the developer's recorded decisions. Call BEFORE suggesting a major tech change, new library, or architecture shift, to avoid contradicting settled decisions.",
    schemaFile: "decision-enforcement.json",
    category: "decisions",
    tags: ["alignment", "decisions", "enforcement", "check"],
    standalone: true,
  },

  // --- Agent (standalone) ---
  agent_memory: {
    summary: "Cross-agent persistent memory: what one AI agent learns, any agent can recall, across sessions and tools. Call to store a discovery, decision, or warning worth keeping, or to recall prior context before starting work.",
    schemaFile: "agent-memory.json",
    category: "agent",
    tags: ["agent", "memory", "persistent", "cross-session"],
    standalone: true,
  },

  // --- Identity (full-mode) ---
  developer_dna: {
    summary: "Export the user's Developer DNA: tech identity, primary and adjacent stack, top dependencies, engaged topics, blind spots, and engagement stats. Call when the user asks for a summary of their developer profile, identity, or tech fingerprint.",
    schemaFile: "developer-dna.json",
    category: "identity",
    tags: ["identity", "dna", "profile", "tech-stack", "export"],
    standalone: false,
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
