// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * record_feedback tool
 *
 * Record user feedback on an item for learning.
 */

import type { FourDADatabase } from "../db.js";
import type { RecordFeedbackParams, FeedbackResult, FeedbackAction } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const recordFeedbackTool = {
  name: "record_feedback",
  description: `Record user feedback on a content item.

Feedback actions:
- "click": User clicked/opened the item (moderate positive signal)
- "save": User saved/bookmarked the item (strong positive signal)
- "dismiss": User dismissed the item (weak negative signal)
- "mark_irrelevant": User marked item as not relevant (strong negative signal)

This feedback helps 4DA learn user preferences over time.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      item_id: {
        type: "number",
        description: "The database ID of the item",
      },
      source_type: {
        type: "string",
        description: 'The source type: "hackernews", "arxiv", or "reddit"',
        enum: ["hackernews", "arxiv", "reddit"],
      },
      action: {
        type: "string",
        description: 'The feedback action: "click", "save", "dismiss", or "mark_irrelevant"',
        enum: ["click", "save", "dismiss", "mark_irrelevant"],
      },
    },
    required: ["item_id", "source_type", "action"],
  },
};

const validActions: FeedbackAction[] = ["click", "save", "dismiss", "mark_irrelevant"];

/**
 * Execute the record_feedback tool
 */
export function executeRecordFeedback(
  db: FourDADatabase,
  params: RecordFeedbackParams
): FeedbackResult {
  if (!params.item_id || !params.source_type || !params.action) {
    return {
      success: false,
      message: "item_id, source_type, and action are required",
    };
  }

  if (!validActions.includes(params.action)) {
    return {
      success: false,
      message: `Invalid action: ${params.action}. Valid actions: ${validActions.join(", ")}`,
    };
  }

  return db.recordFeedback(params.item_id, params.source_type, params.action);
}
