#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * STREETS MCP Server
 *
 * Serves the STREETS Developer Income Playbook over MCP: playbook content,
 * project analysis, and progress tracking. 9 tools across three groups
 * (course content, analysis, progress).
 *
 * Protocol: MCP TypeScript SDK v2, served via `serveStdio` — 2025-era hosts
 * (classic `initialize` handshake) and 2026-07-28 hosts (stateless
 * `server/discover`) are both supported on the same stdio endpoint.
 */
import { serveStdio } from "@modelcontextprotocol/server/stdio";
import { Server } from "@modelcontextprotocol/server";
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
// Server Factory
// =============================================================================

/**
 * Build a Server instance with the tool handlers registered. `serveStdio`
 * takes a factory and pins one instance per connection; handlers close over
 * the module-level content/progress singletons.
 */
function buildServer(): Server {
  const server = new Server(
    {
      name: "streets-server",
      version: "2.0.0",
    },
    {
      capabilities: {
        tools: {},
      },
    }
  );

  // List available tools
  server.setRequestHandler("tools/list", async () => {
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

  // Execute a tool
  server.setRequestHandler("tools/call", async (request) => {
    const { name, arguments: args } = request.params;

    try {
      const contentLoader = getContentLoader();

      switch (name) {
        // ===================================================================
        // Course Content Tools
        // ===================================================================

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

        // ===================================================================
        // Analysis Tools
        // ===================================================================

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

        // ===================================================================
        // Progress Tools
        // ===================================================================

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

  return server;
}

// =============================================================================
// Server Lifecycle
// =============================================================================

function main() {
  // `serveStdio` owns the era decision per connection: a 2025-era
  // `initialize` opening is served exactly as before; a 2026-07-28
  // `server/discover` opening gets the stateless modern protocol.
  serveStdio(buildServer, {
    onerror: (error) => {
      console.error(`[STREETS] stdio serving error: ${error.message}`);
    },
  });

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

  console.error("STREETS MCP Server v2.0 started — 9 tools, stdio transport");
}

main();
