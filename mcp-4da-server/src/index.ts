#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0
/**
 * 4DA MCP Server
 *
 * Provides dependency-intelligence tools (CVE scanning, dependency health,
 * upgrade planning, ecosystem news, decision/agent memory) to Claude Code,
 * Claude Desktop, and other MCP hosts. Runs locally; the only data leaving the
 * machine is public package names/versions sent to registries (OSV, npm, etc.).
 *
 * SECURITY: stdio serving is unauthenticated by design — the MCP host launches
 * this process and already holds local process rights. The optional --http
 * transport defaults to 127.0.0.1 and applies a Host-header DNS rebinding
 * guard to every request. Binding it anywhere else requires a shared auth
 * secret (MCP_AUTH_SECRET) and then verifies an HMAC-SHA256-signed Bearer
 * token on every request, enforcing the token's role at tool dispatch.
 *
 * 14 tools across 5 categories. Live vulnerability scanning (OSV.dev),
 * ecosystem news, persistent memory, and tech stack awareness for any MCP host.
 *
 * Categories (canonical — matches schema-registry.ts `ToolCategory`):
 *   Security (3)      — vulnerability scanning, dependency health, upgrade planning
 *   Intelligence (7)  — briefing, ecosystem pulse, context, content feed,
 *                       actionable signals, knowledge gaps, feedback
 *   Decisions (2)     — decision memory, alignment checking
 *   Agent (1)         — cross-session persistent memory
 *   Identity (1)      — developer DNA profile
 *
 * Protocol: MCP TypeScript SDK v2. stdio serving goes through `serveStdio`,
 * HTTP through `createMcpHandler` — both entries negotiate the era per
 * connection/request, so 2025-era hosts (classic `initialize` handshake) and
 * 2026-07-28 hosts (stateless `server/discover`) are both supported.
 */
import { serveStdio } from "@modelcontextprotocol/server/stdio";
import { Server } from "@modelcontextprotocol/server";
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

// Single source of truth for the server version — read from package.json so
// serverInfo and --version can never drift from the published version again
// (drift shipped in 4.6.1: serverInfo reported 4.6.0).
const SERVER_VERSION: string = (() => {
  try {
    const pkg = JSON.parse(readFileSync(join(__dirname, "..", "package.json"), "utf8")) as { version?: string };
    return pkg.version ?? "0.0.0";
  } catch {
    return "0.0.0";
  }
})();

import { createDatabase, DEPENDENCY_GROUP_QUERY, FourDADatabase, type DatabaseValidationResult } from "./db.js";

// =============================================================================
// Server Setup
// =============================================================================

/**
 * Project root for scanning/scoping. FOURDA_PROJECT_DIR overrides cwd for
 * hosts where cwd is meaningless (Claude Desktop launches extension servers
 * from an app directory) — the .mcpb bundle wires its directory picker to it.
 * A value still containing "${" is an unsubstituted host template; ignore it.
 */
