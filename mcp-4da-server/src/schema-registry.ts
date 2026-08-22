// SPDX-License-Identifier: Apache-2.0
/**
 * Schema Registry for Dynamic Context Discovery
 *
 * Reduces tool listing from ~4500 tokens to ~1300 tokens by:
 * - Returning one-liner summaries in list_tools
 * - Serving the REAL inputSchema only for tools with REQUIRED parameters
 *   (AD-032): a client cannot construct a valid call to those without the
 *   schema, and most clients never read MCP Resources — so a slim
 *   `{type:"object"}` made required-param tools effectively uncallable.
 *   All-optional tools stay slim (`{}` is a valid call) with the full
 *   schema still available as a Resource.
 * - Storing full schemas as MCP Resources (lazy-loaded)
 *
 * Also provides category/tag metadata for tool discovery and filtering.
 *
 * Full schemas available at: 4da://schema/{tool_name}
 * Category manifest at: 4da://categories
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

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
  readOnlyHint: boolean;
  openWorldHint: boolean;
  destructiveHint?: boolean;
  idempotentHint?: boolean;
}

/** Shape of each entry in the tool registry */
export interface ToolRegistryEntry {
  summary: string;
  schemaFile: string;
  category: ToolCategory;
  tags: string[];
  standalone: boolean;
  annotations: ToolAnnotations;
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
    summary: "Scan dependencies for known CVEs via OSV.dev across npm/Rust/Python/Go, zero config. Call when the user asks about security, vulnerabilities, or CVEs, or before you recommend a dependency.",
    schemaFile: "vulnerability-scan.json",
    category: "security",
    tags: ["security", "vulnerabilities", "cve", "dependencies", "osv"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: true },
  },
  dependency_health: {
    summary: "Dependency version freshness, deprecation, and CVE counts across npm/Rust/Python/Go. Call when the user asks whether their dependencies are outdated, stale, or need updating.",
    schemaFile: "dependency-health.json",
    category: "security",
    tags: ["dependencies", "health", "outdated", "deprecated", "versions"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: true },
  },
  upgrade_planner: {
    summary: "Prioritized upgrade plan (CVE severity, deprecation, version distance), quick wins vs breaking changes. Call when the user asks what to upgrade, or after dependency_health surfaces problems.",
    schemaFile: "upgrade-planner.json",
    category: "security",
    tags: ["upgrade", "dependencies", "recommendations", "versions"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: true },
  },

  // --- Intelligence (mixed) ---
  what_should_i_know: {
    summary: "Pre-task briefing: advisories, active decisions, signals, and ecosystem updates for a task. Call BEFORE starting any non-trivial task to get caught up first.",
    schemaFile: "what-should-i-know.json",
    category: "intelligence",
    tags: ["briefing", "advisories", "pre-task", "signals"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: true },
  },
  ecosystem_pulse: {
    summary: "Live Hacker News discussions filtered to the user's tech stack. Call when the user asks what is new or trending in their ecosystem.",
    schemaFile: "ecosystem-pulse.json",
    category: "intelligence",
    tags: ["ecosystem", "news", "hacker-news", "live"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: true },
  },
  get_context: {
    summary: "What 4DA knows about the user: role, tech stack, interests, and learned affinities. Call FIRST when you need to know what the user works on before answering or recommending.",
    schemaFile: "get-context.json",
    category: "intelligence",
    tags: ["context", "interests", "tech-stack", "profile"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  get_relevant_content: {
    summary: "The user's personalized feed: articles, advisories, releases scored by relevance to their stack. Call when the user asks what to read, what is relevant to them, or for content on a topic.",
    schemaFile: "get-relevant-content.json",
    category: "intelligence",
    tags: ["content", "feed", "relevance", "filter"],
    standalone: false,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  get_actionable_signals: {
    summary: "Classify content into prioritized signals (security_alert, breaking_change, tool_discovery, tech_trend, learning, competitive_intel). Call when the user wants what is urgent or actionable, not just relevant.",
    schemaFile: "get-actionable-signals.json",
    category: "intelligence",
    tags: ["signals", "priority", "actionable", "classification"],
    standalone: false,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  knowledge_gaps: {
    summary: "Dependencies the user relies on but never reads about, where a CVE or breaking change could surprise them. Call when the user asks what they are missing or where their blind spots are.",
    schemaFile: "knowledge-gaps.json",
    category: "intelligence",
    tags: ["gaps", "dependencies", "knowledge", "blind-spots"],
    standalone: false,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  record_feedback: {
    summary: "Record click/save/dismiss on a content item to sharpen future scoring. Call AFTER the user reacts to a surfaced item (opens, saves, or dismisses it).",
    schemaFile: "record-feedback.json",
    category: "intelligence",
    tags: ["feedback", "learning", "save", "dismiss"],
    standalone: false,
    annotations: { readOnlyHint: false, openWorldHint: false, destructiveHint: false, idempotentHint: false },
  },

  // --- Decisions (standalone) ---
  decision_memory: {
    summary: "Record, list, update, or supersede the developer's architectural and tech decisions. Call when the user makes, changes, or asks about a settled decision or convention.",
    schemaFile: "decision-memory.json",
    category: "decisions",
    tags: ["decisions", "memory", "record", "architecture"],
    standalone: true,
    annotations: { readOnlyHint: false, openWorldHint: false, destructiveHint: false, idempotentHint: false },
  },
  check_decision_alignment: {
    summary: "Check whether a technology or pattern aligns with the developer's recorded decisions. Call BEFORE suggesting a major tech change, new library, or architecture shift.",
    schemaFile: "decision-enforcement.json",
    category: "decisions",
    tags: ["alignment", "decisions", "enforcement", "check"],
    standalone: true,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },

  // --- Agent (standalone) ---
  agent_memory: {
    summary: "Cross-agent persistent memory: what one agent learns, any agent can recall. Call to store a discovery, decision, or warning, or to recall prior context before starting work.",
    schemaFile: "agent-memory.json",
    category: "agent",
    tags: ["agent", "memory", "persistent", "cross-session"],
    standalone: true,
    annotations: { readOnlyHint: false, openWorldHint: false, destructiveHint: false, idempotentHint: false },
  },

  // --- Identity (full-mode) ---
  developer_dna: {
    summary: "Export the user's Developer DNA: tech identity, primary/adjacent stack, top dependencies, blind spots, engagement stats. Call when the user asks for their developer profile or tech fingerprint.",
    schemaFile: "developer-dna.json",
    category: "identity",
    tags: ["identity", "dna", "profile", "tech-stack", "export"],
    standalone: false,
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
};

/** Schemas live beside this module in both layouts: src/schemas (vitest) and
 * dist/schemas (compiled — populated by the copy-schemas build step). */
const SCHEMAS_DIR = join(dirname(fileURLToPath(import.meta.url)), "schemas");

/** An inputSchema as tools/list serves it — the MCP Tool contract requires
 * the literal `type: "object"` at the root. */
export type ToolInputSchema = { type: "object" } & Record<string, unknown>;

/** Lazy cache: tool name → its real inputSchema when the schema declares
 * required parameters, else null (tool stays slim). */
const requiredParamSchemas = new Map<string, ToolInputSchema | null>();

/**
 * The tools/list discoverability line (AD-032): a tool whose schema declares
 * REQUIRED parameters serves its real inputSchema — `{type:"object"}` hid the
 * required params from every client that never reads MCP Resources, making
 * those tools fail on first call (GPT adversarial audit, finding 7). Tools
 * whose parameters are all optional keep the slim schema: `{}` is a valid
 * call, and the full schema stays one Resource read away. Fail-open: an
 * unreadable or malformed schema file falls back to slim rather than
 * breaking tools/list.
 */
function inputSchemaIfRequired(name: string, schemaFile: string): ToolInputSchema | null {
  if (!requiredParamSchemas.has(name)) {
    let loaded: ToolInputSchema | null = null;
    try {
      const parsed = JSON.parse(readFileSync(join(SCHEMAS_DIR, schemaFile), "utf-8")) as {
        inputSchema?: { type?: unknown; required?: unknown[] } & Record<string, unknown>;
      };
      const schema = parsed.inputSchema;
      if (
        schema &&
        schema.type === "object" &&
        Array.isArray(schema.required) &&
        schema.required.length > 0
      ) {
        loaded = schema as ToolInputSchema;
      }
    } catch {
      loaded = null;
    }
    requiredParamSchemas.set(name, loaded);
  }
  return requiredParamSchemas.get(name) ?? null;
}

/**
 * Get the tool list for the tools/list response: slim summaries throughout,
 * real inputSchema for required-param tools, `{type:"object"}` otherwise
 * (full schema via resources).
 */
export function getSlimToolList(standaloneOnly?: boolean): Array<{
  name: string;
  description: string;
  inputSchema: ToolInputSchema;
  annotations: ToolAnnotations;
}> {
  return Object.entries(TOOL_REGISTRY)
    .filter(([, info]) => standaloneOnly == null || info.standalone === standaloneOnly)
    .map(([name, info]) => ({
      name,
      description: info.summary,
      inputSchema: inputSchemaIfRequired(name, info.schemaFile) ?? { type: "object" as const },
      annotations: info.annotations,
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
