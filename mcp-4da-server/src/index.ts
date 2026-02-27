#!/usr/bin/env node
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
 * 4DA MCP Server v4.0.0 — Intelligence Platform
 *
 * 30 tools across 8 categories. Exposes 4DA's codebase-aware content
 * scoring engine to any AI tool that speaks MCP.
 *
 * Categories:
 *   Core (4)                  — content feed, context, relevance, feedback
 *   Intelligence (9)          — briefings, signals, autopsy, trends, topics
 *   Diagnostic (3)            — source health, config, LLM status
 *   Knowledge & Health (4)    — gaps, project health, mentions, context export
 *   Decision Intelligence (3) — decision memory, tech radar, alignment checks
 *   Agent Autonomy (3)        — persistent memory, session briefs, delegation
 *   Developer DNA (1)         — tech identity profile
 *   Intelligence Metabolism (3)— autophagy, decision windows, compound advantage
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

// Schema registry for slim tool listing
import { getSlimToolList, getSchemaResources, hasToolSchema, getSchemaFilename } from "./schema-registry.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

import { createDatabase, FourDADatabase, type DatabaseValidationResult } from "./db.js";

// Core Tools
import {
  getRelevantContentTool,
  executeGetRelevantContent,
  getContextTool,
  executeGetContext,
  explainRelevanceTool,
  executeExplainRelevance,
  recordFeedbackTool,
  executeRecordFeedback,
} from "./tools/index.js";

// Superpower Tools
import {
  scoreAutopsyTool,
  executeScoreAutopsy,
  trendAnalysisTool,
  executeTrendAnalysis,
  dailyBriefingTool,
  executeDailyBriefing,
  contextAnalysisTool,
  executeContextAnalysis,
  sourceHealthTool,
  executeSourceHealth,
  topicConnectionsTool,
  executeTopicConnections,
  configValidatorTool,
  executeConfigValidator,
  llmStatusTool,
  executeLLMStatus,
  getActionableSignalsTool,
  executeGetActionableSignals,
  exportContextPacketTool,
  executeExportContextPacket,
  knowledgeGapsTool,
  executeKnowledgeGaps,
  signalChainsTool,
  executeSignalChains,
  semanticShiftsTool,
  executeSemanticShifts,
  reverseMentionsTool,
  executeReverseMentions,
  attentionReportTool,
  executeAttentionReport,
  projectHealthTool,
  executeProjectHealth,
  decisionMemoryTool,
  executeDecisionMemory,
  techRadarTool,
  executeTechRadar,
  checkDecisionAlignmentTool,
  executeCheckDecisionAlignment,
  agentMemoryTool,
  executeAgentMemory,
  agentSessionBriefTool,
  executeAgentSessionBrief,
  delegationScoreTool,
  executeDelegationScore,
  developerDnaTool,
  executeDeveloperDna,
  autophagyStatusTool,
  executeAutophagyStatus,
  decisionWindowsTool,
  executeDecisionWindows,
  compoundAdvantageTool,
  executeCompoundAdvantage,
} from "./tools/index.js";

import type {
  GetRelevantContentParams,
  GetContextParams,
  ExplainRelevanceParams,
  RecordFeedbackParams,
} from "./types.js";

import type { ScoreAutopsyParams } from "./tools/score-autopsy.js";
import type { TrendAnalysisParams } from "./tools/trend-analysis.js";
import type { DailyBriefingParams } from "./tools/daily-briefing.js";
import type { ContextAnalysisParams } from "./tools/context-analysis.js";
import type { SourceHealthParams } from "./tools/source-health.js";
import type { TopicConnectionsParams } from "./tools/topic-connections.js";
import type { ConfigValidatorParams } from "./tools/config-validator.js";
import type { LLMStatusParams } from "./tools/llm-status.js";
import type { GetActionableSignalsParams } from "./tools/get-actionable-signals.js";
import type { ExportContextParams } from "./tools/export-context.js";
import type { KnowledgeGapsParams } from "./tools/knowledge-gaps.js";
import type { SignalChainsParams } from "./tools/signal-chains.js";
import type { SemanticShiftsParams } from "./tools/semantic-shifts.js";
import type { ReverseMentionsParams } from "./tools/reverse-mentions.js";
import type { AttentionReportParams } from "./tools/attention-report.js";
import type { ProjectHealthParams } from "./tools/project-health.js";
import type { DecisionMemoryParams } from "./tools/decision-memory.js";
import type { TechRadarParams } from "./tools/tech-radar.js";
import type { CheckDecisionAlignmentParams } from "./tools/decision-enforcement.js";
import type { AgentMemoryParams } from "./tools/agent-memory.js";
import type { AgentSessionBriefParams } from "./tools/agent-session-brief.js";
import type { DelegationScoreParams } from "./tools/delegation-score.js";
import type { DeveloperDnaParams } from "./tools/developer-dna.js";
import type { AutophagyStatusParams } from "./tools/autophagy-status.js";
import type { DecisionWindowsParams } from "./tools/decision-windows.js";
import type { CompoundAdvantageParams } from "./tools/compound-advantage.js";

// =============================================================================
// Server Setup
// =============================================================================

const server = new Server(
  {
    name: "4da-server",
    version: "4.0.1",
  },
  {
    capabilities: {
      tools: {},
      resources: {}, // Enable MCP Resources for lazy schema loading
    },
  }
);

// Database instance (lazy initialized)
let db: FourDADatabase | null = null;

/**
 * Get or create database connection
 */
function getDatabase(): FourDADatabase {
  if (!db) {
    const dbPath = process.env.FOURDA_DB_PATH;
    db = createDatabase(dbPath);
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
  ];

  return { resources };
});

