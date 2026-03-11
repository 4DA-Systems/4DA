---
title: "Building a 30-Tool MCP Server: Lessons from @4da/mcp-server"
published: false
description: "A technical walkthrough of building a large MCP server — schema registry, map-based dispatch, lazy loading, and SQLite as the entire backend."
tags: mcp, typescript, developer-tools, ai
canonical_url: https://4da.ai/blog/mcp-server-architecture
cover_image:
series:
---

The Model Context Protocol spec is simple: expose tools, accept calls, return results. But when your server has 30 tools across 8 categories, that simplicity becomes a design problem. A naive implementation turns into a 2,000-line switch statement, a 4,500-token tool listing that chokes context windows, and a dispatch layer that makes adding new tools painful.

This is a walkthrough of how `@4da/mcp-server` handles these problems. The patterns apply to any MCP server with more than a handful of tools.

## The Architecture at a Glance

The server exposes 30 tools that query a local SQLite database. No network calls to external APIs from the tool layer itself. The data comes from a companion desktop app that scans project manifests, fetches content from sources (Hacker News, arXiv, Reddit, GitHub, RSS), and scores it against the user's detected tech stack.

The MCP server is the read layer. It gives AI coding assistants structured access to that intelligence.

```
src/
  index.ts              # Server lifecycle, request handlers
  tool-dispatch.ts      # Map-based dispatch registry
  schema-registry.ts    # Slim tool listing + category metadata
  db.ts                 # SQLite accessor (better-sqlite3)
  llm.ts                # Optional LLM synthesis (Anthropic/OpenAI/Ollama)
  output-manager.ts     # Token-efficient output formatting
  http-transport.ts     # Streamable HTTP transport (spec 2025-03-26)
  setup.ts              # Editor auto-configuration
  doctor.ts             # Installation health checker
  types.ts              # Shared type definitions
  schemas/              # 30 JSON Schema files (one per tool)
  tools/                # 30 tool implementations (one per file)
    index.ts            # Barrel exports
```

## Problem 1: Tool Listing Blows Up Context Windows

The MCP `list_tools` response includes the full JSON Schema for every tool. With 30 tools, each having 3-8 parameters with descriptions and enums, this balloons to roughly 4,500 tokens. That is context budget the AI model could spend on actually reasoning about the user's question.

The fix is a two-layer schema system.

**Layer 1: Slim listing.** The `list_tools` response returns only a one-liner description per tool. The `inputSchema` is a bare `{ type: "object" }` with no properties. This gets the listing down to around 500 tokens.

```typescript
// schema-registry.ts
export function getSlimToolList() {
  return Object.entries(TOOL_REGISTRY).map(([name, info]) => ({
    name,
    description: info.summary,
    inputSchema: { type: "object" as const },
  }));
}
```

**Layer 2: Full schemas as MCP Resources.** Each tool's complete JSON Schema is stored as a separate `.json` file and exposed via the MCP Resources API at `4da://schema/{tool_name}`. When the AI model needs the full parameter spec, it reads the resource. Lazy loading.

```typescript
// Exposed as MCP Resources
server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const toolName = parseToolNameFromURI(request.params.uri);
  const schemaPath = join(__dirname, "schemas", getSchemaFilename(toolName));
  const schemaContent = readFileSync(schemaPath, "utf-8");
  return { contents: [{ uri, mimeType: "application/json", text: schemaContent }] };
});
```

This is an 80% reduction in initial context cost. The full schemas still exist — they are just loaded on demand.

## Problem 2: Dispatch Without a Giant Switch

The original `index.ts` had a switch statement mapping tool names to handler functions. Every new tool meant editing the switch, importing the function, and hoping you got the case string right.

The replacement is a typed dispatch map:

```typescript
// tool-dispatch.ts
export type ToolExecutor = (db: FourDADatabase, params: any) => unknown | Promise<unknown>;

const DISPATCH_MAP: Record<string, ToolExecutor> = {
  // Core (4 tools)
  get_relevant_content: executeGetRelevantContent,
  get_context:          executeGetContext,
  explain_relevance:    executeExplainRelevance,
  record_feedback:      executeRecordFeedback,

  // Intelligence (9 tools)
  score_autopsy:          executeScoreAutopsy,
  trend_analysis:         executeTrendAnalysis,
  daily_briefing:         executeDailyBriefing,
  // ... 6 more

  // Diagnostic (3), Knowledge (4), Decisions (3), Agent (3), DNA (1), Metabolism (3)
  // ...
};

export async function dispatchTool(name: string, db: FourDADatabase, args) {
  const executor = DISPATCH_MAP[name];
  if (!executor) throw new Error(`Unknown tool: ${name}`);
  const result = await executor(db, args || {});
  return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
}
```

Adding a new tool is three steps: create the tool file, add one line to the dispatch map, add one entry to the schema registry. The dispatch function itself never changes.

The `ToolExecutor` type uses `any` for params intentionally. Type safety lives inside each tool file where the params interface is defined and enforced. The dispatch boundary is a handoff point, not a validation boundary.

## Problem 3: Category-Based Organization

With 30 tools, discoverability matters. A flat list of tool names is not navigable. The schema registry assigns every tool a category and a set of tags:

```typescript
export const TOOL_REGISTRY: Record<string, ToolRegistryEntry> = {
  get_actionable_signals: {
    summary: "Classify content into actionable signals with priority levels",
    schemaFile: "get-actionable-signals.json",
    category: "intelligence",
    tags: ["signals", "priority", "actionable", "classification"],
  },
  // ...
};
```

The eight categories map to distinct capabilities:

| Category | Tools | Purpose |
|----------|-------|---------|
| Core | 4 | Content feed, context, relevance, feedback |
| Intelligence | 9 | Briefings, signals, trends, topic graphs |
| Diagnostic | 3 | Source health, config validation, LLM status |
| Knowledge | 4 | Gaps, project health, mentions, context export |
| Decisions | 3 | Decision memory, tech radar, alignment checks |
| Agent | 3 | Persistent memory, session briefs, delegation |
| Identity | 1 | Developer DNA profile |
| Metabolism | 3 | Autophagy, decision windows, compound advantage |

A `4da://categories` resource exposes this manifest so AI agents can discover tools by category rather than scanning the full list.

## The Tool Implementation Pattern

Every tool follows the same shape. Here is a simplified version of the signal classifier:

```typescript
// tools/get-actionable-signals.ts

import type { FourDADatabase } from "../db.js";

// 1. Define params interface
export interface GetActionableSignalsParams {
  priority_filter?: "critical" | "high" | "medium" | "low";
  signal_type?: string;
  limit?: number;
  since_hours?: number;
}

// 2. Define tool metadata (kept for backward compat; slim registry is primary)
export const getActionableSignalsTool = {
  name: "get_actionable_signals",
  description: "...",
  inputSchema: { /* ... */ },
};

// 3. Implement the execute function
export function executeGetActionableSignals(
  db: FourDADatabase,
  params: GetActionableSignalsParams
) {
  const items = db.getRelevantContent(0.1, undefined, 200, params.since_hours ?? 48);
  const context = db.getUserContext(true, false);
  const detectedTech = (context.ace?.detected_tech || []).map(t => t.name);

  // Classify each item against keyword patterns
  const signals = items
    .map(item => classify(item.title, item.content, item.relevance_score, detectedTech))
    .filter(Boolean);

  // Sort by priority, then relevance
  return { signals: signals.slice(0, params.limit ?? 20), total: signals.length };
}
```

The key constraint: every execute function takes `(db, params)` and returns a plain object. No side effects beyond the database. No network calls. This makes tools testable with a mock database and keeps the dispatch layer trivially simple.

## SQLite as the Entire Backend

The MCP server has exactly two dependencies: `@modelcontextprotocol/sdk` and `better-sqlite3`. No Express, no ORM, no HTTP client library.

The database connection handles contention with the desktop app (which writes to the same WAL-mode database) through a simple retry:

