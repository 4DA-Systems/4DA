/**
 * record_agent_feedback tool
 *
 * Record whether content recommended by 4DA was useful in the agent's task.
 * Feeds into PASIFA scoring system learning.
 */

import type { FourDADatabase } from "../db.js";
import type { RecordAgentFeedbackParams, AgentFeedbackResult, AgentFeedbackOutcome } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const recordAgentFeedbackTool = {
  name: "record_agent_feedback",
  description: `Record whether content recommended by 4DA was useful in the agent's task.

Call this after using get_relevant_content results to help 4DA learn what's actually valuable for AI-assisted development.

Outcomes:
- "used": Agent/developer used the recommended content (strong positive signal)
- "rejected": Content was not useful — outdated, irrelevant, or wrong context (negative signal)
- "partially_used": Some value extracted but not directly applicable (weak positive signal)

This feedback helps 4DA's PASIFA scoring system learn which content sources and topics are genuinely useful when agents recommend them.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      item_ids: {
        type: "array",
        items: { type: "string" },
        description: "Source item IDs that were recommended (from get_relevant_content results)",
      },
      outcome: {
        type: "string",
        enum: ["used", "rejected", "partially_used"],
        description: "Whether the agent/developer used the recommended content",
      },
      context: {
        type: "string",
        description:
          "Brief context about why (e.g., 'recommended to developer, they found it relevant' or 'content was outdated')",
      },
      session_task: {
        type: "string",
        description: "What task the agent was working on when it used this content",
      },
    },
    required: ["item_ids", "outcome"],
  },
};

const validOutcomes: AgentFeedbackOutcome[] = ["used", "rejected", "partially_used"];

/**
 * Execute the record_agent_feedback tool
 */
export function executeRecordAgentFeedback(
  db: FourDADatabase,
  params: RecordAgentFeedbackParams
): AgentFeedbackResult {
  if (!params.item_ids || !Array.isArray(params.item_ids) || params.item_ids.length === 0) {
    return {
      success: false,
      message: "item_ids must be a non-empty array of item ID strings",
      recorded_count: 0,
    };
  }

  if (!params.outcome || !validOutcomes.includes(params.outcome)) {
    return {
      success: false,
      message: `Invalid outcome: ${params.outcome}. Valid outcomes: ${validOutcomes.join(", ")}`,
      recorded_count: 0,
    };
  }

  // Normalize item IDs to strings
  const itemIds = params.item_ids.map((id) => String(id));

  return db.recordAgentFeedback(
    itemIds,
    params.outcome,
    params.context,
    params.session_task
  );
}
