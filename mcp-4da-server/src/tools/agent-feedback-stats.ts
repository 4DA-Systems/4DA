// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_agent_feedback_stats tool
 *
 * Get statistics on how agents have used 4DA recommendations.
 * Shows which sources and topics are most valuable for AI-assisted development.
 */

import type { FourDADatabase } from "../db.js";
import type { GetAgentFeedbackStatsParams, AgentFeedbackStats } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const agentFeedbackStatsTool = {
  name: "get_agent_feedback_stats",
  description: `Get statistics on how agents have used 4DA recommendations.

Shows which sources and topics are most valuable for AI-assisted development:
- Total recommendations: used / rejected / partially_used counts
- By source type: which sources produce the most agent-useful content
- Top used item IDs (most frequently recommended and accepted)
- Recent feedback entries

Use this to understand what content is actually being used by AI agents
and where PASIFA scoring can be improved.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      days: {
        type: "number",
        description: "Number of days to look back (default: 30, max: 365)",
        default: 30,
      },
    },
  },
};

/**
 * Execute the get_agent_feedback_stats tool
 */
export function executeGetAgentFeedbackStats(
  db: FourDADatabase,
  params: GetAgentFeedbackStatsParams
): AgentFeedbackStats {
  const days = Math.max(1, Math.min(365, params.days ?? 30));
  return db.getAgentFeedbackStats(days);
}