/**
 * Read a resource (schema or skill manifest)
 */
server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const uri = request.params.uri;

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

    switch (name) {
      case "get_relevant_content": {
        const params = (args || {}) as GetRelevantContentParams;
        const result = executeGetRelevantContent(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "get_context": {
        const params = (args || {}) as GetContextParams;
        const result = executeGetContext(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "explain_relevance": {
        const params = (args || {}) as unknown as ExplainRelevanceParams;
        const result = executeExplainRelevance(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "record_feedback": {
        const params = (args || {}) as unknown as RecordFeedbackParams;
        const result = executeRecordFeedback(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      // =========================================================================
      // Superpower Tools (async with LLM synthesis)
      // =========================================================================

      case "score_autopsy": {
        const params = (args || {}) as unknown as ScoreAutopsyParams;
        const result = await executeScoreAutopsy(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "trend_analysis": {
        const params = (args || {}) as unknown as TrendAnalysisParams;
        const result = await executeTrendAnalysis(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "daily_briefing": {
        const params = (args || {}) as unknown as DailyBriefingParams;
        const result = await executeDailyBriefing(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "context_analysis": {
        const params = (args || {}) as unknown as ContextAnalysisParams;
        const result = await executeContextAnalysis(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "source_health": {
        const params = (args || {}) as unknown as SourceHealthParams;
        const result = executeSourceHealth(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "topic_connections": {
        const params = (args || {}) as unknown as TopicConnectionsParams;
        const result = await executeTopicConnections(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "config_validator": {
        const params = (args || {}) as unknown as ConfigValidatorParams;
        const result = executeConfigValidator(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "llm_status": {
        const params = (args || {}) as unknown as LLMStatusParams;
        const result = await executeLLMStatus(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "get_actionable_signals": {
        const params = (args || {}) as unknown as GetActionableSignalsParams;
        const result = executeGetActionableSignals(database, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      // =========================================================================
      // Innovation Feature Tools
      // =========================================================================

      case "export_context_packet": {
        const params = (args || {}) as unknown as ExportContextParams;
        const result = executeExportContextPacket(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "knowledge_gaps": {
        const params = (args || {}) as unknown as KnowledgeGapsParams;
        const result = executeKnowledgeGaps(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "signal_chains": {
        const params = (args || {}) as unknown as SignalChainsParams;
        const result = executeSignalChains(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "semantic_shifts": {
        const params = (args || {}) as unknown as SemanticShiftsParams;
        const result = executeSemanticShifts(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "reverse_mentions": {
        const params = (args || {}) as unknown as ReverseMentionsParams;
        const result = executeReverseMentions(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "attention_report": {
        const params = (args || {}) as unknown as AttentionReportParams;
        const result = executeAttentionReport(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "project_health": {
        const params = (args || {}) as unknown as ProjectHealthParams;
        const result = executeProjectHealth(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      // =========================================================================
      // Decision Intelligence Tools
      // =========================================================================

      case "decision_memory": {
        const params = (args || {}) as unknown as DecisionMemoryParams;
        const result = executeDecisionMemory(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "tech_radar": {
        const params = (args || {}) as unknown as TechRadarParams;
        const result = executeTechRadar(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "check_decision_alignment": {
        const params = (args || {}) as unknown as CheckDecisionAlignmentParams;
        const result = executeCheckDecisionAlignment(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      // =========================================================================
      // Agent Autonomy Tools
      // =========================================================================

      case "agent_memory": {
        const params = (args || {}) as unknown as AgentMemoryParams;
        const result = executeAgentMemory(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "agent_session_brief": {
        const params = (args || {}) as unknown as AgentSessionBriefParams;
        const result = executeAgentSessionBrief(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "delegation_score": {
        const params = (args || {}) as unknown as DelegationScoreParams;
        const result = executeDelegationScore(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "developer_dna": {
        const params = (args || {}) as unknown as DeveloperDnaParams;
        const result = executeDeveloperDna(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      // =========================================================================
      // Intelligence Metabolism Tools
      // =========================================================================

      case "autophagy_status": {
        const params = (args || {}) as unknown as AutophagyStatusParams;
        const result = executeAutophagyStatus(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "decision_windows": {
        const params = (args || {}) as unknown as DecisionWindowsParams;
        const result = executeDecisionWindows(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      case "compound_advantage": {
        const params = (args || {}) as unknown as CompoundAdvantageParams;
        const result = executeCompoundAdvantage(database, params);
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }

      default:
        throw new Error(`Unknown tool: ${name}`);
    }
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
    console.log("@4da/mcp-server 4.0.1");
    return;
  }

  // Help
  if (args.includes("--help") || args.includes("-h")) {
    console.log(`
  @4da/mcp-server — 30 tools for codebase-aware developer intelligence

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

  Requires the 4DA desktop app (scans your projects, scores content).
  Download: https://github.com/runyourempire/4DA/releases/latest
  Docs:     https://4da.ai
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
  } else {
    // Log actionable warning — this IS the conversion funnel for MCP-first users
    console.error(`[4DA] Database not found.`);
    console.error(`  The MCP server needs the 4DA desktop app to work.`);
    console.error(`  4DA scans your projects and scores content — this server reads that data.`);
    console.error(``);
    console.error(`  Get started: https://github.com/runyourempire/4DA/releases/latest`);
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
  console.error(`4DA MCP Server v4.0.1 started — ${toolCount} tools, stdio transport`);
  console.error("  Use --http for Streamable HTTP, --setup to configure editors, --doctor to check health");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
