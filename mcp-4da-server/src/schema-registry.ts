// SPDX-License-Identifier: Apache-2.0
/**
 * Tool registry: what tools/list serves, and the schema resources.
 *
 * ONE source of truth per tool: the `xxxTool` definition exported beside its
 * executor. The executor's params interface and the schema sit in the same
 * file, and a test (schema-contract.test.ts) holds every parameter the
 * executor reads to a declared property and back. Until 2026-10-02 the
 * schemas ALSO lived in schemas/*.json, kept by hand, and those copies had
 * drifted: agent_memory's published schema said `topic` where the code reads
 * `query` and omitted the `subject` that store requires, so neither recall nor
 * store could be called correctly by any client that read it (verified live).
 *
 * Every tool serves its full inputSchema. AD-032 served the full schema only
 * for tools with REQUIRED parameters and `{type:"object"}` for the rest, on the
 * theory that the full schema was one Resource read away. Most hosts never read
 * MCP Resources, so 30+ optional parameters (project_path, severity_filter,
 * include_dev, limit, since_hours, ...) were invisible to every agent, while
 * tool output told agents to "pass include_dev". The full list costs about
 * 4k tokens, inside the 3-5k band of well-regarded servers.
 */

import {
  agentMemoryTool,
  checkDecisionAlignmentTool,
  decisionMemoryTool,
  dependencyCheckTool,
  dependencyHealthTool,
  developerDnaTool,
  ecosystemPulseTool,
  getActionableSignalsTool,
  getContextTool,
  getRelevantContentTool,
  knowledgeGapsTool,
  recordFeedbackTool,
  upgradeImpactTool,
  upgradePlannerTool,
  vulnerabilityScanTool,
  whatShouldIKnowTool,
} from "./tools/index.js";

/** Tool categories — maps to the functional groupings in the MCP server */
export type ToolCategory =
  | "security"
  | "intelligence"
  | "decisions"
  | "agent"
  | "identity";

/**
 * MCP tool annotations (spec 2025-03-26+). Directory reviews (Anthropic
 * Connectors, client marketplaces) expect these on every tool; a missing
 * readOnlyHint/destructiveHint is a documented rejection cause.
 */
export interface ToolAnnotations {
  title: string;
  readOnlyHint: boolean;
  openWorldHint: boolean;
  destructiveHint?: boolean;
  idempotentHint?: boolean;
}

/** An inputSchema as tools/list serves it — the MCP Tool contract requires the literal `type: "object"` at the root. */
export type ToolInputSchema = { type: "object" } & Record<string, unknown>;

/** A tool definition as each tool module exports it. */
export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: { type: "object"; properties?: Record<string, unknown>; required?: readonly string[] };
}

/** Shape of each entry in the tool registry */
export interface ToolRegistryEntry {
  /** One-paragraph description served in tools/list: what it does, then when to call it. */
  summary: string;
  definition: ToolDefinition;
  category: ToolCategory;
  tags: string[];
  standalone: boolean;
  annotations: ToolAnnotations;
}

const READ_LOCAL = { readOnlyHint: true, openWorldHint: false } as const;
const READ_NETWORK = { readOnlyHint: true, openWorldHint: true } as const;
const WRITE_LOCAL = { readOnlyHint: false, openWorldHint: false, destructiveHint: false, idempotentHint: false } as const;

/**
 * 16 tools: 11 standalone + 5 that need the 4DA desktop app's database.
 * Order is deterministic (the 2026-07-28 revision asks for it: stable
 * tools/list output keeps host prompt caches warm).
 */
