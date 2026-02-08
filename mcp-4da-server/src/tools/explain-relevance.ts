/**
 * explain_relevance tool
 *
 * Explain why an item was considered relevant.
 */

import type { FourDADatabase } from "../db.js";
import type { ExplainRelevanceParams, RelevanceExplanation } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const explainRelevanceTool = {
  name: "explain_relevance",
  description: `Explain why a content item was considered relevant.

Provides a detailed breakdown of:
- Score components (static match, ACE context, learned preferences, penalties)
- Matching context elements (interests, tech, topics, affinities)
- Human-readable explanation

Use this to understand how 4DA scored a particular item.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      item_id: {
        type: "number",
        description: "The database ID of the item to explain",
      },
      source_type: {
        type: "string",
        description: 'The source type: "hackernews", "arxiv", or "reddit"',
        enum: ["hackernews", "arxiv", "reddit"],
      },
    },
    required: ["item_id", "source_type"],
  },
};

/**
 * Execute the explain_relevance tool
 */
export function executeExplainRelevance(
  db: FourDADatabase,
  params: ExplainRelevanceParams
): RelevanceExplanation | { error: string } {
  if (!params.item_id || !params.source_type) {
    return { error: "item_id and source_type are required" };
  }

  const explanation = db.explainRelevance(params.item_id, params.source_type);

  if (!explanation) {
    return {
      error: `Item ${params.item_id} of type ${params.source_type} not found`,
    };
  }

  return explanation;
}
