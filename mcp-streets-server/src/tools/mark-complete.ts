// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * mark_complete tool
 *
 * Mark a specific lesson as complete in the progress tracker.
 */

import type { ContentLoader } from "../content.js";
import type { ProgressStore } from "../progress.js";
import type { MarkCompleteParams, MarkCompleteResult } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const markCompleteTool = {
  name: "mark_complete",
  description: `Mark a STREETS Playbook lesson as complete. Progress is stored locally in a SQLite database.

Lesson indices are 0-based within each module. For example, Module S has 6 lessons (indices 0-5), Module R has 8 lessons (indices 0-7).

This operation is idempotent — marking an already-completed lesson has no negative effect.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      module_id: {
        type: "string",
        description: 'Module identifier. One of: "S", "T", "R", "E1", "E2", "T2", "S2"',
        enum: ["S", "T", "R", "E1", "E2", "T2", "S2"],
      },
      lesson_idx: {
        type: "number",
        description: "Zero-based lesson index within the module.",
      },
    },
    required: ["module_id", "lesson_idx"],
  },
};

/**
 * Execute the mark_complete tool
 */
export function executeMarkComplete(
  content: ContentLoader,
  progress: ProgressStore,
  params: MarkCompleteParams
): MarkCompleteResult {
  const moduleId = params.module_id.toUpperCase();
  const lessonIdx = params.lesson_idx;

  if (!Number.isInteger(lessonIdx) || lessonIdx < 0) {
    throw new Error(`Invalid lesson_idx: ${lessonIdx}. Must be a non-negative integer.`);
  }

  return progress.markComplete(content, moduleId, lessonIdx);
}
