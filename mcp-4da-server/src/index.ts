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
 * 4DA MCP Server v3.0 - AI Superpower Edition
 *
 * Exposes 4DA's personalized content filtering to AI agents via MCP.
 * Now with local LLM support via Ollama for fully offline AI synthesis.
 *
 * Core Tools (4):
 * - get_relevant_content: Get filtered relevant content from 4DA
 * - get_context: Get the user's context (what 4DA knows about them)
 * - explain_relevance: Explain why an item was considered relevant
 * - record_feedback: Record user feedback for learning
 *
 * AI Superpower Tools (8):
 * - score_autopsy: Deep forensic analysis of relevance scores (AI-powered)
 * - trend_analysis: Statistical patterns and anomaly detection (AI-powered)
 * - daily_briefing: Executive summaries of discoveries (AI-powered)
 * - context_analysis: Optimize your context for better relevance (AI-powered)
 * - source_health: Diagnose source pipeline issues
 * - topic_connections: Build knowledge graphs from content (AI-powered)
 * - config_validator: Validate configuration and detect issues
 * - llm_status: Check LLM configuration and Ollama availability
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

// =============================================================================
// Server Setup
// =============================================================================

const server = new Server(
  {
    name: "4da-server",
    version: "3.3.0", // MCP Protocol Readiness Edition
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

  // Setup command: configure editors
  if (args.includes("--setup") || args.includes("setup")) {
    runSetup();
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
    // Log warning but do not abort — tools will surface errors naturally
    console.error(`[4DA] Database validation warning: ${validation.error}`);
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
  console.error(`4DA MCP Server v3.3 (Intelligence Platform) started — ${toolCount} tools, stdio transport`);
  console.error("  Use --http for Streamable HTTP transport, --setup to configure editors");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
