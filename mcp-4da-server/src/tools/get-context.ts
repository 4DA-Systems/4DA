/**
 * get_context tool
 *
 * Get the user's context (what 4DA knows about them).
 */

import type { FourDADatabase } from "../db.js";
import type { GetContextParams, UserContext } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getContextTool = {
  name: "get_context",
  description: `Get the user's context - what 4DA knows about them.

Returns information about:
- Static Identity: User-declared role, tech stack, domains, interests, and exclusions
- ACE Context (optional): Auto-detected technologies and active topics from recent activity
- Learned Preferences (optional): Topic affinities learned from user behavior

Use this to understand what the user is working on and interested in.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      include_ace: {
        type: "boolean",
        description:
          "Include ACE-detected context (detected tech, active topics). Default: true",
        default: true,
      },
      include_learned: {
        type: "boolean",
        description:
          "Include learned preferences (topic affinities, anti-topics). Default: true",
        default: true,
      },
    },
  },
};

/**
 * Execute the get_context tool
 */
export function executeGetContext(
  db: FourDADatabase,
  params: GetContextParams
): UserContext {
  const includeAce = params.include_ace ?? true;
  const includeLearned = params.include_learned ?? true;

  return db.getUserContext(includeAce, includeLearned);
}
