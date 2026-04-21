#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * 4DA MCP Server - Internal Use Only
 *
 * This MCP server provides tool and resource access to 4DA's local database
 * for use with Claude Desktop and other MCP-compatible hosts.
 *
 * IMPORTANT: This server is designed for LOCAL use only. It reads from the
 * user's local 4DA SQLite database. It does NOT implement authentication
 * or authorization - it trusts the local environment.
 *
 * Do NOT expose this server over a network without adding proper auth.
 */
/**
 * 4DA MCP Server v4.1.1 — Intelligence Platform
 *
 * 36 tools across 9 categories. Live vulnerability scanning (OSV.dev),
 * persistent project memory, and tech stack awareness for any MCP host.
 *
 * Categories (canonical — matches schema-registry.ts `ToolCategory`):
 *   Core (4)                    — content feed, context, relevance, feedback
 *   Intelligence (11)           — briefings, signals, autopsy, trends, topics,
 *                                 chains, shifts, attention, trust, preemption
 *   Diagnostic (3)              — source health, config, LLM status
 *   Knowledge & Health (4)      — gaps, project health, mentions, context export
 *   Decision Intelligence (3)   — decision memory, tech radar, alignment checks
 *   Agent Autonomy (6)          — persistent memory, session briefs, delegation,
 *                                 agent feedback record + stats, what_should_i_know
 *   Developer DNA / Identity (1)— tech identity profile
 *   Intelligence Metabolism (3) — autophagy, decision windows, compound advantage
 *   Security (1)                — live vulnerability scanning via OSV.dev
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

import { startHttpServer } from "./http-transport.js";
import { runSetup } from "./setup.js";
import { runDoctor } from "./doctor.js";
import { scanCurrentProject } from "./project-scanner.js";
import { LiveIntelligence } from "./live/index.js";
import { setLiveIntelligence } from "./live-singleton.js";

// Schema registry for slim tool listing + category metadata
import { getSlimToolList, getSchemaResources, hasToolSchema, getSchemaFilename, getCategoryManifest } from "./schema-registry.js";

// Map-based tool dispatch (replaces per-tool imports + switch statement)
import { dispatchTool } from "./tool-dispatch.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

import { createDatabase, FourDADatabase, type DatabaseValidationResult } from "./db.js";

// =============================================================================
// Server Setup
// =============================================================================

const server = new Server(
  {
    name: "4da-server",
    version: "4.1.1",
  },
  {
    capabilities: {
      tools: { listChanged: true },
      resources: {},
    },
  }
);

// Database instance (lazy initialized)
let db: FourDADatabase | null = null;
let liveIntel: LiveIntelligence | null = null;

/**
 * Get or create database connection.
 * In standalone mode (no existing 4DA database), creates a minimal database
 * and populates it from the current working directory's project files.
 */
function getDatabase(): FourDADatabase {
  if (!db) {
    const dbPath = process.env.FOURDA_DB_PATH;
    db = createDatabase(dbPath);

    // Initialize live intelligence layer
    liveIntel = new LiveIntelligence(db.getRawDb());
    setLiveIntelligence(liveIntel);

    // Standalone mode: auto-populate from project scan
    if (db.isStandalone) {
      const cwd = process.cwd();
      const scan = scanCurrentProject(cwd);
      db.populateFromScan(scan);

      const detected = [
        ...scan.languages,
        ...scan.frameworks,
      ].filter(Boolean);

      console.error(
        `[4DA] Standalone mode: scanned ${scan.projectPath}`
      );
      if (detected.length > 0) {
        console.error(
          `[4DA]   Detected: ${detected.join(", ")} | ${scan.dependencies.length} deps, ${scan.devDependencies.length} dev deps`
        );

        // Initialize live intelligence with resolved versions
        const primaryLang = scan.languages.includes("typescript") || scan.languages.includes("javascript")
          ? "npm"
          : scan.languages.includes("rust")
            ? "rust"
            : scan.languages.includes("python")
              ? "python"
              : scan.languages.includes("go")
                ? "go"
                : "npm";

        liveIntel.initFromProject(cwd, scan.dependencies, scan.devDependencies, primaryLang);

        if (liveIntel.isEnabled()) {
          console.error(`[4DA]   Live intelligence: enabled (OSV.dev + HN)`);
          // Background prefetch — non-blocking, warms cache for first tool call
          const techStack = [...scan.languages, ...scan.frameworks];
          liveIntel.scanVulnerabilities(cwd).catch(() => {});
          liveIntel.fetchHeadlines(techStack).catch(() => {});
        }
      } else {
        console.error(
          `[4DA]   No project manifests found in ${cwd} — tools will return empty results`
        );
      }
    } else {
      // Full 4DA database mode — still initialize live intel from project_dependencies
      try {
        const rawDb = db.getRawDb();
        const deps = rawDb.prepare(
          "SELECT DISTINCT package_name, language FROM project_dependencies WHERE is_dev = 0 LIMIT 200",
        ).all() as Array<{ package_name: string; language: string }>;
        const devDeps = rawDb.prepare(
          "SELECT DISTINCT package_name FROM project_dependencies WHERE is_dev = 1 LIMIT 100",
        ).all() as Array<{ package_name: string }>;

        if (deps.length > 0) {
          const primaryLang = deps[0].language || "npm";
          liveIntel.initFromProject(
            process.cwd(),
            deps.map((d) => d.package_name),
            devDeps.map((d) => d.package_name),
            primaryLang,
          );
        }
      } catch {
        // Non-fatal — live intel just won't have version data
      }
    }
  }
  return db;
}

