// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_next_step tool
 *
 * Recommend what to work on next based on current progress.
 */

import type { ContentLoader } from "../content.js";
import type { ProgressStore } from "../progress.js";
import type { NextStepResult } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getNextStepTool = {
  name: "get_next_step",
  description: `Get a recommendation for what to work on next in the STREETS Playbook.

Walks through modules in order (S -> T -> R -> E1 -> E2 -> T2 -> S2) and finds the first incomplete lesson. Returns the module ID, lesson index, reason, and context about surrounding progress.

If all modules are complete, recommends reviewing Module S2 (Stacking Streams) to optimize the income portfolio.`,
  inputSchema: {
    type: "object" as const,
    properties: {},
  },
};

/**
 * Execute the get_next_step tool
 */
export function executeGetNextStep(
  content: ContentLoader,
  progress: ProgressStore
): NextStepResult {
  return progress.getNextStep(content);
}
