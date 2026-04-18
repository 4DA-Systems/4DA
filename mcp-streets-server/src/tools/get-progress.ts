// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_progress tool
 *
 * Track completion state across all STREETS modules.
 */

import type { ContentLoader } from "../content.js";
import type { ProgressStore } from "../progress.js";
import type { ProgressReport } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getProgressTool = {
  name: "get_progress",
  description: `Get the current completion progress across all STREETS course modules.

Returns per-module completion (completed lessons, total lessons, percentage) and overall course percentage. Progress is stored locally in a SQLite database.`,
  inputSchema: {
    type: "object" as const,
    properties: {},
  },
};

/**
 * Execute the get_progress tool
 */
export function executeGetProgress(
  content: ContentLoader,
  progress: ProgressStore
): ProgressReport {
  return progress.getProgress(content);
}