// =============================================================================
// Tool Handlers
// =============================================================================

/**
 * List available tools (SLIM)
 *
 * Returns one-liner descriptions only (~500 tokens vs ~4500).
 * Full schemas available via MCP Resources: 4da://schema/{tool_name}
 */
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: getSlimToolList(),
  };
});

/**
 * List schema resources
 *
 * Exposes full tool schemas as MCP Resources for lazy loading.
 * Also exposes skill manifest for agent dispatch.
 */
server.setRequestHandler(ListResourcesRequestSchema, async () => {
  const resources = [
    ...getSchemaResources(),
    // Skill manifest for agent dispatch
    {
      uri: "4da://skills",
      name: "Skill manifest",
      description: "Registry of 4DA skills for Claude Code agent dispatch",
      mimeType: "application/json",
    },
    // Category manifest for tool discovery
    {
      uri: "4da://categories",
      name: "Tool categories",
      description: "Tool groupings by category with tag metadata",
      mimeType: "application/json",
    },
  ];

  return { resources };
});

/**
 * Read a resource (schema or skill manifest)
 */
server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const uri = request.params.uri;

  // Handle category manifest
  if (uri === "4da://categories") {
    return {
      contents: [
        {
          uri,
          mimeType: "application/json",
          text: JSON.stringify(getCategoryManifest(), null, 2),
        },
      ],
    };
  }

  // Handle skill manifest
  if (uri === "4da://skills") {
    const skillsPath = join(homedir(), ".local", "share", "4da", "skills", "registry.json");
    if (!existsSync(skillsPath)) {
      throw new Error("Skill manifest not found. Run 4DA setup to create it.");
    }
    const skillsContent = readFileSync(skillsPath, "utf-8");
    return {
      contents: [
        {
          uri,
          mimeType: "application/json",
          text: skillsContent,
        },
      ],
    };
  }

  // Parse tool name from URI: 4da://schema/{tool_name}
  const match = uri.match(/^4da:\/\/schema\/(.+)$/);
  if (!match) {
    throw new Error(`Invalid resource URI: ${uri}`);
  }

  const toolName = match[1];
  if (!hasToolSchema(toolName)) {
    throw new Error(`Unknown tool: ${toolName}`);
  }

  const schemaFile = getSchemaFilename(toolName);
  if (!schemaFile) {
    throw new Error(`No schema file for tool: ${toolName}`);
  }

  // Read schema from file
  const schemaPath = join(__dirname, "schemas", schemaFile);
  const schemaContent = readFileSync(schemaPath, "utf-8");

  return {
    contents: [
      {
        uri,
        mimeType: "application/json",
        text: schemaContent,
      },
    ],
  };
});

/**
 * Execute a tool
 */
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    const database = getDatabase();
    return await dispatchTool(name, database, args as Record<string, unknown> | undefined);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    return {
      content: [
        {
          type: "text",
          text: JSON.stringify({ error: errorMessage }, null, 2),
        },
      ],
      isError: true,
    };
  }
});