```typescript
queryWithRetry<T>(fn: () => T, maxRetries: number = 1): T {
  try {
    return fn();
  } catch (error) {
    const code = (error as { code?: string }).code;
    if (maxRetries > 0 && (code === "SQLITE_BUSY" || code === "SQLITE_LOCKED")) {
      // Busy-wait 100ms, then retry once
      const start = Date.now();
      while (Date.now() - start < 100) { /* busy wait */ }
      return this.queryWithRetry(fn, maxRetries - 1);
    }
    throw error;
  }
}
```

Database path resolution checks five locations in priority order: environment variable, CWD-relative, project-root-relative, platform-specific Tauri app data directory, and a final fallback. This means the server finds the database whether it is running from the project directory, installed globally, or invoked via `npx`.

## Token-Efficient Output

Large tool results waste context tokens. The output manager writes full results to disk and returns compact summaries inline:

```typescript
export interface CompactResult<T> {
  summary: string;           // 2-3 sentence human-readable summary
  key_data: T;               // Essential 20% of data
  full_result_path: string;  // Path to complete JSON file
  retrieval_hints: string[]; // jq patterns for specific data
}
```

The AI model gets a summary and key data immediately. If it needs the full breakdown, it reads the file. This keeps tool responses small without losing detail.

## Transport: Stdio and Streamable HTTP

The server supports two transports. Stdio is the default, compatible with every MCP host. Streamable HTTP (spec 2025-03-26) runs on port 4840 with localhost-only binding and DNS rebinding protection:

```typescript
// Stateless mode: new transport per request
const transport = new StreamableHTTPServerTransport({
  sessionIdGenerator: undefined,
});
await server.connect(transport);
await transport.handleRequest(req, res, body);
```

Stateless mode means no session tracking. Each request gets its own transport instance. Simple, compatible with serverless if needed, and avoids session-related state bugs entirely.

## Zero-Friction Setup

The `--setup` flag detects installed editors and writes MCP configuration automatically:

```bash
npx @4da/mcp-server --setup
```

It checks for Claude Code (`.claude/settings.json`), Cursor (`~/.cursor/mcp.json`), VS Code (`~/.vscode/mcp.json`), and Windsurf (`~/.windsurf/mcp.json`). Each editor has slightly different config formats — VS Code uses `servers` instead of `mcpServers`, for example. The setup handles each variant.

A `--doctor` command validates the installation: Node version, native bindings, database existence, LLM provider configuration. Every check returns pass/warn/fail with actionable next steps.

## What I Would Do Differently

**Schema validation at the dispatch boundary.** Currently, type safety depends on each tool file correctly defining its params interface. A shared validation step using the JSON Schema files would catch malformed inputs before they reach tool code. We opted against it to avoid the runtime cost on every call, but for a public-facing server, it would be worth it.

**Tool versioning.** When a tool's schema changes (parameter added, enum value removed), there is no mechanism to communicate that to clients already caching the old schema. A version field in the registry, combined with the `listChanged` capability notification, would close this gap.

**Streaming results.** Some tools (trend analysis, topic connections) produce large outputs. Streaming partial results via SSE would let the AI model start processing before the full result is ready. The Streamable HTTP transport supports this, but the tool layer does not yet produce streaming output.

## Takeaways

If you are building an MCP server with more than 10 tools:

1. **Separate schema listing from schema detail.** Return one-liners in `list_tools`, expose full schemas as Resources.
2. **Use a dispatch map, not a switch.** Adding a tool should not require editing core server code.
3. **Organize by category.** Flat tool lists do not scale. Categories and tags make discovery possible.
4. **Keep tools pure.** `(db, params) => result`. No side effects, no network calls in the tool layer.
5. **Validate the environment early.** A `--doctor` command saves users hours of debugging.

The full source is at [github.com/runyourempire/4DA](https://github.com/runyourempire/4DA) in the `mcp-4da-server/` directory.

---

*4DA Systems builds privacy-first developer intelligence tools. The MCP server is open source and available as `npx @4da/mcp-server`.*
