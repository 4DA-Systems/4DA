// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_module tool
 *
 * Retrieve a STREETS module by ID, including all parsed lessons.
 */

import type { ContentLoader } from "../content.js";
import type { GetModuleParams, ModuleContent } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getModuleTool = {
  name: "get_module",
  description: `Retrieve a STREETS Playbook module by ID. Returns the module title, description, all lessons (title + full content), and whether it's a free module.

Module IDs: S (Sovereign Setup), T (Technical Moats), R (Revenue Engines), E1 (Execution Playbook), E2 (Evolving Edge), T2 (Tactical Automation), S2 (Stacking Streams).

All modules are free.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      module_id: {
        type: "string",
        description: 'Module identifier. One of: "S", "T", "R", "E1", "E2", "T2", "S2"',
        enum: ["S", "T", "R", "E1", "E2", "T2", "S2"],
      },
    },
    required: ["module_id"],
  },
};

/**
 * Execute the get_module tool
 */
export function executeGetModule(
  content: ContentLoader,
  params: GetModuleParams
): ModuleContent {
  const moduleId = params.module_id.toUpperCase();

  if (!content.isValidModuleId(moduleId)) {
    throw new Error(
      `Invalid module_id: "${params.module_id}". Valid IDs: ${content.getModuleIds().join(", ")}`
    );
  }

  return content.getModule(moduleId);
}