// =============================================================================
// Server Lifecycle
// =============================================================================

/**
 * Start the MCP server
 *
 * Supports three modes:
 *   (default)  stdio transport — classic MCP, works with all hosts
 *   --http     Streamable HTTP transport (spec 2025-03-26)
 *   --setup    Configure editors for 4DA MCP
 */
async function main() {
  const args = process.argv.slice(2);

  // Version
  if (args.includes("--version") || args.includes("-v")) {
    console.log("@4da/mcp-server 4.1.1");
    return;
  }

  // Help
  if (args.includes("--help") || args.includes("-h")) {
    console.log(`
  @4da/mcp-server — 36 tools for codebase-aware developer intelligence

  Usage:
    npx @4da/mcp-server              Start MCP server (stdio transport)
    npx @4da/mcp-server --http       Start Streamable HTTP transport
    npx @4da/mcp-server --setup      Auto-configure your editor
    npx @4da/mcp-server --doctor     Check installation health
    npx @4da/mcp-server --version    Print version

  Options:
    --http              Use Streamable HTTP instead of stdio
    --port <number>     HTTP port (default: 4840)
    --host <address>    HTTP bind address (default: 127.0.0.1)
    --setup             Detect editors and write MCP config
    --doctor            Validate database, bindings, and LLM providers

  Environment:
    FOURDA_DB_PATH      Path to 4DA's SQLite database (auto-detected if omitted)
    FOURDA_OFFLINE      Set to "true" to disable all network calls (OSV.dev, HN)

  Works standalone (scans your project on startup) or with the full
  4DA desktop app for content scoring, source monitoring, and more.
  Desktop app: https://github.com/runyourempire/4DA/releases/latest
  Docs:        https://4da.ai
`);
    return;
  }

  // Setup command: configure editors
  if (args.includes("--setup") || args.includes("setup")) {
    runSetup();
    return;
  }

  // Doctor command: validate installation health
  if (args.includes("--doctor") || args.includes("doctor")) {
    runDoctor();
    return;
  }

  // HTTP transport mode
  if (args.includes("--http")) {
    const portIndex = args.indexOf("--port");
    const port = portIndex !== -1 ? parseInt(args[portIndex + 1], 10) : 4840;
    const host = args.includes("--host") ? args[args.indexOf("--host") + 1] : "127.0.0.1";
    await startHttpServer(server, { port, host });
    return;
  }

  // -------------------------------------------------------------------------
  // Pre-flight: validate the database before accepting tool calls
  // -------------------------------------------------------------------------
  const dbPath = process.env.FOURDA_DB_PATH || undefined;
  const validation: DatabaseValidationResult = FourDADatabase.validateDatabase(dbPath);

  if (validation.valid) {
    console.error(`[4DA] Database validated — ${validation.tables?.length ?? 0} tables found`);
  } else if (validation.standalone) {
    // No existing DB — standalone mode will create one on first tool call
    console.error(`[4DA] No existing database found — standalone mode enabled.`);
    console.error(`  Will scan your project and create a local database on first tool call.`);
    console.error(`  For full features, install the 4DA desktop app: https://4da.ai`);
    console.error(``);
  } else {
    // Database exists but is corrupt or unreadable
    console.error(`[4DA] Database issue: ${validation.error}`);
    console.error(`  Or run: npx @4da/mcp-server --doctor  for diagnostics`);
    console.error(``);
  }

  // Default: stdio transport (existing behavior)
  const transport = new StdioServerTransport();
  await server.connect(transport);

  // Handle graceful shutdown
  process.on("SIGINT", () => {
    console.error("[4DA] Received SIGINT — shutting down gracefully");
    if (db) db.close();
    process.exit(0);
  });

  process.on("SIGTERM", () => {
    console.error("[4DA] Received SIGTERM — shutting down gracefully");
    if (db) db.close();
    process.exit(0);
  });

  const toolCount = getSlimToolList().length;
  console.error(`4DA MCP Server v4.1.1 started — ${toolCount} tools, stdio transport`);
  console.error("  Use --http for Streamable HTTP, --setup to configure editors, --doctor to check health");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