export const TOOL_REGISTRY: Record<string, ToolRegistryEntry> = {
  // --- Dependency Security (standalone) ---
  vulnerability_scan: {
    summary: "Scan this project's lockfiles (npm/pnpm/yarn, Cargo, Python, Go) for known vulnerabilities via OSV.dev, transitives included, with fix versions and where each version is pinned. Call when the user asks about security, vulnerabilities or CVEs, or before you recommend a dependency.",
    definition: vulnerabilityScanTool,
    category: "security",
    tags: ["security", "vulnerabilities", "cve", "dependencies", "osv"],
    standalone: true,
    annotations: { title: "Vulnerability scan", ...READ_NETWORK },
  },
  dependency_health: {
    summary: "Dependency version freshness, deprecation and CVE counts across npm/Rust/Python/Go. Call when the user asks whether their dependencies are outdated, stale or need updating.",
    definition: dependencyHealthTool,
    category: "security",
    tags: ["dependencies", "health", "outdated", "deprecated", "versions"],
    standalone: true,
    annotations: { title: "Dependency health", ...READ_NETWORK },
  },
  upgrade_planner: {
    summary: "Prioritized upgrade plan: the 4DA app's work order when computed (per-line targets, manifest vs lockfile fix), else the smallest version that fixes each vulnerability from the lockfiles. Call when the user asks what to upgrade.",
    definition: upgradePlannerTool,
    category: "security",
    tags: ["upgrade", "dependencies", "recommendations", "versions"],
    standalone: true,
    annotations: { title: "Upgrade planner", ...READ_NETWORK },
  },
  upgrade_impact: {
    summary: "What changes between the installed version of ONE dependency and a target version: changelog entries per release, breaking changes, deprecations, security fixes, and the files in this project that import it. Call before upgrading or bumping a dependency.",
    definition: upgradeImpactTool,
    category: "security",
    tags: ["upgrade", "changelog", "breaking-changes", "migration", "release-notes"],
    standalone: true,
    annotations: { title: "Upgrade impact", ...READ_NETWORK },
  },
  dependency_check: {
    summary: "Verdict (proceed/wait/review/avoid/unknown) with evidence for adding a dependency or bumping one to a version. Call BEFORE you add a package or apply any version bump.",
    definition: dependencyCheckTool,
    category: "security",
    tags: ["dependencies", "supply-chain", "pre-install", "upgrade", "versions"],
    standalone: true,
    annotations: { title: "Dependency check", ...READ_NETWORK, idempotentHint: true },
  },

  // --- Intelligence (mixed) ---
  what_should_i_know: {
    summary: "Pre-task briefing scoped to the task: the dependencies it touches, their installed versions, version-confirmed vulnerabilities, releases since, your recorded decisions, and a delegation verdict. Call BEFORE starting a non-trivial task, especially one that changes dependencies.",
    definition: whatShouldIKnowTool,
    category: "intelligence",
    tags: ["briefing", "advisories", "pre-task", "delegation"],
    standalone: true,
    annotations: { title: "Pre-task briefing", ...READ_NETWORK },
  },
  ecosystem_pulse: {
    summary: "Recent Hacker News discussions that name a dependency or framework this project actually uses. Call when the user asks what is new or being discussed in their ecosystem.",
    definition: ecosystemPulseTool,
    category: "intelligence",
    tags: ["ecosystem", "news", "hacker-news", "live"],
    standalone: true,
    annotations: { title: "Ecosystem pulse", ...READ_NETWORK },
  },
  get_context: {
    summary: "What 4DA knows about the user: role, tech stack, interests, exclusions, and detected project context. Call FIRST when you need to know what the user works on before answering or recommending.",
    definition: getContextTool,
    category: "intelligence",
    tags: ["context", "interests", "tech-stack", "profile"],
    standalone: true,
    annotations: { title: "User context", ...READ_LOCAL },
  },
  get_relevant_content: {
    summary: "The user's personalized feed from the 4DA app: articles, advisories and releases that passed its relevance judge. Call when the user asks what to read, what is relevant to them, or for content on a topic.",
    definition: getRelevantContentTool,
    category: "intelligence",
    tags: ["content", "feed", "relevance", "filter"],
    standalone: false,
    annotations: { title: "Relevant content", ...READ_LOCAL },
  },
  get_actionable_signals: {
    summary: "Feed items the 4DA app classified as actionable (security alerts, breaking changes, tool discoveries, trends), shown only when its relevance judge accepted them, plus this project's live vulnerabilities. Call when the user wants what is urgent, not just relevant.",
    definition: getActionableSignalsTool,
    category: "intelligence",
    tags: ["signals", "priority", "actionable", "classification"],
    standalone: false,
    annotations: { title: "Actionable signals", ...READ_LOCAL },
  },
  knowledge_gaps: {
    summary: "Dependencies of this project with judged-relevant advisories or releases the user has not looked at, where a CVE or breaking change could surprise them. Call when the user asks what they are missing or where their blind spots are.",
    definition: knowledgeGapsTool,
    category: "intelligence",
    tags: ["gaps", "dependencies", "knowledge", "blind-spots"],
    standalone: false,
    annotations: { title: "Knowledge gaps", ...READ_NETWORK },
  },
  record_feedback: {
    summary: "Record click/save/dismiss/mark_irrelevant on a content item; save and mark_irrelevant are also relevance labels. Call AFTER the user reacts to a surfaced item (opens, saves, dismisses, or rates it).",
    definition: recordFeedbackTool,
    category: "intelligence",
    tags: ["feedback", "history", "save", "dismiss"],
    standalone: false,
    annotations: { title: "Record feedback", ...WRITE_LOCAL },
  },

  // --- Decisions (standalone) ---
  decision_memory: {
    summary: "Record, list, update, or supersede the developer's architectural and tech decisions. Call when the user makes, changes, or asks about a settled decision or convention.",
    definition: decisionMemoryTool,
    category: "decisions",
    tags: ["decisions", "memory", "record", "architecture"],
    standalone: true,
    annotations: { title: "Decision memory", ...WRITE_LOCAL },
  },
  check_decision_alignment: {
    summary: "Check whether a technology or pattern aligns with the developer's recorded decisions. Call BEFORE suggesting a major tech change, new library, or architecture shift.",
    definition: checkDecisionAlignmentTool,
    category: "decisions",
    tags: ["alignment", "decisions", "enforcement", "check"],
    standalone: true,
    annotations: { title: "Decision alignment check", ...READ_LOCAL },
  },

  // --- Agent (standalone) ---
  agent_memory: {
    summary: "Cross-agent persistent memory: what one agent learns, any agent can recall. Call to store a discovery, decision, or warning, or to recall prior context before starting work.",
    definition: agentMemoryTool,
    category: "agent",
    tags: ["agent", "memory", "persistent", "cross-session"],
    standalone: true,
    annotations: { title: "Agent memory", ...WRITE_LOCAL },
  },

  // --- Identity (full-mode) ---
  developer_dna: {
    summary: "The user's developer profile from local data: primary and adjacent stack, most-used dependencies across their projects, engaged topics. Call when the user asks for their developer profile or tech fingerprint.",
    definition: developerDnaTool,
    category: "identity",
    tags: ["identity", "dna", "profile", "tech-stack", "export"],
    standalone: false,
    annotations: { title: "Developer DNA", ...READ_LOCAL },
  },
};

/**
 * Get the tool list for the tools/list response: the summary, the full
 * inputSchema and the annotations of every tool.
 */
export function getSlimToolList(standaloneOnly?: boolean): Array<{
  name: string;
  title: string;
  description: string;
  inputSchema: ToolInputSchema;
  annotations: ToolAnnotations;
}> {
  return Object.entries(TOOL_REGISTRY)
    .filter(([, info]) => standaloneOnly == null || info.standalone === standaloneOnly)
    .map(([name, info]) => ({
      name,
      title: info.annotations.title,
      description: info.summary,
      inputSchema: info.definition.inputSchema as ToolInputSchema,
      annotations: info.annotations,
    }));
}

/** The full schema document a `4da://schema/{tool}` resource serves. */
export function getToolSchemaDocument(toolName: string): object | null {
  const entry = TOOL_REGISTRY[toolName];
  if (!entry) return null;
  return {
    name: toolName,
    title: entry.annotations.title,
    description: `${entry.summary}\n\n${entry.definition.description}`,
    inputSchema: entry.definition.inputSchema,
    annotations: entry.annotations,
    category: entry.category,
    standalone: entry.standalone,
  };
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