function resolveProjectDir(): string {
  const dir = process.env.FOURDA_PROJECT_DIR?.trim();
  if (dir && !dir.includes("${")) return dir;
  return process.cwd();
}

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

    // Standalone mode: auto-populate from project scan.
    // FOURDA_PROJECT_DIR overrides cwd for hosts where cwd is meaningless
    // (Claude Desktop launches extension servers from an app directory, not
    // the user's code) — the .mcpb bundle wires its directory picker to this.
    if (db.isStandalone) {
      const cwd = resolveProjectDir();
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

        // Initialize live intelligence with per-ecosystem resolved versions
        // (depTargets carries platform-gated dep info so advisories can be
        // flagged platform-relevant for the host).
        liveIntel.initFromMultiEcosystem(cwd, scan.depsByEcosystem, scan.depTargets);

        if (liveIntel.isEnabled()) {
          console.error(`[4DA]   Live intelligence: enabled (OSV.dev + HN)`);
          // Background prefetch — non-blocking, warms cache for first tool call
          const techStack = [...scan.languages, ...scan.frameworks];
          liveIntel.scanVulnerabilities(cwd).catch((err) => {
            console.error(`[4DA]   Vulnerability prefetch failed: ${err instanceof Error ? err.message : String(err)}. Will retry on first tool call.`);
          });
          liveIntel.fetchHeadlines(techStack).catch((err) => {
            console.error(`[4DA]   Headline prefetch failed: ${err instanceof Error ? err.message : String(err)}.`);
          });
        }
      } else {
        console.error(
          `[4DA]   No project manifests found in ${cwd} — tools will return empty results`
        );
      }
    } else {
      // Full 4DA database mode — resolve each dependency's version from its OWN
      // manifest directory. project_dependencies stores a per-dependency
      // project_path, and dependencies span multiple ecosystems and locations
      // (Rust crates under src-tauri/, relay/, etc.; npm packages at the repo
      // root and in sub-packages). Resolving everything against a single cwd
      // silently dropped all Rust deps (no Cargo.lock at the repo root) and most
      // npm deps living in sub-packages.
      try {
        const rawDb = db.getRawDb();
        const rows = rawDb.prepare(DEPENDENCY_GROUP_QUERY).all() as Array<{ package_name: string; language: string; project_path: string; is_dev: number; is_direct: number }>;

        // Scope to the active project root. Sibling projects tracked in the same
        // database (the ACE engine indexes every local project) must not bleed
        // into this project's vulnerability scan.
        const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase().replace(/\/+$/, "");
        const rootNorm = norm(resolveProjectDir());

        const groups = new Map<string, { dir: string; language: string; deps: string[]; devDeps: string[] }>();
        for (const row of rows) {
          const projectPath = row.project_path || process.cwd();
          const pp = norm(projectPath);
          if (rootNorm && pp !== rootNorm && !pp.startsWith(`${rootNorm}/`)) continue;
          if (pp.includes("/.claude/worktrees/") || pp.includes("/.codex/worktrees/")) continue;
          const language = row.language || "npm";
          const key = `${projectPath}::${language}`;
          let group = groups.get(key);
          if (!group) {
            group = { dir: projectPath, language, deps: [], devDeps: [] };
            groups.set(key, group);
          }
          if (row.is_direct) {
            (row.is_dev ? group.devDeps : group.deps).push(row.package_name);
          }
        }

        if (groups.size > 0) {
          liveIntel.initFromDependencyGroups([...groups.values()]);
        }
      } catch (err) {
        // Non-fatal — live intel just won't have version data — but log to stderr
        // so a silent empty scan (e.g. project_dependencies schema drift) is
        // diagnosable rather than looking like "no vulnerabilities".
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`[4da] dependency-group init from 4DA DB failed (continuing without version data): ${msg}`);
      }
    }
  }
  return db;
}

// =============================================================================
// Server Factory
// =============================================================================

/**
 * Build a Server instance with every handler registered.
 *
 * The v2 serving entries take a factory: `serveStdio` pins one instance per
 * connection; stateless HTTP (`createMcpHandler`) builds one per request.
 * Handlers close over the module-level database singleton, so every instance
 * shares the same lazily-initialized database and live-intelligence layer.
 */
