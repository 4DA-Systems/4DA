#!/usr/bin/env node
/**
 * STREETS MCP Server
 *
 * MCP server for the STREETS Developer Income Course. Provides tools for
 * accessing course content, analyzing projects for revenue engine fit,
 * and tracking lesson progress.
 *
 * 9 Tools:
 * - get_module: Retrieve a course module with all lessons
 * - get_template: Retrieve a worksheet template
 * - search_course: Full-text search across all modules
 * - get_engine: Get details for a specific revenue engine
 * - recommend_engines: Analyze project and recommend revenue engines
 * - assess_readiness: Score against the Sovereign Setup checklist
 * - get_progress: Track completion state across modules
 * - mark_complete: Mark a lesson as complete
 * - get_next_step: Recommend what to work on next
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

import { ContentLoader } from "./content.js";
import { ProgressStore } from "./progress.js";

// Course Content Tools
import {
  getModuleTool,
  executeGetModule,
  getTemplateTool,
  executeGetTemplate,
  searchCourseTool,
  executeSearchCourse,
  getEngineTool,
  executeGetEngine,
} from "./tools/index.js";

// Analysis Tools
import {
  recommendEnginesTool,
  executeRecommendEngines,
  assessReadinessTool,
  executeAssessReadiness,
} from "./tools/index.js";

// Progress Tools
import {
  getProgressTool,
  executeGetProgress,
  markCompleteTool,
  executeMarkComplete,
  getNextStepTool,
  executeGetNextStep,
} from "./tools/index.js";

import type {
  GetModuleParams,
  GetTemplateParams,
  SearchCourseParams,
  GetEngineParams,
  RecommendEnginesParams,
  AssessReadinessParams,
  MarkCompleteParams,
} from "./types.js";

// =============================================================================
// Server Setup
// =============================================================================

const server = new Server(
  {
    name: "streets-server",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// Lazy-initialized instances
let content: ContentLoader | null = null;
let progress: ProgressStore | null = null;

/**
 * Get or create the ContentLoader
 */
function getContentLoader(): ContentLoader {
  if (!content) {
    content = new ContentLoader();
  }
  return content;
}

/**
 * Get or create the ProgressStore
 */
function getProgressStore(): ProgressStore {
  if (!progress) {
    progress = new ProgressStore();
  }
  return progress;
}

// =============================================================================
// Tool Handlers
// =============================================================================

/**
 * List available tools
 */
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      getModuleTool,
      getTemplateTool,
      searchCourseTool,
      getEngineTool,
      recommendEnginesTool,
      assessReadinessTool,
      getProgressTool,
      markCompleteTool,
      getNextStepTool,
    ],
  };
});

/**
 * Execute a tool
 */
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    const contentLoader = getContentLoader();

    switch (name) {
      // =====================================================================
      // Course Content Tools
      // =====================================================================

      case "get_module": {
        const params = (args || {}) as unknown as GetModuleParams;
        const result = executeGetModule(contentLoader, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "get_template": {
        const params = (args || {}) as unknown as GetTemplateParams;
        const result = executeGetTemplate(contentLoader, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "search_course": {
        const params = (args || {}) as unknown as SearchCourseParams;
        const result = executeSearchCourse(contentLoader, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "get_engine": {
        const params = (args || {}) as unknown as GetEngineParams;
        const result = executeGetEngine(contentLoader, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      // =====================================================================
      // Analysis Tools
      // =====================================================================

      case "recommend_engines": {
        const params = (args || {}) as unknown as RecommendEnginesParams;
        const result = executeRecommendEngines(params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "assess_readiness": {
        const params = (args || {}) as unknown as AssessReadinessParams;
        const result = executeAssessReadiness(params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      // =====================================================================
      // Progress Tools
      // =====================================================================

      case "get_progress": {
        const progressStore = getProgressStore();
        const result = executeGetProgress(contentLoader, progressStore);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "mark_complete": {
        const params = (args || {}) as unknown as MarkCompleteParams;
        const progressStore = getProgressStore();
        const result = executeMarkComplete(contentLoader, progressStore, params);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      }

      case "get_next_step": {
        const progressStore = getProgressStore();
        const result = executeGetNextStep(contentLoader, progressStore);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
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

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);

  // Handle graceful shutdown
  process.on("SIGINT", () => {
    console.error("[STREETS] Received SIGINT — shutting down gracefully");
    if (progress) progress.close();
    process.exit(0);
  });

  process.on("SIGTERM", () => {
    console.error("[STREETS] Received SIGTERM — shutting down gracefully");
    if (progress) progress.close();
    process.exit(0);
  });

  console.error("STREETS MCP Server v1.0 started — 9 tools, stdio transport");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