export function buildServer(): Server {
  const server = new Server(
    {
      name: "4da-server",
      version: SERVER_VERSION,
    },
    {
      capabilities: {
        tools: { listChanged: true },
        resources: {},
      },
    }
  );

  // List available tools (SLIM): one-liner descriptions only (~500 tokens vs
  // ~4500). Full schemas available via MCP Resources: 4da://schema/{tool_name}
  server.setRequestHandler("tools/list", async () => {
    const database = getDatabase();
    return {
      tools: getSlimToolList(database.isStandalone ? true : undefined),
    };
  });

  // List schema resources: full tool schemas as MCP Resources for lazy
  // loading, plus the skill and category manifests.
  server.setRequestHandler("resources/list", async () => {
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

  // Read a resource (schema or skill manifest)
  server.setRequestHandler("resources/read", async (request) => {
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

  // Execute a tool
  server.setRequestHandler("tools/call", async (request) => {
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

  return server;
}

// =============================================================================
// Server Lifecycle
// =============================================================================

/**
 * Start the MCP server
 *
 * Supports three modes:
 *   (default)  stdio transport — serves 2025-era and 2026-07-28 hosts
 *   --http     Streamable HTTP transport — stateless, both eras
 *   --setup    Configure editors for 4DA MCP
 */
async function main() {
  const args = process.argv.slice(2);

  // Version
  if (args.includes("--version") || args.includes("-v")) {
    console.log(`@4da/mcp-server ${SERVER_VERSION}`);
    return;
  }

  // Help
  if (args.includes("--help") || args.includes("-h")) {
    console.log(`
  @4da/mcp-server — Dependency intelligence for AI coding agents

  Usage:
    npx @4da/mcp-server              Start MCP server (stdio transport)
    npx @4da/mcp-server --http       Start Streamable HTTP transport
    npx @4da/mcp-server --setup      Auto-configure your editor
    npx @4da/mcp-server --doctor     Check installation health
    npx @4da/mcp-server --version    Print version

  Options:
    --http              Use Streamable HTTP instead of stdio
    --port <number>     HTTP port (default: 4840)
    --host <address>    HTTP bind address (default: 127.0.0.1). A non-loopback
                        address exposes every tool to the network and is
                        refused unless MCP_AUTH_SECRET is set; authentication
                        is then mandatory for every request.
    --setup             Detect editors and write MCP config
    --doctor            Validate database, bindings, and LLM providers

  Environment:
    FOURDA_DB_PATH      Path to 4DA's SQLite database (auto-detected if omitted)
    FOURDA_OFFLINE      Set to "true" to disable all network calls (OSV.dev, HN)
    MCP_AUTH_SECRET     Shared secret for verifying relay-issued Bearer tokens
                        (HMAC-SHA256). Falls back to JWT_SECRET. Without it, no
                        token is accepted.
    MCP_AUTH_REQUIRED   Set to "true" to require auth on a loopback --http bind
                        (always required on a non-loopback bind)
    MCP_ALLOWED_HOSTS   Extra comma-separated hostnames accepted in the Host /
                        Origin headers (needed when binding to 0.0.0.0)

  Works standalone (scans your project on startup) or with the full
  4DA desktop app for content scoring, source monitoring, and more.
  Desktop app: https://github.com/4DA-Systems/4DA/releases/latest
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
    const flagValue = (flag: string): string | undefined => {
      const i = args.indexOf(flag);
      if (i === -1) return undefined;
      const value = args[i + 1];
      // A flag with no value (or followed by another flag) is a typo, not a
      // request to bind everywhere — refuse rather than guess.
      if (value === undefined || value.startsWith("-")) {
        console.error(`[4DA] ${flag} requires a value`);
        process.exit(1);
      }
      return value;
    };

    const rawPort = flagValue("--port");
    const port = rawPort === undefined ? 4840 : Number.parseInt(rawPort, 10);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      console.error(`[4DA] Invalid --port value: ${rawPort}`);
      process.exit(1);
    }
    const host = flagValue("--host") ?? "127.0.0.1";

    try {
      await startHttpServer(buildServer, { port, host });
    } catch (error) {
      // Bind-policy refusals carry an actionable message; print it plainly
      // rather than as an unhandled stack trace.
      console.error(`[4DA] ${error instanceof Error ? error.message : String(error)}`);
      process.exit(1);
    }
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

  // Default: stdio serving. `serveStdio` owns the era decision per connection —
  // a 2025-era `initialize` opening is served exactly as before; a 2026-07-28
  // `server/discover` opening gets the stateless modern protocol.
  serveStdio(buildServer, {
    onerror: (error) => {
      console.error(`[4DA] stdio serving error: ${error.message}`);
    },
  });

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
  console.error(`4DA MCP Server v${SERVER_VERSION} started — ${toolCount} tools, stdio transport`);
  console.error("  Use --http for Streamable HTTP, --setup to configure editors, --doctor to check health");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
